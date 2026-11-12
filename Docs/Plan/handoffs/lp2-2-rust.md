# Handoff lp2-2-rust

## Done
- `Settings::charge` and `Settings::grade` (default false). Comments say the on-paths are not ported.
- New file `src/jobs/services.rs`: `charge_info` (60), `activate_grade` (106), `account_item_service` (82–100). Each has a doc comment with the 307 handler address. All run in `Jobs::handle_fast`.
- With `charge` or `grade` on: Fail code 2 and a log line.
- 91 rename sends command 93 (comment: "bug, not ported").
- Tests in `tests/services_off.rs`: `charge_info_off`, `charge_info_no_session`, `charge_info_on_is_not_ported`, `grade_off_before_session`, `grade_off_with_session_and_malformed`, `item_service_off` (all 7 commands), `item_service_no_session`, `item_service_zero_input`, `item_service_malformed`, `rename_fail_command`.
- `cargo fmt`, `cargo test -p lobbydaemon`, `cargo clippy --all-targets` (also with `--features mssql`) all pass.

## Not done
- No on-paths (NP charge, NP grade, account-inventory link), as the card says.
- No abuse note: the new handlers only send Fail packets and read no client value into state.

## ❓
- 94 (AddSlot): spec `19-jobs-g.md` §1 says bad parameter → code 1 ❓. Implemented: zero label id or item id → 1, then not 2.0 → 5, then 7.
- 97 (UserName): "parameter absent or empty → 1 ❓". Implemented: empty name or zero id → 1, then 7 (the spec puts 7 before 5).
- 100 (Jumping quota): the spec gives no init order and no zero rule. Implemented as 97 (zero → 1, then 7). The spec says "same skeleton as `17-jobs-e.md`", which would give no reply for zero input.
- 82–91: the zero check covers slot id, label id and item instance id. The "channel" of the spec is the account-inventory channel, not a packet field.
- 60 and 106: the handler checks the session only, not the state (2.0). The spec says the job checks the state, so the on-path is not reached.

## Blocked
- None.
