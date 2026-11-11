# Scale rules — how to write Rust in this repository

Read this file before you write or change Rust code in `Program/`. Size: about 2,600 tokens.

## 1. Goal and limit

Parity means: the player can play as in the original. It does not mean the same code or the same structure.

| Tier | What | Rule |
| --- | --- | --- |
| 1. Fixed | All that the game client sees: packet bytes, message order on one connection, reply codes, game rules, game timing that the player feels (cooldowns, cast times, timeouts). | Same as the original. |
| 2. Free, with care | Links between our own daemons, database layout and procedures, the split into processes. | Change them when a measurement shows a gain. Change the two ends together. Write the change in the spec. |
| 3. Free | Threads, tasks, memory, locks, buffers, batching, internal tick rate, logs, config form. | Use the modern form. Do not copy an original structure because it is original. |

Do not copy original code habits: a global lock, a blocking call in a handler, a polling loop, a thread for each connection, a fixed-size buffer copy.

Original bugs and exploits: do not port them. Write the correct behaviour. Record the original bug in the spec with the mark "bug, not ported".

Not allowed: any other change that the client can see. Examples: a changed frame order, a skipped message, a changed cooldown.

The original lag has 2 possible sources. We can remove only the first one.

| Source | Can we remove it? |
| --- | --- |
| Server stalls: a blocked thread, a database call in the game path, a global lock, an unbatched send, Nagle delay | Yes |
| Client protocol design: TCP, server-side hit timing | No (tier 1) |

## 2. State of the code (2026-11-11, counted, not measured)

| Finding | Where | Effect |
| --- | --- | --- |
| One database connection behind one async mutex | `GameDaemon/src/db.rs` (`rows`) | All database calls of the daemon run one by one. |
| Lock held across `.await` | `db.rs`, `LobbyDaemon/src/lib.rs` (lanes, links) | One slow call stops all sessions. |
| Unbounded channels | `GameDaemon/src/link.rs` (`unbounded_channel`) | No backpressure. A slow peer grows memory. |
| One `write_all` for each frame | link and gate writers | One syscall for each packet. |
| A `Vec<u8>` for each frame, 636 `.clone()` | all crates | Allocation in the hot path. |
| `Box::pin` futures in traits (34) | `LobbyDaemon/src/gamed.rs` and others | Heap allocation for each call. |
| 785 `unwrap()` outside tests | all crates | A bad packet can stop a task. |
| `eprintln!` for logs | daemons | Locks stderr. No levels. |
| No `[profile.release]` | root `Cargo.toml` | No LTO, default codegen units. |
| Edition 2021 in 28 of 29 crates | `Cargo.toml` files | No let chains, older rules. |

❓ The slow login was not measured. Measure it (section 8) before you change code for it.

## 3. Target structure of each daemon

```text
 socket ─► reader task ─► decode ─┐                       ┌─► writer task ─► socket
 (1 per connection)               ▼                       │   (1 per connection, batches)
                         bounded mpsc (inbox)             │
                                  ▼                       │
                    ┌──────── shard task ─────────┐       │
                    │ owns its state, no locks    │──── bounded mpsc (outbox)
                    │ runs pure handlers          │
                    └──────┬───────────────┬──────┘
                           ▼               ▼
                  db worker pool      link tasks (other daemons)
                  (requests out, results back as messages)
```

| Layer | Crate part | Content | Must not |
| --- | --- | --- | --- |
| Net | `net/` | accept, framing, crypto, read and write tasks | know game rules |
| Protocol | `PacketIds`, `Packet` | encode and decode, no I/O | allocate for each field |
| Domain | `handlers/`, `jobs/` | pure functions: `(state, message) → (state change, output messages)` | call `.await`, sockets, database, clock |
| Storage | `db/` | connection pool, one function for each stored procedure | hold game state |
| Runtime | `main.rs`, `shard.rs` | tasks, channels, timers, shutdown | hold rules |

