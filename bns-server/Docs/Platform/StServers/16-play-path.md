# 16 — Play path: gap map from client login to the world

Card `play-1`, stage 1. Date: 11 Nov 2026. Client: build 381.
Sources: the specs and crate sources named in the table. No new IDA facts are in this file.

## 1. Path

```text
 BnS client
   │ (1) TCP 6600, STS text frames, SRP-6a, then RC4
   ▼
 CliGate ──(2) route bus 6605──► RouteSrv ──┬─(3) Auth/LoginStart, KeyData, LoginFinish ─► AuthSrv ─(4)─► DbSrv ─► PlatformAcctDb
   │                                        ├─(5) Access/GetAppUserRights ─► AccessSrv
   │                                        ├─(6) Presence/Login ─► PresenceSrv (through SessionGateSrv ❓)
   │                                        └─(7) Auth/RequestGameToken ─► AuthSrv
   │ client has: game token GUID + portal GUID
   ▼
 LobbyGate ──(8) protocol 58──► LobbyDaemon ──(9) Np trait──► GameGate 6601 ─► RouteSrv ─┬─ Auth/ConsumeGameToken ─► AuthSrv
   │                                                                                       ├─ GameAccount/* ─► GameAccountSrv ─► DbSrv
   │ character select, world token, lobby event 26 (address, port)                         └─ Slot/* ─► SlotSrv ─► DbSrv
   ▼
 GameDaemon ──(10) TCP, Hello 11/12, RSA → AES-128, protocol 1278
   │ (11) token 23/24 → entry 31/32 → prepare 41 → load 42 → enter 43 → ping 17/18
   ▼
 player stands in the world
```

## 2. Hops

| # | Hop | Spec | Rust crate | State | Missing |
| --- | --- | --- | --- | --- | --- |
| 1 | Client → CliGate: Connect, LoginStart, KeyData, LoginFinish, RC4 | `11-cligate-protocol.md`, `../Sts/07-client-login-wire.md` | `cligate` | Phase 1, 22 tests, no live test | ❓ RC4 scope and switch point, reply root. A real client must confirm them. |
| 2 | CliGate → RouteSrv → service | `13-routesrv.md`, `14-route-client.md` | `routesrv`, `sts-core::route_client` | Phase 1, 19 + 15 tests | Single router is enough. ❓ key hash is a local choice. |
| 3 | AuthSrv login: SRP verifier, session record, LoginFinish | `15-authsrv.md` §1–§6, `RE/02` | none | Analysed | The full crate. SRP, KeyData codec are in the old `sts` crate. |
| 4 | AuthSrv → DbSrv: `AcctLoginInfoById` and login-finish procs | `12-dbsrv-contract.md`, `15-authsrv.md` §8 | `dbsrv` | Ported. No RouteSrv registration | DbSrv registration on RouteSrv. A test account row in `PlatformAcctDb.dbo.Users` (all tables have 0 rows). |
| 5 | LoginFinish step 2: `Access/GetAppUserRights` | `11-cligate-protocol.md` §10.2 | none | AccessSrv not analysed | A reply with `<AccessMask>`. The card has no stage for AccessSrv. |
| 6 | LoginFinish step 3: `Presence/Login` | `11-cligate-protocol.md` §10.3, `RE/01` §3 | none | SessionGateSrv decompiled, PresenceSrv not analysed | A reply that CliGate accepts. The card has no stage for PresenceSrv. |
| 7 | Game token: `Auth/RequestGameToken`, `Auth/ConsumeGameToken` | `15-authsrv.md` §7 | none (old `sts::tokens`) | RequestGameToken analysed. ConsumeGameToken not done | Both commands in AuthSrv. |
| 8 | Client → LobbyGate → LobbyDaemon: login to character select | `../../Client/RE/04-lobbygate.md` | `lobbygate`, `lobbydaemon` | Done. Test `character_gate.rs` | Nothing for the loopback path. No live client test. |
| 9 | LobbyDaemon / GameDaemon → GameGate | `RE/04` §1 | none | GameGate analysed in part. LobbyDaemon uses `StubNp` in process | GameGate crate. A TCP `Np` implementation in `shared::np_channel` (consume, presence, world entry, name). |
| 9a | GameAccountSrv, SlotSrv | `RE/03` §1, §2 | none | Decompiled | Only the commands that hop 9 calls. |
| 9b | World list | `RE/03` §4 | `lobbydaemon::jobs::Settings.worlds` | Fixture from the command line | WorldSrv has no `StServer.ini` line ❓. Keep the fixture. |
| 10 | Client → GameDaemon: listener, Hello, RSA → AES-128, outer frame | `../../Server/GameDaemon/04-world-connect.md`, `08-client-handshake-key.md` | `gamedaemon` (no sockets), `BloodLib::frames`, `Packet::game` | No listener, no binary | A GameDaemon server binary. Our own RSA key pair (the client key is patched by `keypatch.dll`). ❓ command 929. |
| 11 | Enter world: 23/24, 31/32, 41, 42, 43, ping | `05-enter-world.md`, `06-minimum-session.md` | `Packet::{world_entry, zone_entry, game_session}` codecs | Codecs only. No state machine | ❓ command-24 success body. ❓ full command-32 layout. ❓ spawn and appear packets. ❓ loading-screen exit. IDA work is necessary. |

## 3. Decisions

| Decision | Reason |
| --- | --- |
| The new crates `cligate` + `authsrv` carry the client login. | The card prefers them. They follow the original service split. |
| Move `srp6.rs` and `key_data.rs` from `sts` into `authsrv`. CliGate keeps its own RC4 (`cligate::crypt`). | Only AuthSrv does SRP. The `sts` crate stays as it is until the live test passes. |
| Hops 5 and 6 (AccessSrv, PresenceSrv) become one small crate `play-stubs` in stage 3. It gives fixed replies. | The card has no stage for them. CliGate cannot finish LoginFinish without the two replies. The stub is not parity; the README must say so. |
| WorldSrv stays out. The world list is a start-up fixture. | ❓ No `StServer.ini` line. |
| GameGate (stage 4) implements only the commands behind the `Np` trait. | `shared::np_channel::Np` is the full set that LobbyDaemon calls. |

## 4. Risk per stage

| Stage | Size | Risk |
| --- | --- | --- |
| 2 AuthSrv | medium | Low. The spec is complete for login. ConsumeGameToken needs IDA. |
| 3 Services | medium | Medium. Presence and Access reply shapes need IDA. |
| 4 GameGate | medium | Medium. A TCP `Np` client is also necessary. |
| 5 Run script | small | Low. |
| 6 Loopback test | medium | Low up to character select. |
| 7 Enter world | large | **High.** GameDaemon has no listener. Four packet facts are ❓ (hop 11). This stage decides if the player stands in the world. |
| 8 Live instructions | small | The first live run will find the ❓ items of hop 1. |

The shortest way to a first live result is: stages 2, 3 and 5, then a live login to character select (hops 1–8 with `StubNp`), then stage 7. Stage 4 does not block the first live login, because LobbyDaemon can consume the game token in process. The card order stays; this note is for the user.
