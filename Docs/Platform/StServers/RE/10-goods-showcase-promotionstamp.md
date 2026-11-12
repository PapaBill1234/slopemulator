# 10 — GoodsSrv, ShowcaseSrv, PromotionStampSrv (RE reference)

Summary:
1. GoodsSrv (server type 1056) keeps the shop catalogue: categories, items, goods, displays, prices. It reads and writes `GoodsDb` through DbSrv.
2. GoodsSrv has 55 command ids (`Goods` map `0x140286120`). 53 have a handler; ids 44 and 45 get error 52.
3. GoodsSrv sends change notices to `GameWarehouse.<GameCode>` (commands 7, 8, 9 of its `Game` map `0x1402860E0`).
4. ShowcaseSrv and PromotionStampSrv: only the command maps are known (§2, §3). The handlers are not done.
5. Tables: `10-commands.tsv` (GoodsSrv 55 rows), `10-procs.tsv` (GoodsSrv 136 proc rows).

All addresses are in the binary of the section. ❓ = not verified in IDA.

## Quick find

| Question | Function | Binary and address | Section |
| --- | --- | --- | --- |
| GoodsSrv service init | `GoodsAppInit` | GoodsSrv `0x1400F6BA0` | 1.1 |
| Goods.ini reader | `CGoodsConfigs` slot 0 | GoodsSrv `0x1400EA220` | 1.2 |
| VirtualCurrency.ini reader | `CVirtualCurrencyConfigs` slot 0 | GoodsSrv `0x1400EAE20` | 1.2 |
| BillingPolicy.ini reader | `CBillingPolicy` slot 0 | GoodsSrv `0x14011F1E0` | 1.2 |
| Command dispatch | `CGoodsSocket` slot 0 | GoodsSrv `0x1401038F0` | 1.3 |
| Admin txn result switch | `CGoodsAdminTxn` slot +0x18 | GoodsSrv `0x1400CD6C0` | 1.4 |
| Notice to games | `GameWarehouse` address builder | GoodsSrv `0x14011BF40` | 1.6 |
| Admin HTTP test pages | page table | GoodsSrv `0x1400EAF60` | 1.7 |
| ShowcaseSrv command map | `Showcase` map | ShowcaseSrv `0x140248DC0` | 2 |
| PromotionStampSrv command map | `PromotionStamp` map | PromotionStampSrv `0x140266620` | 3 |

## 1. GoodsSrv

### 1.1 Role and start (`0x1400F6BA0`)

In order:
1. `StsSetServerType(1056, 0)`.
2. `StsConfigMonitorAdd(L"Goods.ini", 0x140285170)` (`CGoodsConfigs`, vtable `0x1401A9F70`, constructor `0x1400E95A0`).
3. `StsConfigMonitorAdd(L"VirtualCurrency.ini", 0x1402852D0)` (`CVirtualCurrencyConfigs`, vtable `0x1401A9F90`, constructor `0x1400E9710`).
4. `0x14011F050`: `StsConfigMonitorAdd(L"BillingPolicy.ini", 0x140233540)` (`CBillingPolicy`, vtable `0x1401B6220`; library `Services\BillingPolicy\BillingPolicy.cpp`).
5. `unknown_libname_6_0("Goods", 0, …)` (same form as HelloSrv, `00-framework.md` §2.2).
6. `0x1400F6A50`: two local logs (`LocalLogInit`): one default log and one `Error` log.
7. `StsRouteStart(0x140233380, 0, 0)`, `StsCountersPageAdd(0)`, then admin pages `0x1400EAF60` (§1.7).

Callers: admin tools (Add/Change/Remove commands carry `ChangerAdminAccount` / `RegistrarAdminAccount`) and shop/game services (`GetGameGoodsBy*`, `ListPurchaseGoods`, `GetSaleGoods`). The callers are not traced ❓.

