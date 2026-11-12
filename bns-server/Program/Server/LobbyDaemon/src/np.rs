//! Compatibility path for the shared NP channel.
//! The NP deposit id of build 381 is here (`DepositMileageJob::depositReq` `0x1400495f0`,
//! `24-compare-381.md` §3).
pub use shared::np_channel::*;

/// Default of the `BnsCommonConfig` key `np-game-code`.
pub const DEFAULT_NP_GAME_CODE: &str = "bnsprog";

/// Bytes of the game code that the 381 build keeps (`quirk_np_game_code_prefix`).
const NP_GAME_CODE_BYTES: usize = 7;

/// Deposit type of the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepositKind {
    Account,
    /// Deposit type 1. 381 drops the PC sex and race that 348 had.
    Pc {
        pc_db_id: u64,
        pc_job: u8,
    },
}

/// Input of `deposit_id`.
#[derive(Clone, Copy, Debug)]
pub struct DepositIdInput<'a> {
    /// ASCII. The first 7 bytes are used (`quirk_np_game_code_prefix`).
    pub np_game_code: &'a str,
    pub location: i32,
    /// Printed with 2 digits.
    pub location_kind: i32,
    /// `Pc` is deposit type 1.
    pub kind: DepositKind,
    pub request_id: u64,
    pub sub_id: u64,
}

/// `quirk_np_game_code_prefix` (`24-compare-381.md` §7, port `keep value`): 381 converts the
/// code into 7 bytes and does not check the result. The prefix is the first 7 bytes of the
/// ASCII code. A byte that is not ASCII ends the prefix (no memory error, no panic).
fn quirk_np_game_code_prefix(code: &str) -> String {
    code.bytes()
        .take(NP_GAME_CODE_BYTES)
        .take_while(u8::is_ascii)
        .map(char::from)
        .collect()
}

/// Check the `np-game-code` config value. A code that is not ASCII is not accepted
/// (fixed: no memory error).
pub fn check_np_game_code(code: &str) -> Result<&str, &'static str> {
    if code.is_ascii() {
        Ok(code)
    } else {
        Err("np-game-code must be ASCII")
    }
}

/// Make the NP deposit request id of build 381.
/// - location 0: `{prefix}_{request_id}`
/// - location not 0, not type 1: `{prefix}_{location}{kind:02}+_{request_id}`
/// - type 1: `{prefix}_{location}{kind:02}+{pc_db_id}+{pc_job}+{request_id}_{sub_id}`
pub fn deposit_id(input: &DepositIdInput<'_>) -> String {
    let prefix = quirk_np_game_code_prefix(input.np_game_code);
    if input.location == 0 {
        return format!("{prefix}_{}", input.request_id);
    }
    // C `%.2d`: at least 2 digits after the sign.
    let sign = if input.location_kind < 0 { "-" } else { "" };
    let kind = format!("{sign}{:02}", input.location_kind.unsigned_abs());
    let location = input.location;
    match input.kind {
        DepositKind::Account => format!("{prefix}_{location}{kind:02}+_{}", input.request_id),
        DepositKind::Pc { pc_db_id, pc_job } => format!(
            "{prefix}_{location}{kind:02}+{pc_db_id}+{pc_job}+{}_{}",
            input.request_id, input.sub_id
        ),
    }
}
