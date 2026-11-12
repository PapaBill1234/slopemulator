# 06 — Minimum standing session, build 381

## Result and limit

Partial static specification. No client test ran.
The checked code does not establish a complete minimum standing session.
It establishes part of the initialization order and ordinary ping behavior.
❓ A successful load-screen exit and a standing player remain unverified.
Do not treat the candidate set below as an acceptance-tested recipe.

Sources: the allowed build-381 `Client_copy.i64` and `GameDaemon.exe`.
Connection details are in [04-world-connect.md](04-world-connect.md).
Packet fields and table sources are in [05-enter-world.md](05-enter-world.md).
The table chain is in [03-minimum-world.md](03-minimum-world.md).

## Required, conditional and optional

In this table, **checked dependency** means that the examined path uses the exchange.
It does not mean that every valid client path was tested.
**Unverified omission** means that this job did not prove safe removal.

| Packet or group | Static classification | Standing-session rule | Evidence / gap |
| --- | --- | --- | --- |
| Separate game connection and Hello 11/12 | Checked dependency | Use game protocol 1278, not LobbyGate protocol 58. | Client `0xF2C0C0`; server `0x1403703D0`; ordinary outer framing checked in 04. ❓ Full handshake acceptance open. |
| Client command 929 after HelloOk | Observed, meaning open | Do not silently remove it or invent its response. | Client `0xF70280` calls `0xF2BEF0`; ❓ server handling and relation to token step open. |
| Token exchange 23/24 | Checked dependency | Consume admission through the lobby link before character entry. | Server `0x140371170`; client command-24 callback `0xF3AF90`; ❓ success body open. |
| Entry request 31 and PC composite 32 | Checked dependency | Initialize the player before prepare-zone processing. Preserve required fields and fragment structure. | Server `0x1403718A0`, sender `0x1405F5980`; client `0xEF6860`; ❓ full payload open. |
| Prepare destination 41 | Checked dependency | Supply a consistent zone, correlation key, destination and transition counters. | Server `0x1404BEED0`; client prepare target `0x73E6C0`. |
| Enter-zone request 42 | Checked dependency | Wait for the client's load/enter request. Do not assume that sending command 41 completes entry. | Client `BUiWorld::RequestEnterZone`, `0x7815C0`; server `0x140371EE0`. |
| Enter-zone success 43 | Checked completion target | Format `hq`; opaque runtime spawn-action value. ❓ Final screen-exit conditions remain open. | Server `0x1402304A0`; client slot → `0xEF6790` → `World::EnterZone`, `0x74C850`. |
| Local-player and nearby-object appearance | Unverified omission | ❓ No safe omission rule. Do not fabricate an appear command ID. | Spawn/appear packet set not recovered. |
| Inventory | Unverified omission | ❓ Not proved optional. An empty collection still needs a valid encoded layout if present. | Large PC composite and supplementary sends not fully mapped. |
| Skills | Unverified omission | ❓ Not proved optional. | Same PC-composite gap. |
| Quests and scripts | Unverified omission | ❓ Not proved optional for this starting path. | Table catalogs exist; client/entry dependencies remain open. |
| Party, friends and other social data | Unverified omission | ❓ Not proved optional. | Group boundaries and supplementary packet IDs remain open. |
| Server ping 17 / client echo 18 | Conditional liveness exchange | Reply with the two unchanged values when enabled. | Server `0x1406832D0`, `0x140370F10`; client `0xF4B430`, `0xF3B230`. |
| Client ping 16 | Checked no-op receive path | Accept after the channel is ready. No reply is proved. Do not use it in place of command 18. | Client `0x47D7A0` → `0xF3B2A0`; server `0x140370210`. |

No inventory, skill, quest or social packet group was proved safe to omit.
No header-only command-32 or command-43 success was proved valid.
These are research limits, not evidence that every supplementary packet is mandatory.

## Ordinary ping packets

Full formats include the command `h` and exclude the outer size field.

