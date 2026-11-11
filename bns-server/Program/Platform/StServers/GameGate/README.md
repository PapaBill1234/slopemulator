# GameGate (Rust port, token path only)

RE reference: `Docs/Platform/StServers/RE/04-gamegate-appgate-mwgate-webgate.md` §1.

```text
LobbyDaemon ──GateNp (TCP 6601)──► GameGate ──route bus──► AuthSrv (Auth/ConsumeGameToken)
```

Run: `GameGate --config StServer.ini --route-ini Route.ini [--listen addr:port]`

| Part | State |
| --- | --- |
| `Sts/Connect` from a daemon | Accepted. ❓ Reply shape (empty OK) |
| `Auth/ConsumeGameToken` (31) | Forward to the bus, reply to the daemon. Not ported: `TokenInfo`, header `f`, client info |
| `Auth/VerifyMobiOtp`, `Auth/VerifySecondPwd` | Error 49, line 1742, `invalid cmd` |
| Default forward (dispatch step 7) | Each other command goes to the bus without a whitelist check ❓ |
| `gamegate::np_client::GateNp` | Daemon side: `consume` over TCP. Presence, name and world entry use the backend given by the daemon (`StubNp` in `lobbyd`) |

Skipped (74 commands in `Gate.ini [Commands.GameGate]`): `Presence/Login` and its `Auth/LoginInternal` chain, `Presence/Logout`, `Auth/LoginTrusted`, `Auth/LoginToken`, `Auth/RequestToken` target conn, all `Gate/*` (streams, RegisterService, RegisterForward), `Charge/Start`, `Charge/Stop`, bus → daemon requests, IP check, `MaxRequestsToGame`.

Test: `cargo test -p gamegate` → `game_token_through_the_gate` (RouteSrv, AuthSrv, GameAccountSrv, GameGate, GateNp on loopback).
