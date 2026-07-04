mod boot_sector;
mod data_runs;
mod reader;
mod record;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Result};

use crate::model::FsArena;
use reader::VolumeReader;

const ROOT_RECORD_NUMBER: u32 = 5;

/// Scans an entire NTFS volume by reading the Master File Table directly, the same
/// technique tools like WizTree use to turn a full-drive scan into a matter of seconds
/// instead of minutes. Requires an elevated process (volume handles are admin-only) and
/// `SeBackupPrivilege` (to bypass per-file read ACL checks on raw MFT records).
pub fn scan_volume(path: &Path, counter: &AtomicU64) -> Result<FsArena> {
    if !crate::privileges::is_elevated() {
        bail!("the MFT engine requires an elevated (Administrator) process");
    }
    let _ = crate::privileges::enable_backup_privileges();

    let drive_letter = path
        .to_string_lossy()
        .chars()
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty path"))?;

    // Bootstrap read at the universal minimum sector size (512) just to parse the boot
    // sector, which tells us the volume's real sector size for everything after.
    let bootstrap = VolumeReader::open(drive_letter, 512)?;
    let boot_buf = bootstrap.read_at(0, 512)?;
    let boot = boot_sector::parse(&boot_buf)?;
    drop(bootstrap);

    let reader = VolumeReader::open(drive_letter, boot.bytes_per_sector)?;

    // Record 0 is $MFT's own base record, stored at the very start of the MFT's first
    // extent (given directly by the boot sector). Its unnamed $DATA attribute's data-run
    // list describes every extent of the (possibly fragmented) MFT stream.
    let record0_offset = boot.mft_start_cluster * boot.bytes_per_cluster;
    let mut record0_buf = reader.read_at(record0_offset, boot.mft_record_size as u64)?;
    if !record::apply_fixup(&mut record0_buf, boot.bytes_per_sector as usize) {
        bail!("failed to read $MFT base record (fixup mismatch)");
    }
    let (extents, mft_stream_size) = extract_mft_data_runs(&record0_buf)
        .ok_or_else(|| anyhow::anyhow!("could not locate $MFT data runs in record 0"))?;

    let total_records = mft_stream_size / boot.mft_record_size as u64;
    let mut parsed: Vec<Option<record::ParsedRecord>> = Vec::with_capacity(total_records as usize);

    const CHUNK_CLUSTERS: u64 = 2048; // ~8-32MB depending on cluster size: amortizes syscalls

    'extents: for extent in &extents {
        let Some(lcn) = extent.lcn else {
            // Sparse ("hole") run: no clusters to read, but the virtual MFT stream still
            // advances past this many records. Skipping it without padding `parsed` would
            // shift every subsequent index out of sync with its true MFT record number,
            // corrupting every parent/child reference from here on.
            let placeholder_records =
                (extent.cluster_count * boot.bytes_per_cluster) / boot.mft_record_size as u64;
            for _ in 0..placeholder_records {
                if parsed.len() as u64 >= total_records {
                    break 'extents;
                }
                parsed.push(None);
            }
            continue;
        };
        let mut remaining = extent.cluster_count;
        let mut cluster_cursor = lcn;

        while remaining > 0 {
            let this_chunk = remaining.min(CHUNK_CLUSTERS);
            let byte_offset = cluster_cursor * boot.bytes_per_cluster;
            let byte_len = this_chunk * boot.bytes_per_cluster;
            let chunk = reader.read_at(byte_offset, byte_len)?;

            let records_in_chunk = byte_len / boot.mft_record_size as u64;
            for i in 0..records_in_chunk {
                if parsed.len() as u64 >= total_records {
                    break 'extents;
                }
                let start = (i * boot.mft_record_size as u64) as usize;
                let end = start + boot.mft_record_size as usize;
                let rec = record::parse(&chunk[start..end], boot.bytes_per_sector as usize);
                parsed.push(rec);
                counter.fetch_add(1, Ordering::Relaxed);
            }

            remaining -= this_chunk;
            cluster_cursor += this_chunk;
        }
    }

    build_tree(path, &parsed)
}

fn extract_mft_data_runs(record0: &[u8]) -> Option<(Vec<data_runs::Extent>, u64)> {
    use record::{ru16, ru32, ru64, ATTR_DATA, ATTR_END};

    let attrs_offset = ru16(record0, 20) as usize;
    let mut pos = attrs_offset;
    while pos + 16 <= record0.len() {
        let attr_type = ru32(record0, pos);
        if attr_type == ATTR_END {
            break;
        }
        let attr_len = ru32(record0, pos + 4) as usize;
        if attr_len == 0 || pos + attr_len > record0.len() {
            break;
        }
        let non_resident = record0[pos + 8] != 0;
        let name_length = record0[pos + 9];

        if attr_type == ATTR_DATA && name_length == 0 && non_resident {
            let real_size = ru64(record0, pos + 48);
            let run_offset = ru16(record0, pos + 32) as usize;
            let runs = data_runs::parse(&record0[pos + run_offset..pos + attr_len]);
            return Some((runs, real_size));
        }
        pos += attr_len;
    }
    None
}

/// Turns the flat, index-addressable table of parsed MFT records into an `FsArena` rooted
/// at the volume's root directory (well-known record 5), via a single BFS from the root —
/// records that never chain back to record 5 (orphaned/deleted entries) are dropped.
fn build_tree(volume_path: &Path, parsed: &[Option<record::ParsedRecord>]) -> Result<FsArena> {
    let mut children_of: HashMap<u32, Vec<u32>> = HashMap::new();
    for (idx, rec) in parsed.iter().enumerate() {
        let Some(rec) = rec else { continue };
        if !rec.in_use || rec.is_extension_record {
            continue;
        }
        let record_number = idx as u32;
        if record_number == ROOT_RECORD_NUMBER {
            continue;
        }
        let parent = rec.parent_frn as u32;
        children_of.entry(parent).or_default().push(record_number);
    }

    let root_name = volume_path.to_string_lossy().to_string();
    let mut arena = FsArena::new(root_name);

    let mut queue = std::collections::VecDeque::new();
    queue.push_back((ROOT_RECORD_NUMBER, arena.root));

    while let Some((record_number, node_id)) = queue.pop_front() {
        let Some(child_numbers) = children_of.get(&record_number) else { continue };
        for &child_num in child_numbers {
            let Some(Some(rec)) = parsed.get(child_num as usize) else { continue };
            if rec.name.is_empty() {
                continue;
            }
            let child_id = arena.add_child(node_id, rec.name.clone(), rec.is_dir, rec.is_reparse);
            if rec.is_dir {
                if !rec.is_reparse {
                    queue.push_back((child_num, child_id));
                }
            } else {
                arena.set_file_size(child_id, rec.size, rec.allocated_size);
            }
        }
    }

    arena.finalize();
    Ok(arena)
}
