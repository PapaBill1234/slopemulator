//! Official packet command IDs and enum values from the client RE tables.
//!
//! `generated/` is made by the bin `gen-ids`. The test `generated_is_current` makes sure
//! that the checked-in files are the same as the generator output.

use std::path::{Path, PathBuf};

mod gen;
pub use gen::{generate, generate_lobbyd, is_valid_ident, sanitize, GenError};

pub mod generated {
    #[rustfmt::skip]
    pub mod commands;
    #[rustfmt::skip]
    pub mod enums;
    #[rustfmt::skip]
    pub mod lobbyd;
}

pub use generated::commands::{GameCmd, InfoGateCmd, LobbyCmd, StsCmd};
pub use generated::enums::*;
pub use generated::lobbyd::{AccountInventoryLobbydCmd, GameLobbydCmd, LobbyGateLobbydCmd};

/// Workspace root, found from the location of this crate.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Directory of the generated files for a workspace root.
pub fn generated_dir(root: &Path) -> PathBuf {
    root.join("Program/Server/PacketIds/src/generated")
}
