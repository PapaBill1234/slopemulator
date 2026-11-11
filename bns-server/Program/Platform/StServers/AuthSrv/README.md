# AuthSrv (Rust port, phase 1)

Spec: `Docs/Platform/StServers/15-authsrv.md`. Play path: `16-play-path.md`.

```text
CliGate ─► RouteSrv ─► AuthSrv (provider `Auth`) ─► GameAccount/ListAccounts (bus)
                          │
                          └─ account file (TSV)   original: DbSrv `AcctLoginInfoById`
```

## Run

| Command | Function |
| --- | --- |
| `AuthSrv --config StServer.ini --route-ini Route.ini --accounts accounts.tsv [--index N] [--server-id N]` | Starts the service and registers `Auth` on RouteSrv. |
| `AuthSrv enroll <LoginName> <UserName>` | Reads the password from stdin. Prints one account line (salt and verifier only). |

Account file: UTF-8 TSV with 6 fields: `LoginName`, `UserId`, `UserName`, `UserCenter`, `Salt` (Base64, 8 bytes), `Verifier` (Base64). Keep the file in `secrets/` (git ignores it).

## Commands

| Id | Command | State | Spec |
| --- | --- | --- | --- |
| 22 | LoginStart | Ported. The account file resolves the login name ❓ (original: `Name.GetIdByLoginName` before AuthSrv) | §3 |
| 21 | KeyData | Ported (SRP from `sts::srp6`, codec from `sts::key_data`) | §4, §5 |
| 23, 27 | LoginFinish | Ported for the path without secondary auth and PC cafe | §6 |
| 32 | RequestToken | Ported. No issuer AppId, no timer log | §7.2, §7.3 |
| 30 | RequestGameToken | Ported | §7.4, §7.5 |
| 31 | ConsumeGameToken | Ported. Not ported: GeoIP address rule, second bus request (step 8) ❓ | §7.7 |
| 18 | GetMyUserInfo | ❓ Reply shape from the client parser only | `../Sts/07-client-login-wire.md` |

## Not ported

| Item | Reason |
| --- | --- |
| DbSrv call `AcctLoginInfoById` | The account DB has 0 rows and no name resolver service exists. The file has the same columns. |
| Pre-checks (sub_1400C94A0, sub_1400C9180, sub_1400DF820) | Not decoded ❓ |
| Forward to the owner AuthSrv (grid) | Single instance |
| Login session time-out | Not decoded ❓. A session stays until KeyData fails or LoginFinish runs. |
| Other `Auth` commands (110 ids) | Not on the login path |

## Tests

`cargo test -p authsrv`: `login_finish_and_tokens`, `wrong_password_deletes_the_session`, `request_errors`, 2 GUID tests.
