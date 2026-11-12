# Card lp2-3-rust: return to lobby over the gamed link

## Goal
A client in the world sends `G_ReturnToLobbyReqCmd` (26). GameDaemon sends LeaveWorldReq to LobbyDaemon on the gamed link, LobbyDaemon issues a type-2 lobby token and replies LeaveWorldOk, and GameDaemon sends `G_ReturnToLobbyOkCmd` (27) with the token key. The client can then log in at LobbyGate with that token (existing `init_retained` / `start_retained` path).

## Context
- Spec: `Docs/Server/GameDaemon/10-session-handlers.md` §26 (lines 179–234) and `Docs/Server/LobbyDaemon/23-return-to-lobby.md` (all, 60 lines). LobbyDaemon job: `Docs/Server/LobbyDaemon/13-jobs-a.md` §3.6 (lines 97–110).
- Command names (use these): `G_ReturnToLobbyReqCmd` (26), `G_ReturnToLobbyOkCmd` (27), `G_ReturnToLobbyFailCmd` (28). Gamed link original numbers: 8 LeaveWorldReq, 9 LeaveWorldOk, 10 LeaveWorldFail, 12 LogoutNotify (`07-gamed-link.md` line 819–831).
- Our gamed link is our own protocol (`packet::gamed_link::Message`). Add variants; keep the original field order.
- GameDaemon state steps 1–6 of the transaction (stop charge, zone leave, character/account update, unload) need subsystems that do not exist in the port. Run them as no-op steps in this order with a `// TODO ❓ step N` comment each. Only step 7 (leave world) is required.
- Quirks (spec §26.4): `quirk_rtl_no_timeout` → **fixed**: add a timeout on the wait for LeaveWorldOk/Fail; on timeout send 28 reason 1952. `quirk_leave_fail_as_enter_fail` → **fixed**: LobbyDaemon sends LeaveWorldFail (not EnterWorldFail) when the job cannot start.
- Read `Docs/Architecture/scale-rules.md` §1, §3, §6 before you write code.

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| GameDaemon spec | Docs/Server/GameDaemon/10-session-handlers.md | §26.1–26.5, lines 179–234 |
| End-to-end spec | Docs/Server/LobbyDaemon/23-return-to-lobby.md | §1–§3, lines 5–47 |
| Job spec | Docs/Server/LobbyDaemon/13-jobs-a.md | §3.6, lines 97–110 |
| Link messages | Program/Server/Packet/src/gamed_link.rs | `enum Message`, `encode`, `decode` |
| GameDaemon link | Program/Server/GameDaemon/src/link.rs | `Link`, `Link::enter` (pattern for a request that waits for a reply) |
| GameDaemon handler | Program/Server/GameDaemon/src/handlers/session.rs | `handle_return_to_lobby` (line 232) |
| LobbyDaemon link | Program/Server/LobbyDaemon/src/gamed_link.rs | `LinkGameDaemon::serve` |
| LobbyDaemon jobs | Program/Server/LobbyDaemon/src/jobs.rs | `Jobs::slow` (line 374), `retain` (line 557) |
| LobbyGate token use | Program/Server/LobbyDaemon/src/protocol.rs | `start_retained` (line 191) |

## Work
1. `packet`: add `Message::Leave`, `LeaveOk`, `LeaveFail`, `LogoutNotify` (fields below). Add encode/decode and round-trip tests.
2. LobbyDaemon link: on `Leave`, find the session by `pc_db_id`. No session or `cli_key` differs → `LeaveFail { reason: 5 }`. Else run the leave job on `Jobs::slow` (job task on a copy; keep the part 1b job core). Job: session state must be 4.0 else reason 6; playing pcid checks (21, 22, 23) as §3.6 step 2; end game-world play; issue a type-2 lobby token with the lobby-token lifetime (reuse the `retain(key, lifetime)` store); fail → reason 2, state 6.0. Ok → state 5.1, `LeaveOk { transaction, token }`. Job cannot start → `LeaveFail { reason: 2 }` (quirk fixed).
3. LobbyDaemon link: on `LogoutNotify`, run the existing logout-in-game-world path (task 118) for that session. If no such path exists, log and leave a `TODO ❓ lp2-4-rust` comment.
4. GameDaemon: `Link::leave(transaction, pc_db_id, cli_key) -> Result<u64, i16>` that sends `Leave` and waits for `LeaveOk`/`LeaveFail` with the same transaction, with a timeout. Follow the shard and `play.rs` structure (scale-rules §3, §6): the handler does not block the shard; it queues the work and the reply goes back through the shard.
5. GameDaemon `handle_return_to_lobby`: keep arena → 28/5. Logout gate fail → 28/1954. Then run steps 1–7. Step 7 result: Ok(token) → 27 `hq` (token key). Err(r) with r in 21..=23 → 28/1954; other Err → 28/1; link send failure or timeout → 28/1952. Update the parity comment to `ported` with anchor `10-session-handlers.md#26--g_returntolobbyreqcmd`.
6. Replace the local `Server::retain` path only where the link path covers it. Keep the tests that use `retain`, or move them to the new path.

