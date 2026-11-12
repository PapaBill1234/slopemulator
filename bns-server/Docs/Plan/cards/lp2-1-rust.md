# Card lp2-1-rust: LobbyDaemon session states, logout table, wait info, give up wait

## Goal
The LobbyDaemon session uses the session states of the spec. Logout (LobbyGate 18, 19, 23) follows the LogoutJob table. LobbyGate 28 GetWaitInfoReq and 31 GiveupWaitReq get their original replies. An unknown command no longer closes the gate link.

## Context
- Crate `lobbydaemon` (`Program/Server/LobbyDaemon`). Read `Docs/Architecture/scale-rules.md` §3–§5 first.
- Job core (step 6 part 1b, `src/lib.rs`): one task for each LobbyGate link owns a `Jobs` value (`src/jobs.rs`). `Jobs::handle_fast` runs commands that do not wait. `Jobs::slow(cmd)` lists the commands that wait (9, 24, 44, 47, 51, 54, 103); a job task runs them on a private copy (`checkout` / `merge`). Do not change this structure.
- `State` in `src/jobs.rs` has 8 local values. Spec `13-jobs-a.md` §2 has 12 session states (major.minor). Map the local values to spec states. Add the states that the new code needs. Keep the client-visible behaviour of all existing paths.
- Today `handle_fast` returns `Err` for an unsupported command. The link task (`lib.rs` near line 344) uses `?`, so the whole link ends and all sessions on it drop. Change this: log at `warn` (or `eprintln!` if the crate has no `tracing`), send no reply, keep the link. A `Hello required` error stays as it is.
- World queue: `src/gamed.rs` has `WaitInfo` and `Admission`. State `WorldQueued` is spec state 3.2.
- Wire helpers: `src/protocol.rs` (`packet`, `fields`, `failure`, `notice`). Format letters: `h` i16, `i` i32, `q` u64, `Q` u8 ❓ (check `bloodlib::packet`), `o` id128, `E` i16 error code, `C…;` list.

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| Session states | `Docs/Server/LobbyDaemon/13-jobs-a.md` | §2 (lines 21–38) |
| LogoutJob, GiveupWaitGameWorldJob, EndWaitGameWorldJob | `13-jobs-a.md` | §3.3–§3.5 (lines 62–96) |
| Handler checks, reply formats of 28 and 31 | `Docs/Server/LobbyDaemon/20-handlers.md` | §4 and §4.1 |
| Client side of wait info (field meaning) | `Docs/Client/RE/04-lobbygate.md` | `rg -n "WaitInfo" Docs/Client/RE/04-lobbygate.md` |
| Current code | `Program/Server/LobbyDaemon/src/jobs.rs`, `src/jobs/world.rs`, `src/gamed.rs`, `src/lib.rs` | `State`, `handle_fast`, the 18/19/23 arm |

## Work
1. Add a doc comment on `State` with the spec value of each variant (for example `Lobby` = 2.0 `0x20000`). Add variants only when a spec rule needs them.
2. Rewrite the 18/19/23 arm as one `logout(link, client, close: bool)` function that follows the `13-jobs-a.md` §3.3 table. 18 finds the session by gate key; 19 and 23 by client key. `close` is true for 23 ❓ and 19 ❓; read `20-handlers.md` §4 and write your choice in the handoff. `SendCloseSessionNotify` is LobbyGate command 21, format `hq` (`wire::notice(21, key)`). Keep the 2 existing behaviours that tests check (absorb in `Retained`/`WorldReady`, notify on 23 in `Lobby`) unless the table says otherwise; if the table says otherwise, follow the table and change the test, and write why in the handoff.
3. Add 28 GetWaitInfoReq: decode `hq` (fail → no reply). No session on this link → Fail 30 `hqE` code 4. Session not 3.2 → Fail 30 with the error of `GetGameWorldWaitInfo` (spec: read `13`/`20`; if ❓, use 5 and mark ❓). Else Ok 29 `hqiiCch;` from the session `WaitInfo`. Field meaning: position, turn ❓, then a list; take it from the client RE file. Write each ❓ field in the handoff.
4. Add 31 GiveupWaitReq: decode `hq` (fail → no reply). No session → Fail 33 `hqE` code 4. State not 3.2 → Fail 33 code 5. No world location → Fail 33 code 2. Else leave the queue (drop the `Admission`, which releases the place), set state 2.0, reply Ok 32 `hq`.
5. Unknown command: no reply, no link close (see Context).
6. Add the tests below to `Program/Server/LobbyDaemon/tests/` (one new file `queue_logout.rs` is fine). Use the existing test fixtures (see `tests/world.rs` and `tests/end_to_end.rs` for the gate-link fixture and `StubGameDaemon` with a wait).

## Files
- `Program/Server/LobbyDaemon/`
- `Docs/Server/LobbyDaemon/13-jobs-a.md` (only to mark a ❓ as checked, with source)

## Acceptance
| Test name | Input | Expected result |
| --- | --- | --- |
| `wait_info_in_queue` | session in queue (stub with wait), then 28 | 29 with the position of the stub |
| `wait_info_no_session` | 28 with unknown key | 30, code 4 |
| `giveup_wait_ok` | session in queue, then 31 | 32; state is `Lobby`; a new 24 works |
| `giveup_wait_wrong_state` | session in lobby, then 31 | 33, code 5 |
| `unknown_command_keeps_link` | 2 sessions on one link; unknown command 200 from one | no reply; the other session still gets a 35 reply |
| `logout_table` | 23 in each local state | the §3.3 result (notify or not, session gone or kept) |
| all existing `lobbydaemon` tests | — | pass |

Also run `cargo clippy -p lobbydaemon --all-targets --features mssql -- -D warnings`.

## Out of scope
- No change to `Jobs::slow`, `checkout`, `merge` or the link task structure.
- No GameDaemon change. No new link message.
- Commands 57, 60, 82–100, 106 (card `lp2-2-rust`).
