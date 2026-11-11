# StServers port tracker

Goal: a Rust port of the original StServers (NP/STS platform, build 381 deployment from
the BNSX image) with 100 % parity. Keys, passwords and certificates become the
placeholder `keyhere`. Inventory and topology: `../Sts/10-stservers-original.md`.

Updated 8 Nov 2026.

## Terms

| Term | Meaning |
| --- | --- |
| Ported | Rust crate exists, behaviour checked against the binary, tests pass |
| In progress | Analysis done, Rust crate being written |
| Analysed | Decompiled and documented, no Rust yet |
| Not ported yet | The original has it; the port does not yet |
| Not used | The shipped config does not turn it on |

## Shared parts

| Part | Crate | State | Spec |
| --- | --- | --- | --- |
| STS message, framing, errors, INI, XML | `sts-core` | Ported (used by DbSrv) | `../Sts/01-transport.md`, `../Sts/02-messages.md` |
| Connect handshake | `sts-core::connect` | Ported | `11-cligate-protocol.md` |
| Route message, commands, locations, `t` targets | `sts-core::route` | Types ported | `13-routesrv.md` |
| Route bus client (service → RouteSrv registration) | `sts-core::route_client` | Ported (15 tests) | `14-route-client.md` |
| Socket transaction map (`p`/`t`/`s` headers) | `routesrv::relay` | Ported in RouteSrv (25000 ms map) | `13-routesrv.md` §8.1 |
| Secrets loader (`keyhere` → real value at start-up) | `sts-core::secrets` | Ported; DbSrv uses it. Names and rules: `secrets/README.md` | — |
| TLS between servers | — | Not ported yet; key = `keyhere` | `13-routesrv.md` §6 |
| Service start-up (arguments, registration, request loop) | `sts-core::service` | Local helper for the small services | — |
| Admin HTTP pages | — | Not ported yet (low priority) | `../Sts/10-stservers-original.md` |

## Port parity (11 Nov 2026)

Counted by service type. The 49 types are the services that `StServer.ini` starts.
Scoring: Ported = 100 %, In progress = 50 %, all other states = 0 %.

| Measure | Count | Percent |
| --- | --- | --- |
| **Rust port parity** | 1 ported + 5 in progress of 49 | **7 %** |
| Ported (DbSrv) | 1 of 49 | 2 % |
| In progress (RouteSrv, CliGate, AuthSrv, GameAccountSrv, SlotSrv) | 5 of 49 | 10 % |
| Analysed or decompiled (no Rust yet) | 3 of 49 | 6 % |
| Not started | 40 of 49 | 82 % |
| By process count (DbSrv runs 33 instances) | 33 of 81 processes ported | 41 % ❓ misleading, one binary |

```
Port parity  [###-----------------------------------------------] 7 %
Analysis     [#########-----------------------------------------] 18 %
```

The percent changes when a service reaches "Ported". RouteSrv is next.

## Services started by `StServer.ini` (49)

| Service | State | Notes |
| --- | --- | --- |
| DbSrv (33 instances) | **Ported** | All reply, error and partial rules verified live on SQL Server. Not ported yet: RouteSrv registration. Storage path: not used. Spec `12-dbsrv-contract.md` |
| RouteSrv | **In progress** | Phase 1 crate `routesrv` (19 tests): Connect, service commands, Direct/Key/Block routing, reply relay. ❓ key hash, provider codes and command reply shapes are local choices. Not yet: forward scope, multi-router, TLS. Spec `13-routesrv.md`, crate `Program/Platform/StServers/RouteSrv/README.md` |
| CliGate | **In progress** | Phase 1 crate `cligate` (22 tests): forward of `GameAccount/ListMyAccounts`, `Slot/ListSlots`, `VirtualCurrency/GetBalance`, `Auth/GetMyUserInfo` for a logged-in user; login flow, logout, idle and auth timeouts, key routing through RouteSrv. Spec `11-cligate-protocol.md`. The `sts` crate still covers the client login path today |
| AuthSrv | **In progress** | Phase 1 crate `authsrv` (5 tests): LoginStart, KeyData, LoginFinish, RequestToken, RequestGameToken, ConsumeGameToken, route bus registration. Accounts come from a TSV file, not from DbSrv. Spec `15-authsrv.md`, crate `Program/Platform/StServers/AuthSrv/README.md` |
| GameGate | Analysed (partial) | Daemon gate for LobbyGate/GameDaemon, 74 commands |
| GameAccountSrv | **In progress** | Login path only: `ListAccounts`, `ListMyAccounts` from a TSV file (2 tests). Crate `Program/Platform/StServers/GameAccountSrv` |
| SlotSrv | **In progress** | Login path only: `ListSlots` with an empty list (1 test). Crate `Program/Platform/StServers/SlotSrv` |
| SessionGateSrv, SpawnSrv | Decompiled | SessionGateSrv is not on the game client login path (`PlayStubs/README.md`) |
| AccessSrv, ApiGate, ApiManageSrv, ApiSrv, AppAuthSrv, AppGate, AuctionSrv, BanSrv, CacheSrv, ChargeSrv, CouponSrv, DeliverySrv, EventLogSrv, ExAuthSrv, ExAuthnSrv, ForwardSrv, FriendSrv, GoodsSrv, GradeMembersSrv, GradeSrv, IpCheckSrv, LevelSrv, MwGate, NameSrv, PresenceSrv, PrivacySrv, ProfileSrv, PromotionStampSrv, PurchaseLimitSrv, PurchaseSrv, RefundSrv, RewardSrv, RoleSrv, ShowcaseSrv, SvcDispatcher, SysInfoSrv, VirtualCurrencySrv, WarehouseSrv, WebGate, WishSrv | Not ported yet | Order follows the login and in-game flow |

## Programs in the image but not started (65)

WorldSrv is decompiled but has no `StServer.ini` line ❓ (check where the world list comes from before porting it).

StorageSrv and MongoDbSrv belong here: no `StServer.ini` line, no `[Sql.StorageDb]`,
and `MongoDbSrv` is commented out. They are "not used" in this deployment. They can be
ported later with local Redis and MongoDB.

## Order of work

1. RouteSrv (single router) → DbSrv registration.
2. CliGate, AuthSrv, SessionGateSrv, GameAccountSrv, SlotSrv, GameGate (client login to character select).
3. SpawnSrv, then the remaining started services.
4. Not-used programs (StorageSrv with Redis/Mongo, others).
