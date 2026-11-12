//! Write `src/generated/commands.rs`, `src/generated/enums.rs` and `src/generated/lobbyd.rs`.
//!
//! Usage: `cargo run -q -p packet-ids --bin gen-ids [workspace-root]`

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = match std::env::args_os().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => packet_ids::workspace_root(),
    };
    let (commands, enums) = match packet_ids::generate(&root) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("gen-ids: {e}");
            return ExitCode::FAILURE;
        }
    };
    let lobbyd = match packet_ids::generate_lobbyd(&root) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("gen-ids: {e}");
            return ExitCode::FAILURE;
        }
    };
    let dir = packet_ids::generated_dir(&root);
    for (name, text) in [
        ("commands.rs", commands),
        ("enums.rs", enums),
        ("lobbyd.rs", lobbyd),
    ] {
        let path = dir.join(name);
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("gen-ids: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