Errors that the handlers write: code at +0, `1056` (server type) at +8, `3` (module) at +12, source line at +16, text at +24 (for example `GetGoods` `0x1400FE140` lines 39–48). The text is also logged as `..\..\GoodsSocket.cpp,<line>,<code>,<text>` by `0x1400F6900`. Note: the offsets differ from `00-framework.md` §3.1 (+4 server, +8 module, +12 line) ❓ (the record pointer may be 8 bytes before the `StsError`).

### 1.2 Config keys

`Goods.ini` (`0x1400EA220`; object `0x140285170`):

| Section | Key | Default | Field | Use |
| --- | --- | --- | --- | --- |
| `Log` | `File` | 0 | +8 = 1 when value is 1 | log to file ❓ |
| `Common` | `SearchDbName` | empty | +40 (text) | DB name for 3 search procs; empty → `GoodsDb` (`0x1400F6A20`, call sites `0x1400F40A3`, `0x1400F44AC`, `0x1400F4413`) |
| `Common` | `AllGoodsInformation` (many) | none | list of `CConfigAppGroupCode` | app groups that get all goods ❓ |
| `Common` | `GoodsInfoMultiLanguage` (many) | none | list of `CConfigAppGroupCode` | app groups with multi-language info ❓ |
| `Common` | `MaxCheckGoodsDuration` | 180 | +36 | `CheckNewGoods` range ❓ |
| `Paging` | `UnsafePendingSize` | 58982 | +32 | ❓ |
| `Paging` | `GameServerSize` | 10 | +12 (word) | page size ❓ |
| `Paging` | `IngameGoodsSize` | 10 | +14 (word) | `ListIngameGoods` page size ❓ |
| `Paging` | `IngameItemsSize` | 10 | +16 (word) | `ListIngameItems` page size ❓ |
| `Cache` | `DbIgnoreDuration` | 0 | +20 = value × 10000000 (100-ns units, so seconds) | cache keeps DB data this long ❓ |
| `Currency` | `DefaultPriceDisplayUnitCode` | 0 | +28 (word) | |
| `Currency` | `DefaultCurrencyGroupId` | 0 | +30 (word) | |
| `Currency` | `Mapping` (many, 2 ints) | none | hash of {a, b} | currency mapping |
| `IngameNotification` | `GameCode` (many) | none | hash +120 | games that get notices (§1.6) ❓ |
| `TransNotiAppGroupCode` | `Trans` (many) | none | hash +288 (`CTransAppGroupCode`) | app group translation for notices ❓ |
| `Reserved` | `AccountHoldMode` | 0 | +24 = 1 | ❓ |

`VirtualCurrency.ini` (`0x1400EAE20`): section `CurrencyConfig`, key `Config` (many). Each value is 2 ints `(a, b)`; both must be ≠ 0. A 0x30-byte record stores `{b, a, b}` as words and goes into a hash (`0x1400D6830`).

`BillingPolicy.ini` (`0x14011F1E0`, shared library): strings `Language`, `Default`, `IsPayBackAvailable`, `Mask`, `MobileDb`, `MobileGoods`, `MobileDelivery`, `MobileWarehouse`, `MobileCoupon`, `MobileReward`, `RequiredReceiverParam`. Key layout not decoded ❓.

### 1.3 Dispatch (`CGoodsSocket` vtable `0x1401AE478`, slot 0 `0x1401038F0`)

1. Parse with `0x14011E2B0` (calls `NameMapFind` on map `0x140286120`).
2. `switch` on the id. Each case calls `handler(this, err, req)`. The handler list is in `10-commands.tsv`. Ids 23, 24 and 25 share `0x140101720`.
3. Default (ids 0, 44 `IngameGoodsChanged`, 45 `IngameItemChanged`): log `..\..\GoodsSocket.cpp,209,ENetError(52),%s [%d]`, error 52.
4. When the handler sets an error (`err` code ≠ 0): log `..\..\GoodsSocket.cpp,215,%d,%s` with `Server Socket Process Fail`, then `StsRouteReply(txn, err, 0)`.
5. Id → name function: `0x14011DFF0` (55 cases).

