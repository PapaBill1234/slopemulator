//! GUID text of spec 15 §8.1: `%08X-%04X-%04X-%02X%02X-%02X…`, upper case.
use rand::RngCore;

/// Bytes 0–3 as u32le, 4–5 and 6–7 as u16le, then bytes 8–15 in order.
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

/// Upper-case GUID text, or `None` when the text is not a GUID.
pub fn normalize(text: &str) -> Option<String> {
    let t = text.trim().trim_start_matches('{').trim_end_matches('}');
    let ok = t.len() == 36
        && t.bytes().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        });
    ok.then(|| t.to_ascii_uppercase())
}

/// New id from 16 random bytes (the fallback path of the original, §2).
pub fn random() -> String {
    let mut b = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut b);
    format(&b)
}

/// UserId of a `t` header. Forms: `<GUID>`, `<dc>$<GUID>`, each with optional `;params`.
pub fn from_target(t: &str) -> Option<String> {
    let t = t.split(';').next().unwrap_or("");
    normalize(t.rsplit('$').next().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_example() {
        let b: [u8; 16] = std::array::from_fn(|i| i as u8);
        assert_eq!(format(&b), "03020100-0504-0706-0809-0A0B0C0D0E0F");
    }

    #[test]
    fn target_forms() {
        let g = "03020100-0504-0706-0809-0a0b0c0d0e0f";
        let up = g.to_ascii_uppercase();
        assert_eq!(from_target(g), Some(up.clone()));
        assert_eq!(from_target(&format!("1${g};x=1")), Some(up));
        assert_eq!(from_target("login:name"), None);
    }
}