The domain layer is "sans-I/O". A test gives a message and reads the outputs. No socket and no database is necessary for a test.

## 4. Ownership rules

| # | Rule | Reason |
| --- | --- | --- |
| O1 | One task owns each piece of mutable state. Other tasks send messages to it. | No lock, no deadlock, the order is clear. |
| O2 | Shard key: GameDaemon = zone (or channel of a zone). LobbyDaemon = account id. Gates = connection. | Players in different zones do not wait for each other. |
| O3 | Do not use `Arc<Mutex<_>>` for game state. Permitted for: read-mostly config (`ArcSwap` or `watch`), counters (`Atomic*`). | A lock in the hot path does not scale. |
| O4 | Never hold a lock across `.await`. | One slow call stops all users of the lock. |
| O5 | The order of messages from one connection stays the same to the end of the pipeline. | Parity. |
| O6 | A message between shards is a value (`enum`), not a shared pointer. | Keeps O1. |
| O7 | Static game data (tables from data files) loads one time into `Arc<T>` and is read-only. | Free to share. |

## 5. Hot path rules

The hot path is all code that runs for each packet or each tick.

| # | Rule |
| --- | --- |
| H1 | No database call, file read or DNS call in the hot path. Send a request to the db pool. Continue when the result message comes. |
| H2 | No allocation for each packet if a reuse is possible. Read into one `BytesMut` for each connection. Decode from `&[u8]` or `Bytes`. Encode into the `BytesMut` of the writer. |
| H3 | Share a broadcast payload as `Bytes` (reference count). Do not clone a `Vec<u8>` for each receiver. |
| H4 | The writer task drains its queue fully, then writes one time (`write_vectored` or one buffer), then flushes. Frame order stays the same. |
| H5 | Set `TCP_NODELAY` on each socket. |
| H6 | Each channel is bounded. Write the full-queue policy in the code: client outbox full → disconnect the client; daemon link full → wait. |
| H7 | Each read from a peer has a length limit and a timeout. A bad packet gives an error value, not a panic. |
| H8 | No `unwrap()`, `expect()` or index that can panic on peer data. Use `?`, `get()`, `let … else`. |
| H9 | CPU work above about 100 µs (crypto key generation, pathfinding, big data load) goes to `spawn_blocking` or a dedicated thread pool. |
| H10 | Use ids and dense storage in the game loop: `Vec` or slab indexed by a small id. Use `HashMap` only at the edge (lookup by account id). Give a capacity when the size is known. |
| H11 | Timers: one timer wheel or `tokio::time::Interval` for each shard. Not one `sleep` task for each cooldown. |
| H12 | Logs: `tracing` with levels. No log line for each packet at `info`. No `eprintln!`. |

## 6. Game loop (GameDaemon)

| # | Rule |
| --- | --- |
| G1 | Each zone shard runs a fixed tick. The internal period is ours (tier 3). The times that the client sees (cooldowns, cast times, send period of position updates) are the original values from the spec (❓ until the spec gives them). |
| G2 | Tick order: drain inbox → run handlers → run simulation step → flush outboxes one time. |
| G3 | Use `Interval` with `MissedTickBehavior::Skip`. Write the tick time to a histogram. |
| G4 | Area broadcast uses a spatial grid of the zone. Do not loop over all players of the zone for each event. |
| G5 | Persistence is write-behind: the shard sends a save message, the db pool writes. The order of saves for one character stays the same. A save that parity needs before a reply (trade, item consume) waits for the result message. |

## 7. Database rules

| # | Rule |
| --- | --- |
| D1 | One pool for each database (start: 8 connections, config value). No single connection behind a mutex. |
| D2 | One Rust function for each database operation. Start with the original stored procedures. A new layout or query is permitted (tier 2) if the client-visible result is the same. |
| D3 | Each call has a timeout (original value from the spec, 10 s in LobbyDaemon). |
| D4 | Operations for one account or one character go through one ordered queue. Different accounts run in parallel. |
| D5 | Read at login, keep in the shard, write on change. Do not read the same row again in a session if the original does not. |

