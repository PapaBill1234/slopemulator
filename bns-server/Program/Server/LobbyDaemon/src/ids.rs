//! Official LobbyDaemon command IDs as `i16` constants, for `match` arms.
//!
//! `build.rs` makes one constant for each variant of the `packet-ids` enums
//! `LobbyGateLobbydCmd` (module `gate`), `GameLobbydCmd` (module `game`) and
//! `AccountInventoryLobbydCmd` (module `account_inventory`). The constant has the name of the
//! variant and takes its value from `<Enum>::<name>.id()`. A wrong name does not compile.
//! Only rows with a verified 307 name have a constant.

include!(concat!(env!("OUT_DIR"), "/lobbyd_cmd_ids.rs"));
