use anyhow::{bail, Result};

pub struct BootSector {
    pub bytes_per_sector: u32,
    pub bytes_per_cluster: u64,
    pub mft_start_cluster: u64,
    pub mft_record_size: u32,
}

/// Parses the NTFS boot sector (first 512 bytes of the volume). Layout reference: the
/// Microsoft-documented NTFS BPB (BIOS Parameter Block) extension.
pub fn parse(buf: &[u8]) -> Result<BootSector> {
    if buf.len() < 512 {
        bail!("boot sector buffer too short");
    }
    if &buf[3..11] != b"NTFS    " {
        bail!("not an NTFS volume (oem id mismatch)");
    }

    let bytes_per_sector = u16::from_le_bytes([buf[11], buf[12]]) as u32;
    let sectors_per_cluster = buf[13] as u32;
    let mft_start_cluster = u64::from_le_bytes(buf[48..56].try_into().unwrap());
    let clusters_or_bytes_per_record = buf[64] as i8;

    let bytes_per_cluster = bytes_per_sector as u64 * sectors_per_cluster as u64;

    let mft_record_size = if clusters_or_bytes_per_record > 0 {
        clusters_or_bytes_per_record as u32 * bytes_per_cluster as u32
    } else {
        1u32 << (-(clusters_or_bytes_per_record as i32))
    };

    if bytes_per_sector == 0 || sectors_per_cluster == 0 || mft_record_size == 0 {
        bail!("invalid NTFS boot sector geometry");
    }

    Ok(BootSector {
        bytes_per_sector,
        bytes_per_cluster,
        mft_start_cluster,
        mft_record_size,
    })
}
