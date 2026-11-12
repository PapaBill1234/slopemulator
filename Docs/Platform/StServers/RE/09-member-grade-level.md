# 09 — Member library, GradeSrv, GradeMembersSrv, LevelSrv (RE reference)

Summary:
1. The "member library" (§0) is the newer framework build that GradeSrv, GradeMembersSrv, LevelSrv, GoodsSrv, ShowcaseSrv, PromotionStampSrv, AuctionSrv and ExAuthnSrv link. It adds the grid layer (`StsGrid`), the DB proc builder without a schema, heartbeat, direct connections and socket crypt. The framework fingerprints of `00-framework.md` do not match most of it (only 48–50 names apply).
2. GradeSrv (type 1063) keeps paid grades (subscriptions), grade scores and user levels in `GradeDb` (one DB per app group code when `Separated`). It answers the `Grade` protocol (27 ids, 20 with a handler). Two background jobs: the virtual currency job (`VirtualCurrency.Deposit`) and the expiration notice job (`Reward.NotifyUseComplete`). See §1.
3. GradeMembersSrv: see §2.
4. LevelSrv: see §3.
5. All three use the framework of `00-framework.md` plus §0. This chapter gives only the service code.

Sources: the `.i64` copies in `ida/lanes/st/` (build 381). All addresses are in the binary of the section. Error records are `{code, module, line}` (framework 3.1); code names are from `errcodes.tsv`. Long lists: `09-commands.tsv`, `09-procs.tsv`, `member-names.tsv`. ❓ = not verified in IDA.

## Quick find

| Question | Function | Binary, address | Section |
| --- | --- | --- | --- |
| Member library parts and names | `member-names.tsv` | GradeSrv | 0 |
| Reply to a request (grid build) | `StsRouteReply` (route manager slot +136) | GradeSrv `0x14015A5C0` | 0.2 |
| Send a request to a provider and wait | `SendToProviderTxn` | GradeSrv `0x140159E60` | 0.2 |
| Send a grid request (per-user object) | `GridTxnSend` | GradeSrv `0x1401370F0` | 0.3 |
| DB proc call (no schema) | `DbProcBegin` | GradeSrv `0x140110960` | 0.4 |
| Heartbeat keys | `HeartbeatReadCfg` | GradeSrv `0x140143E80` | 0.5 |
| GradeSrv start (type 1063, timers) | `GradeAppInit` | GradeSrv `0x1400C6B90` | 1.1 |
| GradeSrv `Grade.ini` keys | `GradeConfigLoad` | GradeSrv `0x1400E3DF0` | 1.2 |
| GradeSrv request parse (fields, required checks) | `GradeParseRequest` | GradeSrv `0x1400FBD10` | 1.3 |
| GradeSrv command switch | `GradeDispatch` | GradeSrv `0x1400F9BB0` | 1.4 |
| GradeSrv DB reply → handler again | `GradeTxnOnReply` | GradeSrv `0x1400FFD60` | 1.4 |
| GradeSrv base data (grades, levels) load | `GradeBaseDataLoad` | GradeSrv `0x1400E2CE0` | 1.5 |
| GradeSrv user level load | `GradeUserLevelLoad` | GradeSrv `0x1400EBBA0` | 1.5 |
| GradeSrv virtual currency job | `CVCJobTimer::OnTimer` | GradeSrv `0x1400FAFA0` | 1.7 |
| GradeSrv expiration notice job | `CExpJobTimer::OnTimer` | GradeSrv `0x1400FA650` | 1.7 |

## 0. Member library (shared code, `shared_with` ≥ 6)

### 0.1 What it is

`funcs/GradeSrv.tsv` has 363 functions with `shared_with` ≥ 6. They are not service code. They are a newer build of the framework (`Services\StsRoute`, `StsInetSocket`, `StsSocket`, `StsGrid`, `StsPort`, `StsHttpConn`, `StsHttpCmd`, `StsMain`, `LocalLog`, `Tls`, `Net\Srv2Route`, `ExeAddress`, `ExeThread`) that the eight "member" programs link. Most bodies changed a little (new fields, new slots), so the fingerprints of `framework-funcs.tsv` do not match. Address groups in GradeSrv (source file from the strings in the function):

| GradeSrv range | Count | Part | Note |
| --- | --- | --- | --- |
| `0x140128920`–`0x140129CF0` | 16 | `LocalLog.cpp`, XML writer | `0x140128EE0` log init (`Log`) |
| `0x140132A60`–`0x140138800` | 22 | `StsGrid.cpp` | grid objects, grid txn, signed grid data (`Signature`, `Data`, `Base64Data`) |
| `0x140139F10`–`0x140145150` | 102 | `StsHttpConn`, `StsInetSocket`, heartbeat, `StsMain` | 0.5 |
| `0x14014D120`–`0x14015A800` | 101 | `StsSocket`, `StsPort`, `StsRoute` | reply and send calls 0.2 |
| `0x14015C0A0`–`0x14015E180` | 16 | `StsSpawnConn`, counters (`/sts/counters` `0x14015D420`) | |
| `0x14018A2F0`–`0x140190290` | 37 | `StsDirectConn`, `SocketCrypt`, `Tls` | |
| other | 69 | small helpers | |

`member-names.tsv` lists the names that this chapter gives (hash, GradeSrv address, name, role). Later slices (`st-07`, `st-10`, `st-11`) find the same functions by hash.

### 0.2 Reply and send calls (route manager `qword_14028A418`)

| Call | GradeSrv | Slot | Difference from `00-framework.md` 3.3 |
| --- | --- | --- | --- |
| `StsRouteReply(txn, err, msg)` | `0x14015A5C0` | +136 | slot was +128 |
| `StsReplyErrorF(txn, err, fmt, …)` | `0x140159B80` | +152 | slot was +144 |
| `SendToProviderTxn(msg, txn, ms, flag)` | `0x140159E60` | provider hash `qword_14023D448` | same logic: find the provider by the protocol name of the message |
| `SendToProvider(msg, flag)` (no txn) | `0x14015A800` | same hash | used for one-way messages (`ExpirationNotiPing`, `Reward.NotifyUseComplete`) |
| `ErrInit(err, code)` | `0x14013AE00` | — | the record is 0x38 bytes: +48 = 256 (was +40) |

### 0.3 Grid layer (`StsGrid.cpp`)

- A grid object is one in-memory object per key (GradeSrv: one per user and app group code) that lives on one process of a federation. `0x140136A70(grid, 0)` starts the grid (strings `Route.ini`, `/grid/`, `Grid`).
- `GridObjectTimeoutSet(name, ms)` `0x140136C60`: finds the grid class by name in hash `qword_14023CD20` and sets the object life time (`0x1401369F0`). GradeSrv calls it with `"Grade"` and `[Cache] Timeout` × 1000.
- `GridTxnSend(msg, record, type, 0, ms, 0)` `0x1401370F0`: sends a DB request for a grid object; the reply comes back to the object's txn (GradeSrv: `GradeTxnOnReply` `0x1400FFD60`). All GradeSrv DB calls use 25000 ms.
- `StsGridBlockCfg` `0x1401347A0`: `NetCfgInt` `BlockMask` (default 7) and `AlwaysReparse` (default 0); log `StsGrid: federation mask change %u, was %u, %s`.
- `0x1401361E0`: limit check with log `grid objects allowed per one block (total %u block…` ❓ (limit value not decoded).
- `0x1401336F0` / `0x1401340F0`: grid data with `Signature`, `Type` (`none`), `Data`, `Base64Data` ❓ (signature check not decoded).

### 0.4 DB call layer (member form)

- `DbProcBegin(writer, msg, db, proc, flag)` `0x140110960`: same as `00-framework.md` §4 but **without** the schema argument (it writes `<Request><ProcName/><Params>`).
- Parameter writers: `ParamGuid` `0x140110A20`, `ParamStr(w, s, -1)` `0x140110A40`, `ParamInt` `0x140110A70`; end `ParamsEnd` `0x1401109F0`.
- The DB name is a text `GradeDb.<code>` (see 1.6).

### 0.5 Heartbeat and start

