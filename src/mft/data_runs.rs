/// A contiguous physical extent of an attribute's data: `cluster_count` clusters starting at
/// absolute logical cluster number `lcn`. `lcn` is `None` for sparse ("hole") runs.
pub struct Extent {
    pub lcn: Option<u64>,
    pub cluster_count: u64,
}

/// Decodes an NTFS data-run list (the variable-length encoding used by non-resident
/// attributes to describe which clusters hold their data). Each run is a header byte
/// `(length_size << 4) | offset_size` followed by a little-endian length and a signed
/// little-endian offset (delta from the previous run's LCN); a header byte of 0 terminates
/// the list.
pub fn parse(buf: &[u8]) -> Vec<Extent> {
    let mut extents = Vec::new();
    let mut pos = 0usize;
    let mut current_lcn: i64 = 0;

    while pos < buf.len() {
        let header = buf[pos];
        if header == 0 {
            break;
        }
        let length_size = (header & 0x0F) as usize;
        let offset_size = ((header >> 4) & 0x0F) as usize;
        pos += 1;

        if length_size == 0 || pos + length_size > buf.len() {
            break;
        }
        let cluster_count = read_le_unsigned(&buf[pos..pos + length_size]);
        pos += length_size;

        if offset_size == 0 {
            // Sparse run: no LCN delta, current_lcn is unchanged for subsequent runs.
            extents.push(Extent { lcn: None, cluster_count });
            continue;
        }
        if pos + offset_size > buf.len() {
            break;
        }
        let delta = read_le_signed(&buf[pos..pos + offset_size]);
        pos += offset_size;

        current_lcn += delta;
        if current_lcn < 0 {
            break;
        }
        extents.push(Extent { lcn: Some(current_lcn as u64), cluster_count });
    }

    extents
}

fn read_le_unsigned(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(buf)
}

fn read_le_signed(bytes: &[u8]) -> i64 {
    let mut buf = [0u8; 8];
    buf[..bytes.len()].copy_from_slice(bytes);
    // Sign-extend based on the most significant bit of the last real byte.
    if bytes[bytes.len() - 1] & 0x80 != 0 {
        for b in buf.iter_mut().skip(bytes.len()) {
            *b = 0xFF;
        }
    }
    i64::from_le_bytes(buf)
}