Handler pattern (example `AddCategory` `0x1400FB7A0`, `GetGoods` `0x1400FE140`):
1. Read request fields through the message vtable +208 (int read by name).
2. Check: negative id or code → error 10 `Invalid parameter`, line from `10-commands.tsv`.
3. Allocate `CGoodsAdminTxnData` (0x140 bytes, `0x14010D5F0`). Fail → error 48 `CGoodsAdminTxnData allocation failed`.
4. Build the DB request with a proc builder (`10-procs.tsv` column `builder`) and allocate `CGoodsAdminTxn` (0x30 bytes, ctor `0x1400C9DF0(obj, type, req+12, req+8, data)`).
5. `SendToProviderTxn(msg, txn, 25000, 0)`, `MsgFree`.
- `GetGoods` with `IsCacheMode` ≠ 0 uses the cache path (`GoodsCache.cpp`, `0x1400E5CE0`) ❓ (cache branch not decoded).
- Item kinds (admin page names, `0x1400EAF60`): charge, time, game, additional_service, coupon, grade, tag, ingame_coupon, license, role, gacha_charge, grade_score. Item DB kinds (`0x1400F30B0`): Charge, Game, Offline, AdditionalService, Subscription, Role, Coupon, Time.
- `CacheFlush` (40, `0x1400FFCC0`): gets the cache (`0x1400E5CE0`), calls `0x1400E09C0` → `0x1400E0BB0` (cache rebuild ❓), replies an empty `Reply` with no error.

### 1.4 Txn classes

| Class | Vtable | Slot +0x18 (OnSuccess) | Slot +0x20 (OnError) |
| --- | --- | --- | --- |
| `CGoodsAdminTxn` | `0x1401A2EB0` | `0x1400CD6C0`: `switch(type at +24)`, 52 cases; result builders below | `0x1400CD650` |
| `CGoodsTxn` | `0x1401B0A90` | `0x14010A9C0`: `switch(a1[6] − 11)` and sub-switch on `a1[7]` | `0x14010A5C0` (`Goods not found(GoodsId: %d)`, `GoodsTxn.cpp` 2293, 2301) |
| `CGoodsGameTxn` | `0x1401B0A58` | `0x14010A760` | `0x14010A380` |
| `CGoodsChangeTxn` | `0x1401A9560` | — | — |
| `CGoodsChangeNotifyTxn` | `0x1401A9598` | — | — |

`0x1400CD6C0` cases → builder: 1 `0x1400C6020`; 2 `0x1400C6940`; 3 `0x1400C62A0`; 4, 7, 10, 14, 35 `0x1400CA0B0`; 5 `0x1400CB740`; 6, 15, 16 `0x1400CB270`; 8 `0x1400CDE60`; 9 `0x1400CC1B0`; 11 `0x1400CCEB0`; 12 `0x1400CA7C0`; 13 `0x1400CC460`; 17 `0x1400C69F0`; 18 `0x1400C6EE0`; 19 `0x1400C8230`; 20 `0x1400C7240`; 21 `0x1400C7E30`; 22 `0x1400C76E0`; 32 `0x1400C8570`; 33 `0x1400C8B40`; 34 `0x1400C8770`; 37 `0x1400CA550`; 38 `0x1400CA1E0`; 48 `0x1400C6CE0`; 49 `0x1400C82F0`; 50 `0x1400C73E0`; 51 `0x1400C63D0`; 52 `0x1400C87B0`. Cases 23–31, 36, 39–47: inline. Error at `GoodsAdminTxn.cpp` line 464: `ENetError(48),Error ItemId: %d`. Note: the txn type numbers in the handlers (1…82) are not the same as these case numbers for every handler ❓ (check the ctor arg against `+24`).