## Interfaces
```rust
// packet::gamed_link::Message (new variants)
Leave { transaction: u64, pc_db_id: u64, cli_key: u64 },      // orig 8 hqqq
LeaveOk { transaction: u64, token: u64 },                     // orig 9 hqq
LeaveFail { transaction: u64, reason: i16 },                  // orig 10 hqE
LogoutNotify { pc_db_id: u64, cli_key: u64 },                 // orig 12 hqq

// gamedaemon::link::Link
pub async fn leave(&self, transaction: u64, pc_db_id: u64, cli_key: u64) -> Result<u64, i16>;
pub async fn logout_notify(&self, pc_db_id: u64, cli_key: u64) -> std::io::Result<()>;
```
Error codes: 2 job/issue failure, 5 no session or key mismatch, 6 wrong state, 21 no playing character, 22 world differs, 23 pcid differs (LobbyDaemon); 1952 link failure or timeout (GameDaemon only).

## Files
- Program/Server/Packet/src/gamed_link.rs
- Program/Server/LobbyDaemon/src/gamed_link.rs
- Program/Server/LobbyDaemon/src/jobs.rs
- Program/Server/LobbyDaemon/src/jobs/world.rs
- Program/Server/LobbyDaemon/src/lib.rs
- Program/Server/LobbyDaemon/src/protocol.rs
- Program/Server/LobbyDaemon/tests/ (new or changed test files)
- Program/Server/GameDaemon/src/link.rs
- Program/Server/GameDaemon/src/handlers/session.rs
- Program/Server/GameDaemon/src/play.rs
- Program/Server/GameDaemon/src/shard.rs
- Program/Server/GameDaemon/tests/ (new or changed test files)
- Docs/Plan/handoffs/lp2-3-rust.md

## Acceptance
Run `cargo test -p packet`, `cargo test -p lobbydaemon`, `cargo test -p gamedaemon`.

| Test name | Input | Expected result |
| --- | --- | --- |
| `leave_messages_round_trip` | each new `Message` variant | decode(encode(m)) == m |
| `leave_ok_issues_type2_token` | session in state 4.0 playing pcid P; `Leave{t,P,key}` | `LeaveOk{t,token}`; state 5.1; LobbyGate init with `token` and `key` passes once |
| `leave_token_single_use` | use the token twice | second use fails (reason 20) |
| `leave_wrong_cli_key` | `Leave` with other key | `LeaveFail{t,5}` |
| `leave_no_session` | unknown pcid | `LeaveFail{t,5}` |
| `leave_wrong_state` | session not 4.0 | `LeaveFail{t,6}` |
| `leave_job_start_fail_sends_leave_fail` | job cannot start | `LeaveFail{t,2}`, not EnterWorldFail (`quirk_leave_fail_as_enter_fail`) |
| `rtl_ok_sends_27_token` | 26 with link reply `LeaveOk{t,K}` | client gets 27 `hq` with K |
| `rtl_fail_21_to_1954` | reply `LeaveFail{t,22}` | client 28 reason 1954 |
| `rtl_fail_other_to_1` | reply `LeaveFail{t,5}` | client 28 reason 1 |
| `rtl_timeout_1952` | no reply | client 28 reason 1952 (`quirk_rtl_no_timeout`) |
| `rtl_arena_5` | arena world | client 28 reason 5 |

## Out of scope
- Arena path (config +237/+248), `SecurityAddClient`/external integration notices.
- Real steps 1–6 (charge stop, zone leave, character/account save, cache unload): stubs only.
- LogoutTran on socket close and kick (`lp2-4-rust`), except the `LogoutNotify` message itself.
- The LobbyGate side (already ported).
