use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;

use crate::model::{FsArena, NodeId};

/// Intermediate, thread-buildable tree. Parallel directory fan-out builds one of these
/// per subtree (via rayon) with no shared mutable state; the result is flattened into the
/// index-based `FsArena` in a single cheap sequential pass at the end.
enum RawEntry {
    File {
        name: String,
        size: u64,
        allocated_size: u64,
    },
    Dir {
        name: String,
        is_reparse: bool,
        children: Vec<RawEntry>,
    },
}

/// Recursively walks `root` building an `FsArena`. This is the portable fallback engine
/// used for non-NTFS volumes, non-elevated processes, or single-subfolder scans.
///
/// Reparse points (symlinks/junctions/mount points) are recorded as leaves but never
/// descended into, which avoids double-counting and cycles.
pub fn scan(root: &Path, counter: &AtomicU64) -> FsArena {
    let children = scan_dir_children(root, counter);

    let root_name = root.to_string_lossy().to_string();
    let mut arena = FsArena::new(root_name);
    let root_id = arena.root;
    for child in children {
        flatten_into(&mut arena, root_id, child);
    }

    arena.finalize();
    arena
}

fn scan_dir_children(dir: &Path, counter: &AtomicU64) -> Vec<RawEntry> {
    let entries: Vec<(String, PathBuf, fs::Metadata)> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                Some((entry.file_name().to_string_lossy().to_string(), entry.path(), meta))
            })
            .collect(),
        Err(_) => return Vec::new(), // permission denied / gone — skip silently
    };

    entries
        .into_par_iter()
        .map(|(name, path, meta)| {
            counter.fetch_add(1, Ordering::Relaxed);

            let is_reparse = meta.file_type().is_symlink();
            if meta.is_dir() {
                let children = if is_reparse {
                    Vec::new()
                } else {
                    scan_dir_children(&path, counter)
                };
                RawEntry::Dir { name, is_reparse, children }
            } else {
                let size = meta.len();
                RawEntry::File {
                    name,
                    size,
                    allocated_size: allocated_size_on_disk(&meta, size),
                }
            }
        })
        .collect()
}

fn flatten_into(arena: &mut FsArena, parent: NodeId, entry: RawEntry) {
    match entry {
        RawEntry::File { name, size, allocated_size } => {
            let id = arena.add_child(parent, name, false, false);
            arena.set_file_size(id, size, allocated_size);
        }
        RawEntry::Dir { name, is_reparse, children } => {
            let id = arena.add_child(parent, name, true, is_reparse);
            for child in children {
                flatten_into(arena, id, child);
            }
        }
    }
}

fn allocated_size_on_disk(_meta: &fs::Metadata, len: u64) -> u64 {
    // Rust's std::fs::Metadata doesn't expose the NTFS allocation size (rounded up to the
    // nearest cluster) portably; approximating with the logical length is close enough for
    // the walker fallback. The MFT engine reports true allocation size directly.
    len
}
