# Handoff lp2-3-rust (return to lobby over the gamed link)

## Done
- packet: `Message::{Leave, LeaveOk, LeaveFail, LogoutNotify}` (8, 9, 10, 12). Test `leave_messages_round_trip`.
- LobbyDaemon: `leave_world` job, `LeaveOk` / `LeaveFail`, type-2 token. Tests in `tests/leave.rs` (`leave_ok_issues_type2_token`, `leave_token_single_use`, `leave_wrong_cli_key`, `leave_no_session`, `leave_wrong_state`, `leave_job_start_fail_sends_leave_fail`, 1 more).
- GameDaemon: `Link::leave` (timeout, `set_leave_limit`), `Link::logout_notify`, `Effect::LeaveWorld`, `Event::Left`, `play::left`, steps 1–6 as stubs. Tests in `tests/return_to_lobby.rs`: `rtl_ok_sends_27_token`, `rtl_fail_21_to_1954`, `rtl_fail_other_to_1`, `rtl_timeout_1952`, `rtl_arena_5`, `rtl_second_request_1954`, `rtl_without_user_no_reply`.
- `handle_return_to_lobby` parity comment is now `ported`. Abuse notes: `Docs/Plan/abuse-notes/lp2-3-rust.md`.
- `cargo fmt`, `cargo test`, `cargo clippy -D warnings` pass for packet, lobbydaemon, gamedaemon.

## Not done
- Steps 1–6 of the transaction are stubs (`// TODO ❓ step N` in `play.rs`). No subsystem exists.
- `LogoutNotify` only logs on LobbyDaemon (`TODO ❓ lp2-4-rust`): `Jobs::logout` has no path for state 4.0 (task 118).
- Item 6: `Server::retain` stays. Many tests use it, and the link path does not cover them.
- `main.rs` of LobbyDaemon (outside Files) must call `LinkGameDaemon::set_leave` with `Server::leave_world`.

## ❓
- `init_retained` gives reason 20 (not 19) when no token is found. 19 only for a found, expired token. Tier 2. `tests/rules.rs` changed to match.
- Default `lobby_token_lifetime` 120 s and leave timeout 10 s are local values. Original values are not verified.
- A second 26 gives 28/1954. The spec is not clear.
- `handlers/mod.rs` is outside Files, so no `DaemonRequest::LobbyLeaveWorld`. The handler keeps `ctx.queue(26, …)`. The live path is `play.rs`.
- `tests/gr2_handlers.rs` was not changed: its 26 test still passes (queue Fail → 28/2).

## Blocked
- None.
