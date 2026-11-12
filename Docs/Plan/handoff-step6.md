# Handoff: refactor step 6 (LobbyDaemon job core)

Date: 2026-11-12. Read `Docs/Architecture/scale-rules.md` first (section 10 has the step list).

## 1. State

| Step | State |
| --- | --- |
| 1 to 4 | Done (see `scale-rules.md` §10). |
| 5 GameDaemon shards | Done for the structure (`conn.rs`, `shard.rs`, `play.rs`, `entry.rs`, `db::Ordered`). |
| 6 part 1a: job split in `Jobs` | **Done, in this commit.** |
| 6 part 1b: link task without the lane lock | **Open. Do this next.** |
| 6 part 2: job port from specs `13` to `22` | Open. Start only after the user tests login. |

No real client has tested steps 2 to 6. The play stack runs the build of step 5, not this commit.

## 2. What part 1a changed (`LobbyDaemon/src/jobs.rs`)

| Item | Use |
| --- | --- |
| `Jobs::slow(cmd)` | True for commands 9, 24, 44, 47, 51, 54, 103. These wait for NP, GameDaemon or the database. |
| `Jobs::handle_fast(link, packet)` | Sync. All other commands. No wait. |
| `Jobs::handle(link, packet)` | Async. Runs fast or slow, then `flush`. Tests and job tasks use it. Its behaviour did not change. |
| `expire_now(busy)`, `kick_now`, `close_anonymous_now`, `disconnect_now` | Sync forms. The async forms call them, then `flush`. |
| `finish` | Sync now. It puts the last-logout write into a list. |
| `flush()` / `take_logouts()` | Write the list, or give it to the caller. |
| `Jobs::target(link, packet)` | The session key that a packet is for (command 18: by gate id; command 12: by lobby token). |
| `Jobs::checkout(link, key)` | A private `Jobs` copy with one session, for a job task. |
| `Jobs::merge(key, job)` | Takes the result of the job back. |
| `protocol::key(packet)` | First `q` field, or 0. |

Checks: 73 tests pass (`cargo test -p lobbydaemon`), clippy is clean with `--features mssql`. The 4 tests against the local `LobbyDB` did not run for this commit.

## 3. Part 1b: what to write (`LobbyDaemon/src/lib.rs`)

Today `Lane = Arc<Mutex<Jobs>>`. `connection()` holds the lock across `jobs.handle(..).await`. One slow call stops all sessions of the link. Remove the lock in this way:

```text
 LobbyGate socket ─► link task (owns Jobs, the busy map, a JoinSet, the socket writer)
                        │  fast command ─► handle_fast ─► write
                        │  slow command ─► checkout ─► job task: handle(..) on the copy
                        │                     result ─► merge ─► write ─► run the queue of that key
 Server API ──Control message (oneshot reply)──► link task
```

| # | Rule |
| --- | --- |
| 1 | One task for each link owns `Jobs`. No `Mutex` around it. |
| 2 | `busy: HashMap<u64, VecDeque<Item>>`, key = `Jobs::target`. `Item` = packet or control message. An item for a busy key goes to the queue (limit 64, then close the link). |
| 3 | A slow command: put the key into `busy`, `checkout`, spawn `handle` on the copy in a `JoinSet`. Return `(key, copy, result)`. |
| 4 | On a job result: `merge`, write the outputs, then run the queue of that key until it is empty or a new job starts. |
| 5 | The loop is one `select!`: socket frame, control message, job result, 10 ms tick. Call `expire_now(\|k\| busy.contains_key(&k))` before each packet and on each tick. |
| 6 | The link task writes outputs itself, one `write_all` for each event. All outputs of a link `Jobs` are for that link. |
| 7 | `Server::consume_world`, `promote_world`, `kick`, `close_anonymous`, `retain` send a `Control` enum with a `oneshot` reply. Find the link with `owners` (key → link), then a directory `std::sync::Mutex<HashMap<LinkId, mpsc::Sender<Control>>>`. Keep the old results: no session gives `Err(16)` for `consume_world`, "session absent" for `promote_world` and `retain`, `Ok(())` for the two others. |
| 8 | After each event, send `take_logouts()` to one writer task for the link (bounded queue, `send().await`). It calls `Store::update_last_logout` in order. |
| 9 | At the end of a link: wait for all job tasks, merge them, then `disconnect_now(link)` and write the logouts. Do not abort a job (a `start` job has an NP presence to release). |
| 10 | The shutdown signal ends the loop of rule 5, then rule 9 runs. |

Known limit to write into the code: a last-logout write and a login of the same account in the same moment can change order. ❓ Not seen as a problem.

## 4. Checks for part 1b

1. `cargo fmt`, `cargo clippy -p lobbydaemon --all-targets --features mssql -- -D warnings`.
2. `LOBBYD_MSSQL_URL="$(cat secrets/lobbyd.mssql.url)" cargo test -p lobbydaemon --features mssql`. Target: 82 pass. Do not print the variable.
3. Add 2 tests: (a) a slow NP call of session A does not delay a packet of session B on the same link; (b) 2 packets of one session keep their order when the first is slow.
4. Update `scale-rules.md` §2 (lane lock row) and §10 step 6.
5. Commit, `tools/push_github.sh`. Restart with `tools/run_play.sh` only when no client is connected (ports 6600, 10900, 10241).

## 5. Other open items (lower priority)

| Item | Where |
| --- | --- |
| Read timeout on client sockets (rule H7) | `GameDaemon/src/conn.rs`, LobbyGate |
| LobbyGate still uses `wire::read_frame` | move to `shared::net::ClientFrameReader` |
| PC create on the LobbyDaemon link does not use `db::Ordered` | `GameDaemon/src/link.rs` |
| StServers: 7 unbounded queues, 1 lock across a wait | step 7 |
| `character_states` and `deletion_deadlines` live in the `Jobs` of one link | they are lost when the player comes through another LobbyGate link ❓; fix in part 2 (load from the database) |

## 6. Standing rules (short)

- ASD-STE100 text. Never write or print keys, passwords or connection strings; write `keyhere`.
- Clean room: spec → new Rust. No original bugs or exploits; abuse notes stay in `Docs/Plan/abuse-notes/` (never `git add`).
- Commit author `Claude <noreply@anthropic.com>`. Leave `tools/slices/queue.tsv` out of commits.
- Do not start the client, the VM or the original StServers.
