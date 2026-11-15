//! Property table of command 32 (build 381, static FieldDef table `0x1407CA330`, 422 rows).
//! Source: `Docs/Server/GameDaemon/05-enter-world.md`, "Command 32 property table (381)".
//! One [`Run`] is a range of ids with the same type. The offset is the byte offset of the first
//! id in the Property object (FieldDef +40). It does not go on the wire.
//! Wire rules: `Property::GetPureData` `0x140697E50`, pure mode 1.

/// FieldDef type. The stat types keep more than one value in the object. The wire carries one
/// signed value of the given width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Type 1: i8, 1 byte.
    I8,
    /// Type 2: i16, 2 bytes.
    I16,
    /// Type 3: i32, 4 bytes.
    I32,
    /// Type 4: i64, 8 bytes.
    I64,
    /// Type 7: stat, 2 bytes on the wire (object stride 8).
    Stat16,
    /// Type 8: stat, 4 bytes on the wire (object stride 16).
    Stat32,
    /// Type 9: stat, 8 bytes on the wire (object stride 32).
    Stat64,
    /// Type 10: bit field. The first row of a byte sends the byte, the others send nothing.
    Bit,
    /// Type 11: bool. Same bit-run rule as type 10.
    Bool,
    /// Type 12: wide string, u16 character count and 2 bytes for each character (id 97).
    WStr,
    /// Type 14: byte blob, i16 length and the bytes (id 150).
    Blob,
}

impl Kind {
    /// Bytes of a fixed-size value. `None`: a bit row or a variable-size value.
    pub const fn size(self) -> Option<usize> {
        match self {
            Kind::I8 => Some(1),
            Kind::I16 | Kind::Stat16 => Some(2),
            Kind::I32 | Kind::Stat32 => Some(4),
            Kind::I64 | Kind::Stat64 => Some(8),
            _ => None,
        }
    }
}

/// Ids `first..=last` (inclusive) share one type.
pub struct Run {
    pub first: u16,
    pub last: u16,
    pub kind: Kind,
    pub offset: u16,
}

const fn run(first: u16, last: u16, kind: Kind, offset: u16) -> Run {
    Run {
        first,
        last,
        kind,
        offset,
    }
}

pub const PROPERTY_COUNT: u16 = 422;
/// Fixed part of the blob for an empty name and an empty survey, without the two length fields.
pub const BLOB_FIXED: usize = 1020;
/// Longest name (characters) and survey (bytes) of the table.
pub const NAME_MAX: usize = 32;
pub const SURVEY_MAX: usize = 10;

use Kind::{Bit, Blob, Bool, Stat16, Stat32, Stat64, WStr, I16, I32, I64, I8};

pub static RUNS: &[Run] = &[
    run(1, 1, I16, 48),
    run(2, 4, I8, 50),
    run(5, 96, I8, 53),
    run(97, 97, WStr, 146),
    run(98, 98, I32, 216),
    run(99, 102, I16, 220),
    run(103, 103, I8, 228),
    run(104, 104, I32, 232),
    run(105, 105, I8, 236),
    run(106, 110, I64, 240),
    run(111, 112, I8, 280),
    run(113, 113, I64, 288),
    run(114, 114, I8, 296),
    run(115, 115, I32, 300),
    run(116, 121, I16, 304),
    run(122, 122, I8, 316),
    run(123, 123, I64, 320),
    run(124, 126, I8, 328),
    run(127, 127, I64, 336),
    run(128, 128, I8, 344),
    run(129, 129, I64, 352),
    run(130, 130, I32, 360),
    run(131, 142, I64, 368),
    run(143, 143, I8, 464),
    run(144, 144, I64, 472),
    run(145, 149, I8, 480),
    run(150, 150, Blob, 486),
    run(151, 153, I16, 498),
    run(154, 154, I32, 504),
    run(155, 156, I8, 508),
    run(157, 160, I32, 512),
    run(161, 165, I8, 528),
    run(166, 173, Bit, 533),
    run(174, 174, I64, 536),
    run(175, 176, I8, 544),
    run(177, 177, I32, 548),
    run(178, 194, I64, 552),
    run(195, 195, I16, 688),
    run(196, 199, I64, 696),
    run(200, 200, I16, 728),
    run(201, 201, I64, 736),
    run(202, 202, I8, 744),
    run(203, 204, I64, 752),
    run(205, 206, I16, 768),
    run(207, 207, I8, 772),
    run(208, 209, I32, 776),
    run(210, 211, I64, 784),
    run(212, 212, I16, 800),
    run(213, 213, I64, 808),
    run(214, 214, I32, 816),
    run(215, 215, I64, 824),
    run(216, 216, I8, 832),
    run(217, 217, I32, 836),
    run(218, 241, Bit, 840),
    run(242, 242, I8, 843),
    run(243, 243, I32, 844),
    run(244, 244, Stat64, 848),
    run(245, 245, Stat32, 880),
    run(246, 246, Stat64, 896),
    run(247, 247, Stat32, 928),
    run(248, 251, Stat16, 944),
    run(252, 255, Stat32, 976),
    run(256, 264, Stat16, 1040),
    run(265, 266, Stat32, 1112),
    run(267, 283, Stat16, 1144),
    run(284, 285, Stat32, 1280),
    run(286, 287, Stat16, 1312),
    run(288, 289, Stat32, 1328),
    run(290, 294, Stat16, 1360),
    run(295, 295, Stat32, 1400),
    run(296, 296, Stat16, 1416),
    run(297, 297, Stat32, 1424),
    run(298, 320, Stat16, 1440),
    run(321, 322, Stat32, 1624),
    run(323, 324, Stat16, 1656),
    run(325, 325, Stat32, 1672),
    run(326, 335, Stat16, 1688),
    run(336, 336, Stat32, 1768),
    run(337, 337, Stat16, 1784),
    run(338, 338, Stat32, 1792),
    run(339, 343, Stat16, 1808),
    run(344, 367, Bool, 1848),
    run(368, 368, I32, 1872),
    run(369, 369, I8, 1876),
    run(370, 370, I32, 1880),
    run(371, 371, I8, 1884),
    run(372, 373, I32, 1888),
    run(374, 374, I64, 1896),
    run(375, 382, I8, 1904),
    run(383, 384, I32, 1912),
    run(385, 387, I8, 1920),
    run(388, 388, I32, 1924),
    run(389, 391, I8, 1928),
    run(392, 392, I32, 1932),
    run(393, 395, I8, 1936),
    run(396, 401, I32, 1940),
    run(402, 405, Stat16, 1968),
    run(406, 409, Stat32, 2000),
    run(410, 422, Stat16, 2064),
];

