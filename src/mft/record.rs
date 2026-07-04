pub(crate) const ATTR_STANDARD_INFORMATION: u32 = 0x10;
pub(crate) const ATTR_FILE_NAME: u32 = 0x30;
pub(crate) const ATTR_DATA: u32 = 0x80;
pub(crate) const ATTR_END: u32 = 0xFFFFFFFF;

const FLAG_IN_USE: u16 = 0x0001;
const FLAG_IS_DIRECTORY: u16 = 0x0002;

const FILENAME_NAMESPACE_DOS: u8 = 2;

const FILE_ATTR_REPARSE_POINT: u32 = 0x0400;

pub struct ParsedRecord {
    pub in_use: bool,
    pub is_dir: bool,
    pub is_extension_record: bool,
    pub parent_frn: u64,
    pub name: String,
    pub is_reparse: bool,
    pub size: u64,
    pub allocated_size: u64,
}

pub(crate) fn ru16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
pub(crate) fn ru32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub(crate) fn ru64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// Applies the NTFS "update sequence array" fixup in place. Each sector's last two bytes
/// are placeholders written over on-disk to detect torn writes; this restores their real
/// content. Returns `false` (caller should skip the record) if the fixup signature doesn't
/// match, which indicates a corrupt or torn record.
pub fn apply_fixup(buf: &mut [u8], bytes_per_sector: usize) -> bool {
    if buf.len() < 8 {
        return false;
    }
    let usa_offset = ru16(buf, 4) as usize;
    let usa_count = ru16(buf, 6) as usize;
    if usa_count == 0 || usa_offset + usa_count * 2 > buf.len() {
        return false;
    }

    let usa_value = [buf[usa_offset], buf[usa_offset + 1]];
    for i in 0..usa_count.saturating_sub(1) {
        let sector_end = (i + 1) * bytes_per_sector;
        if sector_end < 2 || sector_end > buf.len() {
            return false;
        }
        let check_off = sector_end - 2;
        if buf[check_off] != usa_value[0] || buf[check_off + 1] != usa_value[1] {
            return false;
        }
        let orig_off = usa_offset + 2 + i * 2;
        buf[check_off] = buf[orig_off];
        buf[check_off + 1] = buf[orig_off + 1];
    }
    true
}

pub fn parse(raw: &[u8], bytes_per_sector: usize) -> Option<ParsedRecord> {
    let mut buf = raw.to_vec();
    if buf.len() < 48 || &buf[0..4] != b"FILE" {
        return None;
    }
    if !apply_fixup(&mut buf, bytes_per_sector) {
        return None;
    }

    let flags = ru16(&buf, 22);
    let in_use = flags & FLAG_IN_USE != 0;
    let is_dir = flags & FLAG_IS_DIRECTORY != 0;
    let base_ref = ru64(&buf, 32) & 0x0000_FFFF_FFFF_FFFF;
    let is_extension_record = base_ref != 0;
    let attrs_offset = ru16(&buf, 20) as usize;

    if !in_use || is_extension_record {
        return Some(ParsedRecord {
            in_use,
            is_dir,
            is_extension_record,
            parent_frn: 0,
            name: String::new(),
            is_reparse: false,
            size: 0,
            allocated_size: 0,
        });
    }

    let mut parent_frn = 0u64;
    let mut name = String::new();
    let mut best_namespace: Option<u8> = None;
    let mut is_reparse = false;
    let mut size = 0u64;
    let mut allocated_size = 0u64;

    let mut pos = attrs_offset;
    while pos + 16 <= buf.len() {
        let attr_type = ru32(&buf, pos);
        if attr_type == ATTR_END {
            break;
        }
        let attr_len = ru32(&buf, pos + 4) as usize;
        if attr_len == 0 || pos + attr_len > buf.len() {
            break;
        }
        let non_resident = buf[pos + 8] != 0;
        let name_length = buf[pos + 9];

        match attr_type {
            ATTR_STANDARD_INFORMATION => { /* timestamps/basic flags not currently needed */ }
            ATTR_FILE_NAME if !non_resident => {
                let value_offset = ru16(&buf, pos + 20) as usize;
                let value = &buf[pos + value_offset..];
                if value.len() >= 66 {
                    let ns = value[65];
                    let take = match best_namespace {
                        None => true,
                        Some(prev) => prev == FILENAME_NAMESPACE_DOS && ns != FILENAME_NAMESPACE_DOS,
                    };
                    if take {
                        parent_frn = ru64(value, 0) & 0x0000_FFFF_FFFF_FFFF;
                        let fname_flags = ru32(value, 56);
                        is_reparse = fname_flags & FILE_ATTR_REPARSE_POINT != 0;
                        let name_len_chars = value[64] as usize;
                        let name_bytes = &value[66..66 + name_len_chars * 2];
                        let utf16: Vec<u16> = name_bytes
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        name = String::from_utf16_lossy(&utf16);
                        best_namespace = Some(ns);
                    }
                }
            }
            ATTR_DATA if name_length == 0 => {
                if non_resident {
                    allocated_size = ru64(&buf, pos + 40);
                    size = ru64(&buf, pos + 48);
                } else {
                    let value_length = ru32(&buf, pos + 16) as u64;
                    size = value_length;
                    allocated_size = value_length;
                }
            }
            _ => {}
        }

        pos += attr_len;
    }

    Some(ParsedRecord {
        in_use,
        is_dir,
        is_extension_record,
        parent_frn,
        name,
        is_reparse,
        size,
        allocated_size,
    })
}
