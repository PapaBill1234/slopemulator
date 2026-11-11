# Card play-1: client connects to our StServers port and enters the world

Interactive session (not the slice queue). Model: Fable. Work dir: `/home/ubuntu/Documents/Codex/private-project/bns-server`.

## Goal
The 2020 client (build 381) logs in through **our Rust StServers**, reaches character select, and enters the world on our GameDaemon. Today the client never ran against the Rust stack end to end.

Do the work in this order. Each stage ends with a commit that passes the checks. Stop at the first stage that you cannot finish, and write why.

## What exists (read the status first)
| Part | State | Where |
| --- | --- | --- |
| StServers tracker | 4 % ported | `Docs/Platform/StServers/README.md` |
| `sts-core` (framing, messages, connect, route types, route client, secrets) | ported | `Program/Platform/StServers/StsCore` |
| DbSrv | ported | `Program/Platform/StServers/DbSrv`, spec `Docs/Platform/StServers/12-dbsrv-contract.md` |
| RouteSrv | phase 1 (19 tests) | `Program/Platform/StServers/RouteSrv`, spec `13-routesrv.md`, `14-route-client.md` |
| CliGate | phase 1 (22 tests) | `Program/Platform/StServers/CliGate`, spec `11-cligate-protocol.md` |
| AuthSrv | analysed, no Rust | spec `15-authsrv.md` |
| Old single crate `sts` (SRP, KeyData, RC4, listener). No live test. | implemented | `Program/Platform/Sts`, status `Docs/Platform/Sts/STATUS.md` |
| LobbyGate + LobbyDaemon | done, login to character select (test `LobbyDaemon/tests/character_gate.rs`) | `Program/Server/Lobby*` |
| GameDaemon | 502 handlers, many partial, no live test | `Program/Server/GameDaemon`, `Docs/Server/GameDaemon/STATUS.md` |

## Documents (sizes in tokens)
Read with `python3 tools/spec_index.py <doc>` first. Then read only the line ranges you need.

| Topic | Document | Tokens |
| --- | --- | ---: |
| Original StServers topology, programs, ports | `Docs/Platform/Sts/10-stservers-original.md` | 3,000 |
| Login flow (client view) | `Docs/Platform/Sts/03-login-flow.md` | 2,200 |
| Client login wire (SRP, KeyData, RC4) | `Docs/Platform/Sts/07-client-login-wire.md` | 5,400 |
| Client STS code | `Docs/Client/RE/03-sts-client.md` | 6,000 |
| Client → LobbyGate | `Docs/Client/RE/04-lobbygate.md` | 6,200 |
| CliGate protocol | `Docs/Platform/StServers/11-cligate-protocol.md` | 5,800 |
| DbSrv contract | `Docs/Platform/StServers/12-dbsrv-contract.md` | 2,600 |
| RouteSrv | `Docs/Platform/StServers/13-routesrv.md` | 2,300 |
| Route bus client | `Docs/Platform/StServers/14-route-client.md` | 3,300 |
| AuthSrv login and tokens | `Docs/Platform/StServers/15-authsrv.md` | 4,700 |
| RE: framework | `Docs/Platform/StServers/RE/00-framework.md` | — |
| RE: gate library, CliGate, SessionGateSrv | `Docs/Platform/StServers/RE/01-gate-cligate-sessiongate.md` | 13,200 |
| RE: AuthSrv, AppAuthSrv | `Docs/Platform/StServers/RE/02-authsrv-appauthsrv.md` | 12,300 |
| RE: GameAccountSrv, SlotSrv, SpawnSrv, WorldSrv | `Docs/Platform/StServers/RE/03-gameaccount-slot-spawn-world.md` | 10,700 |
| RE: GameGate (daemon gate for LobbyGate and GameDaemon) | `Docs/Platform/StServers/RE/04-gamegate-appgate-mwgate-webgate.md` | 8,900 |
| RE: commands and DB procs per service | `Docs/Platform/StServers/RE/0N-commands.tsv`, `0N-procs.tsv`, `config-keys.tsv` | grep only |
| World connect, enter world, minimum session | `Docs/Server/GameDaemon/03-minimum-world.md`, `04-world-connect.md`, `05-enter-world.md`, `06-minimum-session.md` | 2,000–4,100 each |
| Client handshake key | `Docs/Server/GameDaemon/08-client-handshake-key.md` | — |
| Rust rules | `tools/slices/builder-rules.md` (quirks, abuse notes, checks) | — |

