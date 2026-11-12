# Handoff lp2-4-rust

## Done
- Packet: `Message::{Kick,KickOk,KickFail}` (14/15/16); test `message_round_trip`.
- LobbyDaemon: `logout_in_world` (message 12 `LogoutNotify`), `world_players`, `kick_world`, link close ends 4.0 sessions of the world, `Control::{LogoutInWorld,WorldClosed}`.
- GameDaemon: `link.rs` (`logout_notify`, kick hook, `KICK_NOT_FOUND`=24, `KICK_FAIL`=2), `shard.rs` (`Event::Kick`, `World::kick`).
- Tests in `LobbyDaemon/tests/world_exit.rs` (10): `logout_in_world_ends_session`, `logout_in_world_wrong_pcid`, `world_link_close_ends_sessions`, `kick_in_world_ok`, `kick_in_world_timeout`, `kick_does_not_block_link` and 4 more.
- Tests in `GameDaemon/tests/return_to_lobby.rs`: `gamed_kick_disconnects_pc`, `close_in_world_sends_logout_notify`, `close_without_user_sends_no_logout_notify`, `close_after_leave_sends_no_logout_notify`.
- Choice for task 4: the kick runs as a job task in the link task (`LinkTask::start`). The reply goes out after the merge.
- Checks pass: fmt, test (3 crates), clippy (lobbydaemon with `mssql`).

## Not done
- Shard unit test: kick of a `moved` session forwards to the new shard.
- PC save is a stub (`play.rs` TODO step 3): `LogoutNotify` goes out without a save.

## ❓
- 4th `q` of `Kick` (`hqqqqQ`) is unknown. We send 0 and ignore it.
- `GetGameWorldKickReason` is not verified. We use the gate mapping (`kick_reason`).
- `Server::kick` returns `Ok(())` also for result 2.
- If a leave fails, `logged_out` stays true: a later close sends no `LogoutNotify` (as the original). The lobby session stays 4.0 until link close.
- Client packet 951 on kick has no official name: not sent.

## Blocked
- LobbyDaemon `main.rs` (outside Files) must call `LinkGameDaemon::attach(server)`. GameDaemon `main` needs no change.