/// Type of property `id` (1..=422). `None` for any other id.
pub fn kind(id: u16) -> Option<Kind> {
    // The runs are sorted and have no gap (checked by a test): search by the last id.
    let index = RUNS.partition_point(|run| run.last < id);
    let run = RUNS.get(index)?;
    (run.first <= id).then_some(run.kind)
}

/// Reference encoder of the property blob (`Property::GetPureData`, pure mode 1, ids 1..=422).
/// `scalar(id)` gives the value of a fixed-size row (the low bytes go on the wire; a bit or
/// bool row is set when the value is not 0). `name`: UTF-16 units of id 97. `survey`: id 150.
/// A longer name or survey is cut to the table maximum.
pub fn encode_blob(scalar: impl Fn(u16) -> u64, name: &[u16], survey: &[u8]) -> Vec<u8> {
    let name = &name[..name.len().min(NAME_MAX)];
    let survey = &survey[..survey.len().min(SURVEY_MAX)];
    let mut out = Vec::with_capacity(BLOB_FIXED + 4 + 2 * name.len() + survey.len());
    let mut id = 1;
    while id <= PROPERTY_COUNT {
        let Some(kind) = kind(id) else { break };
        match kind {
            Bit | Bool => {
                // The row with bit 0 sends the byte. The next 7 rows (or fewer at the end of
                // the run) are part of it and send nothing.
                let mut byte = 0u8;
                let mut next = id;
                for bit in 0..8 {
                    if self::kind(next) != Some(kind) {
                        break;
                    }
                    if scalar(next) != 0 {
                        byte |= 1 << bit;
                    }
                    next += 1;
                }
                out.push(byte);
                id = next;
                continue;
            }
            WStr => {
                out.extend((name.len() as u16).to_le_bytes());
                for unit in name {
                    out.extend(unit.to_le_bytes());
                }
            }
            Blob => {
                out.extend((survey.len() as i16).to_le_bytes());
                out.extend(survey);
            }
            fixed => {
                let size = fixed.size().unwrap_or(0);
                out.extend(&scalar(id).to_le_bytes()[..size]);
            }
        }
        id += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_cover_1_to_422_without_gap() {
        let mut next = 1;
        for run in RUNS.iter() {
            assert_eq!(run.first, next);
            assert!(run.last >= run.first);
            next = run.last + 1;
        }
        assert_eq!(next, PROPERTY_COUNT + 1);
        assert_eq!(kind(0), None);
        assert_eq!(kind(423), None);
        assert_eq!(kind(97), Some(WStr));
        assert_eq!(kind(422), Some(Stat16));
    }

    #[test]
    fn empty_blob_has_the_size_of_the_spec() {
        // 1,020 + (2 + 2n) + (2 + survey) = 1,024 for n = 0 and no survey.
        assert_eq!(encode_blob(|_| 0, &[], &[]).len(), BLOB_FIXED + 4);
        assert_eq!(encode_blob(|_| 0, &[0x41; 5], &[]).len(), 1024 + 10);
    }

    #[test]
    fn bit_runs_make_one_byte_for_eight_rows() {
        // Row 168 is bit 2 of the byte of row 166. Rows 344 and 351 are bits 0 and 7.
        let blob = encode_blob(|id| u64::from([168, 344, 351].contains(&id)), &[], &[]);
        assert_eq!(blob.len(), 1024);
        // Bytes before id 166: ids 1..=165 without 97 and 150 (counted as 2 each).
        let mut fixed = 0;
        for id in 1..166 {
            fixed += kind(id).and_then(Kind::size).unwrap_or(2);
        }
        assert_eq!(blob[fixed], 0b100);
    }
}
