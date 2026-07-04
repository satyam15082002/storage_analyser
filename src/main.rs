mod export;
mod mft;
mod model;
mod privileges;
mod recycle;
mod scan;
mod ui;
mod walker;

use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

use anyhow::Result;
use clap::Parser;
use humansize::{format_size, DECIMAL};

use scan::Engine;

/// High-performance Windows storage analyser: find out what's taking up disk space.
#[derive(Parser)]
#[command(name = "storage-analyser", version)]
struct Cli {
    /// Path to analyse. Defaults to the system drive root (e.g. `C:\`).
    path: Option<PathBuf>,

    /// Scan engine to use. `auto` picks the NTFS MFT fast path when possible (whole-drive
    /// scan, NTFS volume, elevated process) and falls back to a recursive walk otherwise.
    #[arg(long, value_enum, default_value = "auto")]
    engine: Engine,

    /// Non-interactive: export the full scan to this file (.csv or .json) instead of
    /// opening the TUI.
    #[arg(long)]
    export: Option<PathBuf>,

    /// Non-interactive: print the top N largest entries (files and folders) instead of
    /// opening the TUI.
    #[arg(long)]
    top: Option<usize>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = cli.path.clone().unwrap_or_else(default_path);

    maybe_offer_elevation(&path, cli.engine);

    if cli.export.is_some() || cli.top.is_some() {
        return run_headless(&path, cli.engine, cli.export, cli.top);
    }

    ui::run(path, cli.engine)
}

/// If the target is a whole NTFS drive but the process isn't elevated, the MFT fast path
/// is unavailable (volume handles are admin-only) and the scan will silently fall back to
/// the much slower recursive walker. Offer to relaunch elevated instead of scanning slow.
fn maybe_offer_elevation(path: &std::path::Path, engine: Engine) {
    if engine == Engine::Walk || privileges::is_elevated() {
        return;
    }
    if !scan::is_drive_root(path) || !scan::is_ntfs(path) {
        return;
    }

    eprintln!("'{}' is a whole NTFS drive, but this process isn't elevated, so the fast", path.display());
    eprintln!("MFT scan engine can't run — falling back to a much slower full directory walk.");
    eprint!("Relaunch elevated (UAC prompt) for a fast scan instead? [y/N] ");
    use std::io::Write;
    let _ = std::io::stderr().flush();

    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y") {
        match privileges::relaunch_elevated() {
            Ok(()) => std::process::exit(0),
            Err(e) => eprintln!("Could not relaunch elevated: {e}. Continuing with the slower walker."),
        }
    }
}

fn default_path() -> PathBuf {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    PathBuf::from(format!("{drive}\\"))
}

fn run_headless(path: &std::path::Path, engine: Engine, export: Option<PathBuf>, top: Option<usize>) -> Result<()> {
    eprintln!("Scanning {}...", path.display());
    let counter = AtomicU64::new(0);
    let outcome = scan::run(path, engine, &counter)?;
    eprintln!(
        "Done ({} engine, {} entries).",
        outcome.engine_used,
        counter.load(std::sync::atomic::Ordering::Relaxed)
    );

    if let Some(dest) = export {
        match dest.extension().and_then(|e| e.to_str()) {
            Some("json") => export::export_json(&outcome.arena, outcome.arena.root, &dest)?,
            _ => export::export_csv(&outcome.arena, outcome.arena.root, &dest)?,
        }
        println!("Exported to {}", dest.display());
    }

    if let Some(n) = top {
        print_top_n(&outcome.arena, n);
    }

    Ok(())
}

fn print_top_n(arena: &model::FsArena, n: usize) {
    let mut ids: Vec<model::NodeId> = (0..arena.nodes.len()).collect();
    ids.sort_by(|a, b| arena.nodes[*b].size.cmp(&arena.nodes[*a].size));

    for &id in ids.iter().skip(1).take(n) {
        let node = &arena.nodes[id];
        let kind = if node.is_dir { "DIR " } else { "FILE" };
        println!("{kind}  {:>12}  {}", format_size(node.size, DECIMAL), arena.path_of(id).display());
    }
}