- `HeartbeatReadCfg` `0x140143E80` (`NetCfgInt`, section `unk_1402899A0`): `DisableShutdownTimeout` (0), `HeartbeatWaitTimeout` (4 s, stored × 1000), `HeartbeatWaitCount` (3), `HeartbeatStartupDelay` (2 s).
- Timer `0x1401440A0` (notify `off_14023D088`): after the startup delay it logs `Heartbeat startup delay timer(%u [ms]) expired - set port state on.`, calls the port object slot +48 and sets the port state to 1 (on). One shot (returns −1).
- `0x1401440F0` (notify `off_14023D030`): phase timer of `StsMain` (`MainInitializeApp`).
- `Srv2Route.cpp` `0x140116130`: `Forwards` / `Forward` list ❓ (not decoded).
- `StsDirectConn` `0x14018AAA0`: log `%s: UpdateServerList: %s is not defined at this sy…` ❓.
- `SocketCrypt` `0x140188EA0`: log `SocketCrypt [module(%d),line(%d),err(%d)]: %s conn…`, roles `server` / `client` ❓.
- Account key alias `RouteAddKeyAlias` `0x14015A1B0` (shared_with 2): GradeSrv adds key type `shanda` → `AssociationAcctDb.GetIdByShanda` (schema `v2017021701`) when `[EXAUTH] UseAccountForwardingForSnda` = true.

## 1. GradeSrv

### 1.1 Role and start

GradeSrv sells and keeps "grades" (paid membership grades, for example a premium period) for each user (UserId GUID) and app group code. A subscription (`UserSubscriptionId`) gives one or more user grades with an effective period. Grade scores (`AddGradeScore`) raise a user level inside a level group. Callers: other services and admin tools through the route (`Grade` protocol). The client does not call it directly ❓.

Start `GradeAppInit` `0x1400C6B90` (`CApplication` slot 0):
1. `StsSetServerType(1063, 0)`; program name `Grade`.
2. `StsConfigMonitorAdd(L"Grade.ini", config)` (config object from `0x1400C4C10`; getter `GradeCfg()` `0x1400E3DE0`).
3. Log `0x140128EE0(…, L"biz", 1, 0)` (business log) ❓.
4. `0x1400FD9B0`, `0x1400FE5C0` (counters ❓), grid start `0x140136A70(grid, 0)`.
5. If `[Cache] Timeout` ≠ 0: `GridObjectTimeoutSet("Grade", Timeout × 1000)`.
6. Counters page `0x14015D420`, admin pages `0x1400C5EA0` ❓.
7. If `UseAccountForwardingForSnda`: key alias `shanda` (0.5).
8. Timers: `CVCJobTimer` (`off_1402342A0`) at `StartVCJobAfter(sec)` × 1000 ms; ping timer (`off_140234318`) at `StartExpJobAfter(sec)` × 1000 ms; `CExpJobTimer` (`off_140234330`) at (`StartExpJobAfter(sec)` + 1) × 1000 ms.

Error module ids: parse 130; handlers one id per source file (110 CreateSubscription, 111 ActivateGrade, 112 PauseUserGrades, 113 ResumeUserGrades, 114 ListGrades, 115 AddGradeScore, 116 GetLevel, 117 DeleteSubscription, 118 CancelGradeScore, 119 PageValidUserGrades, 120 GetUserGrade, 123 GetCurrentGrade, 124 ListLevelChanges, 125 ListGradeScoreChanges, 127 ListVirtualCurrencyJobs, 128 RetryVirtualCurrencyJob, 129 ChangeGradeScore, 108 base data).

### 1.2 Config (`Grade.ini`, `GradeConfigLoad` `0x1400E3DF0`)

Boolean keys are text; the value is true only when it is `true` (case rule of `0x140103530` ❓).

| Section | Key | Default | Field | Use |
| --- | --- | --- | --- | --- |
| `DB` | `AppGroupCode` | — | +8 (32 wchar) | DB suffix when not `Separated` |
| `DB` | `AnyAppGroupCode` | — | +8 | overrides `AppGroupCode` when present |
| `DB` | `Separated` | 0 | +40 | ≠ 0: DB name = `GradeDb.<request AppGroupCode>` |
| `BM` | `UseLevels` | true | +44 | `false` → 0: level system off (error 5611 `ErrGradeUserLevelSystemDisabled`) |
| `BM` | `UseNotification` / `UseGradeChangedNotification` | false | +48 | send grade changed notices (the 2nd key overwrites the 1st) |
| `BM` | `GradeChangedNotificationAppGroupCodes` | — | +56, 16 bytes each, count +52 | app group codes for grade notices. `quirk_grade_notify_codes_overflow` (1.8) |
| `BM` | `UseChildGradeIdForGradeChangedNoti` | false | +568 | |
| `BM` | `UseScoreChangedNotification` | false | +572 | |
| `BM` | `ScoreChangedNotificationAppGroupCodes` | — | +580, count +576 | same overflow quirk |
| `BM` | `1MonthIsNot30Days` | false | +1092 | month unit = calendar month, else 30 days ❓ |
| `EXAUTH` | `UseAccountForwardingForSnda` | false | +1096 | key alias `shanda` (0.5) |
| `TEST` | `AllowCreateSubscription` | false | +1100 | test path of CreateSubscription ❓ |
| `TEST` | `PasswordForCreateSubscription` | — | +1104 (100 bytes) | value `keyhere`; use ❓ |
| `Cache` | `Timeout` | 86400 s | +1204 (ms) | grid object life time |
| `VCJOB` | `UseVCJob` | false | +1208 | run the virtual currency job |
| `VCJOB` | `SizePerJob` | 1000 | +1212 | rows per DB load |
| `VCJOB` | `StartVCJobAfter(sec)` | 60 | +1216 | first timer delay |
| `VCJOB` | `StartVCJobWhen(sec)` | 23400 | +1220 | second of the day when the job may start |
| `VCJOB` | `CheckingTime` | `06:00:00.000 +09:00` | +1224 | time part of the job date (`%04d-%02d-%02d %s` in `0x1400FE220`) |
| `VCJOB` | `PeriodPerVC(msec)` | 10 | +1264 | delay between two deposits |
| `VCJOB` | `AddDayForTest` | 0 | +1268 | test day offset |
| `VCJOB` | `JobInfo` (many) | — | +1276 / +1920 / +2048 (int arrays), count +1272 | value 0 = GradeId, 1 and 2 = currency id and amount ❓. A row is kept only when all 3 values ≠ 0. Max 31 rows (`quirk_grade_jobinfo_31`) |
| `ExpirationNoti` | `UseExpirationNoti` | false | +2176 | run the expiration job |
| `ExpirationNoti` | `StartExpJobAfter(sec)` | 60 | +2180 | first delay |
| `ExpirationNoti` | `PingPeriod(sec)` | 60 | +2184 | ping timer period |
| `ExpirationNoti` | `StartExpirationNotiFromDate` | `2014-11-01 00:00:00.000 +09:00` | +2188 | lower date limit |
| `ExpirationNoti` | `MaxTimeToLoad(min)` | 180 | +2228 | load window and wait |
| `ExpirationNoti` | `MaxSizeToLoad` | 1000 | +2232 | rows per load |
| `ExpirationNoti` | `DBAppGroupCodes` | — | +2240, 16 bytes each, count +2236 | DBs that the job reads; same overflow quirk |
| — | (load time) | — | +2752 | FILETIME of the load |

### 1.3 Request parse (`GradeParseRequest` `0x1400FBD10`)

1. The protocol must be `Grade` (log `Grade Parse,unknown protocol (%s)`, error `{10, 130, 55}` or `{10, 130, 60}`); the command must be in map `0x1402887D0` (`{10, 130, 67}`); a body must exist (`{10, 130, 77}`).
2. It reads all fields into one record (no per-command filter). Record offsets:

| Off | Field | Off | Field |
| --- | --- | --- | --- |
| +8 | `UserId` (GUID) | +104 | `Registered` (text) |
| +24 | `AppGroupCode` or `GameCode` (text) | +116 | `PageIndex` |
| +36 | `EffectiveDuration` | +120 | `PageSize` |
| +40 | `EffectiveDurationUnitType` (word) | +128 | `UserGradeScoreId` (int64) |
| +44 | `GradeId` | +136 | `DeductionStartType` |
| +48 | `GradeScore` | +140 | `IsRefundable` |
| +52 | `GradeScoreChangeReasonCode` (word) | +144 | `IsGifted` |
| +56 | `LevelGroupId` | +152 | `GameGradeKey` (text) |
| +60 | `RequestFromCode` (word) | +168/+176/+184 | `ResultSetType`, `TargetTimeFrom`, `TargetTimeTo` |
| +64 | `RequestKey` (text) | +192 | `ProvidingVirtualCurrencyJobId` |
| +80 | `UserSubscriptionId` (int64) | +200 | `AppId` (GUID) |
| +88 | `Brief` or `NoList` | +216 | `ContentSizeMax` |
| +92 | `LevelInfo` or `WithLevelInfo` | +96 | `AllGrades` |

