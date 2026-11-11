# GameAccountSrv (Rust port, login path only)

RE reference: `Docs/Platform/StServers/RE/03-gameaccount-slot-spawn-world.md` §1.

Run: `GameAccountSrv --config StServer.ini --route-ini Route.ini --accounts game-accounts.tsv`

Account file: TSV with 4 fields: `UserId`, `GameCode`, `Alias`, `GameAccountId`. The original reads GameDb through DbSrv (0 rows today).

| Command | State |
| --- | --- |
| `ListAccounts` (1), `ListMyAccounts` (3) | Ported. Reply: `GameAccount` items with `Alias` and `GameAccountId`. ❓ Source of the target code; `type` attribute of the list |
| All other `GameAccount` commands (see `RE/03-commands.tsv`) | Skipped. Reply: error 52 |

Test: `cargo test -p gameaccountsrv` (2 tests).
