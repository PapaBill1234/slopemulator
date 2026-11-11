//! FSD records ("format + symbol data"), as GameDaemon builds them.
//! Source: build 348 GameDaemon (PDB names): `FsdComposer::ComposeV`
//! 0x1406E9900, `ComposeShowcaseFsd` 0x1400BD3E0, `DecomposeMakePcFsd`
//! 0x1400BCC10. Build 381 has the same showcase format string.
//!
//! Layout: `u16 total size`, `u16 format size`, format chars, data.
//! Data: for each format char a `u16` symbol id, then the value.
//! `c` 1 byte, `w` 2, `d` 4, `q` 8, `s` UTF-16 text with an end zero,
//! `N` a `u16` count; the next format char repeats count times without ids.

/// Format of `FsdShowcaseRecord` (59 fields, symbol ids 0..=57 with `N` at 3).
pub const SHOWCASE_FORMAT: &str = "cccNcssdwwwcddwqdcddccccccccddcccdcccdcccqdcddddccccwcwccdd";
pub const APPEARANCE_BYTES: usize = 92;

/// `FsdMakePcRecord`: what the client sends in `L_CreateCharacterReq`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakePc {
    pub race: u8,
    pub sex: u8,
    pub job: u8,
    pub appearance: [u8; APPEARANCE_BYTES],
}

/// Symbols of the make-PC record: 0 race, 1 sex, 2 job, 3 appearance, 4 name.
pub fn decode_make_pc(fsd: &[u8]) -> Option<MakePc> {
    let format_size = usize::from(u16::from_le_bytes([*fsd.get(2)?, *fsd.get(3)?]));
    let format = fsd.get(4..4 + format_size)?;
    let mut data = fsd.get(4 + format_size..)?;
    let mut out = MakePc {
        race: 0,
        sex: 0,
        job: 0,
        appearance: [0; APPEARANCE_BYTES],
    };
    let mut take = |n: usize| -> Option<&[u8]> {
        let (head, rest) = data.split_at_checked(n)?;
        data = rest;
        Some(head)
    };
    let mut chars = format.iter().copied();
    while let Some(kind) = chars.next() {
        let Some(id) = take(2) else {
            break; // The record can end before the format does.
        };
        let id = u16::from_le_bytes([id[0], id[1]]);
        match kind {
            b'c' => {
                let v = take(1)?[0];
                match id {
                    0 => out.race = v,
                    1 => out.sex = v,
                    2 => out.job = v,
                    _ => {}
                }
            }
            b'N' => {
                let count = take(2)?;
                let count = usize::from(u16::from_le_bytes([count[0], count[1]]));
                let width = match chars.next()? {
                    b'c' => 1,
                    b'w' => 2,
                    b'd' | b'f' => 4,
                    b'q' => 8,
                    _ => return None,
                };
                let bytes = take(count * width)?;
                if id == 3 && width == 1 {
                    let n = count.min(APPEARANCE_BYTES);
                    out.appearance[..n].copy_from_slice(&bytes[..n]);
                }
            }
            b'w' => {
                take(2)?;
            }
            b'd' | b'f' => {
                take(4)?;
            }
            b'q' => {
                take(8)?;
            }
            b's' => loop {
                let unit = take(2)?;
                if unit == [0, 0] {
                    break;
                }
            },
            _ => return None,
        }
    }
    Some(out)
}

/// Showcase of a new character: race, sex, job, appearance, name and level.
/// ❓ All other fields are zero. The original fills position, hp and the
/// first equipment from the `PcInitialSetting` data (`CreateCharacterTran`).
pub fn compose_new_showcase(pc: &MakePc, name: &[u16], level: u8) -> Vec<u8> {
    let mut data = Vec::new();
    let mut id = 0u16;
    let mut chars = SHOWCASE_FORMAT.bytes();
    while let Some(kind) = chars.next() {
        data.extend(id.to_le_bytes());
        match (id, kind) {
            (0, _) => data.push(pc.race),
            (1, _) => data.push(pc.sex),
            (2, _) => data.push(pc.job),
            (3, _) => {
                chars.next(); // the repeated `c`
                data.extend((APPEARANCE_BYTES as u16).to_le_bytes());
                data.extend(pc.appearance);
            }
            (4, _) => {
                for unit in name.iter().take(32) {
                    data.extend(unit.to_le_bytes());
                }
                data.extend([0, 0]);
            }
            // Symbol 10 is `level` (after title, geo_zone, x, y, z).
            (10, _) => data.push(level),
            (_, b'c') => data.push(0),
            (_, b'w') => data.extend([0; 2]),
            (_, b'd') => data.extend([0; 4]),
            (_, b'q') => data.extend([0; 8]),
            (_, b's') => data.extend([0; 2]),
            _ => unreachable!("showcase format"),
        }
        id += 1;
    }
    let total = 4 + SHOWCASE_FORMAT.len() + data.len();
    let mut out = Vec::with_capacity(total);
    out.extend((total as u16).to_le_bytes());
    out.extend((SHOWCASE_FORMAT.len() as u16).to_le_bytes());
    out.extend(SHOWCASE_FORMAT.bytes());
    out.extend(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pc_fsd() -> Vec<u8> {
        let format = b"cccNcs";
        let mut data = Vec::new();
        for (id, v) in [(0u16, 2u8), (1, 1), (2, 5)] {
            data.extend(id.to_le_bytes());
            data.push(v);
        }
        data.extend(3u16.to_le_bytes());
        data.extend(92u16.to_le_bytes());
        data.extend((0..92).map(|i| i as u8));
        data.extend(4u16.to_le_bytes());
        for unit in "Abc".encode_utf16() {
            data.extend(unit.to_le_bytes());
        }
        data.extend([0, 0]);
        let total = (4 + format.len() + data.len()) as u16;
        let mut out = total.to_le_bytes().to_vec();
        out.extend((format.len() as u16).to_le_bytes());
        out.extend(format);
        out.extend(data);
        out
    }

    #[test]
    fn make_pc_to_showcase() {
        let pc = decode_make_pc(&make_pc_fsd()).unwrap();
        assert_eq!((pc.race, pc.sex, pc.job), (2, 1, 5));
        assert_eq!(pc.appearance[91], 91);
        let name: Vec<u16> = "Abc".encode_utf16().collect();
        let fsd = compose_new_showcase(&pc, &name, 1);
        assert!(fsd.len() <= 512);
        assert_eq!(usize::from(u16::from_le_bytes([fsd[0], fsd[1]])), fsd.len());
        assert_eq!(u16::from_le_bytes([fsd[2], fsd[3]]), 59);
        assert_eq!(&fsd[4..63], SHOWCASE_FORMAT.as_bytes());
        // id 0, race; id 1, sex; id 2, job; id 3, count 92.
        assert_eq!(&fsd[63..76], [0, 0, 2, 1, 0, 1, 2, 0, 5, 3, 0, 92, 0]);
        // The showcase has the make-PC symbols at the front: it decodes the same way.
        assert_eq!(decode_make_pc(&fsd).unwrap(), pc);
        assert!(decode_make_pc(&[1, 2, 3]).is_none());
    }
}