3. `AllGrades` = 1 when `AppGroupCode` is set and `GameGradeKey` and `GradeId` are not set; an explicit `AllGrades` element overrides it.
4. `UserId` must be a non-zero GUID (`{10, 130, 163}` `Grade Parse,invalid userId`); for some ids the GUID comes from the name-ref header (`unknown name ref header`, code 116 ❓).
5. Required elements per command (error code 10 `ErrBadParam`, module 130, line):

| Id | Command | Required (line) |
| --- | --- | --- |
| 1 | CreateSubscription | `GradeId` or (`AppGroupCode` and `GameGradeKey`) (171), `RequestFromCode` (173), `RequestKey` (174), `EffectiveDurationUnitType` (175), `EffectiveDuration` (176) |
| 2 | ActivateGrade | `GradeId` or `AppGroupCode` (181) |
| 6 | ListGrades | `GradeId` or `AppGroupCode` (192) |
| 7 | AddGradeScore | `GradeId` or `LevelGroupId` or (`AppGroupCode` and `GameGradeKey`) (198), `GradeScore` (200), `RequestFromCode` (201), `RequestKey` (202) |
| 8 | GetLevel | `LevelGroupId` (207) |
| 9 | DeleteSubscription | `UserSubscriptionId` (212), `AppGroupCode` (213) |
| 10 | CancelGradeScore | `UserGradeScoreId` or (`RequestFromCode` and `RequestKey`) (218) |
| 11 | PageValidUserGrades | `AppGroupCode` (223); optional `OrderBy`, `InverseOrder` |
| 12 | GetUserGrade | `UserSubscriptionId` (237), `AppGroupCode` (236) |
| 13, 15 | ListGradeBaseData, ListPolicies | `AppGroupCode` (242) |
| 14 | ListLevelBaseData | `LevelGroupId` (246) |
| 16 | GetCurrentGrade | `AppGroupCode` (250) |
| 17 | ListLevelChanges | `AppGroupCode` (254), `LevelGroupId` (255) |
| 18 | ListGradeScoreChanges | `AppGroupCode` (259), `LevelGroupId` (260) |
| 19 | AddGradeScoreByAdmin | `AppGroupCode` (264), `LevelGroupId` (265), `GradeScoreChangeReasonCode` (266), `GradeScore` (267), `RequestKey` (268) |
| 25 | ChangeGradeScore | `GradeScore` (280), `LevelGroupId` (278) |
| other | — | no check |

### 1.4 Dispatch (`GradeDispatch` `0x1400F9BB0`)

1. `ErrInit`, parse into the global record `dword_140287430`. Parse failed: `StsReplyErrorF(txn, err, …)`.
2. Copy the request into a grid request record (`0x1400FDF00`, `0x140101FD0`), then `switch` on the id. Each case calls the handler with (grid object, record, 0) and then a per-command counter (`0x1400FDA20`–`0x1400FDB50`, 7-byte functions).
3. Ids 5, 13, 14, 15, 22, 23, 26 have no case: the record is freed (`0x1400FE050`) and **no reply** is sent (`quirk_grade_no_reply_ids`, 1.8).
4. Id 24 (`ExpirationNotiPing`) writes the current FILETIME to `off_140234330`+24 (the expiration job object) and sends no reply.
5. `GradeTxnOnReply` `0x1400FFD60` (txn vtable `0x1401AF280`): on a DB reply it checks the reply (`GradeTxn got reply, no body`, `GradeTxn got reply, no GridData`) and calls the same handler again with the stored record. So every handler is a state machine: the record +32 (`a2[8]`) is the state, `GradeSetState(record, n)` `0x1401021D0`.
6. `0x1400FA040` (`V1GradeHandler.cpp`): handler for a request on an object that is not on this process (OnUnmatched): it runs only the stateless commands (12 GetUserGrade, 17, 18, 20, 21, 24) and `Flush` replies `Result` `flush complete(OnUnmatched)`.

### 1.5 Shared handler steps

Most handlers start with the same two steps:
1. `GradeBaseDataLoad(record, txn)` `0x1400E2CE0` (`V1GradeBaseData.cpp`): if the static grade and level tables are not in memory, calls `ListGrades` (`0x140101030`) and `ListLevels` (`0x1401011A0`) and returns "pending". Errors module 108: 5605 `ErrGradeNoStaticGradeData` (224), 5649 `ErrGradeDbParentGradeDataError` (242, 249, 258), 5650 `ErrGradeDbError` (198, 204, 218, 280, 286, 300), 48 (310).
2. `GradeUserLevelLoad(grid, record, txn)` `0x1400EBBA0`: loads the user state for the app group code: `GetLastBatchJobCompleted`, `ListUserGradesByUserIdAppGroupCodeEffectiveTo`, `ListUserGradeScoreChangesByUserIdAppGroupCodeIsUserLevelApplied` (unapplied score changes), `GetLastUserLevelByUserIdAppGroupCodeLevelGroupId`; then it applies the open score changes to the level: `AddUserLevelCore` or `UpdateUserLevelCore`, `UpdateUserGradeScoreChangeCore(id, 1)` (applied), `UpdateUserGradesByUserIdAppGroupCodeLevelGroupIdEffectiveTo` (grades that a level gives). It loops with state 1 until the grid object +32 (loaded flag) ≠ 0.
3. Level row (grid +280, 56 bytes each, count +292): +16 LevelGroupId, +32 LevelId, +36 AccumulateGradeScore, +40 MinimumLevelScore, +44 MaximumLevelScore, +48 MaximumGradeScore. A reply gives `AccumulateGradeScore` = min(score, MaximumGradeScore) (`0x1400F5390`).

### 1.6 Commands

`09-commands.tsv` has every id with handler, request, reply and errors. Step notes:

| Id | Command | Handler | Steps |
| --- | --- | --- | --- |
| 1 | CreateSubscription | `0x1400F13E0` | 1.5; `GetUserSubscriptionByRequestKeyRequestFromCode` (state 32; a match → reply the old `UserSubscriptionId` in `<extra>`, duplicate guard); `AddUserSubscriptionCore` (status 4 or 1); `AddUserGrade` per grade (current, next); `AddUserGradeScoreChangeCore` (state 7) when the grade gives a score; level update (states 21, 22). Reply `UserSubscriptionId`, `Grade` list (`Activation`, `GradeCount`, `TotalEffectiveFrom/To`, `RemainingGradeTimePerSecond`, `ActivatedWithAdding`, `Added`), `RelatedGrades`, `LevelChanged`, level fields |
| 2 | ActivateGrade | `0x1400ED3E0` | 1.5; `UpdateUserGrade` (activate the next ready grade). Reply `Grade` (`EffectiveFrom`, `EffectiveTo`, `Activation`, `Reason`, `Activated`) |
| 3 | PauseUserGrades | `0x1400F8960` | `AppId`/`AppGroupCode` check (5616 `ErrGradeInvalidAppId` line 78, 5617 `ErrGradeInvalidAppGroupCode` 88); `UpdateUserSubscriptionsStatusCore`, `SplitUserGrades` (cut the running grade at now), `UpdateUserGrade` |
| 4 | ResumeUserGrades | `0x1400F92B0` | same checks (79, 89); 5615 `ErrGradeWrongDataFound` (167); `UpdateUserGrade` |
| 6 | ListGrades | `0x1400F6170` | 1.5; reply `UserId`, `AppGroupCode`, `Activation`, `GradeListCount`, `LevelId`, `AccumulateGradeScore`, `TotalEffectiveFrom/To`, `GradeList`/`Grade` |
| 7 | AddGradeScore | `0x1400EDDE0` | 1.5; level off → 5611 (47); `GetUserGradeScoreChangeByRequestKeyRequestFromCode` (duplicate guard); `AddUserGradeScoreChangeCore`; level update. Reply `UserGradeScoreId`, `UserLevelId`, `LevelId`, `AccumulateGradeScore`, `RelatedGrades`, `LevelChanged`, min/max scores |
| 8 | GetLevel | `0x1400F5390` | 1.5 (state 14 / 15); level group not in the base data → `GetLastUserLevelByUserIdAppGroupCodeLevelGroupId`; reply `UserId`, `AppGroupCode`, `LevelId`, `AccumulateGradeScore`, `MinimumLevelScore`, `MaximumLevelScore`, `MaximumGradeScore`. Bad state → `{48, 116, 133}`; no record memory → `{48, 116, 74}` |
| 9 | DeleteSubscription | `0x1400F2E00` | `GetUserGradeSubscription`, `UpdateUserGradesByCheckingExpirationNotified`, `AddUserGradeScoreChangeCore` (remove the score), `UpdateUserGrade`; sends `Reward.NotifyUseComplete` (1.9). Errors 5650 (311), 48 (177, 368, 388, 520) |
| 10 | CancelGradeScore | `0x1400EF0E0` | `GetUserGradeScoreChangeCore` or `…ByRequestKeyRequestFromCode`; `AddUserGradeScoreChangeCore` (negative); 5650 (279), 10 (62), 48 (308, 322) |
| 11 | PageValidUserGrades | `0x1400F7A60` | `GetUserSubscriptionCore`; reply `Grades` (`Grade` rows), `TotalCount`. Errors 10 (224), 48 (334) |
| 12 | GetUserGrade | `0x1400F58C0` | `GetUserGradeSubscription` (`0x140100B60`); reply grade row (`SubscriptionStatus`, `UserGradeStatus` = `Stopped`/`Collected`/`Exhausted`/`Using`/`NotUsed`/`Ready`). 5650 (100, 107, 147), 48 (152) |
| 16 | GetCurrentGrade | `0x1400F4CC0` | 1.5; reply `GradeCount`, `Grades` (`GradeId`, `GradeName`, `LevelGroupId`, `LevelGroupName`, `LevelId`, `AccumulateGradeScore`, `Activation`, `EffectiveGradeCount`, `EffectiveFrom/To`, `EffectiveDurationUnitType`, `EffectiveDuration`). 48 (195) |
| 17 | ListLevelChanges | `0x1400F7770` | `ListUserLevelsByUserIdAppGroupCodeLevelGroupIdRegisteredAdmin`; reply `UserLevel` rows (`LevelId`, `AccumulateGradeScore`, `Registered`, `RegisteredDate`). 48 (79) |
| 18 | ListGradeScoreChanges | `0x1400F7320` | `ListUserGradeScoreChangesByUserIdAppGroupCodeRegisteredAdmin`; reply `UserGradeScoreChange` rows. 48 (85) |
| 19 | AddGradeScoreByAdmin | `0x1400EF0D0` | sets record +112 = 2 (admin source ❓), then AddGradeScore |
| 20 | ListVirtualCurrencyJobs | `0x1400F79D0` | after 1.5 always error: `{51 ErrNotImplemented, 127, line ❓}` or `{48, 127, 56}` when grid +32 ≠ 0 |
| 21 | RetryVirtualCurrencyJob | `0x1400F9A90` | same form, module 128 |
| 24 | ExpirationNotiPing | `0x1400FA630` | 1.4 step 4 |
| 25 | ChangeGradeScore | `0x1400F0640` | 1.5; 5611 (47); level update. Errors 48 (62, 110, 143) |
| 27 | Flush | `0x1400ECC70` | destroy the grid object; reply `Result` `flush complete(destroy grid)` |

### 1.7 Background jobs and timers

| Timer | Notify | OnTimer | Period | Work |
| --- | --- | --- | --- | --- |
| VC job | `off_1402342A0` | `CVCJobTimer::OnTimer` `0x1400FAFA0` | 60000 when off; wait to `StartVCJobWhen`; 100 between steps; 86400000 or `86410 − now` s when the day is done; 1000000000 when stopped | for each `JobInfo` row: `ListUserGradesByAddingProvidingVirtualCurrencyJob(date, GradeId, SizePerJob)` (`0x1400FE220`), then one `VirtualCurrency.Deposit` per user grade (`0x1401172C0`: `CurrencyId`, `Amount`, `RequestId`, `RequestCode`, `ClientNetAddress`, `AdditionalInfo`, `DepositReasonCode`, `IsRefundable`, `EffectiveFrom`), then `UpdateProvidingVirtualCurrencyJobsCore` (`0x1400FE420`). Logs `Over count!!!`, `Over time!!!`, `All Failed!!!` with success and fail counters |
| Ping | `off_140234318` | `0x1400FAF10` | `PingPeriod(sec)` × 1000 | when `UseExpirationNoti`: send `Grade.ExpirationNotiPing` (one-way, `0x140112110`). The route sends it to one GradeSrv process; that process becomes the job owner |
| Exp job | `off_140234330` | `CExpJobTimer::OnTimer` `0x1400FA650` | 100 while working; `PingPeriod` × 1000; `MaxTimeToLoad` × 60000; 3600000; 1000000000 when off | runs only on the process that got the last ping (else `Deselected while doing job`). For each `DBAppGroupCodes` entry: `ListUserGradesByEffectiveToExpirationNotified(code, from, now + MaxTimeToLoad)`; per row `GetUserGradeSubscription`, then `Reward.NotifyUseComplete` and `UpdateUserGradesByCheckingExpirationNotified` (txn `CGradeExpJobTxn` `0x1400FF380`, error path `0x1400FEDB0`) |

### 1.8 Quirks

| Name | Address | What the code does | Value | Reach | Port |
| --- | --- | --- | --- | --- | --- |
| `quirk_grade_no_reply_ids` | `0x1400F9BB0` | ids 5, 13, 14, 15, 22, 23, 26 parse OK but have no case | no reply; the caller gets its own txn timeout | any caller of these ids | fixed: reply `{52 ErrBadCommand}` |
| `quirk_grade_notify_codes_overflow` | `0x1400E3DF0` lines 59, 76, 150 | the `…AppGroupCodes` loops have no count limit | entries after 32 write over the next config fields | `Grade.ini` only | fixed: stop at 32 |
| `quirk_grade_jobinfo_31` | `0x1400E3DF0` | `JobInfo` loop breaks when the index is ≥ 31 before the count grows | max 31 jobs; the 32nd valid row is read and dropped | `Grade.ini` | keep value (max 31) |
| `quirk_grade_vc_not_implemented` | `0x1400F79D0`, `0x1400F9A90` | ids 20, 21 always reply an error | `ErrNotImplemented` 51 | admin tools | keep value |

### 1.9 Calls out

| Target | Command | Builder | When |
| --- | --- | --- | --- |
| GradeDb (`Db.GradeDb.<code>`) | procs of `09-procs.tsv` | 0.4 | every handler and job |
| `Grade` (self) | `ExpirationNotiPing` | `0x140112110` | ping timer |
| `Reward` | `NotifyUseComplete` (`UserId`, `RequestFromCode`, `RequestKey`, `DeliveryRequestFromCode`, `DeliveryRequestKey`, `CompletedReason`, `CompletedQuantity`) | `0x140113B60` | DeleteSubscription, exp job |
| `VirtualCurrency` | `Deposit` | `0x1401172C0` | VC job |
| `AssociationAcctDb` | `GetIdByShanda` | key alias | `shanda` key requests |

The binary also has client builders for `GradeMembers` (Abandon, Activate, GetItem, Get, GetPolicy, List, Page, Register, Resume, Reward, Suspend: `0x140112670`–`0x1401131E0`), `Level` (AddScore, GetLevel, ListLevels, SubstractScore: `0x1401132B0`–`0x1401137C0`) and `Presence.Forward` (`0x140113960`). Their callers (`0x1400D12F0` tree, `0x1400DB110`…) are not reached from `GradeDispatch` ❓ (no path found; old BM code).

### 1.10 Worked example (CreateSubscription)

Request `Grade/CreateSubscription`: `UserId` = `{11111111-…}`, `AppGroupCode` = `BNS`, `GameGradeKey` = `premium30`, `RequestFromCode` = 3, `RequestKey` = `order-1001`, `EffectiveDurationUnitType` = 1 (day ❓), `EffectiveDuration` = 30. Parse fills the record (1.3). The handler loads base data and the user state, checks `GetUserSubscriptionByRequestKeyRequestFromCode(3, "order-1001")` (no row), adds the subscription and the user grade (30 days from the end of the last grade), and replies `<Reply><UserSubscriptionId>…</UserSubscriptionId><Grade>…<Activation>…<TotalEffectiveTo>…</Grade></Reply>`. The same request again finds the row and replies the old `UserSubscriptionId` in `<extra>` (idempotent).

## 2. GradeMembersSrv

### 2.1 Role and start

GradeMembersSrv keeps "memberships" (`MembershipKey`, `MembershipItemId`) for each user and app group code in `GradeMembersDb`. A membership has a state (`MembershipStatus`: activate, suspend, resume, abandon, exhaust) and an effective period. It gives daily and consecutive rewards (Level score or virtual currency) and sends end-of-membership alerts. Callers: other services through the route (`GradeMembers` protocol). The grid sends the grid commands to the process that owns the user object.

