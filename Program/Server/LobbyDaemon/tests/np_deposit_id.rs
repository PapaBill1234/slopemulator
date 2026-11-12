//! NP deposit id of build 381 (`24-compare-381.md` §3, §7).
use lobbydaemon::np::{check_np_game_code, deposit_id, DepositIdInput, DepositKind};

fn input(code: &str, location: i32, kind: DepositKind) -> DepositIdInput<'_> {
    DepositIdInput {
        np_game_code: code,
        location,
        location_kind: 2,
        kind,
        request_id: 77,
        sub_id: 9,
    }
}

#[test]
fn deposit_id_location_zero() {
    let id = deposit_id(&input("bnsprog", 0, DepositKind::Account));
    assert_eq!(id, "bnsprog_77");
}

#[test]
fn deposit_id_location() {
    let id = deposit_id(&input("bnsprog", 3, DepositKind::Account));
    assert_eq!(id, "bnsprog_302+_77");
}

#[test]
fn deposit_id_pc() {
    let kind = DepositKind::Pc {
        pc_db_id: 5001,
        pc_job: 4,
    };
    assert_eq!(
        deposit_id(&input("bnsprog", 3, kind)),
        "bnsprog_302+5001+4+77_9"
    );
}

#[test]
fn deposit_id_long_code() {
    // `quirk_np_game_code_prefix`: first 7 bytes.
    let id = deposit_id(&input("bnsprog2", 0, DepositKind::Account));
    assert_eq!(id, "bnsprog_77");
}

#[test]
fn deposit_id_short_code_and_bad_input() {
    assert_eq!(deposit_id(&input("bns", 0, DepositKind::Account)), "bns_77");
    // A non-ASCII code ends the prefix. No panic.
    assert_eq!(
        deposit_id(&input("bn\u{e9}prog", 0, DepositKind::Account)),
        "bn_77"
    );
    assert!(check_np_game_code("bn\u{e9}prog").is_err());
    assert!(check_np_game_code("bnsprog").is_ok());
}
