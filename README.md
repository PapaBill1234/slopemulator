# slopemulator

A Linux Rust reimplementation of the Blade & Soul server set (client build 381,
LobbyDaemon build 307), with its tools, launcher and reverse-engineering notes.

Game client files, server binaries, databases, IDA databases and keys are **not**
in this repository. See [`.gitignore`](.gitignore).

## Plan (11 Nov 2026)

| # | Phase | State |
| --- | --- | --- |
| 1 | Client 381 full reverse engineering | **Done** |
| 2 | StServers (NP platform) full reverse engineering | Next |
| 3 | 2020 server set full reverse engineering | Waiting |
| 4 | Port to Rust from the specs | Some servers done (see below) |

Play path (11 Nov 2026): the real 2020 client logs in, shows character select and makes a character on the Rust servers.
`lobbyd` uses `LobbyDB`. `lobbyd` and `gamedaemon` use the original link messages (create 20/21/22, world entry 5/6/7).
`gamedaemon` creates and loads the PC in `BlGame`. Enter world has no live proof yet.

Details and the work method: [`bns-server/README.md`](bns-server/README.md).

## Map

```
slopemulator/
├── bns-server/                 Rust workspace (full git history)
│   ├── Program/
│   │   ├── Server/             Server crates
│   │   │   ├── BloodLib/       Session, framing, packet fields, ciphers
│   │   │   ├── Packet/         Fragment packets, command dispatch tables
│   │   │   ├── Shared/         NP channel trait, shared types
│   │   │   ├── LobbyGate/      Gate server (build 381)
│   │   │   ├── LobbyDaemon/    Lobby daemon (build 307)
│   │   │   ├── GameDaemon/     Game server (build 381)
│   │   │   ├── CacheGate/      Gate in front of CacheDaemon (348)
│   │   │   ├── AchievementDaemon/  Achievements (348)
│   │   │   ├── RankingDaemon/  Ranking boards and seasons (348)
│   │   │   ├── SmartCouponDaemon/  Game-event coupons (348)
│   │   │   └── Data/
│   │   │       ├── Lmxb/       LMXBOSLB codec
│   │   │       └── GameData/   Typed game data
│   │   ├── Platform/
│   │   │   ├── Sts/            NP / STS login server
│   │   │   └── StServers/      Original NP platform services
│   │   │       ├── StsCore/    Framework: STS messages, Connect, route bus client
│   │   │       ├── DbSrv/      Stored-procedure gateway (ported)
│   │   │       ├── RouteSrv/   Service bus router (phase 1)
│   │   │       └── CliGate/    Platform client gate (phase 1)
│   │   └── Tools/
│   │       ├── BnsEdit/        BnsEdit GUI editor (own Cargo project)
│   │       ├── ClientKey/      Handshake RSA-1024 key generator and client patcher
│   │       ├── ClientDbg/      Live client debugger (plugin + Linux host)
│   │       ├── DataFile/       Data file crates (BnsData, BnsModel, BnsSchema)
│   │       └── Emu/            Python handshake / session emulator for build 381
│   ├── Docs/                   Specs and status for each part
│   │   ├── Server/             LobbyGate, LobbyDaemon, GameDaemon, CacheGate, AchievementDaemon, RankingDaemon, SmartCouponDaemon
│   │   ├── Client/RE/          Client 381 reverse engineering (18 chapters)
│   │   ├── Platform/           STS login, StServers specs and port tracker
│   │   ├── Plan/               Work cards and handoffs
│   │   ├── Reviews/            Parity reviews against the 348 binaries
│   │   └── DataFile/           Data file formats, data file book
│   └── tools/                  Slice runner, push script, index tools
├── launcher-src/               BnsLauncher fork for the private server (C#, MIT)
├── notes/                      Client reverse-engineering notes
├── tabledefs/BNSR_116_x64/     Table definitions (519 JSON files)
└── dist/bnsedit-v1/            BnsEdit v1 builds (Linux, Windows)
```

## Server parity (11 Nov 2026)

How much of each original NCSoft server binary is ported to Rust.
The original binaries come from the `Service_348_8349` server set (not in this repository).
### All original server binaries

**Handlers (348 PDB)** = packet handler functions in the original PDB (`Handle…` functions;
for ChannelChattingServer, `ChattingPacketParserHandler::Recv_…`). Both directions are counted,
for example a `…Req` handler and its `…Ok` / `…Fail` reply handlers.
**Code** = size of the `.text` section. About 200–300 KB of each is the shared network and
console library, so small servers are mostly that library.