Start `0x1400EA580` (`CApplication` slot 0):
1. `StsSetServerType(1124, 0)`; program name `GradeMembers`.
2. `StsConfigMonitorAdd(L"GradeMembers.ini", 0x14027EAE0)` (class `CGmConfig`, reader `0x1400C9320`) and `StsConfigMonitorAdd(L"Grade.ini", 0x14011A1B0())` (second config object, `BillingPolicy.cpp` ❓).
3. Txn class registration: `0x1400CCA10` (`CLoadMembershipGridTxn`, `CLoadConsecutiveMembershipTxn`, `CLoadDailyRewardsTxn`), `0x1400DA090` (`CRegisterTxn`, `CPageTxn`), `0x1400F3C90` (`CDailyRewardTxn`, `CConsecutiveRewardTxn`, `CRetrieveRewardTxn`).
4. Business log `0x140128490(…, L"biz", 1, 0)`; grid start `0x140135DF0(grid, 0)` with name `GradeMembers`; admin pages `0x1400CA7D0` (`/GradeMembers/`, `/GradeMembers/command_console`, `Web/GradeMembersSrv/index.html`).
5. Timers `0x1400E7FE0`: DB time timer (`off_14022D080`) at 0 ms; loader timer (`off_14022D040`) at `LoadBatchTimerStartSleepSec` × 1000 ms.
6. Retriable txn queue `0x14012F4D0` (`RetriableTxn.cpp`) slot +16 gets (`QueuedTxnConcurrentMax`, `QueuedTxnIntervalMs`).

Error module ids: 2 parse, 7 stateless handler (`0x1400F1630`), 8 reward txn (`GmRewardTxn.cpp`), 9 grid handlers (`GmGrid.cpp`). Error codes (`errcodes.tsv`): 11001 `ErrGradeMembersChangeStateFailed`, 11002 `ErrGradeMembersNoMembership`, 11003 `ErrGradeMembersNotFoundMembershipItemId`, 11004 `ErrGradeMembersDatabaseFlushing`. The error record helper in this binary is `0x1400CB8F0(err, code, module, line)`.

### 2.2 Config (`GradeMembers.ini`, `CGmConfig` reader `0x1400C9320`)

Offsets are from the config object `0x14027EAE0`. The getter view (`0x1400C8CB0` = object +8, vtable `0x1401A8770`) gives the values in ms when the key is in seconds. The match of slot and field was checked for all 11 getter slots (second keys × 1000, ms keys raw).

| Section | Key | Default | Field | Getter slot | Use |
| --- | --- | --- | --- | --- | --- |
| `NotifyBeforeCompleted` | `BillPushSec` | — | list +24, type 2 | — | up to 20 values (index < 0x14); −1 = no entry. Seconds before the end for `BillingPush.GradeLeft` ❓ |
| `NotifyBeforeCompleted` | `ReaminTimeAlertSec` (spelling of the binary) | — | list +24, type 3 | — | same form; remain-time alert ❓ |
| `ConsecutiveRewards` | `<name> = type, v1, v2, v3, v4` | — | hash +40 (row +56 type, +60 word v1, +64 v2, +72, +80) | — | type text `Level` → 2, `VC` → 1, else 0. Then section `ConsecutiveReward.<name>` (`0x1400C8FD0`) ❓ |
| `DailyRewards` | `<name> = s0, s1, s2, n3 … n7` | — | hash +96 (row +56 mode, +60 type, +64 list, +88 word, +92…+104) | — | mode text `OnDemand` / `Daily` (+56); type `Level` → 2, `VC` → 1 (+60). Then section `DailyReward.<name>` ❓ |
| `Timer` | `MembershipTickTimeoutSec` | 1 | +152 | +16 (× 1000) | membership tick ❓ |
| `Timer` | `BatchLoadingIntervalSec` | 600 | +156 | +24 (× 1000) | loader window |
| `Timer` | `LoadTimerIntervalSec` | 60 | +160 | +40 (× 1000) | loader period |
| `Timer` | `LoadBatchTimerStartSleepSec` | 60 | +164 | +48 (× 1000) | loader first delay |
| `Timer` | `DbTransTimeoutSec` | 60 | +168 | +32 (value + 5) | DB txn timeout ❓ |
| `Timer` | `QueuedTxnIntervalMs` | 100 | +172 | +56 | retriable queue |
| `Timer` | `QueuedTxnConcurrentMax` | 10000 | +176 | +64 | retriable queue |
| `Timer` | `DbTimeSyncTimeoutMaxMs` | 10 | +180 | +72 | DB time sync |
| `Timer` | `DbTimeSyncIntervalSec` | 600 | +184 | +80 (× 1000) | DB time timer period ❓ |
| `Timer` | `UseDbTimeSync` | false | +192 | — | text `true` |
| `Log` | `BusinessLogEnable` | false | +188 | +88 | text `true` |
| `Mail` | `Recipient`, `MailIdleTimeSec`, `ExcludeFilter` | — | +200, +224, list | — | mail report |

### 2.3 Parse and dispatch

- Front dispatch `0x1400EAD20`: `0x1400EEE90` parses (strings `Parse`, `invalid command`, `xml body missing`, `unknown name ref header`); `0x1400EC1E0` reads the fields per command and makes the grid key (`0x14015CF50`). Errors `{10 ErrBadParam, 2, 322}` (`invalid params(%s)`), `{51 ErrNotImplemented, 2, 317}` (`invalid command`).
- Map `0x1402803B0` (18 ids). Switch on the id:

| Ids | Path |
| --- | --- |
| 1, 4, 14, 18 | stateless `0x1400F1630`: switch on record +36: 1 → `0x1400E7EA0`, 4 → `0x1400F2C40`, 14 → `0x1400F07B0`, 18 → `0x1400F1BB0`; other → `{51, 7, 337}` |
| 2, 3, 5–13, 15, 17 | grid: `0x1400CE920` sends `GradeMembersDb.ListAvailableMemberships` as a grid txn (`0x140136430`, 25000 ms) that loads the user object; the request continues in the grid object |
| 16 Flush | replies an empty `<Reply/>` at once (`0x1401591F0`) |

- Grid request entry `0x1400EA6B0` (log `grid data will be lost`) calls the grid switch `0x1400E2510`. Ids without a case there (1, 14, 18) → `{51, 9, 1006}`.

### 2.4 Commands

`09-commands.tsv` has every id. Steps:

| Id | Command | Handler | Steps |
| --- | --- | --- | --- |
| 1 | StartBatch | `0x1400E7EA0` | replies `Reply`; no DB call found ❓ |
| 2 | LoadUser | `0x1400E2460` | grid load (2.3), then empty `Reply` |
| 3 | Alert | `0x1400DD7D0` → `0x1400DE350` | finds the active membership (`no active membership` → `{11003, 9, 726}`); exhausts or closes it (`exhaust: %s, %s`, `close: %s, %s`; fail → `{11001, 9, 732/738}`); backlog `UpdateMembershipCore` (OpCode `Exhaust`) |
| 4 | GetItem | `0x1400F2C40` | stateless: `GetMembershipByRequestKeyRequestFromCode` or `GetMembershipByMembershipItemId`; no key → `{10, 7, 206}` `nothing params`; `{48, 7, 213}` |
| 5 | Get | `0x1400E0BE0` | `ListConsecutiveRewardsEffectiveDays` (`AppGroupCode`, `MembershipKeyList`, `PurchaseOnly`), `ListMemberDailyRewardsAvailableDays` (`StartFrom`, `EndTo`), `Level.GetLevel` (`0x1400CE240`); reply `MembershipKey`, `RemainTimeSec`, `MembershipItems`, `Count`, `MembershipStatus`, `EffectiveFrom`, `EffectiveTo` (`0x1400D9780`) |
| 6 | List | `0x1400E1850` | same loads; reply `MembershipCount`, `Memberships`/`Membership` |
| 7 | Page | `0x1400E7620` | `ListMembershipsByAppGroupCodeMembershipKey` or `ListMembershipsByAppGroupCode` (txn `CPageTxn`) |
| 8 | Activate | `0x1400DBCC0` | no membership → `{11002, 9, 585}`; `%s already activated` / `%s already suspended` / fail → `{11001, 9, 591/597/603}`; backlog `UpdateMembershipCore` (OpCode `Activate`) |
| 9 | Register | `0x1400E7870` | `GetMembershipByRequestKeyRequestFromCode` (duplicate guard); `AddMembership` (`0x1400E4FA0`: `MembershipItemId`, `MembershipStatus`, `LeftDurationSec`, `Registered`; txn `CRegisterTxn`, reply `0x1400E6C00`). Errors `{48, 9, 280}`, `{4, 9, 213}` ❓, `{51, 9, 227}` |
| 10 | Abandon | `0x1400DAB70` | `%s not found` → `{11003, 9, 629}`; fail → `{11001, 9, 641}`; backlog OpCode `Abandon` |
| 11 | Suspend | `0x1400E35A0` | a second call logs `DEBUG suspend again`; backlog OpCode `Suspend` |
| 12 | Resume | `0x1400E2BA0` | `DEBUG resume again, %s`; backlog OpCode `Resume` |
| 13 | Reward | `0x1400F3D10` | reward txn (2.5); `{51, 8, 1131}` for a bad reward type |
| 14 | GetPolicy | `0x1400F07B0` | stateless; replies the config rewards: `DailyReward`, `Membership` (`MembershipKey`), `Levels`/`Level` (`LevelId`), `Rewards` (`RewardType`, `TargetType`, `TargetKey`, `EffectiveDays`) |
| 15 | RetrieveReward | `0x1400F3D10` | same txn, retrieve form (`CRetrieveRewardTxn`) |
| 16 | Flush | `0x1400DFEC0` (grid) | DB queue not empty → `{11004, 9, 916}` `database flush not completed`; the front path replies at once (2.3) |
| 17 | GetGridData | `0x1400E1780` | replies the grid object data |
| 18 | SetVirtualTime | `0x1400F1BB0` | sets the test clock from `VirtualTime` ❓ |

