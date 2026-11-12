# Card ids-3-rust: official command names in LobbyDaemon and the rest of GameDaemon

## Goal
LobbyDaemon and the LobbyGate side of the LobbyGate–LobbyDaemon link use official command names. GameDaemon uses `ids::` names for its remaining command IDs. Behaviour does not change.

## Context
- `packet-ids` has the client protocols only (`GameCmd`, `LobbyCmd`, `StsCmd`, `InfoGateCmd`). Run `python3 tools/rust_api.py Program/Server/PacketIds` for the interface. The bin `gen-ids` writes `src/generated/`. The test `generated_is_current` compares the files with the generator output.
- The LobbyGate → LobbyDaemon protocol has different IDs. Example: 6 is `L_LoginWithLobbyTokenReq` for the client, but 6 is the init command in LobbyDaemon.
- The official LobbyDaemon names are in `Docs/Server/LobbyDaemon/command-tables.json`. They come from the handler tables of LobbyDaemon 307. Each table has `name` and `rows`. Each row has `command`, `name` (for example `HandleLobbyLobbyGate_StartSessionWithNPTokenReq`) and `name_status` (✅ or ❓).

| Table (`name` starts with) | Rows | ✅ names |
| --- | --- | --- |
| `?LobbyLobbyGatePacketLobbySideHandlerList` | 35 | 35 |
| `?LobbyGamePacketLobbySideHandlerList` | 51 | 15 |
| `?AccountInventoryLobbyPacketLobbySideHandlerList` | 23 | 3 |
| `?LobbyManagePacketLobbySideHandlerList` | 6 | 0 |
| `?LobbyInfoGatePacketLobbySideHandlerList` | 3 | 0 |

- LobbyDaemon dispatch: `match cmd` on bare numbers in `Program/Server/LobbyDaemon/src/jobs.rs` (`handle_slow` and `handle_fast`; the files under `src/jobs/` can also have bare numbers). Example: `9 => self.start(...)`.
- GameDaemon: `ids-2-rust` named the dispatch. About 99 local `const F: u16 = <n>;` lines are left. They are reply or failure command IDs. Example: `handlers/mentoring.rs` has `const F: u16 = 1099;`, which is `G_ChangeMentoringTypeFailCmd`. Bare command numbers are also left in `src/play.rs` (`packet()`, arms 23, 31, 42, 929) and in `src/kick.rs`.
- The gamed link (`packet::gamed_link::Message`) is our own protocol (plan `step6-part2.md` decision 3). Do not rename it to the 307 names.

## Work
1. Extend `gen-ids` in `packet-ids`. It reads `Docs/Server/LobbyDaemon/command-tables.json` and writes `src/generated/lobbyd.rs`. Write one enum for each table that has 1 or more ✅ rows: `LobbyGateLobbydCmd`, `GameLobbydCmd`, `AccountInventoryLobbydCmd`. Write only the ✅ rows. The variant name is the row `name` without the `HandleLobby<Table>_` prefix. Example: `StartSessionWithNPTokenReq = 9`. Give each enum `id()`, `from_id()` and `name()`, the same as `GameCmd`. `generated_is_current` must cover the new file.
2. Add `packet-ids` to LobbyDaemon. Give LobbyDaemon `u16` constants for `match` arms, the same way GameDaemon does (`build.rs` + `src/ids.rs`).
3. In LobbyDaemon, replace each bare request command number with its name. Keep the same handler for each ID. Keep a number that has no ✅ name, and add `// ❓ no verified 307 name`.
4. In LobbyGate, replace the bare numbers for requests that it sends to LobbyDaemon. Use the same names.
5. Reply IDs (LobbyDaemon → LobbyGate) are not in `command-tables.json`. Keep them as numbers. List them in the handoff with the file and line.
6. In GameDaemon, replace each `const F: u16 = <n>;` with the `ids::` name for `<n>`. Do this also for the command numbers in `play.rs` and `kick.rs`. Do not change numbers that are not command IDs (reason codes, zone IDs, field offsets).
7. If a number and the `GameCmd` value differ, keep the number, add `// packet-ids says <n>`, and write it in the handoff under ❓.

## Files
- `Program/Server/PacketIds/`
- `Program/Server/LobbyDaemon/`
- `Program/Server/LobbyGate/`
- `Program/Server/GameDaemon/`
- `Cargo.lock`

## Acceptance
| Test name | Input | Expected result |
| --- | --- | --- |
| `generated_is_current` | generator output against `src/generated/` | same, `lobbyd.rs` included |
| `lobbyd_names_from_table` (new, `packet-ids`) | `LobbyGateLobbydCmd::from_id(9)`, `from_id(24)`, `from_id(28)` | `StartSessionWithNPTokenReq`, `GetEnterWorldTokenReq`, `GetWaitInfoReq` |
| `lobbyd_unverified_absent` (new, `packet-ids`) | `GameLobbydCmd` | has 15 variants (the ✅ rows only) |
| check, LobbyDaemon dispatch | `rg -n '^\s*[0-9]+( *\| *[0-9]+)* *=>' Program/Server/LobbyDaemon/src/jobs.rs` | only arms with `// ❓ no verified 307 name` (put the command and its output in the handoff) |
| check, GameDaemon | `rg -n 'const F: u16 = [0-9]' Program/Server/GameDaemon/src` | no output |
| all existing tests of the 4 crates | — | pass unchanged |

## Out of scope
- Behaviour changes, payload field changes and new handlers.
- Names for ❓ rows. Do not guess names.
- The gamed link protocol (`packet::gamed_link`).
- AchievementDaemon, RankingDaemon and the StServers crates.