| ID | Direction | Full format | Body | Checked behavior |
| ---: | --- | --- | --- | --- |
| 16 | Client → GameDaemon | `h` | None in the checked constructor | Client checks handshake-finished byte +0x5C. Server generic receive handler logs and returns true. |
| 17 | GameDaemon → client | `hqq` | sequence:q, timestamp:q | Server timer stores the sent pair at PC +20712. Client receive slot points to `0xF4B430`. |
| 18 | Client → GameDaemon | `hqq` | Same sequence:q, same timestamp:q | Client `0xF3B230` constructs command 18 and copies both values. Server stores the received pair at PC +20728. |

The client command-17 callback calls the echo builder on the checked ordinary branch.
The timestamp comes from a server `GetTickCount64`-based calculation.
Treat it as an opaque echo value. Do not convert it to wall-clock time.
The sequence value also belongs to the server's ping state.
An empty command 18 is not a valid echo.

The server command-18 handler also has a conditional GameGuard hook at `0x14036DE10`.
That hook is out of scope. No hook internals were inspected.
❓ Ordinary ping support does not prove vendor-client acceptance under all configurations.

## Ping interval and disconnect checks

Timer worker: `0x1406832D0`.
Configuration object: the server network configuration reached through the daemon manager.
Descriptor entries identify these options and offsets:

| Option name | Configuration offset | Checked use |
| --- | ---: | --- |
| `enable-server-ping` | +1444 | Enables periodic server ping work. Descriptor `0x140ED1060`. |
| `server-ping-send-sec` | +1448 | Interval input. Descriptor `0x140ED1088`. |
| `server-ping-disconnect-count` | +1452 | Maximum pending sequence difference. Descriptor `0x140ED10B0`. |
| `server-ping-disconnect-no-packet-enabled` | +1445 | No-packet policy flag. Descriptor `0x140ED10D8`. ❓ Final close-helper behavior not inspected. |
| `server-ping-disconnect-invalid-packet-enabled` | +1446 | Invalid-packet policy flag. Descriptor `0x140ED1100`. ❓ Final close-helper behavior not inspected. |

The worker computes the interval as `max(1000, 1000 × server-ping-send-sec)`.
The result is consistent with milliseconds and the seconds option name.
❓ Scheduler wall-time calibration was not independently traced.

Descriptor layout (381, 40 bytes each, from `0x140ED1050`): description text pointer, config offset, name pointer, type (1 = int, 4 = bool), default pointer.
Built-in defaults (381, gw-enter stage 3):

| Option | Type | Default pointer | Default value |
| --- | --- | --- | --- |
| `enable-server-ping` (+1444) | bool | null | false ❓ (null pointer read as "no default", thus false) |
| `server-ping-send-sec` (+1448) | int | `0x1407E30E4` | 60 |
| `server-ping-disconnect-count` (+1452) | int | `0x1407E3D18` | 3 |
| `server-ping-disconnect-no-packet-enabled` (+1445) | bool | null | false ❓ |
| `server-ping-disconnect-invalid-packet-enabled` (+1446) | bool | null | false ❓ |

Thus with built-in defaults the server sends no ping 17. A server config file can enable it. ❓ The deployed config values are not known.
Port default: ping off; if on, interval 60 s and disconnect count 3.

Let `S` be the last sent pair and `R` the last received pair.
Let `d_seq = S.sequence − R.sequence`.
Let `d_time = S.timestamp − R.timestamp`.
Let `K` be `server-ping-disconnect-count`.
The checked worker makes these decisions before the next send:

| Condition, in checked order | Action |
| --- | --- |
| No positive previous sent sequence | Initialize the tracked pair and send a ping. |
| Both pairs are equal | Send the next ping. |
| Unequal pairs and `d_seq > K` | Call failure helper `0x140683680` with the `NoPacket` category. |
| Unequal pairs, `d_seq <= K`, and either difference is negative | Call the same helper with the `InvalidPacket` category. |
| Unequal pairs, `d_seq <= K`, and both differences are nonnegative | Log a pending response and send the next ping. |

The worker reschedules through `0x140674A00`.
❓ The exact close action, policy-flag effects and complete cleanup path remain open.
Do not state a universal disconnect duration in seconds.
A count threshold is not a fixed deadline from TCP accept.

## Other timeouts and readiness

