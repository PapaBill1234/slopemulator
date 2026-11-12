//! Build script. It writes one `pub const <Name>: i16` for each variant of the LobbyDaemon
//! command enums of `packet-ids` (`LobbyGateLobbydCmd`, `GameLobbydCmd`,
//! `AccountInventoryLobbydCmd`).
//!
//! The source is the generated file `lobbyd.rs` of `packet-ids`. The constants let `match`
//! arms use official names. The value of each constant comes from `<Enum>::<Name>.id()`.
//! `wire::command` gives an `i16`, so the constants are `i16`.

use std::{env, fs, path::PathBuf};

/// Enum name and the module of its constants.
const ENUMS: [(&str, &str); 3] = [
    ("LobbyGateLobbydCmd", "gate"),
    ("GameLobbydCmd", "game"),
    ("AccountInventoryLobbydCmd", "account_inventory"),
];

fn main() {
    let source = "../PacketIds/src/generated/lobbyd.rs";
    println!("cargo:rerun-if-changed={source}");
    let text = fs::read_to_string(source).expect("read packet-ids lobbyd.rs");
    let mut out = String::new();
    for (enum_name, module) in ENUMS {
        let header = format!("pub enum {enum_name} ");
        let mut inside = false;
        out.push_str(&format!(
            "pub mod {module} {{\n    use packet_ids::{enum_name};\n"
        ));
        for line in text.lines() {
            if line.starts_with(&header) {
                inside = true;
                continue;
            }
            if !inside {
                continue;
            }
            if line.starts_with('}') {
                break;
            }
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }
            if let Some((name, _)) = trimmed.split_once(" = ") {
                out.push_str(&format!(
                    "    #[allow(non_upper_case_globals, dead_code)]\n    pub const {name}: i16 = {enum_name}::{name}.id();\n"
                ));
            }
        }
        out.push_str("}\n");
    }
    let path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("lobbyd_cmd_ids.rs");
    fs::write(path, out).expect("write lobbyd_cmd_ids.rs");
}
