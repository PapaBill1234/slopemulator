# Card gw-enter-rust: GameDaemon 381 enter world to a standing PC (Rust)

## Goal
`gamedaemon` takes the 381 client from game Hello to a PC that stands in zone 1101 and stays connected. It sends the 381 packets in the 381 order with the 381 values of the spec.

## Source rule (user decision, 2026-11-12)
| What | Source |
| --- | --- |
| Logic: order of steps, checks, default values, timers | 2019 + PDB GameDaemon |
| Packets: ids, formats, field order and sizes, values that the client reads | GameDaemon 381 |
| 381 logic that differs from 2019 and the client can see it | 381 wins |

## Context
Names come from the 381 client table `Docs/Client/RE/game-commands.tsv`. Use these names for constants (add a missing one to `gamedaemon::ids` with the same name; never invent a name):
`G_HelloReqCmd` (11), `G_HelloOkCmd` (12), `G_PingReqCmd` (16), `G_ServerPingNotifyCmd` (17), `G_ServerPingReqCmd` (18), `G_LobbyTokenAuthReqCmd` (23), `G_LobbyTokenAuthOkCmd` (24), `G_LobbyTokenAuthFailCmd` (25), `G_EnterWorldReqCmd` (31), `G_EnterWorldOkCmd` (32), `G_EnterWorldFailCmd` (33), `G_PrepareEnterZoneCmd` (41), `G_EnterZoneReqCmd` (42), `G_EnterZoneOkCmd` (43), `G_EnterZoneFailCmd` (44), `G_QuestListOkCmd` (87), `G_QuestCompleteListOkCmd` (90), `G_CurrentUTCTimeOkCmd` (423), `G_EnergyPropertySyncCmd` (836), `G_ContentsRuleListReqCmd` (929), `G_ContentsRuleListOkCmd` (930), `G_ContentsRuleListFailCmd` (931), `G_CollectingListOkCmd` (1122), `G_FishingListOkCmd` (1126).

Wire order for a new level-1 PC (05 "Ordered path", "Packets after command 32"):
11 → 12, 929 → 930, 23 → 24, 31 → 32, 87, 90, 423, 1122, 1126, 836, 41, client 42 → 43. Do not send 627, 1097, 912 or the builder-auth packet (conditions false for the default config). Ping 17 is off by default (06 "Ping interval").

Existing code: `entry.rs` (`enter_world`, `prepare_zone`, `rule_list`, `hello`, `PC_OBJECT_ID`, `SPAWN_KEY`), `play.rs` (`Play`, `Effect`, `packet`, `entered`), `conn.rs` (`run`, `log` flag), `link.rs` (`START_ZONE` = 1101, `START_POSITION`, `START_YAW`), `shard.rs`. Today these send local guesses (`play-1.md` §10). Replace the guesses with spec values.

Architecture (`Docs/Architecture/scale-rules.md`): §1 tiers and no exploit copy, §3 layers (handlers are pure: bytes in, `Effect` list out, no I/O), §4 ownership (the shard owns the PC; the connection task owns only the socket and crypto), §5 hot path (no lock, no allocation per packet that you can avoid; cache the 930 packet once), §6 game loop (zone work on the shard tick), §11 check before each commit. Keep the `conn.rs` / `shard.rs` / `play.rs` / `entry.rs` split (§10 step 5).

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| Ordered path | `Docs/Server/GameDaemon/05-enter-world.md` | "Ordered path" (lines 35–57) |
| 929/930 handler, body, XML text, rule table | same | "Commands 929–932" (lines 58–318) |
| 24 body | same | "Command 24" (lines 319–341) |
| 32 header, property rules, property table | same | "Command 32: initial player composite" to "Command 32 property table (381)" (lines 360–757) |
| Packets after 32 | same | "Packets after command 32" (lines 758–784) |
| 41, 42, 43, sight | same | "Command 41" to "Sight packets" (lines 785–891) |
| Ping, timers, spawn, packet log contract | `Docs/Server/GameDaemon/06-minimum-session.md` | "Ping interval", "Other timeouts", "Packet log contract" |
| Rust API | `python3 tools/rust_api.py Program/Server/GameDaemon`, `Program/Server/Packet`, `Program/Server/PacketIds` | public items |