| Original binary | What it does | Handlers (348 PDB) | Code | Rust crate | Ported | Ported % |
| --- | --- | ---: | ---: | --- | --- | --- |
| `GameDaemon.exe` | World server: movement, skills, items, party, guild, chat, quests, shop, post, market | 1435 | 7300 KB | `gamedaemon` (381) | 74 / 1598 commands (+423 partial) | `▌░░░░░░░░░` **4.6 %** (31 % started) |
| `CacheDaemon.exe` | Shared character / world data cache | 171 | 1175 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `LobbyDaemon.exe` | Account sessions, character list, create / delete / restore, enter-world token | 124 | 1145 KB | `lobbydaemon` (307) | 18 / 118 table rows | `█▌░░░░░░░░` **15 %** |
| `LobbyGate.exe` | Client entry gate: handshake, login, world list, relay to LobbyDaemon | 112 | 381 KB | `lobbygate` (381) | 56 / 56 commands | `██████████` **100 %** |
| `MarketAgent.exe` | Marketplace agent between GameDaemon and MarketDealerDaemon | 106 | 473 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `ArenaLobby.exe` | Arena matchmaking lobby | 60 | 704 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `AccountInventoryDaemon.exe` | Account-wide inventory | 44 | 801 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `InfoGateDaemon.exe` | Info gate for LobbyDaemon and GameDaemon | 44 | 441 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `PostOfficeDaemon.exe` | In-game mail | 33 | 381 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `MarketDealerDaemon.exe` | Marketplace back end | 30 | 442 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `ChannelChattingServer.exe` | Channel chat (different code base, no shared library) | 29 | 1087 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `DuelBotDaemon.exe` | Duel bot matches (+ `AIAgentDLL`) | 23 | 529 KB | — | 0 | `░░░░░░░░░░` 0 % |
| `RankingDaemon.exe` | Rankings and leaderboards | 16 | 535 KB | `rankingdaemon` (348) | 16 / 16 handlers dispatched; hellos, seasons, weekly result schedules (local time), black list done; RankingDB, boards phase 2 | `█████░░░░░` **48 %** ❓ |
| `CacheGate.exe` | Gate in front of CacheDaemon | 9 | 231 KB | `cachegate` (348) | 9 / 9 handlers, cached-list reload, reconnect | `██████████` **100 %** handlers ❓ |
| `AchievementDaemon.exe` | Achievements: GameDaemon events → registers, steps, rewards | 7 | 448 KB | `achievementdaemon` (348) | 7 / 7 handlers; gameplay event rules phase 2 (MSSQL not started) | `███████░░░` **70 %** ❓ |
| `SmartCouponDaemon.exe` | Game-event coupons: GameDaemon events → NP coupon publication | 6 | 327 KB | `smartcoupondaemon` (348) | 6 / 6 handlers (MSSQL + NP transport not started) | `██████████` **100 %** handlers ❓ |
| `ArenaDaemon` | Arena match server. It runs the same `GameDaemon.exe` and `GameDaemon.pdb` | (GameDaemon) | (GameDaemon) | — | 0 | `░░░░░░░░░░` 0 % |
| STS / NP login (`StsMsgSrv64.dll` peer) | Platform login: SRP-6a password login, RC4 session, game tokens | — | — | `sts` (2020 client) | 7 / 7 client commands | `██████████` **100 %** ❓ |
| **All except GameDaemon** | | **814** | | | | |

Not counted: `ncgagent.exe`, `ncguardserver64.dll` and `ggauth64_27.dll` (anti-cheat, left out on purpose),
`bsw.exe` and `StartServer` (service tools).

### StServers (original NP platform)

The platform services that `StServer.ini` starts: 49 service types, 81 processes.
Scoring: Ported = 100 %, In progress = 50 %. Tracker: [`bns-server/Docs/Platform/StServers/README.md`](bns-server/Docs/Platform/StServers/README.md).