Data classes (RTTI): `Srv2Goods::C{Category,Display,Item,Goods,GoodsCategory,GoodsAccountGrade,GoodsSalePricePolicy,GoodsBasicPrice,GoodsItem,GoodsItemBasicPrice,GoodsSaleLimit}Data` (`0x1401A5628`…`0x1401A56C8`). Cache classes: `CCacheGoods` `0x1401A7708`, `CCacheCategory` `0x1401A7738`, `CRevisionCategory` `0x1401A7718`, `CGameCategory` `0x1401A7758`, `CGameGoods` `0x1401A7778`, `CSearchGoods` `0x1401A7788`, `CClientSearchGoods` `0x1401A7798`, `CAppGroupCodeCategory` `0x1401A76D0`.

### 1.5 DB procs

- All 136 rows: `10-procs.tsv`. Logical DB `GoodsDb`; 3 procs use `SearchDbName` when set (§1.2).
- 23 builders select the proc name at run time (for example `0x1400F4300`: `ListItemByItemIdItemNameItemAppGroupCodeAdmin` when an item name is given and not empty, else `ListItemByItemIdItemAppGroupCodeAdmin`). The TSV lists every name of such a builder on the same call site.
- `GetMaxGoodsId` uses builder `0x1400F3270` (`GetGoodsMaxNoAdmin` / `GetItemBlankMinNoAdmin`). `GetMaxItemId` uses `0x1400F32F0` (`GetItemMaxNoAdmin` / `GetGoodsBlankMinNoAdmin`). So with `IsBlankId` the goods command reads the item blank id and the item command reads the goods blank id ❓ (possible `quirk_blank_id_swap`; check the branch in `0x1400F3270`).
- Timeout of all calls seen: 25000 ms.

### 1.6 Calls out

- `0x14011BF40(msg, cmd, gameCode, …)` sets protocol `GameWarehouse.%s` or `GameWarehouse.%s.%s` and the command name from map `0x1402860E0` (`Game`: 1 NotifyGoodsInfoChanged, 2 NotifyGoodsInventoryChanged, 3 DeliverIngameCoupon, 4 ActivateSlot, 5 DeactivateSlot, 6 GetBalance, 7 IngameGoodsChanged, 8 IngameItemChanged, 9 IngameCategoryChanged, 10 Test).
- Call sites: `0x1400C8CA0` cmd 9 (from admin results AddCategory `0x1400C6020`, ChangeCategory `0x1400C69F0`); `0x1400C8E30` cmd 8 (from `0x1400C4550`, `0x1400C53E0`); `0x1400E7040` cmd 9 (`SendToProvider`, no reply); `0x1400E7140` cmd 7 (2 sites); `0x1400E8060` cmd 8. Body element `Message`.
- `0x1400E73B0` (`GoodsChangeTxn.cpp`) reads the DB change rows (`0x1400E7600` category, `0x1400E8210` goods, `0x1400E7BE0` ❓) and then sends the notice.

### 1.7 Admin HTTP pages (`0x1400EAF60`)

80 strings. Pages `/goods/` ("GoodsSrv Test - Select") with one page per read command (`/goods/get_game_categories` … `/goods/get_goods_revision`), and `/goods/core` ("GoodsSrv Test - Core") with one page per write command and item kind (`/goods/add_category`, `/goods/add_item_<kind>`, `/goods/change_item_<kind>`, …). Full list not copied ❓.

### 1.8 State and timers

- `timers()`: 45 calls, all framework. No service timer found.
- Caches: `GoodsCache.cpp` objects (§1.4). Refresh rule (`DbIgnoreDuration`) not decoded ❓.

## 2. ShowcaseSrv (not done)

