# Card lp2-5-rust: character lifecycle state from the database

## Goal
The state of each character (normal, deletion wait, deleted) and its deletion time come from the database with the character list. They no longer live in the `Jobs` of one LobbyGate link. A character that a player loaded through one link can be deleted, restored or recovered through another link. Expired deletion waits end on a timer.

## Context
- Crate `lobbydaemon` (`Program/Server/LobbyDaemon`). Read `Docs/Architecture/scale-rules.md` §3–§5 and §7 first.
- Today `Jobs` has 2 maps: `character_states: HashMap<u64, InternalState>` and `deletion_deadlines: HashMap<u64, Instant>` (`src/jobs.rs`, `src/jobs/characters.rs`). `checkout` and `merge` copy them. Each LobbyGate link has its own `Jobs`, so the maps are lost when the player comes through another link. Also, a character row loaded from the database has no entry, so the jobs refuse to change it ("freshly loaded rows lack lifecycle metadata", `Docs/Server/LobbyDaemon/STATUS.md` step 11).
- `store::Character` is the cached row. `Store` (`src/store.rs`) is async; `MssqlStore` is in `src/mssql.rs` (feature `mssql`); in-memory stores implement `SyncStore`.
- The SQL adapter today rejects non-NULL character times (`STATUS.md` step 11). Convert SQL `datetime` to a Rust time value (UTC ❓; write the choice in the handoff).
- Deletion wait: `usp_FinishCharacterDeletion` returns the deletion datetime (OUT). Restore and cancel procedures exist. The original has a deletion scheduler timer.

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| Character select contract, procedures | `Docs/Server/LobbyDaemon/11-character-select.md` | §6–§8 (lines 181–289), §12 (lines 419–449), §15 |
| Create, delete, cancel, recover jobs | `Docs/Server/LobbyDaemon/16-jobs-d.md` | §1–§4 |
| Deletion scheduler | `Docs/Server/LobbyDaemon/20-handlers.md` | §9 |
| Wrappers, caller groups | `Docs/Server/LobbyDaemon/21-lobbydb-wrappers.md` | §3 rows `RetrieveExpiredDeletionWaitingList`, `ExpireCharacterDeletionWaiting`, `LoadCharacterList` ❓ name; §4 |
| Schema | `Docs/Server/LobbyDaemon/lobbydb-schema.json` | `rg -n "Deletion|deletion|state" ` (do not read in full) |
| Current code | `src/jobs.rs`, `src/jobs/characters.rs`, `src/character.rs`, `src/store.rs`, `src/mssql.rs` | `InternalState`, `refresh_deletion_counts`, the 2 maps |

## Work
1. Add the lifecycle to `store::Character`: `state: InternalState` and `deletion_time: Option<SystemTime>` (or the type that the SQL adapter gives). The list procedure gives the state and time columns; read the column binds from `db-binding.json` / `08-db-binding.md`.
2. `MssqlStore::load_character_list`: read the state and time. Accept non-NULL times. Keep a unit test that does not need SQL.
3. Remove `character_states` and `deletion_deadlines` from `Jobs`. The jobs read and write the lifecycle in the session `characters` cache and in the store. Remove the map code from `checkout` and `merge`. A deletion deadline for the wire uses the stored time (wall clock), not a monotonic `Instant`.
4. Deletion scheduler: one `tokio::time::Interval` task in `main.rs` / `Server` (period from config, local default 60 s ❓, write it in the handoff). Each tick: `RetrieveExpiredDeletionWaitingList(count)`, then `ExpireCharacterDeletionWaiting(pcid)` for each row. Add the 2 `Store` methods (in-memory and MSSQL). The task does not hold the session table. A session that shows a character in deletion wait refreshes it when the client asks for the list (keep the current refresh rule of `refresh_deletion_counts`, now from the stored time).
5. Update the tests that set the maps. Add the tests below.
6. Update `Docs/Server/LobbyDaemon/11-character-select.md` §14 or §15: write that lifecycle state is stored, not kept per link.

## Files
- `Program/Server/LobbyDaemon/`
- `Docs/Server/LobbyDaemon/11-character-select.md`

## Acceptance
| Test name | Input | Expected result |
| --- | --- | --- |
| `loaded_character_can_be_deleted` | store has a normal character; start session; 47 | delete runs (no "missing lifecycle" refusal) |
| `deletion_wait_across_links` | delete on link A; logout; login on link B; 51 | cancel works on link B |
| `deletion_time_from_store` | store row in deletion wait with a time | 35 list shows the remaining time from the stored time |
| `scheduler_expires_wait` | row in deletion wait, time in the past; one tick | row is deleted in the store; next list does not show it |
| `mssql_reads_times` (unit, no SQL) | row values with non-NULL time | `Character` with the time |
| all existing `lobbydaemon` tests | — | pass (change a test only when it set the removed maps; write each change in the handoff) |

Also run `cargo clippy -p lobbydaemon --all-targets --features mssql -- -D warnings`. If `secrets/lobbyd.mssql.url` exists in the worktree, run `LOBBYD_MSSQL_URL="$(cat secrets/lobbyd.mssql.url)" cargo test -p lobbydaemon --features mssql` and give the counts. Never print the variable.

## Out of scope
- Delete and restore on GameDaemon (the stub stays).
- Jumping characters, slot bind (57).
- A new SQL procedure or a schema change. If a procedure that you need is missing in the local `LobbyDB`, write it in the handoff and use the in-memory store for the test.