## Stages
1. **Gap map.** Follow one login from the client to the world: client → CliGate → RouteSrv → AuthSrv → DbSrv → SessionGateSrv / GameAccountSrv / SlotSrv → GameGate → LobbyGate → GameDaemon. For each hop, write: the spec, the Rust crate, the state, and what is missing. Put it in `Docs/Platform/StServers/16-play-path.md` (ASCII diagram + table). Decide here whether the old `sts` crate or the new crates carry the client login. Prefer the new crates (`cligate` + `authsrv`); reuse code from `sts` (SRP, KeyData, RC4) by moving it into `sts-core` or the new crate.
2. **AuthSrv.** New crate `Program/Platform/StServers/AuthSrv` from `15-authsrv.md` and RE/02: login, SRP verifier, LoginFinish, game token. Register on RouteSrv with `sts-core::route_client`. DB calls through DbSrv.
3. **Session and account services.** Only the commands the login path needs: SessionGateSrv, GameAccountSrv, SlotSrv (RE/01, RE/03). One crate each. List the skipped commands in the crate README.
4. **GameGate.** Only the commands that LobbyGate and GameDaemon need for the token check and account data (RE/04).
5. **Run script.** `tools/run_play.sh start|stop|status`: start DB container check, DbSrv, RouteSrv, AuthSrv, the services, CliGate, GameGate, LobbyDaemon, LobbyGate, GameDaemon in tmux, with logs in `logs/`. Write a test account helper (`keyhere` for the password in docs).
6. **Loopback test.** An integration test with a scripted client (SRP login → token → LobbyGate → character select → enter world packets from `05-enter-world.md`). Use the client models in existing tests.
7. **Enter world.** Make the GameDaemon minimum session from `06-minimum-session.md` accept the character from LobbyGate and send the enter-world packets. The player must stand in the world. Other features stay out.
8. **Live test instructions** for the user: client config (`release.config2.xml` server address), launcher, what to look for in each log. The user runs the client on the laptop. Do not try to start the client.

## Missing RE
- If a spec lacks a fact, use IDA (MCP `ida`). The StServers databases are under `ida/` (use `list_databases`). Load `tools/ida_lib.py` with `boot(WT)`; it applies the StServers framework names.
- Reference behaviour: the original StServers run under Wine on this host: `~/Documents/Codex/stsrv-wine/run.sh <Prog>.exe /AppIndex N /Email test@localhost`. Use them to capture real bytes. Stop them with `WINEPREFIX=~/Documents/Codex/stsrv-wine/prefix /usr/lib/x86_64-linux-gnu/wine/wineserver -k`. Never `pkill -f <name>`.
- Write each new fact into the spec at once, with address and ❓ where not verified.

## Rules
- Clean room: binary → spec → Rust. Do not paste Hex-Rays code.
- Copy original bugs and exploits (parity first). Abuse cases go to `Docs/Plan/abuse-notes/play-1.md` only.
- Never write keys, passwords, certificates or connection strings. Write `keyhere`. Real secrets load through `sts-core::secrets`.
- Never print, copy or read the gateway key or the IDA licence.
- DB: container `bns-mssql` at 127.0.0.1:14333. Never print credentials.
- Before each commit: `cargo fmt`, `cargo test -p <crate>`, `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Commit author `Claude <noreply@anthropic.com>`. End the message with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Push each verified commit with `tools/push_github.sh`.
- Update `Docs/Platform/StServers/README.md` (tracker and parity numbers) when a service changes state.
- Do not read a file over 40,000 bytes in full.
- Write all docs in ASD-STE100. Sizes in tokens.
- Do not ask questions between stages. Decide, write the reason, continue.
- At the end, write `Docs/Plan/handoffs/play-1.md`: done (with test names), not done, ❓, blocked, next step.
