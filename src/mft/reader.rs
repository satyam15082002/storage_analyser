use anyhow::{bail, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, SetFilePointerEx, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

/// Raw sector-aligned reader over a volume handle (e.g. `\\.\C:`). Volume handles reject
/// unaligned offsets/lengths, so every read is expanded to the nearest sector boundary and
/// the caller's requested slice is sliced back out of the aligned buffer.
pub struct VolumeReader {
    handle: HANDLE,
    bytes_per_sector: u64,
}

impl VolumeReader {
    pub fn open(drive_letter: char, bytes_per_sector: u32) -> Result<Self> {
        let path = format!("\\\\.\\{}:", drive_letter);
        let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        let handle = unsafe {
            CreateFileW(
                PCWSTR(path_w.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }?;

        Ok(VolumeReader { handle, bytes_per_sector: bytes_per_sector as u64 })
    }

    /// Reads `len` bytes starting at `offset`, both of which may be unaligned; internally
    /// rounds out to sector boundaries and returns exactly the requested window.
    pub fn read_at(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        let sector = self.bytes_per_sector;
        let aligned_start = (offset / sector) * sector;
        let end = offset + len;
        let aligned_end = end.div_ceil(sector) * sector;
        let aligned_len = aligned_end - aligned_start;

        let mut buf = vec![0u8; aligned_len as usize];
        unsafe {
            let mut pos = aligned_start as i64;
            SetFilePointerEx(self.handle, pos, None, FILE_BEGIN)?;
            let mut total_read = 0usize;
            while total_read < buf.len() {
                let mut bytes_read = 0u32;
                ReadFile(self.handle, Some(&mut buf[total_read..]), Some(&mut bytes_read), None)?;
                if bytes_read == 0 {
                    bail!("unexpected EOF reading volume at offset {}", pos);
                }
                total_read += bytes_read as usize;
                pos += bytes_read as i64;
            }
        }

        let start = (offset - aligned_start) as usize;
        let end = start + len as usize;
        Ok(buf[start..end].to_vec())
    }
}

impl Drop for VolumeReader {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

unsafe impl Send for VolumeReader {}