### 2.5 Reward txn (`GmRewardTxn.cpp`)

- Consecutive reward: `AddAndGetMemberRewardRequestsByRewardTypeMemberId` (`0x1400FE110`: `RewardType`, `TargetType`, `TargetKey`, `Rewards`, `RewardDescription`, `Registered`; `{48, 8, 326}`).
- Daily reward: `AddAndGetMemberRewardRequestsByRewardTypeLastRewarded` (`0x1400FF4D0`).
- Retrieve: `GetMemberRewardRequestsByUserIdAppGroupCodeGameGradeKeyRewardTypeMemberId`, then `UpdateMemberRewardRequestsCore` (`0x1401008F0`, state `Retrieved`).
- Delivery per reward row: type Level → `Level.AddScore` (`0x1400F7220` → builder `0x140114A40`; result `ScoreChangeId`); type VC → `VirtualCurrency.Deposit` (`0x1400F8190` → `0x140118650`; result `DepositId`); retrieve → `Level.SubstractScore` (`0x1400F9D20` → `0x140114CF0`). After each result: `UpdateMemberRewardRequestsCore` (`0x1400F48E0`, `0x1400F5660`, `0x1400F6470`, `0x1400FAAD0`, `0x1400FB280`, `0x1400FBA60`). The request rows make the reward idempotent (one row per `RewardType` and member id, or per last rewarded day).

### 2.6 Backlog (`GmBacklog.cpp`)

Retriable work items (`RetriableTxn`, log `Retrial Error`, queue limits in 2.2):

| Item | Function | Sends |
| --- | --- | --- |
| state change | `0x1400C4B00` | `GradeMembersDb.UpdateMembershipCore` with `OpCode` = `Activate`, `Suspend` (4), `Resume` (5), `Abandon` (6), `Exhaust` |
| reward | `0x1400C58A0` | `GradeMembers.Reward` to self (`0x140114710`: `AppGroupCode`, `RewardType`, `MembershipKey`, `MembershipItemId`) |
| time left | `0x1400C6150` | `BillingPush.GradeLeft` (`0x140113DB0`: `AppGroupCode`, `GameGradeKey`, `LeftDurationSec`, `EstimatedEndDate`) |
| use complete | `0x1400C63E0` | `Reward.NotifyUseComplete` (`0x140114EF0`) |
| grade change | `0x1400C65B0` | `Grade.NotifyGradeChanged` message (`0x140114260`: `UserId`, `GradeGroupCode`, `GameGradeKey`, `IsActivated`, `GradeCount`, `TotalEffectiveFrom/To`, `RemainingGradeTimePerSecond`, `Reason`) |

The state change item is reached from handlers 3, 8, 10, 11, 12 (caller walk). The other items are reached only through txn vtables ❓ (which handler queues them was not traced).

### 2.7 Timers

| Timer | Notify | OnTimer | Period | Work |
| --- | --- | --- | --- | --- |
| DB time | `off_14022D080` | `0x1400E8B60` | first 0 ms; then ❓ (`DbTimeSyncIntervalSec`) | `GradeMembersDb.GetDbTime` (`GmLoader.cpp`) |
| Loader | `off_14022D040` | `0x1400E8920` | first `LoadBatchTimerStartSleepSec` × 1000; then `LoadTimerIntervalSec` × 1000 (getter +40) | when the retriable queue is free: `ListEstimatingUsersByEndFromTo` or `…ByEndTo` (`0x1400E7CE0`; `EnedFrom` = now − 60 s, `EnedTo` from `BatchLoadingIntervalSec`), then sends a `You-are-Loader` request (`0x1400EC0D0`, `0x140159430`) so that the grid loads those users ❓ |
| Membership event | — | `0x140101A30` (`MembershipEvent`), `0x140101230` (`GmTimer.cpp`) | per membership (`MembershipTickTimeoutSec`) ❓ | end and alert events of one membership ❓ |

### 2.8 Calls out

| Target | Command | Builder | When |
| --- | --- | --- | --- |
| `GradeMembersDb` | procs of `09-procs.tsv` | `0x140113ED0` (DbProcBegin, member form) | handlers, loader, backlog |
| `Level` | `GetLevel`, `AddScore`, `SubstractScore` | `0x140114BD0`, `0x140114A40`, `0x140114CF0` | Get/List, reward, retrieve |
| `VirtualCurrency` | `Deposit` | `0x140118650` | VC reward |
| `GradeMembers` (self) | `Reward` | `0x140114710` | backlog reward item |
| `BillingPush` | `GradeLeft` | `0x140113DB0` | time-left notice |
| `Reward` | `NotifyUseComplete` | `0x140114EF0` | backlog |
| `Grade` | `NotifyGradeChanged` (message) | `0x140114260` | backlog |

### 2.9 Worked example (Register)

Request `GradeMembers/Register`: `UserId` = `{22222222-…}`, `AppGroupCode` = `BNS`, `MembershipKey` = `vip`, `RequestFromCode` = 3, `RequestKey` = `order-2001` ❓ (field names from the procs). The front dispatch sends `ListAvailableMemberships` to load the user object. The grid handler `0x1400E7870` calls `GetMembershipByRequestKeyRequestFromCode(3, "order-2001")`. No row → `AddMembership` writes the new `MembershipItemId` with `MembershipStatus` and `LeftDurationSec`; the reply has `MembershipItemId`. The same request again finds the row and replies the old `MembershipItemId`.

### 2.10 Quirks

| Name | Address | What the code does | Value | Reach | Port |
| --- | --- | --- | --- | --- | --- |
| `quirk_gm_grid_no_case` | `0x1400E2510` | ids 1, 14, 18 have no case in the grid switch; only the front path handles them | `{51, 9, 1006}` if they come through the grid entry | grid forward only | keep value |
| `quirk_gm_reamin_key` | `0x1400C9320` | the key name is `ReaminTimeAlertSec` (spelling) | the ini must use this spelling | `GradeMembers.ini` | keep value |

## 3. LevelSrv (in progress)

### 3.1 Start (`0x1400DD8A0`)

1. `StsSetServerType(1098, 0)`; program name `Level`.
2. `StsConfigMonitorAdd(L"Level.ini", 0x1400CB780())`. The object is `CLevelConfig` (`0x140225260`, vtable `0x1401A2B20`, reader slot 0 `0x1400CB7C0`). Getter view `0x1400CB770` = object +8 (vtable `0x1401A2B40`).
3. `0x1400CD9B0`, `0x1400EA920` (txn class registration ❓); grid start `0x14012EA70(grid, 0)`; `0x1400CC060` (admin pages ❓); `0x140154C90(0)`; `0x1400D8F40` starts the batch timer (3.7).

### 3.1a Config (`Level.ini`)

`CLevelConfig` reader `0x1400CB7C0`. Offsets are from the object. Getter slot = offset in the getter vtable `0x1401A2B40` (view = object +8).