| Service | Rust crate | State | Ported % |
| --- | --- | --- | --- |
| DbSrv (33 instances) | `dbsrv` | Ported; replies, errors and partial rules checked live on SQL Server | `██████████` **100 %** |
| RouteSrv | `routesrv` | Phase 1 (19 tests): Connect, routing, reply relay. Not yet: forward scope, multi-router, TLS | `█████░░░░░` **50 %** |
| CliGate | `cligate` | Phase 1 (22 tests): login flow, logout, timeouts, key routing | `█████░░░░░` **50 %** |
| Shared framework | `sts-core` | Messages, Connect, route types, route bus client, secrets loader | (used by all) |
| AuthSrv, GameGate, SessionGateSrv, GameAccountSrv, SlotSrv, SpawnSrv | — | Analysed or decompiled, no Rust yet | 0 % |
| 40 other services | — | Not started | 0 % |
| **All 49 service types** | | | `▍░░░░░░░░░` **4 %** |

Phase 2 of the plan reverses all of StServers before more porting.

### Porting plan for the remaining servers

Order: smallest first. Estimates are rough: handler count is only one part of the work.
Each server also has database procedures, game rules and links to other servers.

| Group | Servers (handlers) | Estimate |
| --- | --- | --- |
| Very small | SmartCouponDaemon (6), AchievementDaemon (7), CacheGate (9), RankingDaemon (16) | About 1 day each |
| Medium | DuelBotDaemon (23), ChannelChattingServer (29), MarketDealerDaemon (30), PostOfficeDaemon (33), InfoGateDaemon (44), AccountInventoryDaemon (44), ArenaLobby (60) | Several days each |
| Large | MarketAgent (106), CacheDaemon (171) | Similar to LobbyDaemon (124 handlers: 11 steps so far, 15 %) |

### STS sources

The Rust `sts` crate was built from these original files (not in this repository):

| File | Origin | Used for | Docs |
| --- | --- | --- | --- |
| `StsMsgSrv64.dll` + PDB | Server set, `GameDaemon/bin/` | Transport, messages, login flow, token validation, password login (SRP-6a setup) | `01`–`06` |
| `StsMsgCli64.dll` (no PDB) | 2020 UE4 client (BNSR), 488 960 bytes | Client login wire: SRP-6a, KeyData, client XML, RC4 session; token verify RSA-2048 key at `0x58D60` | `07`–`09` |
| `Client.exe` + PDB | 2020 client | Client login flow | `03` |
| ❓ `StsMsgCli32.dll` | 2020 UE3 client | Not checked yet | `07` |

### How each number was measured

| Column / server | Source |
| --- | --- |
| Handlers (348 PDB) | `llvm-pdbutil dump --symbols` on each `Service_348_8349/Service/*/bin/*.pdb`: unique procedure names that start with `Handle` + capital letter. |
| Code | `.text` virtual size from `llvm-pdbutil dump --section-headers`. |
| LobbyGate ported | All 56 commands in `Program/Server/Packet/tables/381.json` (client: not-authed 3, any-authed 2, authed 2 + 28 relayed; LobbyDaemon side 21) have a Rust handler in `Program/Server/LobbyGate/src/`. The 348 PDB has 112 functions because it has a separate function for each relayed command; 381 relays them through one path. |
| LobbyDaemon ported | `Docs/Server/LobbyDaemon/command-tables.json` lists 118 handler rows in 5 tables. Rust handles 18 rows of the LobbyGate-side table (35 rows). The Manage (6), InfoGate (3), Game (51) and AccountInventory (23) tables have no Rust handlers yet; GameDaemon is a local stub. |
| GameDaemon ported | `Docs/Server/GameDaemon/parity-ledger.tsv`: 1598 commands; `rust` column = 74 ported, 423 partial, 1101 none. |
| STS ported | `Docs/Platform/Sts/STATUS.md`: 7 client XML commands are implemented. ❓ No real-client login has run yet. |

The handler counts come from build 348. The ported counts come from the Rust target builds (381 / 307),
so the two columns do not match exactly. ❓ The descriptions of the servers with 0 % come from their binary
names only.

## Rust crates (`bns-server/`)