## Work
1. Add the names above to `gamedaemon::ids` where they are missing.
2. 929 → 930: build the 930 packet once at start from a generated table of the 167 rule rows (name, type, default; a config file value replaces the default). Elements: XML header (major 74, minor 0), one `<option>` per row in table order, `</config>`. Each element is UTF-8 with no terminator, written as `b`. Revision = −1. Answer rule: one list per 30 s per session; the first request always gets 930; else 931 with reason 2167.
3. 24: free-to-play body `00 FF FF FF FF 00 00 00 00 00 00 00`.
4. 32: header (9 fields) + property blob for ids 1..422 by the `GetPureData` rules + the groups after the blob. Generate the property table from the spec table (type, wire size, offset). Unknown defaults are 0; the name is the PC name.
5. After 32 send 87 (count 0), 90 (empty), 423 (current UTC time), 1122 (count 0), 1126 (count 0), 836 (energy values), in this order.
6. 41: zone object id, channel 1, zone 1101, spawn key from a global counter that skips 0, world clock hour, arenaWaitCancel 0, zone key bytes (`keyhere` in fixtures), position and yaw from `link.rs` start values, PC transition counter +1, summoned counter 0.
7. 42: accept only when the spawn key equals the current key and the action is in state 2; else 44 with reason 1150. Clamp the x/y/z/yaw of 42 on the server (see Abuse fix). Then send 43 with q = 0.
8. Ping: when the config enables it, send 17 every `max(1000, 1000 × send_sec)` ms; compare with the echo 18 by the 06 table. Default: off, 60 s, count 3.
9. Packet log (06 "Packet log contract"): when `GAMED_PACKET_LOG` is set, log each sent and received packet as one line; hex dump only when the value is `hex`.

## Interfaces
```rust
// entry.rs (keep names; change bodies)
pub fn rule_list(config: &str) -> io::Result<Vec<Vec<u8>>>;      // cached 930 fragments
pub fn token_ok(apply_type: u8) -> Vec<u8>;                      // 24
pub fn enter_world(entry: &Pc) -> io::Result<Vec<Vec<u8>>>;      // 32 then 87, 90, 423, 1122, 1126, 836
pub fn prepare_zone(entry: &Pc, spawn_key: u32, hour: u8, counter: u16) -> io::Result<Vec<u8>>; // 41
pub fn enter_zone_ok(enter_social_id: u64) -> Vec<u8>;          // 43
// play.rs
pub fn packet(play: &mut Play, rules: &[Bytes], packet: &[u8], out: &mut Vec<Effect>) -> io::Result<()>;
// conn.rs
pub fn log_line(time_ms: u64, dir: Dir, id: u16, size: usize) -> String; // "time dir id name size"
```

## Files
- `Program/Server/GameDaemon/src/entry.rs`
- `Program/Server/GameDaemon/src/play.rs`
- `Program/Server/GameDaemon/src/conn.rs`
- `Program/Server/GameDaemon/src/shard.rs`
- `Program/Server/GameDaemon/src/ids.rs`
- `Program/Server/GameDaemon/src/rules381.rs` (new, generated rule table)
- `Program/Server/GameDaemon/src/property381.rs` (new, generated property table)
- `Program/Server/GameDaemon/tests/gw_enter.rs` (new)
- `Program/Server/Packet/src/game.rs`, `Program/Server/Packet/tables/game381.json` (only to add 24, 32, 930 codecs)

## Acceptance
`cargo test -p gamedaemon` passes.

| Test name | Input | Expected result |
| --- | --- | --- |
| `loopback_order` | Loopback client: 11, 929, 23, 31, 42 | Server ids in this order: 12, 930, 24, 32, 87, 90, 423, 1122, 1126, 836, 41, 43. No 627, 1097, 912, 17. |
| `golden_930` | Default config | 167 option elements; first element is the header text with major 74; last is `</config>`; revision `FF FF FF FF`; row 1 is `limit-money-market-sale-price` value `15000000000`; bool rows give `yes`/`no`. |
| `rule_list_rate` | 929 twice in 1 s after 30 s uptime | 930, then 931 reason 2167. |
| `golden_24` | Free-to-play account | Body after `18 00` = `00 FF FF FF FF 00 00 00 00 00 00 00`. |
| `golden_32` | New PC, name of n characters | Header 9 fields; blob size 1,024 + 2n; first property id 1 at blob offset 0. |
| `golden_41` | Spawn key 1, hour h | Fields in the 05 order; key bytes `keyhere` fixture; channel 1; zone 1101. |
| `enter_zone_bad_key` | 42 with key + 1 | 44 reason 1150; no 43. |
| `enter_zone_clamp` | 42 with x/y/z far from the start point | Server keeps its own position; no move to the client value. |
| `golden_43` | First entry | `2B 00` + 8 zero bytes. |
| `packet_log_line` | `GAMED_PACKET_LOG=1`, one 43 sent | One line `<time> S 43 G_EnterZoneOkCmd 12` (size field + command + q); no hex. With `hex`: one dump line of 8 zero bytes. |
| `packet_log_off` | `GAMED_PACKET_LOG` not set | No log line. |

Operator step: the 381 client enters zone 1101 and stays 5 minutes. Record the packet log.

## Abuse fix
Command 42 carries client x/y/z/yaw for the summoned object. The server must clamp them to its own spawn position and permitted distance. Do not trust the client value. (Spec: "bug, not ported".)

## Out of scope
- Other zones, saved PCs with items or quests, movement, combat, NPC handlers.
- Packet 4 for self (only if the operator step fails).
- 627, 1097, 912 (config off).
