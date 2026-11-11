//! GUID text ↔ 16 bytes in Windows GUID memory order (spec 15 §8.1):
//! bytes 0–3 as u32le, 4–5 and 6–7 as u16le, then bytes 8–15 in order.

pub fn format(b: &[u8; 16]) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        u16::from_le_bytes([b[4], b[5]]),
        u16::from_le_bytes([b[6], b[7]]),
        b[8],
        b[9],
        b[10],
        b[11],
        b[12],
        b[13],
        b[14],
        b[15]
    )
}

pub fn parse(text: &str) -> Option<[u8; 16]> {
    let t = text.trim().trim_start_matches('{').trim_end_matches('}');
    let shape = t.len() == 36
        && t.bytes().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        });
    if !shape {
        return None;
    }
    let hex: String = t.chars().filter(|c| *c != '-').collect();
    let mut raw = [0u8; 16];
    for (i, b) in raw.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    raw[0..4].reverse();
    raw[4..6].reverse();
    raw[6..8].reverse();
    Some(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let b: [u8; 16] = std::array::from_fn(|i| i as u8);
        let text = format(&b);
        assert_eq!(text, "03020100-0504-0706-0809-0A0B0C0D0E0F");
        assert_eq!(parse(&text), Some(b));
        assert_eq!(parse(&text.to_ascii_lowercase()), Some(b));
        assert_eq!(parse("0302010-00504-0706-0809-0A0B0C0D0E0F"), None);
        assert_eq!(parse("x"), None);
    }
}
