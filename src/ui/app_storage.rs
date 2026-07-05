//! Reports how much disk space Storage Analyzer itself is using: the running executable
//! plus the persistent scan-cache directory (`disk_cache`). Surfaced via the `<space> i`
//! chord in the browsing view (`keys.rs`) — distinct from the per-entry `i` details popup,
//! since this is about the tool rather than the tree currently being browsed.

use std::path::PathBuf;

use super::disk_cache;

pub struct AppStorageInfo {
    pub exe_path: PathBuf,
    pub exe_size: u64,
    pub cache_dir: PathBuf,
    pub cache_size: u64,
    pub cache_file_count: u64,
}

pub fn gather() -> AppStorageInfo {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_size = std::fs::metadata(&exe_path).map(|m| m.len()).unwrap_or(0);

    let cache_dir = disk_cache::cache_dir();
    let mut cache_size = 0u64;
    let mut cache_file_count = 0u64;
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    cache_size += meta.len();
                    cache_file_count += 1;
                }
            }
        }
    }

    AppStorageInfo { exe_path, exe_size, cache_dir, cache_size, cache_file_count }
}
