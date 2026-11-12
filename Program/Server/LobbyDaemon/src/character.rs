//! Opt-in character-screen fixture policy. Not a production eligibility check.
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct CharacterPolicy {
    pub slots: HashSet<[u8; 16]>,
    pub worlds: HashSet<i16>,
    pub min_name_units: usize,
    pub max_name_units: usize,
    pub deletion_wait_seconds: u32,
}
impl CharacterPolicy {
    pub fn valid(&self) -> bool {
        self.min_name_units > 0
            && self.min_name_units <= self.max_name_units
            && self.max_name_units <= 32
            && self.deletion_wait_seconds <= i32::MAX as u32
            && !self.slots.contains(&[0; 16])
            && self.worlds.iter().all(|w| *w > 0)
    }
    pub fn validate_name(&self, name: &[u16], user: bool) -> Result<(), i16> {
        if name.is_empty() || name.contains(&0) {
            return Err(6);
        }
        if name.len() < self.min_name_units {
            return Err(if user { 224 } else { 6 });
        }
        if name.len() > self.max_name_units {
            return Err(if user { 225 } else { 6 });
        }
        let mut space = true;
        for &unit in name {
            if unit == 32 {
                if user || space {
                    return Err(if user { 226 } else { 6 });
                }
                space = true;
            } else {
                if !matches!(unit, 65..=90 | 97..=122) && !(user && matches!(unit, 48..=57)) {
                    return Err(if user { 227 } else { 6 });
                }
                space = false;
            }
        }
        if space {
            return Err(if user { 226 } else { 6 });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InternalState {
    Creating,
    Normal,
    Deleting,
    Waiting,
    Restoring,
    Deleted,
}
impl InternalState {
    /// The `CharacterState` column of LobbyDB: 1 creating, 2 normal, 3 deleting,
    /// 4 waiting, 5 restoring, 6 deleted (`11-character-select.md` §15).
    pub fn from_db(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::Creating,
            2 => Self::Normal,
            3 => Self::Deleting,
            4 => Self::Waiting,
            5 => Self::Restoring,
            6 => Self::Deleted,
            _ => return None,
        })
    }
    pub fn db(self) -> u8 {
        match self {
            Self::Creating => 1,
            Self::Normal => 2,
            Self::Deleting => 3,
            Self::Waiting => 4,
            Self::Restoring => 5,
            Self::Deleted => 6,
        }
    }
    pub fn wire(self) -> i8 {
        match self {
            Self::Normal => 1,
            Self::Waiting => 2,
            Self::Deleting | Self::Restoring => 3,
            _ => 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct CreateCharacter {
    pub account: [u8; 16],
    pub slot: [u8; 16],
    pub world: i16,
    pub name: Vec<u16>,
    pub right: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedCharacter {
    pub job: i16,
    pub level: i16,
    pub showcase: Vec<u8>,
}
/// SQL procedure returns are separate from execution errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcedureError(pub i32);
impl std::fmt::Display for ProcedureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "character procedure return {}", self.0)
    }
}
impl std::error::Error for ProcedureError {}
pub fn procedure_error(code: i32) -> std::io::Error {
    std::io::Error::other(ProcedureError(code))
}