| Section | Key | Default | Field | Getter slot | Use |
| --- | --- | --- | --- | --- | --- |
| `Mail` | `Recipient`, `MailIdleTimeSec`, `ExcludeFilter` | — | +16 … | +8 (obj +40) ❓ | mail report |
| `Timer` | `QueuedTxnIntervalMs` | 1000 | +44 | +24 | retriable queue `0x140127F90` slot +24 (arg 2) |
| `Timer` | `QueuedTxnConcurrentMax` | 10000 | +48 | +32 | retriable queue (arg 1) |
| `Timer` | `BatchJobTimerIntervalSec` | 30 | +52 | +40 (× 1000) | batch timer period |
| `Timer` | `BatchTimerStartSleepSec` | 30 | +56 | +48 (× 1000) | batch timer first delay (`0x1400D8F40`) |
| `Timer` | `BatchMonthlyScoreRawDataMax` | 1000 | +60 | +56 | `top` of `ListMonthlyUserScoreRawData` (`0x1400DA9A0`) |
| `Timer` | `BatchJobProcessingMax` | 64 | +64 | +64 | batch job limit ❓ |
| `Timer` | `UseVirtualTime` | false | +68 | +16 | text `true`; test clock (SetVirtualTime) |

After the keys, the reader calls the level policy reader (`0x1400E6A10()` object, slot 0) with the same ini, then sets the retriable queue (slot +24, args `QueuedTxnConcurrentMax`, `QueuedTxnIntervalMs`).

Level policy (`CLevelPolicyManager`, object `0x140276D30`, second vtable `0x1401A3FB0`, reader `0x1400E7BA0`; built in `0x14019B870`):

| Section | Content |
| --- | --- |
| `Policy` | `Version` (default 1) → `PolicyVersion` in replies |
| `Scores` | one entry per score: value 0 = name text, value 1 = int (+156) ❓ meaning; stored in hash at policy +168 (`0x1400E5620`, `0x1400E6200`) |
| `PlusScore` | entries: v0 = word key (`0x1400E53D0`), v1 (+68), v2 (+100), v3 text, v4 text, v5 int ❓ meaning |
| `LevelApps` | entries: v0 = app text; list at +112 (`0x1400E60F0`). Gives the notify mask per app ❓ (3.8) |
| `Level.<group>` | `0x1400E6CC0`: v0 text, v1 word (level id), v2 int (default 100), v3 int (default 9), v4 int, v5 int; then section `Level.<group>.<level>` ❓ |

### 3.1b Errors

Error helper `0x1400CCD50(err, code, module, line, 0)`. Modules: 1 front dispatch, 2 parse, 3 grid load (`LvGrid`), 4 txn/handlers (`LvTxn.cpp`, `LvJob.cpp`). Service codes: 8701 `ErrLevelPolicyNotInitialized`, 8702 `ErrLevelPolicyNotFoundGameLevelKey`, 8703 `ErrLevelPolicyNotFound`, 8704 `ErrLevelPolicyNotPermitted`, 8705 `ErrLevelNotFoundScoreChangeId`.

| Function | Errors (code/module/line) |
| --- | --- |
| front `0x1400DDA00` | 51/1/314, 48/1/273 |
| `0x1400DEBF0` | 51/1/251 |
| parse `0x1400E28E0` | 10/2/262, 269, 277, 298 (`invalid command`, `xml body missing`, `unknown name ref header`) |
| fields `0x1400E0030` | 10/2/241 `invalid params(%s)` |
| grid load results `0x1400D13E0`, `0x1400D1BE0`, `0x1400D2A70`, `0x1400D3170` | 48/3/921, 483, 408, 289 (`import error no data(%s)`) |
| `CLoadLevelGridTxn` result `0x1400D3A20` | 4/3/1158, 1166 (`no sets`, `no sets for user scores`, `no set for monthly user scores`) |
| grid switch `0x1400EDB30` | 51/4/1614 |
| GetLevel `0x1400EBD80` | 8702/4/1303 (`check policy!`) |
| AddScore `0x1400FD080` | 48/4/253 |
| SubstractScore `0x140100950` | 48/4/560 |
| ChangeLevel `0x1400FD6D0` | 8702/4/1010, 48/4/1054 (`check policy for %s!`) |
| GetScoreChange `0x1400FF560` | 48/4/1417 |
| PageLevelChanges `0x1400FFBE0` | 48/4/1441 |
| pageScoreChanges `0x1401002A0` | 48/4/1482 |
| AddMonthlyRawData `0x1400EB630` | 48/4/1532 |
| StartBatch `0x1400EEC20` / StopBatch `0x1400EF240` | 48/4/1503 / 48/4/1518 |
| `0x1400F2CA0` (AddScore txn step) | 10/4/326 (`policy not found for %s`), 51/4/342 (`not support %s`), 48/4/368 |
| `0x1400F4000` (ChangeLevel txn step) | 10/4/1189, 51/4/1200, 8704/4/1207, 48/4/1255 |
| `0x1400F6AF0` (SubstractScore txn step) | 8705/4/716, 10/4/729, 51/4/744, 48/4/768 |
| `0x1400F6200`, `0x1400F11B0` | 48/4/680; 48/4/1126, 4/4/1116, 1112 (`no reply %s`) |
| txn reply readers `0x1400F05B0`, `0x1400F21F0`, `0x1400F81D0` | 4/4/388, 382; 4/4/1276, 1269; 4/4/791, 783 (`no reply body`) |
| txn bases `0x1400F8E10`, `0x1400F9850`, `0x1400FA340` | 48/4/467, 4/4/453; 48/4/1098, 4/4/1080; 48/4/648, 4/4/630 (`no body`, `not implemented %s`) |
| `0x1400FB7B0`, `0x1400FBE70` | 4/4/61; 4/4/86 (`bad message from LevelDb`) |

### 3.2 Dispatch

- Map `0x140277CF0` (17 ids). Front dispatch `0x1400DDA00` (log `grid data will be lost`; consts 51, 273, 314); second entry `0x1400DEBF0` (OnUnmatched form ❓).
- Grid ids 2, 3, 4, 5, 9, 12, 17 → grid switch `0x1400EDB30` (`Load Grid Data`, `load error - %s`): 2 GetLevel `0x1400EBD80`, 3 ListLevels `0x1400ECA60`, 4 AddScore `0x1400FD080`, 5 SubstractScore `0x140100950`, 9 LoadGrid `0x1400EDA80`, 12 GetGridData `0x1400EBCB0`, 17 ChangeLevel `0x1400FD6D0`; other ids → `0x1400C4610` (error ❓).
- Id 11 Flush → `MsgInit` path (empty reply ❓).
- Stateless ids → `0x1400EEA10`: 1 ListPolicies `0x1400ED5B0`, 6 GetScoreChange `0x1400FF560`, 7 PageLevelChanges `0x1400FFBE0`, 8 pageScoreChanges `0x1401002A0`, 10 AddMonthlyRawData `0x1400EB630`, 13 SetVirtualTime `0x1400EEB20`, 15 StartBatch `0x1400EEC20`, 16 StopBatch `0x1400EF240`.
- Id 14 RequestBatch → `0x1400D8E70`: replies an empty `Reply` with `StsRouteReply`.
- Per-command field reader: `0x1400E0030` (switch on id, cases 1–13, 15–17; field names are not string literals there; they come through `0x140155B80` ❓).

### 3.3 Txn classes

| Class | Vtable | Base step | Steps |
| --- | --- | --- | --- |
| `CAddScoreTxn` | `0x1401A4440` | `0x1400F8E10` | reply reader `0x1400F05B0`; DB step `0x1400F2CA0` (`AddAccumulatedUserScore` or `AddMonthlyUserScore`, chosen by score policy) |
| `CChangeLevelTxn` | `0x1401A4738` | `0x1400F9850` | `0x1400F11B0` (`GetUserScoreChangesByRequestFromCodeRequestKey` result, `no reply %s`), `0x1400F4000` (`AddMonthlyUserScore`; fields `Score`, `IsRefundable`, `IsGifted`; policy checks), reply reader `0x1400F21F0` |
| `CSubstractScoreTxn` | `0x1401A45E8` | `0x1400FA340` | `0x1400F6200` (`GetUserScoreChangesByUserScoreChangeId`), `0x1400F6AF0` (`SubstractAccumulatedUserScore` or `SubstractMonthlyUserScore`; `IsRefundable`), reply reader `0x1400F81D0` |
| `CLoadLevelGridTxn` | `0x1401A31C8` | — | `0x1400D10E0` sends `GetUserLevelsAndScores` (`ReloadCount`); result `0x1400D3A20` → `0x1400D4F30` → assess level `0x1400D5380` |
| `CBatchJobTxn` | near `0x1401A36A0` | — | result `0x1400DA8A0`: job type (+52) 1 → `0x1400D9000` (`GetFirstBatchJob`), 2 → `0x1400D8F70` → `0x1400DA9A0` (`ListMonthlyUserScoreRawData`) |

