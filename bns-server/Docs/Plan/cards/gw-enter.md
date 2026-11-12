# Card gw-enter: GameDaemon 381 enter world to a standing PC (spec)

## Goal
The spec gives every packet and value that GameDaemon 381 sends from the client connect to a PC that stands in zone 1101 and stays connected. The Rust slice `gw-enter-rust` can build it with no IDA work. You also write the card `Docs/Plan/cards/gw-enter-rust.md`.

## Source rule (user decision, 2026-11-12)
| What | Source |
| --- | --- |
| Logic: order of steps, checks, default values, timers | 2019 + PDB GameDaemon (full names and types) |
| Packets: command ids, formats, field order and sizes, values that the client reads | GameDaemon 381 |
| 381 logic that differs from 2019 and the client can see it | 381 wins |

Names: use the 381 client name table (`Docs/Client/RE/game-commands.tsv`; `Client.exe` 381 table `0x1953620`). Never invent a packet name.

## Context
- Live state (`Docs/Plan/handoffs/play-1.md` §1, §2, §10, §11): login, LobbyGate, LobbyDaemon and the GameDaemon Hello 11/12 work with the real 381 client. The client then sends 929 and waits. `gamedaemon` answers 31 and 42 with local guesses (§10 "Local choices", all ❓).
- Open items in the specs: 929 → 930 body (§11 has the layout: 167 rules, major 74), 24 success body, the full 32 composite (422 property ids, 11 groups), the spawn and appear packets before 43, ping 17 and its timeouts.
- The 381 IDB has names that were moved from 2019 + PDB (8,880 of 16,299). A comment with `port:` gives the evidence; ❓ marks a weak name. The handler slot registry is `ida/port2020/GameDaemon/handlers381.tsv` (outside the worktree; the IDB comments carry the same names).

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| GameDaemon 381 (target: packets) | `/home/ubuntu/Documents/Codex/private-project/ida/port2020/GameDaemon/tgt/GameDaemon.exe.i64` | `boot(WT, stservers=False)`; do not save |
| GameDaemon 2019 + PDB (logic) | `/home/ubuntu/Documents/Codex/private-project/ida/port2020/GameDaemon/src/GameDaemon.exe.i64` | `boot(WT, stservers=False)`; do not save |
| Enter world | `Docs/Server/GameDaemon/05-enter-world.md` | all (238 lines) |
| Minimum session | `Docs/Server/GameDaemon/06-minimum-session.md` | all (153 lines) |
| Live results | `Docs/Plan/handoffs/play-1.md` | §10, §11 |
| Rust API | `python3 tools/rust_api.py Program/Server/GameDaemon` and `Program/Server/Packet` | public items |

## Work
1. 929 → 930: the full 381 body. The 167 rule values for a default server (config file and defaults); revision value `dword_140EE586C` (where it comes from).
2. 24: the success body.
3. 32: the full composite for a new level-1 PC: every group and property id with its type and the default value for a new PC. Use the 381 builder for the layout and the 2019 + PDB code for the default values. Give a table that the Rust code can generate from.
4. 41, 42, 43: real values (object id rule, zone, channel, spawn key).
5. Every packet that the server sends between 32 and the first ping that the client needs to show and control the PC (spawn, appear, sync, stats, skills, inventory, quick slots). For each: id, official name, format, value source, order.
6. Ping 17 and the session timeouts (`06-minimum-session.md` §3–§5): values from 381.
7. Write the results into `05-enter-world.md` and `06-minimum-session.md`. Replace each local guess of `play-1.md` §10 with the 381 value, or keep the ❓ with the reason.
8. Packet log contract for the Rust card: each sent and received packet as one line `time dir id name size` (name from the client table) with `GAMED_PACKET_LOG`, and an optional hex dump of the body.
9. Write `Docs/Plan/cards/gw-enter-rust.md` (template `Docs/Plan/card-template.md`, max 3,000 tokens). Crates: `gamedaemon`, `packet`, `packet-ids`. Copy the source rule and the architecture rules. Acceptance: a loopback test with the exact packet order, golden bytes for 930, 24, 32 and 41, and the operator step "the 381 client enters zone 1101 and stays 5 minutes". It contains `cargo test -p gamedaemon` and a "## Files" list.

## Architecture rules for the Rust card
- `Docs/Architecture/scale-rules.md` §1 tiers, §3 layers (handlers are pure), §4 ownership, §5 hot path, §6 game loop (shards, tick), §11 check before each commit. Use the existing `conn.rs`, `shard.rs`, `play.rs`, `entry.rs` structure (§10 step 5).

## Files
- `Docs/Server/GameDaemon/05-enter-world.md`
- `Docs/Server/GameDaemon/06-minimum-session.md`
- `Docs/Plan/cards/gw-enter-rust.md` (new)

## Out of scope
- No Rust change. No client IDA work.
- Only a new level-1 PC in zone 1101. No other zone, no saved PC with items or quests.
- No movement, combat or NPC handlers.
