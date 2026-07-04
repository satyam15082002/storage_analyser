use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};
use windows::Win32::System::WindowsProgramming::{DRIVE_NO_ROOT_DIR, DRIVE_UNKNOWN};

pub struct DriveInfo {
    pub letter: char,
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

impl DriveInfo {
    pub fn path(&self) -> PathBuf {
        PathBuf::from(format!("{}:\\", self.letter))
    }

    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.free_bytes)
    }

    pub fn used_fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes() as f64 / self.total_bytes as f64
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Enumerates mounted drive letters (fixed disks, removable media, network shares — anything
/// with a real volume mounted) with their capacity, for an interactive "pick a drive" prompt.
pub fn list_drives() -> Vec<DriveInfo> {
    let mask = unsafe { GetLogicalDrives() };
    let mut drives = Vec::new();

    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let root_w = wide(&root);

        let drive_type = unsafe { GetDriveTypeW(PCWSTR(root_w.as_ptr())) };
        if drive_type == DRIVE_UNKNOWN || drive_type == DRIVE_NO_ROOT_DIR {
            continue;
        }

        let mut label_buf = [0u16; 256];
        let label = unsafe {
            GetVolumeInformationW(PCWSTR(root_w.as_ptr()), Some(&mut label_buf), None, None, None, None)
        }
        .ok()
        .map(|()| {
            let len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf16_lossy(&label_buf[..len])
        })
        .unwrap_or_default();

        let mut free_to_caller = 0u64;
        let mut total = 0u64;
        let mut total_free = 0u64;
        let got_space = unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(root_w.as_ptr()),
                Some(&mut free_to_caller),
                Some(&mut total),
                Some(&mut total_free),
            )
        }
        .is_ok();
        if !got_space {
            // Drive with no media (e.g. an empty card reader slot) — skip it.
            continue;
        }

        drives.push(DriveInfo {
            letter,
            label: if label.is_empty() { "Local Disk".to_string() } else { label },
            total_bytes: total,
            free_bytes: total_free,
        });
    }

    drives
}