Vtable refs `0x1400E95F0`/`0x1400E9650`/`0x1400E96D0` and `0x1400EF860`/`0x1400EF9A0`/`0x1400EFB90` make these txn objects (factory / retry restore ❓). The ChangeLevel handler `0x1400FD6D0` itself sends `ChangeUserLevel`; which handler creates `CChangeLevelTxn` is ❓.

### 3.4 Commands

| Id | Command | Handler | Steps |
| --- | --- | --- | --- |
| 1 | ListPolicies | `0x1400ED5B0` | stateless; reply `PolicyVersion`, `Tier` array from the policy (`0x1400E6A00`) |
| 2 | GetLevel | `0x1400EBD80` | grid; level from the grid object; no policy → 8702 (`check policy!`) |
| 3 | ListLevels | `0x1400ECA60` | grid; reply `Tiers`/`Tier` array |
| 4 | AddScore | `0x1400FD080` | grid; creates `CAddScoreTxn`; first `GetUserScoreChangesByRequestFromCodeRequestKey` (duplicate guard, 25000 ms); then `AddAccumulatedUserScore` / `AddMonthlyUserScore`; reply `ScoreChangeId`, `Registered` |
| 5 | SubstractScore | `0x140100950` | grid; `CSubstractScoreTxn`; same duplicate guard; reads `ScoreId`, `ScoreChangeId`; unknown change → 8705 |
| 6 | GetScoreChange | `0x1400FF560` | stateless; `GetUserScoreChangesByUserScoreChangeId` or `…ByRequestFromCodeRequestKey` |
| 7 | PageLevelChanges | `0x1400FFBE0` | stateless; `ListUserLevelChangesByAppGroupCodeGameLevelGroupKeyUserIdRegisteredFrom` |
| 8 | pageScoreChanges | `0x1401002A0` | stateless; `ListUserScoreChangesByUserIdScoreIdRegisteredFrom` |
| 9 | LoadGrid | `0x1400EDA80` | grid; empty `Reply` after the load |
| 10 | AddMonthlyRawData | `0x1400EB630` | stateless; `AddMonthlyRawData` |
| 11 | Flush | front | empty reply ❓ |
| 12 | GetGridData | `0x1400EBCB0` | grid; grid object data |
| 13 | SetVirtualTime | `0x1400EEB20` | stateless; sets the test clock (`0x1400E9020`) ❓ |
| 14 | RequestBatch | `0x1400D8E70` | sets the master flag `0x140276D08` = 1; empty `Reply` (`StsRouteReply`) |
| 15 | StartBatch | `0x1400EEC20` | stateless; `StartBatchJob` |
| 16 | StopBatch | `0x1400EF240` | stateless; `StopBatchJob` |
| 17 | ChangeLevel | `0x1400FD6D0` | grid; policy check (8702, `check policy for %s!`); `ChangeUserLevel` |

### 3.5 DB procs (`LevelDb`, DbProcBegin `0x14010F680`: rcx, rdx, r8 = db, r9 = proc)

| Proc | Call site | Function |
| --- | --- | --- |
| ChangeUserLevel | `0x1400CA0C5` | `0x1400CA040` (`Help.cpp`, from assess level `0x1400D5380`) |
| ChangeUserLevel | `0x1400FF45D` | ChangeLevel `0x1400FD6D0` |
| GetUserLevelsAndScores | `0x1400D1142` | `0x1400D10E0` (grid load) |
| GetFirstBatchJob | `0x1400D9072`, `0x1400DBD19` | `0x1400D9000`, `0x1400DAFF0` |
| SetBatchJobCompleted | `0x1400D96CD`, `0x1400DB791` | `0x1400D95D0`, `0x1400DAFF0` |
| ListMonthlyUserScoreRawData | `0x1400DAA3B` | `0x1400DA9A0` |
| AddMonthlyRawData | `0x1400EB6EE` | `0x1400EB630` |
| StartBatchJob / StopBatchJob | `0x1400EECDE` / `0x1400EF2FE` | `0x1400EEC20` / `0x1400EF240` |
| GetUserScoreChangesByRequestFromCodeRequestKey | `0x1400F17D2`, `0x1400F64E6`, `0x1400FD14C`, `0x1400FF672`, `0x140101016` | `0x1400F11B0`, `0x1400F6200`, `0x1400FD080`, `0x1400FF560`, `0x140100950` |
| AddAccumulatedUserScore or AddMonthlyUserScore | `0x1400F3986` | `0x1400F2CA0` |
| AddMonthlyUserScore | `0x1400F56DA` | `0x1400F4000` |
| GetUserScoreChangesByUserScoreChangeId | `0x1400F64AB`, `0x1400FF637` | `0x1400F6200`, `0x1400FF560` |
| SubstractAccumulatedUserScore or SubstractMonthlyUserScore | `0x1400F75C5` | `0x1400F6AF0` |
| ListUserLevelChangesByAppGroupCodeGameLevelGroupKeyUserIdRegisteredFrom | `0x1400FFC9E` | `0x1400FFBE0` |
| ListUserScoreChangesByUserIdScoreIdRegisteredFrom | `0x14010035E` | `0x1401002A0` |

### 3.6 Grid load and level assess

`0x1400D10E0` sends `GetUserLevelsAndScores` (`CLoadLevelGridTxn`). Result `0x1400D3A20` reads sets `LevelGroups`, `PermanentScores`, `MonthlyScores` (`0x1400D13E0`, `0x1400D2A70`). Then `0x1400D4F30` → `0x1400D5380` assesses the level of each group (logs `assess level`, `%s, %s can not associated`, `%s, asess level failed`, `config error`). A changed level is written with `ChangeUserLevel` (`0x1400CA040`) and notified (3.8).

### 3.7 Batch timer (`CBatchTimer`, object `0x140276CF8`, vtable `0x1401A36F8`)

- Start `0x1400D8F40`: `StsTimerSet(timer, BatchTimerStartSleepSec × 1000)`. Stop `0x1400D8DC0` (period −1).
- OnTimer `0x1400DA900`: only when `0x14013CF80()` (framework flag `dword_140278E80` ❓ meaning) is not 0. If the master flag (+16) is set, runs the batch step `0x1400DAFF0` (`BatchJobQ`; `GetFirstBatchJob`, `SetBatchJobCompleted`). Then clears the flag, sends `Level.RequestBatch` with header `You-are-Master` (`0x14010FE40`, `StsRouteSend`), and sets the timer again with `BatchJobTimerIntervalSec` × 1000.
- The process that gets `RequestBatch` (id 14) sets the flag. So only one process runs the batch at the next tick.
- Job results `0x1400DA8A0`; raw data rows `0x1400D95D0` (`UserId`, `JobKey`, `ScoreId`, `Score`, `Month`, `ParentJobKey`; `BatchJob Error` / `bad data %s`), then `SetBatchJobCompleted`.

### 3.8 Calls out

| Target | Command | Builder | When |
| --- | --- | --- | --- |
| `LevelDb` | procs of 3.5 | `0x14010F680` | handlers, grid load, batch |
| `Level` (self, master) | `RequestBatch` (`You-are-Master`) | `0x14010FE40` | batch timer |
| `Grade` | `NotifyScoreChanged` message (`UserId`, `GradeGroupCode`, `GameLevelKey`, `LevelChanged`, `LevelId`, `AccumulateGradeScore`, `MinimumLevelScore`, `MaximumLevelScore`, `MaximumGradeScore`) | `0x14010F8D0` | notify `0x1400C5F30`, mask bit 1 |
| `GradeMembers` | `Flush` request (`AppGroupCode`, `FlushFilter`) | `0x14010FA80` | mask bit 2 |
| `BillingPush` | `LevelChanged` message (`AppGroupCode`, `GameLevelKey`, `BeforeLevelId`, `AfterLevelId`) | `0x14010F560` | mask bit 4 |

Notify `0x1400C5F30` is called from assess level `0x1400D5380` and score reply `0x1400FAF00` (from `0x1400F11B0`, `0x1400F21F0`). The mask source (`LevelApps` ❓) is not verified.

Not done: worked example (AddScore), quirks, request field lists per command (`0x1400E0030`). See the handoff.
