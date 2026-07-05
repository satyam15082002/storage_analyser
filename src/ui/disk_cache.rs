//! Persists a completed scan's `FsArena` to `%LOCALAPPDATA%\storage-analyzer\scan-cache\`
//! keyed by the scanned path, so reopening the app on a drive/folder already scanned in a
//! previous run loads instantly instead of blocking on a fresh scan. Same "instant load,
//! silent background refresh" pattern as the in-memory session cache in `mod.rs` — this is
//! just the disk-backed counterpart that survives process exit.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::FsArena;

/// Bumped whenever `FsArena`'s on-disk shape changes, so a cache file written by an older
/// version is ignored (falls back to a fresh scan) instead of failing to deserialize weirdly.
const CACHE_VERSION: u32 = 1;

#[derive(Serialize)]
struct OnDiskRef<'a> {
    version: u32,
    engine_used: &'a str,
    arena: &'a FsArena,
}

#[derive(Deserialize)]
struct OnDiskOwned {
    version: u32,
    engine_used: String,
    arena: FsArena,
}

pub(super) fn cache_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("storage-analyzer").join("scan-cache")
}

fn cache_file_for(path: &Path) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    cache_dir().join(format!("{:016x}.cache", hasher.finish()))
}

/// Loads a previously persisted scan for `path`, if one exists and matches the current
/// cache format. `engine_used` round-trips through the same two static strings `scan::run`
/// produces ("mft"/"walk"), so it's remapped back to a `&'static str` here rather than kept
/// as an owned `String`.
pub fn load(path: &Path) -> Option<(FsArena, &'static str)> {
    let bytes = std::fs::read(cache_file_for(path)).ok()?;
    let on_disk: OnDiskOwned = bincode::deserialize(&bytes).ok()?;
    if on_disk.version != CACHE_VERSION {
        return None;
    }
    let engine_used = if on_disk.engine_used == "mft" { "mft" } else { "walk" };
    Some((on_disk.arena, engine_used))
}

/// Writes `arena` to disk for `path`. Best-effort: a failure here (no disk space, no
/// permission on `%LOCALAPPDATA%`, etc.) just means the next launch scans fresh, so errors
/// are swallowed rather than surfaced to the user.
pub fn save(path: &Path, arena: &FsArena, engine_used: &'static str) {
    let on_disk = OnDiskRef { version: CACHE_VERSION, engine_used, arena };
    let Ok(bytes) = bincode::serialize(&on_disk) else { return };
    let dir = cache_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(cache_file_for(path), bytes);
}
