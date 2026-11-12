# st-10 progress note

## Done
- GoodsSrv §1 of the chapter; `10-commands.tsv` and `10-procs.tsv` hold GoodsSrv rows.

## In progress
- ShowcaseSrv start: init = caller of `StsSetServerType`; RTTI vtable list; `0x1400E9A40` (shared target for most Showcase ids).

## Hypotheses
- `0x1400E9A40` in ShowcaseSrv is the dispatch or a generic proxy handler. Check with `hr`.
- PromotionStampSrv dispatch is in the socket class slot 0 (find RTTI `C*Socket` vtable, as GoodsSrv `CGoodsSocket` `0x1401AE478`).
- GoodsSrv `quirk_blank_id_swap` (`0x1400F3270`, `0x1400F32F0`).

## Ruled out
- GoodsSrv `cfgkeys()` and `timers()`: only framework rows. Service keys are read with `IniValueAs*` (Framework gaps).
- GoodsSrv `CGoodsChangeTxn` / `CGoodsChangeNotifyTxn` vtables: slot +0x18/+0x20 are `nullsub_1`.

## Next
1. ShowcaseSrv: `hr(init)`, RTTI list (exclude framework classes; fix f-string quoting), `hr(0x1400E9A40)`, `cfgkeys()`, `dbprocs()`; write §2 and TSV rows.
2. PromotionStampSrv: same steps; find dispatch; §3 and TSV rows.
3. GoodsSrv gaps in the handoff "Not done".

## IDA state
- Names applied by `boot` and `member-names.tsv` only. No service renames saved. Re-run boot in each database.
