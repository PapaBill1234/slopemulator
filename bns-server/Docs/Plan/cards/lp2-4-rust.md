# Card lp2-4-rust: logout in world and kick over the GameDaemon link

## Goal
When a player leaves the game from the world (client disconnect or logout), GameDaemon tells LobbyDaemon, and LobbyDaemon ends the session. A LobbyDaemon kick of a player in the world goes to GameDaemon and waits for its result.

## Context
- Crates: `lobbydaemon` (`Program/Server/LobbyDaemon`), `gamedaemon` (`Program/Server/GameDaemon`), `packet` (`Program/Server/Packet`). Check the package names in each `Cargo.toml`. Read `Docs/Architecture/scale-rules.md` §3–§6 first.
- The gamed link is our own protocol: `Program/Server/Packet/src/gamed_link.rs` (`Message`). LobbyDaemon side: `src/gamed_link.rs` (`LinkGameDaemon`). GameDaemon side: `src/link.rs`. Add new `Message` variants with encode, decode and a round-trip test, at both ends.
- Card `lp2-3-rust` (done before this card) added return to lobby on the same link. Read its handoff `Docs/Plan/handoffs/lp2-3-rust.md` and `Docs/Server/LobbyDaemon/23-return-to-lobby.md` §"other exits" for the GameDaemon send sites of `LogoutNotify`.
- LobbyDaemon job core (step 6 part 1b): one link task for each LobbyGate link owns `Jobs`. Other tasks reach it with a `Control` message and a `oneshot` reply (`src/lib.rs`, `Server::kick` and others). Find the link of a session with `owners`. Keep this structure.
- Session states: `State` doc comments in `src/jobs.rs` (spec state 4.0 = in world).

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| LogoutInGameWorldJob | `Docs/Server/LobbyDaemon/13-jobs-a.md` | §3.7 |
| KickJob | `Docs/Server/LobbyDaemon/18-jobs-f.md` | §5 (lines 181–208) |
| Original link messages 12, 14, 15, 16 and link close | `Docs/Server/LobbyDaemon/07-gamed-link.md` | lines 559, 622, 828–831 (file is over 40,000 bytes: read only these lines) |
| Return to lobby | `Docs/Server/LobbyDaemon/23-return-to-lobby.md` | all |
| GameDaemon kick boundary | `Program/Server/GameDaemon/src/kick.rs`, `Docs/Server/GameDaemon/10-session-handlers.md` | `rg -n "kick" -i` |

## Work
1. Link message `LogoutInWorld { world, client, pcid }` (GameDaemon → LobbyDaemon, original 12 `hqq`). GameDaemon sends it when a PC in the world disconnects or logs out, after the PC save is queued. LobbyDaemon runs §3.7: the session must be 4.0 with the same playing character and world, else no change; then end the session (state 6.0, `finish`).
2. Link close: when the link of a world closes, LobbyDaemon ends all 4.0 sessions of that world (original: link close starts logout jobs, `07` line 831).
3. Link messages `Kick { transaction, client, pcid, reason }` (LobbyDaemon → GameDaemon, original 14 `hqqqqQ`), `KickOk { transaction }` (15), `KickFail { transaction, reason }` (16). GameDaemon: disconnect the PC (use the existing kick path of `kick.rs` or the connection task close), then reply. LobbyDaemon `KickJob` (§5): for a 4.0 session, send `Kick`, wait for the reply or the time limit (use the existing `TRANSACTION_LIMIT` of `gamed_link.rs`), then end the session. Fail reason 24 → result 1; other fail and timeout → result 2. Sessions not in 4.0 keep the current kick behaviour.
4. `Server::kick` stays the entry. The wait for GameDaemon must not block the link task: run it as a job task (put the kick into the slow path), or wait outside the link task and send a `Control` message for the end. Write the choice in the handoff.
5. Tests: LobbyDaemon side with a fake link peer (see `tests/play_path.rs` and `tests/world.rs`), GameDaemon side with its link test fixture.

## Files
- `Program/Server/LobbyDaemon/`
- `Program/Server/GameDaemon/`
- `Program/Server/Packet/`
- `Cargo.lock`

## Acceptance
| Test name | Input | Expected result |
| --- | --- | --- |
| `logout_in_world_ends_session` | session in world, link sends `LogoutInWorld` | session gone, NP presence released, last-logout write done |
| `logout_in_world_wrong_pcid` | same with another pcid | no change |
| `world_link_close_ends_sessions` | 2 sessions in world, link closes | both sessions end |
| `kick_in_world_ok` | session in world, `Server::kick` | link gets `Kick`; after `KickOk` the session ends; `kick` returns `Ok(())` |
| `kick_in_world_timeout` | no reply | session ends after the limit (use `tokio::time::pause`) |
| `kick_does_not_block_link` | kick waits; other session on the same LobbyGate link sends 35 | 35 reply comes before the kick ends |
| `gamed_kick_disconnects_pc` | GameDaemon link gets `Kick` | the PC connection closes; `KickOk` sent |
| `message_round_trip` | each new `Message` | decode(encode(m)) == m |
| all existing tests of the 3 crates | — | pass |

Run `cargo test -p lobbydaemon`, `cargo test -p gamedaemon`, `cargo test -p packet`, and `cargo clippy -p lobbydaemon --all-targets --features mssql -- -D warnings`.

## Out of scope
- NP presence kick (gamed 17) and the manage port.
- Delete and restore on GameDaemon.
