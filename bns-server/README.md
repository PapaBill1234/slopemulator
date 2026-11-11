# bns-server

A Linux Rust reimplementation of the Blade & Soul server set (client build 381).
The folder layout follows the original NCSoft source tree, as seen in the build
paths inside the original binaries (`...\depot\Branch\381NC\Program\Server\...`).

| Folder | Original | Contents |
| --- | --- | --- |
| `Program/Server/BloodLib` | `Program\Server\BloodLib` | Session, framing, packet fields, ciphers (crate `bloodlib`) |
| `Program/Server/Packet` | `Program\Server\Packet` | Fragment packets, command dispatch tables `tables/381.json` (crate `packet`) |
| `Program/Server/Shared` | `Program\Server\Shared` | NP channel trait, shared types and local stub (crate `shared`) |
| `Program/Server/LobbyGate` | LobbyGate | Gate server, build 381 (crate `lobbygate`) |
| `Program/Server/LobbyDaemon` | LobbyDaemon | Lobby daemon, build 307 (crate `lobbydaemon`) |
| `Program/Server/GameDaemon` | GameDaemon | World server, build 381 (crate `gamedaemon`) |
| `Program/Server/CacheGate` | CacheGate | Cache gate (crate `cachegate`) |
| `Program/Server/AchievementDaemon` | AchievementDaemon | Achievements (crate `achievementdaemon`) |
| `Program/Server/RankingDaemon` | RankingDaemon | Ranking boards and seasons (crate `rankingdaemon`) |
| `Program/Server/SmartCouponDaemon` | SmartCouponDaemon | Smart coupons (crate `smartcoupondaemon`) |
| `Program/Server/Data` | `Program\Server\Data` | LMXBOSLB codec (`Lmxb`) and typed game data (`GameData`) |
| `Program/Platform/Sts` | platform-common | NP client-login server (crate `sts`) |
| `Program/Platform/StServers/StsCore` | StServers framework | STS messages, Connect, Route types, INI, errors (crate `sts-core`) |
| `Program/Platform/StServers/DbSrv` | `DbSrv.exe` | Stored-procedure gateway to SQL Server (crate `dbsrv`) |
| `Program/Platform/StServers/RouteSrv` | `RouteSrv.exe` | Service bus router (crate `routesrv`, in progress) |
| `Program/Platform/StServers/CliGate` | `CliGate.exe` | Client gate for the platform login, phase 1 (crate `cligate`) |
| `Program/Tools` | — | Data file tools, BnsEdit, emulator scripts, client key tool, live client debugger (`ClientDbg`) |
| `Program/output` | `Program\output` | Build output (not in git) |
| `Docs/` | — | Specs and status for each part |
| `Docs/Plan/` | — | Work cards and handoffs for the slice pipeline |
| `tools/` | — | Slice runner, push script, spec and API index tools |

## Plan

The work goes in 4 phases. Reverse engineering comes first, so that the port uses specs and not guesses.

| # | Phase | State |
| --- | --- | --- |
| 1 | Client 381 full reverse engineering | **Done** (10 Nov 2026) |
| 2 | StServers (NP platform) full reverse engineering | Next |
| 3 | 2020 server set full reverse engineering (GameDaemon and others) | Waiting |
| 4 | Port to Rust from the specs | Some parts done (see Status) |

## Status (11 Nov 2026)

| Part | State | Tracker |
| --- | --- | --- |
| LobbyGate 381 | Ported; handshake, framing, dispatch, handlers | `Docs/Server/LobbyGate/` |
| LobbyDaemon 307 | Ported; login and links | `Docs/Server/LobbyDaemon/` |
| GameDaemon 381 | 502 client handlers: 74 ported, 423 partial, 5 not started | `Docs/Server/GameDaemon/09-parity.md`, `parity-ledger.tsv` |
| CacheGate, AchievementDaemon, RankingDaemon, SmartCouponDaemon | Ported; parity reviews done | `Docs/Server/<name>/README.md` |
| STS client login (`sts`) | SRP-6a login server; real-client login not yet verified | `Docs/Platform/Sts/STATUS.md` |
| StServers (original NP platform) | DbSrv ported. RouteSrv phase 1, CliGate phase 1, route bus client in `sts-core`. AuthSrv analysed | `Docs/Platform/StServers/README.md` |
| Play path (card `play-1`) | Real client: login, character select and create work. `lobbyd` uses `LobbyDB`. `lobbyd` ↔ `gamedaemon` link (create 20/21/22, world entry 5/6/7). `gamedaemon` creates and loads the PC in `BlGame`. Enter world 23 → 43: no live proof yet | `Docs/Plan/handoffs/play-1.md` |
| Client 381 reverse engineering | **Done**: 18 chapters (start-up, transport and crypto, STS, LobbyGate, game dispatch, movement, combat, items, quests, social, InfoGate/arena, data files, UI, engine, errors), command and enum tables | `Docs/Client/RE/INDEX.md` |
| Client data files | Book: 10 chapters (format, protection, schema, loader, versions, server side, tools, one merged XML source for client and server) | `Docs/DataFile/book/00-index.md` |
| Live client debugger (`ClientDbg`) | Part 1: plugin in the client, host on Linux | `Program/Tools/ClientDbg/README.md` |

Next work in the queue (`tools/slices/queue.tsv`):

| Card | What |
| --- | --- |
| `ids-1-rust` | Crate `packet-ids`: official packet and enum names from the client RE tables, instead of bare numbers |
| `dfx-4-rust` | Tool `bnsxml`: dump client and server data to XML, merge, build both sides again |
| Phase 2 cards | StServers full reverse engineering |

## How the work is done

| Step | Who | Output |
| --- | --- | --- |
| Reverse engineering (IDA) | Claude Opus | Spec chapters in `Docs/` and a Rust card in `Docs/Plan/cards/` |
| Rust code from a card | Claude Sonnet | Crate code, tests, handoff in `Docs/Plan/handoffs/` |
| Help jobs | Sol (Codex) | Scripts and checks |
| Gate | `tools/slice_runner.py` | fmt, test, clippy, allowed paths; then commit and push |

Original bugs in the NCSoft code (for example out-of-bounds reads) are copied as named `quirk_<name>` functions, because game data and scripts can depend on them.

Build and check:

```
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

LobbyDaemon keeps the public paths `lobbydaemon::np`, `store::AccountId`,
and `gamed::{GameFuture, ChargeMode}` as re-exports of shared types.
STS uses `shared` directly. Enable its `np-adapter` feature to use
`sts::np_adapter::AuthorityNp`.

The two LMX readers remain separate. BnsData keeps raw UTF-16 units,
three header padding bytes, and children on text nodes. It takes a key
from the caller and permits depth 256. Lmxb uses Unicode strings,
requires zero header padding and leaf text nodes, and permits depth 128.
It does not take a key from the caller. These differences prevent a
behavior-preserving replacement. See `Program/Tools/DataFile/BnsData/src/lmx.rs`
and `Program/Server/Data/Lmxb/src/lib.rs`.
The source comparison is in `notes/client-datafiles.md`.

No keys, passwords or game data go into this repository.
