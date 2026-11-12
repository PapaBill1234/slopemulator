# Handoff ld381-1-rust

## Done
- `gamed_link.rs`: `HELLO_OK` = `hCQiiiqqChiiii;;qqqi`, `ENTER_OK` = 381 string (list entry `ihcccc`, 2 new empty lists `Cihqq;` and `Chq;` at the end). Header cites `0x14017ba60` and `0x140179cd0`. `PACKET_VERSION = 83` is marked verified (`0x140083ee0`). The ❓ lines about 381 are removed.
- `EnterOk` encode adds 2 empty lists. Decode accepts and skips non-empty new lists.
- `round_trip` comment now says 18 for each rule. The size assert is unchanged (the rule lists stay empty).
- `np.rs`: `DepositKind`, `DepositIdInput`, `deposit_id`, `check_np_game_code`, `DEFAULT_NP_GAME_CODE` (`bnsprog`), private `quirk_np_game_code_prefix`.
- `main.rs`: new argument `--np-game-code` (ASCII check, default `bnsprog`).
- Tests: `round_trip`, `hello_ok_381_format`, `enter_ok_381_tail` (packet); `deposit_id_location_zero`, `deposit_id_location`, `deposit_id_pc`, `deposit_id_long_code`, `deposit_id_short_code_and_bad_input` (`tests/np_deposit_id.rs`).
- `cargo fmt`, `cargo test` (packet, lobbydaemon, gamedaemon) and `cargo clippy -D warnings` pass.

## Not done
- The new `EnterOk` lists have no data (needs commands 110 and 113: `ld381-r1`, `ld381-r4`, `ld381-r5`).
- The deposit job does not call `deposit_id` yet. `main.rs` only logs the value.

## ❓
- I put `np_game_code` in a start argument, not in `Settings`: `jobs.rs` is not in the card Files. The job slice must pass it on.
- `quirk_np_game_code_prefix`: not tested on Windows (as the card says).
- `hello_ok_381_format` checks the constant. The 381 `HelloOk` rule entry has no encode data, so no byte test of the entry exists.
- `%.2d` with a negative `location_kind` is made like C (sign, then 2 digits). Not seen in normal play.

## Blocked
- None.
