# Handoff lp2-5-rust

## Done
- `store::Character` has `state: InternalState` and `deletion_time: Option<SystemTime>`. The store is the truth; `Jobs` no longer has `character_states` or `deletion_deadlines`.
- `MssqlStore` reads state and time (non-NULL accepted, UTC). Unit tests: `mssql_reads_times`, `unknown_state_and_restricted_rows_are_refused`.
- Deletion scheduler: `scheduler::tick` and `scheduler::run`, `Server::spawn_deletion_scheduler`, `--deletion-period` in lobbyd. 2 new `Store` methods (memory and MSSQL).
- Spec: `11-character-select.md` §15 updated.
- New tests (`tests/characters.rs`): `loaded_character_can_be_deleted`, `deletion_wait_across_links`, `deletion_time_from_store`, `scheduler_expires_wait`, `scheduler_run_ticks_each_period_and_stops`, `stale_link_cannot_delete_twice`, `sql_timestamp_reads_as_utc`.
- Checks pass: `cargo fmt`, `cargo test -p lobbydaemon`, clippy with and without `--features mssql`.
- Tests changed: 7 files (`leave`, `queue_logout`, `end_to_end`, `play_path`, `rules`, `world`, `store_growth`) use the new `Character` literal. `end_to_end` and `rules` expect wire state 1 (was 0, raw test value). `characters.rs` `rig()` has a paused-time wall clock for `Settings` and `MemoryStore`.
- Abuse notes: `Docs/Plan/abuse-notes/lp2-5-rust.md`.

## Not done
- Live SQL test not run (no `secrets/lobbyd.mssql.url`).
- The scheduler does not tell the GameDaemon or an open session.

## ❓
- SQL `datetime` is read as UTC. LobbyDB writes `GETDATE()` (local time); the SQL host must run in UTC.
- Scheduler period 60 s and batch 16 are local defaults.
- Tier 1: the wire state of a loaded row is `InternalState::wire()` (normal = 1). Before, the raw DB value was sent (normal = 2).
- An open session keeps an expired row with countdown 0 until relogin.
- After a delete the cache time is `clock() + seconds`, not the SQL OUT value.
- Verified from `lobbydb-schema.json` (not in spec before): `usp_LoadCharacterList` returns only states 2–5; column order 8 state, 9 DeletionDelayTime, 10 LastPlayEndTime, 13 Blocked.
- Confirm that cancel and expire are atomic in SQL (abuse note `exploit_cancel_vs_expire_open`).

## Blocked
- None.