- Service map `0x140248DC0` (36 ids): 1 GetGoodsDisplayRevision, 2 ListCurrentGoodsByCategoryId, 3 GetUserSalePriceByGoodsId, 4 ListGoodsSalePrice, 5 GetMaxDisplayPolicyIdFromRange, 6 AddDisplayPolicy, 7 PageDisplayPolicy, 8 UpdateDisplayPolicy, 9 GetDisplayPolicy, 10 CheckNewDisplayPolicy, 11 AddGoodsToDisplayPolicy, 12 DeleteGoodsFromDisplayPolicy, 13 GetMaxSalePricePolicyIdFromRange, 14 AddSalePricePolicy, 15 PageSalePricePolicy, 16 UpdateSalePricePolicy, 17 GetSalePricePolicy, 18 CheckNewSalePricePolicy, 19 AddSaleCurrencyGroup, 20 AddQuantityForSalePricePolicy, 21 UpdateQuantityForSalePricePolicy, 22 DeleteQuantityForSalePricePolicy, 23 GetSalePricePolicyOfGoods, 24 AddGoodsToSalePricePolicy, 25 DeleteGoodsFromSalePricePolicy, 26 GetMaxLevelSaleLimitPolicyIdFromRange, 27 AddLevelSaleLimitPolicy, 28 PageLevelSaleLimitPolicy, 29 UpdateLevelSaleLimitPolicy, 30 GetLevelSaleLimitPolicy, 31 CheckNewLevelSaleLimitPolicy, 32 AddGoodsToLevelSaleLimitPolicy, 33 DeleteGoodsFromLevelSaleLimitPolicy, 34 AddQuantityForLevelSaleLimitPolicy, 35 UpdateQuantityForLevelSaleLimitPolicy, 36 DeleteQuantityForLevelSaleLimitPolicy.
- `table()` gives `0x1400E9A40` for most ids (a shared function, maybe the dispatch) ❓. Ids 1, 11, 24, 32: no handler found yet.
- Client maps in the binary (calls out ❓): `Goods` `0x140248D40` (55, same names as GoodsSrv), `Limit` `0x140248D80` (50: Check, Set, AddLimit, … GetUserPurchasedGoodsQuantity), `Grade` `0x140248D60` (27, same as GradeSrv, `09-member-grade-level.md`).

## 3. PromotionStampSrv (not done)

- Service map `0x140266620` (33 ids): 1 GetMaxPromotionId, 2 AddPromotion, 3 AddPromotionDisplay, 4 AddProbabilityPolicy, 5 AddRewardPolicy, 6 ChangePromotion, 7 SuspendPromotion, 8 ResumePromotion, 9 ChangePromotionDisplay, 10 ChangeProbabilityPolicy, 11 ChangeRewardPolicy, 12 RemovePromotionDisplay, 13 RemoveProbabilityPolicy, 14 RemoveRewardPolicy, 15 GetPromotion, 16 PagePromotions, 17 PageUserPromotionRewards, 18 PageUserPromotionAttendances, 19 ListCurrentPromotions, 20 GetPromotionPolicy, 21 GetUserPromotionStatus, 22 AttendPromotion, 23 AttendPromotionSecured, 24 RequestRewardDelivery, 25 UserCacheFlush, 26 DeliverRewardPolicy, 27 ListInvalidRewardDeliveries, 28 AddChanceSpot, 29 AddPromotionChance, 30 ChangePromotionChance, 31 RemoveChanceSpot, 32 RemovePromotionChance, 33 PolicyCacheFlush.
- `table()` found no handler for this map.
- Client maps (calls out ❓): `Grade` `0x140266600` (27), `VirtualCurrency` `0x140266660` (17: Deposit, Withdraw, GetBalance, …), `Delivery` `0x1402665C0` (11: Deliver, GetDelivery, …), `Ban` `0x140266580` (5: ListUserBans, ListContentBans, BanUser, UnbanUser, ListUserBansByAdmin), `Log` `0x1402665E0` (2: LogLines, LogEvent).

## Framework gaps

- Service INI notify objects use a section/key reader API: `0x1401209B0(cfg, section)`, `0x1401208D0(section, key, iter)`, `0x140120A80(key, iter)` (next value of a repeated key), `IniValueAsInt(key, index, default)`, `IniValueAsStr(key, index, default)` (GoodsSrv). `cfgkeys()` does not find these reads.
- `recipe.py` `table()` returns the handler as text `sub_XXXX`; `dbprocs()` args are `[writer, msg, db, proc, flag]` (no schema in GoodsSrv).

## Differences from existing specs

- None found yet.