| Crate path | Contents | In workspace |
| --- | --- | --- |
| `Program/Server/BloodLib` | Session, framing, packet fields, ciphers | Yes |
| `Program/Server/Packet` | Fragment packets, dispatch tables (`tables/381.json`) | Yes |
| `Program/Server/Shared` | NP channel trait, shared types, local stub | Yes |
| `Program/Server/LobbyGate` | Gate server, build 381 | Yes |
| `Program/Server/LobbyDaemon` | Lobby daemon, build 307 | Yes |
| `Program/Server/GameDaemon` | Game server, build 381 | Yes |
| `Program/Server/Data/Lmxb` | LMXBOSLB codec | Yes |
| `Program/Server/Data/GameData` | Typed game data | Yes |
| `Program/Platform/Sts` | NP / STS login server | Yes |
| `Program/Platform/StServers/StsCore` | StServers framework: STS messages, Connect, route bus client | Yes |
| `Program/Platform/StServers/DbSrv` | DbSrv: stored-procedure gateway to SQL Server (ported) | Yes |
| `Program/Platform/StServers/RouteSrv` | RouteSrv: service bus router (phase 1) | Yes |
| `Program/Platform/StServers/CliGate` | CliGate: platform client gate (phase 1) | Yes |
| `Program/Tools/ClientDbg` | Live client debugger: plugin in the client, host on Linux | Yes |
| `Program/Tools/DataFile/BnsData` | Reader and writer for LMXBOSLB / TADBOSLB data files | Yes |
| `Program/Tools/DataFile/BnsModel` | Data model for BnsData | Yes |
| `Program/Tools/DataFile/BnsSchema` | Passive TableDefs export from PE client files | Yes |
| `Program/Tools/ClientKey` | Handshake key pair generator and client patcher | Yes |
| `Program/Tools/BnsEdit` | BnsEdit GUI editor | **No**: build it from its own folder |

Build and check (from `bns-server/`):

```
cargo test
cargo clippy --workspace --all-targets -- -D warnings
```

Build BnsEdit:

```
cd bns-server/Program/Tools/BnsEdit && cargo build --release
```

## Documents

| Path | Contents |
| --- | --- |
| `bns-server/Docs/Server/LobbyGate/` | 01–05: config, handshake, framing, dispatch, handlers |
| `bns-server/Docs/Server/LobbyDaemon/` | 00–11: overview, config, links, DB, login, world entry, GameDaemon link, character select; JSON evidence |
| `bns-server/Docs/Server/GameDaemon/` | 01–25: tables, world connect, enter world, session, handshake key, parity, handler groups (movement, items, skills, party, guild, chat, quests, shop, post, market, arena); parity ledger and evidence |
| `bns-server/Docs/Platform/Sts/` | 01–09: transport, messages, login flow, token validation, password login, token key patch, token login server |
| `bns-server/Docs/DataFile/` | 01–10: LMXBOSLB, TADBOSLB, tabledefs, table body, strings, model, schema from exe; JSON exports |
| `bns-server/Docs/Client/RE/` | Client 381 reverse engineering: 18 chapters, command and enum tables (`INDEX.md`) |
| `bns-server/Docs/DataFile/book/` | Data file book: format, protection, schema, loader, versions, server side, tools, merged XML |
| `bns-server/Docs/Platform/StServers/` | StServers specs: CliGate, DbSrv, RouteSrv, route client, AuthSrv; error codes, procedures |
| `bns-server/Docs/Plan/` | Work cards and handoffs |
| `bns-server/Docs/DataFile/client-exe/` | Python scripts for client exe and PDB analysis |
| `notes/client-datafiles.md` | `datafile.bin`, `datafile64.bin`, `locale.bin`: generation, format, keys, versions |
| `notes/client-exe-reversing.md` | Client exe reverse-engineering notes |
| `notes/client-381-game-commands.tsv` | Client build 381 game command table |
| `notes/sol-usage.jsonl` | Token usage log of the delegated coding sessions |

## Launcher (`launcher-src/`)

Fork of [notscuffed/BnsLauncher](https://github.com/notscuffed/BnsLauncher) (MIT, see `launcher-src/LICENSE`).

| Path | Contents |
| --- | --- |
| `Source/BnsLauncher/` | WPF app: views, view models, logging |
| `Source/BnsLauncher.Core/` | Profiles, services, DPAPI password storage, private server sample profile |
| `Source/BnsLauncher.Tests/` | Unit tests |
| `Source/BnsLauncher.sln` | Visual Studio solution |
| `Source/Publish.bat` | Publish script |

## Not in this repository

| Excluded | Reason |
| --- | --- |
| Game clients, server binaries, `.exe` / `.dll` / `.pdb` (except `dist/`) | NCSoft copyright |
| Databases (`.mdf`, `.ldf`, `.rar`), `datafile*.bin`, `locale.bin` | Game data |
| IDA databases, decompiled output | Derived from NCSoft binaries |
| `server-keys/`, `*.key`, `*.pem`, `*.blob` | Secrets |
| `target/`, `Program/output/`, `bin/`, `obj/` | Build output |
