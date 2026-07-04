use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use crate::model::FsArena;
use crate::walker;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Engine {
    Auto,
    Mft,
    Walk,
}

pub struct ScanOutcome {
    pub arena: FsArena,
    pub engine_used: &'static str,
}

/// Events sent from the background scan thread to the UI: periodic progress ticks and a
/// final `Done` carrying the completed scan (or its error).
pub enum ScanEvent {
    Progress(u64),
    Done(Result<ScanOutcome>),
}

/// Runs a scan on a background thread, streaming progress/completion back over `tx`.
///
/// Progress is tracked via a shared `AtomicU64` that the (possibly multi-threaded) scan
/// engines bump directly — cheap, `Sync`, and lock-free — while a separate lightweight
/// ticker thread polls it on a fixed cadence and forwards ticks to the UI. This decouples
/// UI update rate from how many files are actually being processed per second.
pub fn spawn(path: std::path::PathBuf, engine: Engine, tx: Sender<ScanEvent>) {
    std::thread::spawn(move || {
        let counter = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));

        let ticker = {
            let counter = counter.clone();
            let stop = stop.clone();
            let tx = tx.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(120));
                    if tx.send(ScanEvent::Progress(counter.load(Ordering::Relaxed))).is_err() {
                        return;
                    }
                }
            })
        };

        let result = run(&path, engine, &counter);
        stop.store(true, Ordering::Relaxed);
        let _ = ticker.join();
        let _ = tx.send(ScanEvent::Done(result));
    });
}

/// Picks and runs the fastest available scan engine for `path`.
///
/// The MFT engine only applies to a whole-drive scan (e.g. `C:\`) on an NTFS volume from an
/// elevated process; everything else (subfolder scans, non-NTFS volumes, non-elevated
/// processes) uses the portable recursive walker.
pub fn run(path: &Path, requested: Engine, counter: &AtomicU64) -> Result<ScanOutcome> {
    let use_mft = match requested {
        Engine::Walk => false,
        Engine::Mft => true,
        Engine::Auto => is_drive_root(path) && is_ntfs(path) && crate::privileges::is_elevated(),
    };

    if use_mft {
        match crate::mft::scan_volume(path, counter) {
            Ok(arena) => return Ok(ScanOutcome { arena, engine_used: "mft" }),
            Err(e) if requested == Engine::Mft => return Err(e),
            Err(_) => { /* fall through to walker */ }
        }
    }

    let arena = walker::scan(path, counter);
    Ok(ScanOutcome { arena, engine_used: "walk" })
}

pub fn is_drive_root(path: &Path) -> bool {
    let mut components = path.components();
    match components.next() {
        Some(std::path::Component::Prefix(_)) => {}
        _ => return false,
    }
    matches!(components.next(), Some(std::path::Component::RootDir)) && components.next().is_none()
}

pub fn is_ntfs(path: &Path) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;

    let root = drive_root_string(path);
    let root_w: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let mut fs_name = [0u16; 32];

    let ok = unsafe {
        GetVolumeInformationW(
            PCWSTR(root_w.as_ptr()),
            None,
            None,
            None,
            None,
            Some(&mut fs_name),
        )
    };
    if ok.is_err() {
        return false;
    }
    let len = fs_name.iter().position(|&c| c == 0).unwrap_or(fs_name.len());
    String::from_utf16_lossy(&fs_name[..len]).eq_ignore_ascii_case("NTFS")
}

fn drive_root_string(path: &Path) -> String {
    let s = path.to_string_lossy();
    let drive_letter = s.chars().next().unwrap_or('C');
    format!("{}:\\", drive_letter)
}