## 8. Measure first

| # | Rule |
| --- | --- |
| M1 | Do not optimise without a number. Add a `tracing` span for each stage of a flow (login: connect, auth, token, lobby, character list). Read the times from the log. |
| M2 | Each daemon exports counters: connections, queue depth, tick time p50/p99, database call time p50/p99. |
| M3 | Put a `criterion` bench next to each codec and each hot handler. |
| M4 | A change to the hot path gives the bench result before and after in the commit message. |

Targets (proposal, change them when measured):

| Metric | Target |
| --- | --- |
| Packet decode + handler + encode, no database | p99 below 50 µs |
| Tick overrun | below 1 % of ticks |
| Server-side part of login, local database | below 300 ms |
| Sessions for each gate process | 10,000 |

## 9. Rust use

Toolchain: `rustc 1.99.1`. Before you use a new language feature, compile a small test. Do not use nightly features.

| Use | In place of |
| --- | --- |
| Edition 2024 for each crate that you refactor | edition 2021 |
| `async fn` in traits, generic parameter `T: Trait` | `Box::pin` futures, `dyn` in the hot path |
| `let … else`, let chains | nested `if let` |
| `bytes::{Bytes, BytesMut, Buf, BufMut}` | `Vec<u8>` for frames |
| `tokio::task::JoinSet`, `CancellationToken` | detached `tokio::spawn` with no shutdown path |
| `tokio::select!` with `biased;` where order matters | unordered select on inbox and shutdown |
| `std::sync::LazyLock`, `OnceLock` | `lazy_static`, `once_cell` |
| `thiserror` enum for each crate | `io::Error::other(string)` for domain errors |
| `#[expect(lint)]` with a reason | `#[allow]` |
| Newtype ids (`struct Pcid(u64)`), `#[repr(u16)]` enums for codes | bare integers |
| `#[inline]` only after a bench shows a gain | `#[inline(always)]` by guess |

Release profile (in the root `Cargo.toml`; `panic = "abort"` is not set there until rule H8 holds in all crates):

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
debug = "line-tables-only"
```

`unsafe` is not permitted without a bench that shows the need and a comment that gives the proof of safety.

## 10. Refactor order

Refactor one crate at a time. Parity tests must pass before and after each step.

| Step | Work |
| --- | --- |
| 1 | Add spans and counters (M1, M2). Measure login and enter world. Write the numbers to `Docs/Architecture/baseline.md`. |
| 2 | Shared `net` module (`Program/Server/Shared/src/net.rs`): framed reader, batching writer, bounded queues (H2 to H7). Move each daemon to it. Done 2026-11-11: all game daemons and gates read and write through it or through one batched write (`LobbyGate/src/wire.rs` `write_frames` for client frames); all sockets have `TCP_NODELAY`; no unbounded frame queue is left in `Program/Server`. Not moved, by decision: StServers (`Program/Platform`, login only, other frame form). Open: the client frame reader of LobbyGate and GameDaemon moves into the module when GameDaemon gets its connection tasks (step 5); spans and counters (M1, M2). |
| 3 | Database pools (D1 to D4) in GameDaemon and LobbyDaemon. |
| 4 | Split handlers into pure domain functions (section 3). Remove locks (O1 to O4). |
| 5 | Shards and tick loop in GameDaemon (section 6). |
| 6 | Release profile, edition 2024, benches (M3). |

## 11. Check before each commit

- [ ] No lock across `.await`. No new `Arc<Mutex>` for game state.
- [ ] No unbounded channel. Full-queue policy is written.
- [ ] No database or file call in a handler.
- [ ] No panic path on peer data.
- [ ] Wire bytes and message order are the same (golden tests pass).
- [ ] `cargo fmt`, `cargo test -p <crate>`, `cargo clippy -p <crate> --all-targets -- -D warnings`.
- [ ] Hot path change: bench result is in the commit message.
