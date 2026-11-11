# PlayStubs (not a port)

Fixed replies for two services that the login path calls. AccessSrv and VirtualCurrencySrv are not analysed. Remove a row when the real service exists.

Run: `PlayStubs --config StServer.ini --route-ini Route.ini`. `StServer.ini` needs a `PlayStubs` line.

| Command | Caller | Reply | Why |
| --- | --- | --- | --- |
| `Access/GetAppUserRights` | CliGate, LoginFinish step 2 (`11-cligate-protocol.md` §10.2) | `<AccessMask>0</AccessMask>` ❓ | CliGate stops the login without `AccessMask` |
| `VirtualCurrency/GetBalance` | game client, after LoginFinish | `<BalanceInfoListCount>0</BalanceInfoListCount>` | Same reply as `sts-local` in the live login of 8 Nov 2026 |

Not in this crate:

| Service | Reason |
| --- | --- |
| SessionGateSrv | The game client login path sends no `Session` command. Its 309 commands serve web and app sessions (`RE/01` §3). |
| PresenceSrv (`Presence/Login`, LoginFinish step 3) | CliGate phase 1 does this step locally. |

Test: `cargo test -p play-stubs` (1 test).
