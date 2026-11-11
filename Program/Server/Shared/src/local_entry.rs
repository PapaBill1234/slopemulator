//! Local hand-off of a character from `lobbyd` to `gamedaemon` (card play-1).
//! ❓ Not parity. The original uses the LobbyDaemon ↔ GameDaemon link and the game DB.
//! `lobbyd` writes one file for each admitted world entry. `gamedaemon` reads it at command 23.
//! Directory: environment variable `BNS_LOCAL_ENTRY_DIR`. File: `<client key>.pc`.
//! Layout: `u64 pcid`, race, sex, job, 92 appearance bytes, name (UTF-16LE units).
use crate::fsd::{MakePc, APPEARANCE_BYTES};
use std::{fs, io, path::PathBuf};

pub const DIR_VARIABLE: &str = "BNS_LOCAL_ENTRY_DIR";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalEntry {
    pub pcid: u64,
    pub pc: MakePc,
    pub name: Vec<u16>,
}

fn dir() -> Option<PathBuf> {
    std::env::var_os(DIR_VARIABLE).map(PathBuf::from)
}

impl LocalEntry {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.pcid.to_le_bytes().to_vec();
        out.extend([self.pc.race, self.pc.sex, self.pc.job]);
        out.extend(self.pc.appearance);
        for unit in &self.name {
            out.extend(unit.to_le_bytes());
        }
        out
    }
    pub fn decode(data: &[u8]) -> Option<Self> {
        let head = 11 + APPEARANCE_BYTES;
        if data.len() < head || !(data.len() - head).is_multiple_of(2) {
            return None;
        }
        Some(Self {
            pcid: u64::from_le_bytes(data[..8].try_into().ok()?),
            pc: MakePc {
                race: data[8],
                sex: data[9],
                job: data[10],
                appearance: data[11..head].try_into().ok()?,
            },
            name: data[head..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|u| u16::from_le_bytes(*u))
                .collect(),
        })
    }
    /// No directory variable: no hand-off, no error.
    pub fn write(&self, client_key: u64) -> io::Result<()> {
        let Some(dir) = dir() else { return Ok(()) };
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("{client_key}.pc")), self.encode())
    }
    /// The file of `client_key`. ❓ Fallback: the newest file, because the
    /// relation "lobby owner = command-23 client key" has no live proof yet.
    pub fn read(client_key: u64) -> Option<Self> {
        let dir = dir()?;
        if let Ok(data) = fs::read(dir.join(format!("{client_key}.pc"))) {
            return Self::decode(&data);
        }
        let newest = fs::read_dir(&dir)
            .ok()?
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "pc"))
            .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
        Self::decode(&fs::read(newest.path()).ok()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip() {
        let entry = LocalEntry {
            pcid: 7,
            pc: MakePc {
                race: 1,
                sex: 2,
                job: 3,
                appearance: [9; APPEARANCE_BYTES],
            },
            name: vec![0x41, 0x42],
        };
        assert_eq!(LocalEntry::decode(&entry.encode()), Some(entry));
        assert_eq!(LocalEntry::decode(&[0; 5]), None);
    }
}
