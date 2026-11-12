# Card lp2-2-rust: LobbyDaemon replies for charge, grade and account-inventory item requests

## Goal
LobbyGate commands 60, 106 and 82–100 get the original replies of a server with charge, grade and account inventory off. The client gets a Fail packet, not silence and not a closed link.

## Context
- Crate `lobbydaemon` (`Program/Server/LobbyDaemon`). Read `Docs/Architecture/scale-rules.md` §3–§5 first.
- `Jobs::handle_fast` in `src/jobs.rs` runs commands that do not wait. Put all commands of this card there (none of them waits when the feature is off). Do not add them to `Jobs::slow`.
- The model to copy: `goods_unavailable` in `src/jobs.rs` (commands 64–76 with account inventory off). It drops a malformed request, sends 4 with no session, 5 outside the lobby, 7 else.
- Spec state 2.0 = `State::Lobby` (doc comments on `State` from card `lp2-1-rust`).
- Config: add `Settings::charge: bool` (default false) and `Settings::grade: bool` (default false). Account inventory is always off in this card. Write a comment that the on-paths are not ported (they need NP charge, NP grade and the account-inventory link).

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| Handler checks, reply formats, command numbers | `Docs/Server/LobbyDaemon/20-handlers.md` | §4.1 (main source) |
| Item job init order and codes | `Docs/Server/LobbyDaemon/17-jobs-e.md` | §3 (lines 34–56) |
| Add slot, user name, jumping quota init | `Docs/Server/LobbyDaemon/19-jobs-g.md` | §1, §2, §5 |
| Charge, grade jobs | `Docs/Server/LobbyDaemon/15-jobs-c.md` | §1, §3 |
| Request formats | `Docs/Server/LobbyDaemon/02-links.md` | lines 224–239 |

## Work
1. 60 GetChargeInfoReq `hq`: decode fail → no reply. No session on this link → 62 code 4. `charge` off → 62 code 7. (On-path: not ported; with `charge` on, reply 62 code 2 and log.)
2. 106 ActivateGradeReq `hqQi`: decode fail → no reply. `grade` off → 108 code 7 (this check is before the session lookup). No session → 108 code 4. (On-path: as above, code 2.) Fail format `hqQiE` echoes the game grade key and the grade id.
3. 82, 85, 88, 91 (character item services): decode with the request format of `02-links.md`. Fail → no reply. No session → Fail code 4. Then the job init of `17-jobs-e.md` §3: a zero slot id, label id or item instance id → no reply; account-inventory channel absent → Fail code 7. Fail format `hqoqqE`: client key, slot id, label id, item instance id, code. Fail commands: 84, 87, 90, 93.
4. 94, 97, 100 (account item services): same order. Read the zero-input rule of each job in `19-jobs-g.md`. Fail format `hqqqE`: client key, label id, item instance id, code. Fail commands: 96, 99, 102.
5. Rename (91) sends its failures with command 93. The original sends one failure with command 84; this is "bug, not ported" (`20-handlers.md` §4.1). Write a one-line comment.
6. Give each new function a doc comment with the 307 handler address from `20-handlers.md` §4.1.
7. Add the tests below in `Program/Server/LobbyDaemon/tests/` (a new file `services_off.rs` is fine). Use the gate-link fixture of the existing tests.

## Files
- `Program/Server/LobbyDaemon/`

## Acceptance
| Test name | Input | Expected result |
| --- | --- | --- |
| `charge_info_off` | lobby session, 60 | 62, code 7 |
| `charge_info_no_session` | 60 with unknown key | 62, code 4 |
| `grade_off_before_session` | 106 with unknown key | 108, code 7, echoes grade key and grade id |
| `item_service_off` (one case for each of 82, 85, 88, 91, 94, 97, 100) | lobby session, valid request | Fail command of the table, code 7, echoed fields |
| `item_service_zero_input` | 82 with label id 0 | no reply |
| `item_service_malformed` | 85 with a short body | no reply, link stays |
| `rename_fail_command` | 91, valid request | command 93, not 84 |
| all existing `lobbydaemon` tests | — | pass |

Also run `cargo clippy -p lobbydaemon --all-targets --features mssql -- -D warnings`.

## Out of scope
- Command 57 (slot bind). The NP slot service and the DB bind are not specified.
- An account-inventory link, NP charge or NP grade calls.
- Changes to `goods_unavailable` behaviour.
