# SlotSrv (Rust port, login path only)

RE reference: `Docs/Platform/StServers/RE/03-gameaccount-slot-spawn-world.md` §2.

Run: `SlotSrv --config StServer.ini --route-ini Route.ini`

| Command | State |
| --- | --- |
| `ListSlots` (4) | Reply `<Reply/>` (no slot). SlotDb has 0 rows; the DB call is not ported |
| `CreateSlots`, `DeleteSlot`, `GetSlot`, `ResetData`, `SetData`, `SetClientData`, `ExpireSlot`, `Flush` | Skipped. Reply: error 52 |

Test: `cargo test -p slotsrv` (1 test).
