# Baseline — measured state before the refactor

Step 1 of `scale-rules.md` section 10. Size: about 900 tokens. Add a new dated section for each new measurement. Do not change old sections.

## 1. How to measure

| Step | Command | Result |
| --- | --- | --- |
| 1 | `tools/run_play.sh start` | The stack starts. Each log line has a time (`HH:MM:SS.mmm`). A capture of the client ports starts (`logs/play/cap/<time>.pcap`). |
| 2 | Log in with the client. Go to character select. Enter the world. Move. Log out. | — |
| 3 | `tools/run_play.sh report` | Table: server response time for each connection. List: each response of 20 ms or more. |

- The capture runs on the server host. The time from a client data packet to the next server data packet is the server-side part. Network time is not in it.
- "Client idle" is the time that the server waits for the client. We cannot change it.
- `PLAY_CAPTURE=0` gives no capture. The capture needs `sudo -n` and the interface `tailscale0`.
- Tool for another capture file: `tools/pcap_latency.py <file> [--slow MS] [--ports PORT:NAME,...]`.

## 2. Measurement 1 — login, capture of 2026-11-09

Source: `logs/client-runs/20261109-024009/cap.pcap` (real client, debug build, 4 login tries; the code is older than the present code). ❓ No capture of the present code exists. ❓ No capture of enter world exists.

| Service | Connections | Requests each | Life of the connection (ms) | Response p50 (ms) | Response max (ms) | Client idle (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| CliGate (login) | 4 | 8 | 630 to 662 | 0.76 to 1.39 | 10.9 to 50.9 | 453 to 462 |
| LobbyGate | 4 | 3 | 199 to 216 | 2.43 to 3.09 | 39.9 to 46.3 | 65 to 79 |

Findings:

| # | Finding | Evidence |
| --- | --- | --- |
| B1 | The server-side part of the login stage is small. Of about 640 ms on the CliGate connection, the server waits about 455 ms for the client. | Columns "Life" and "Client idle". |
| B2 | 6 responses took 40 to 51 ms. All other responses took 0.8 to 3 ms. The value 40 ms is the delayed-ACK time of Linux. ❓ Probable cause: Nagle delay, because the socket has no `TCP_NODELAY` and the reply goes out in more than one write. | Slow list of the tool: LobbyGate first reply (66 B) in 4 of 4 connections, CliGate reply (293 B) in 2 of 4. |
| B3 | ❓ The slow login that the user sees is not in this stage. Candidates: client load time, the stage after LobbyGate (character list, database), GameDaemon connect. | B1. Needs measurement 2. |

## 3. Sockets with no `TCP_NODELAY` (counted 2026-11-11)

| File | Path |
| --- | --- |
| `Platform/StServers/CliGate/src/gate.rs` | accept (game client) |
| `Platform/Sts/src/listener.rs` | accept and connect |
| `Server/LobbyDaemon/src/lib.rs` | accept (LobbyGate link) |
| `Server/LobbyGate/src/daemon.rs` | connect (lobbyd link) |
| `Server/CacheGate/src/server.rs` | accept and connect |
| `Server/AchievementDaemon/src/server.rs`, `RankingDaemon/src/server.rs`, `SmartCouponDaemon/src/server.rs` | accept |

The shared net module (step 2) sets the option in one place (rule H5).

## 4. Build and test state (2026-11-11)

| Fact | Value |
| --- | --- |
| Build that the play stack runs | debug (`Program/output/debug`). Only `rsa` and `num-bigint-dig` have `opt-level = 3`. |
| Release profile | none |
| Time in the logs before this step | none |
| Counters, spans (`tracing`) | none. They come with the shared net module (step 2), in one place. |

Tests (count of `#[test]` and `#[tokio::test]`):

| Crate | Tests | Crate | Tests |
| --- | --- | --- | --- |
| GameDaemon | 578 (36 files in `tests/`) | Sts | 84 |
| LobbyDaemon | 79 | StsCore | 37 |
| RankingDaemon | 52 | CliGate | 22 |
| Packet | 49 | RouteSrv | 19 |
| LobbyGate | 44 | CacheGate | 19 |
| BloodLib | 42 | DbSrv | 11 |
| AchievementDaemon | 38 | PacketIds | 8 |
| SmartCouponDaemon | 23 | AuthSrv 5, GameAccountSrv 2, GameGate 1, PlayStubs 1, SlotSrv 1, Shared 1 | |

❓ Not checked: how many GameDaemon tests compare full packet bytes (golden tests). Check this before the handler split (step 4).

## 5. Open

| Item | Need |
| --- | --- |
| Measurement 2: full login, character select, enter world, movement on the present code | One client session by the user, then `tools/run_play.sh report`. |
| Time of each stage inside the servers (CliGate → AuthSrv → GameGate → lobbyd → database) | Compare the log times of the services for the same session. |
| Cause of B2 | Set `TCP_NODELAY`, measure again. |
| Debug build against release build | Measure after the release profile exists. |