| State | Evidence | Limit |
| --- | --- | --- |
| Zone spawn wait | PcSpawnAction start `0x1404BEAE0` schedules `0x140674A00` with value `1800000`. Timeout callback `0x1404BEE30` logs that value and cancels the action path. | ❓ Complete unit conversion and resulting client disconnect behavior not verified. Do not state a fixed wall-clock timeout. |
| Scheduler | `0x140674A00` divides its input by the owning scheduler field +16 before queueing. 2019 + PDB `PcSpawnAction::startAction` `0x1404B4B00` line 83: `_postTick = 1800000 / serializer->_interval`, then `Serializer::post`. Thus +16 is `Serializer::_interval` (ms per tick) and 1,800,000 is in ms (30 minutes). | ❓ Value of `_interval` not traced. |
| Spawn position (2019) | `PcSpawnAction::startAction` `0x1404B4B00` lines 44–60: if the action has a creature id, use that creature's x/y/z; else call `GeoZone::GetPcSpawn(geozone, 1, &x, &y, &z, &yaw)`. zupper = z + 1. Then `checkSpawnPosition`; on success `prepareSpawn` (sends 41). On failure: log and `finishAction`. | ❓ Meaning of argument 1 in `GetPcSpawn` (index or type) not traced. |
| TCP/Hello/token wait | Separate session and authentication states exist. | ❓ Timer values and all failure-close rules not recovered. |
| Client load/cinematic wait | `0x78E8F0` can defer command 42 through a cinematic branch. | ❓ No universal loading-finished timer or separate final ready command recovered. |
| Idle standing | Ordinary ping fields and server checks recovered. | ❓ Complete client-side idle-disconnect rules and mandatory background packets remain open. |

## Implementation gate

Step-4 update: ordinary framing rules and RSA → AES-128 families are checked.
See `04-world-connect.md` for the private-object stop boundary and acceptance limits.
Commands 42 and 43 now have checked structural layouts in `05-enter-world.md`.
The separate Packet game module supplies codecs, not a successful-entry state machine.
The checklist below retains the wider standing-session gates from step 3.

Do not claim a working standing session until all rows below have evidence.

| Gate | Required next evidence |
| --- | --- |
| Game channel | Ordinary framing and RSA → AES-128 families are checked. ❓ Full processor compatibility and client secret ownership remain open. Observe the private-object stop boundary. Resolve command 929. |
| Admission | Recover command-24 success layout. Keep the token opaque and single use. |
| Character initialization | Recover the full command-32 layout, property types, fragment contract and field provenance. |
| Destination | Command-42 correlation and summoned-object fields are checked. ❓ Coordinate units, special branches and initial-spawn selection remain open. Do not use a first-record default. |
| Standing | Command 43 is `hq`. ❓ Resolve its q meaning, spawn/appear packets and final loading-screen exit outside restricted code. |
| Optional data | Prove omission safety for each inventory, skill, quest and social group. Do not infer safety from names. |
| Liveness | Verify scheduler units, configured ping policy and ordinary timeout behavior. |

Step 4 adds a separate game-protocol dispatch table and generated codec tests in Packet.
See `05-enter-world.md#rust-codec-contract-step-4` for the implementation contract.
❓ Compilation and test results are not available: the execution tool requires approval.
Keep unknown packet bodies unsupported.
Do not reuse the LobbyGate dispatch table as a game table.
Do not implement a successful enter-world path from these partial layouts.

## Restricted boundaries and scope

| Boundary | Address | Stop rule |
| --- | --- | --- |
| Client anti-cheat state | `0x741AB2` | Stop the World::EnterWorld path. |
| Client security-version getter | Calls `0xF3B00C`, `0xF3B1F6` | Do not inspect the getter. |
| Client command-32 security interface | `0xEF68C8` | Do not follow the security call. |
| Server zone-entry anti-cheat hook | `0x1405DF767`, containing function `0x1405DE040` | Do not follow the hook path. |
| Server ping anti-cheat hook | `0x14036DE10`, from `0x140370F10` | Do not inspect hook internals. |
| Licence/vendor-secret dependency | Static RSA private-object load at `0x1406E3100`; ❓ acceptance dependency remains open | Stop this key-analysis path. No secret material was read. No licence work. |

No anti-cheat or licence implementation is specified.
No prohibited binary, network connection or git command was used.
No binary patch or explicit database save was made.
Step 4 also changes the Packet game module, its table and its generated tests.
Shell execution requests did not run: the tool requires approval.
