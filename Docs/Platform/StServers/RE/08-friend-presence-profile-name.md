# 08 — FriendSrv, PresenceSrv, ProfileSrv, NameSrv (RE reference)

Summary:
1. FriendSrv keeps friend lists, proposals, groups and user options in `FriendDb` (schema `v2015050401`). It answers the `Friend` protocol (51 ids, 41 with a handler). It can run in a "smart" mode (protocol `Friend.smart`). See §1.
2. PresenceSrv (type 1002) keeps user presences in memory only (no DB). It answers the `Presence` protocol (27 ids) through the per-user dispatch `0x1400D7ED0`, with cleanup and reconnect timers. See §2.
3. ProfileSrv (type 1034) keeps profiles, contacts and preferences in `ProfileDb` (`v2016082501`); 20 commands, each one proc. See §3.
4. NameSrv (type 113) reserves and checks names: local sessions in `LocalNameDb`, global names in `GlobalNameDb` (`v2015062301`) on the master datacenter; 11 commands, `Exec` runs only procs from the `ProcNames` config. See §4.
5. All four services use the framework of `00-framework.md`. This chapter gives only the service code.

Sources: the `.i64` copies in `ida/lanes/st/` (build 381). All addresses are in the binary of the section. Error records are `{code, module, line}` (framework 3.1); code names are from `errcodes.tsv`. Long lists: `08-commands.tsv`, `08-procs.tsv`. ❓ = not verified in IDA.

## Quick find

| Question | Function | Binary, address | Section |
| --- | --- | --- | --- |
| FriendSrv start (server type 1038, key aliases, providers) | `FriendAppInit` | FriendSrv `0x1400CA4F0` | 1.1 |
| FriendSrv `Friend.ini` keys | `FriendConfigLoad` | FriendSrv `0x1400C57C0` | 1.2 |
| FriendSrv first message filter (smart users, Presence/Privacy/Update events) | `FriendOnMessage` | FriendSrv `0x1400CA770` | 1.3 |
| FriendSrv request parse | `FriendParseRequest` | FriendSrv `0x140119B00` | 1.3 |
| FriendSrv command switch (ids 1–51) | `FriendDispatch` (vtable `0x1401B0698` slot 0) | FriendSrv `0x1401058C0` | 1.4 |
| FriendSrv sequence factory (45 types) | `FriendSeqCreate` (`FriendSequence.cpp`) | FriendSrv `0x1400DAB50` | 1.5 |
| FriendSrv id → command name | `FriendCmdName` | FriendSrv `0x140119810` | 1.4 |
| PresenceSrv start (type 1002, providers) | `PresAppInit` | PresenceSrv `0x1400C55D0` | 2.1 |
| PresenceSrv `Presence.ini` keys | config load | PresenceSrv `0x1400C5720` | 2.2 |
| PresenceSrv command dispatch (27 ids) | `CPresUser` vtable `0x14017B2C8` slot 1 | PresenceSrv `0x1400D7ED0` | 2.4 |
| PresenceSrv presence expiry / reconnect | `CPresCleanup`, `CReconnectTimer` | PresenceSrv `0x1400D67B0`, `0x1400D68A0` | 2.5 |
| ProfileSrv start (type 1034) | `ProfAppInit` | ProfileSrv `0x1400CAC40` | 3.1 |
| ProfileSrv command switch (ids 1–20) | `ProfDispatch` | ProfileSrv `0x1400C85B0` | 3.3 |
| NameSrv start (type 113) | `NameAppInit` | NameSrv `0x1400C6880` | 4.1 |
| NameSrv `Name.ini` (`NameSrvOptions`, `NameTypes`, `ProcNames`) | config load | NameSrv `0x1400CD320` | 4.2 |
| NameSrv command switch (ids 1–10) / Exec (11) | `NameDispatch` / `NameExec` | NameSrv `0x1400CCF30` / `0x1400CC6E0` | 4.3 |
| NameSrv session recovery / expired-name cleanup | timers | NameSrv `0x1400CEFE0` / `0x1400CEEC0` | 4.5 |

## 1. FriendSrv

### 1.1 Role and start

FriendSrv keeps, for each user (UserId GUID): friends (with alias, favourite flag and characters), friend groups, friend proposals (sent, received, deferred), blocks and user options (`IsSuggestionAllowed`, `IsAutoAddAllowed`). It subscribes to presence, privacy and update events of the friends and sends friend events to the clients. Callers: the client through the gates (`Friend` protocol), other services (event commands ids 33–40, 49–50) and other FriendSrv processes (`Friend.smart`).

Start `FriendAppInit` `0x1400CA4F0` (`CApplication` slot 0):
1. `StsSetServerType(1038, 0)`.
2. Program name: `Friend` when the run mode (`0x140145330`) is `smart`, else `Friend.smart` (text in `0x140286D40`) ❓ (the test result of `0x14010B050` is "not equal", so the name is `Friend` when the mode is **not** `smart`).
3. Route key aliases (`0x14015AF30`, same form as RoleSrv `06` §3.1, error 3002 `ErrAccountNotFound`): key type `""`/username → `$Name.GetIdByUserName` (only when `Auth.ini [UserName] UniqueProperty` ≠ 0, default 1, `0x140117B50`); `userid` → `$Name.GetIdByUserId`; `gacct` → `$GameAccount.GetUserIdByAcct`.
4. Provider notify objects (`0x14015B670`): `Presence` (`off_140235190`), `Privacy` (`off_140235198`), `Update` (`off_140235188`), `Friend.smart` (`off_1402351A0`, only when the mode is not `smart`), `MobileFriend` (`off_1402351A8`).
5. `StsConfigMonitorAdd(L"Friend.ini", &FriendConfig 0x140286C30)`, then `0x1400C6120` ❓ (admin pages), then `StsAppCommonStart(0)`.

### 1.2 Config (`Friend.ini`, `FriendConfigLoad` `0x1400C57C0`)

The object `FriendConfig` is at `0x140286C30` (static init `0x1400C0E40`).

| Section | Key | Default | Field | Use |
| --- | --- | --- | --- | --- |
| `Common` | `IsUserNameChangeNoti` | 0 | +12 (bool) | send user name change events to friends |
| `Common` | `GridObjectTimeoutMs` | 86400000 | +8 | only in smart mode ❓ (test inverted, see 1.1); passed to `0x1401385F0("Friend", ms)` = grid object life time |
| `Common` | `UsePrivacy` | 1 | +16 (bool) | use PrivacySrv checks |
| `Common` | `UseFriendLocationNoti` | 0 | +20 (bool) | send friend location events |
| `Common` | `TxnTimeoutMs` | 25000 | +24 | timeout of outgoing requests |
| `Common` | `TimeoutRetryCount` | 0 | +28 | retries after a timeout |
| `GameCharPrivacyLevelConfig` | `config` (many) | — | hash `0x140286630` | value 0 = app group id, value 1 = level text. Logs `Friend.ini: duplicated game character privacy level configuration (app group id: %d)`, `Friend.ini: invalid game char private level (app group id: %d)` |
| `ProposeLimitConfig` | `limit` (many) | — | hash ❓ | value 0 = app group id, value 1 = period in s (stored ×10 000 000, FILETIME units), value 2 = max proposals in the period. Log `duplicated propose limit configuration` |
| `Restriction` | `MaxFriendCount` | −1 (0xFFFFFFFF, no limit) | +200 | max friends |
| `Restriction` | `IgnoreUser` (many) | — | GUID set at +144 | value 1 = UserId. Logs `fail get ignore user configuration`, `zero ignore user id configuration`, `empty ignore user id configuration` |
| `OptNotifyRecvList` | `config` (many) | — | list at +208, count×2 at +224 | each `config` entry: value 0 is parsed as a GUID (`0x14012D300`); a GUID that is not all zero is appended to the array at +208 (capacity +216, count +220, growth step +224, grow `0x1400C5260`). Zero GUIDs are skipped. Use: app ids that get user option events ❓ |
| `History`, `Error`, `Recv`, `Timeout`, `Debug` | `Use` 0, `Target` 0, `InfixFilename` "", `ExtName` `log`, `Period` 0, `CommandOn` "", `GridOn` 0 | — | per log object | service log files. Logs `invalid log target`, `invalid log period` |

The GUID set at +144 has two sources: `IgnoreUser` keys and the `SetSmartUser` command (1.3). `FriendCfgHasUser` `0x1400C56E0` tests it; `FriendCfgAddUser` `0x1400C5190` adds to it.

Framework keys only otherwise (`cfgkeys()`: 56 calls). `NetCfgInt` `BlockMask` (7) and `AlwaysReparse` (0) are in `0x140136050` (framework grid code ❓). `timers()` finds no service timer.

### 1.3 First filter and parse (`FriendOnMessage` `0x1400CA770`)

1. If the message protocol is `Privacy`, `Presence` or `Update` (an event from a subscription): build a new `Friend` message with command `PrivacyInfo` (38), `PresInfo` (37) or `ChangeUserNameInfo` (36) and handle it local (`0x1400C8CC0`). For `Update` the body must be `Update/Object/Scope/State/UserName` with state `changed`; the new message carries `Message` + the object, key from attribute `key`.
2. Else parse with `FriendParseRequest(err, &R 0x140286A60, msg)` `0x140119B00`. Fail → `ReplyError` (lines 102, 116, 144 for `unknown command`, `unknown name ref to header`, `unknown name ref from header`, `no message body`; field errors at lines 737–834).
3. Command `SetSmartUser` (50): add the user (R+0x78 = `0x140286AD8`) to the set +144. Not smart mode: forward the message to `Friend.smart` (`0x14015ACF0`); smart mode: reply OK (empty error).
4. User in the set and not smart mode and `dword_140286A58` ≠ 0: forward to `Friend.smart`.
5. User not in the set and smart mode: reply `ErrInit(4)` = `ErrBadData`.
6. Else: normal path → `FriendDispatch` ❓ (the call path from 0x1400CA770 to `0x1401058C0` is through the request object vtable `0x1401B0698`).

Request fields that the parse reads (XML element names, in order): `ProposalId`, `FriendId`, `UserAlias`, `AppGroupId`, `CharKey`, `FriendAlias`, `FriendAppGroupId`, `FriendCharKey`, `FromGroupId`, `ProposalStatus`, `DisplayStatus`, `GroupId`, `GroupName`, `PageIndex`, `PageSize`, `RetryCount`, `OrderBy`, `TargetId`, `ToGroupId`, `CharFriends`, `Groups`, `AppDataSet`, `Locations`, `Contacts`, `PrivacyType`, `Relation`, `LoginName`, `UserName`, `PlayncUserId`, `FriendPhoneNumber`, `FriendContactName`, `AppId`, `Scope`, `ScopeId` (+216), `MessageName`, `Content`, `IsFavorite` (+444), `IsSuggestionAllowed` (+448), `IsAutoAddAllowed` (+452). Enum maps: `ProposalStatus`/relation (`0x140297E58`: 1 none, 2 proposed, 3 proposing, 4 proposed,proposing, 5 friend, 6 blocked, 7 blocking, 8 blocked,blocking), `Scope` (`0x140297E70`: 1 all, 2 group, 3 user), privacy relation (`0x140297F90`: 1 none, 2 deny, 4 denyby, 8 deny,denyby), presence status (`0x140297F28`: 1 online, 2 offline, 3 away, 4 kicking), `0x140286D90` (1 registered, 2 online).

### 1.4 Commands (`FriendDispatch` `0x1401058C0`)

`switch` on the command id (request +8). Default (ids 13, 41–48, 50 and unknown): error `{52 ErrBadCommand, module 2, line 2002}`. All field errors are `{10 ErrBadParam, module 2, line}`. A handler that starts a sequence builds a `Data` XML of the fields, calls `FriendSeqCreate(type)` `0x1400DAB50` and starts it with `0x14015D010(seq-type, conn, txn, data)` (grid sequence runner ❓); the sequence replies. Full list with fields and lines: `08-commands.tsv`.

| Id | Command | Handler | Checks (error line) | Action |
| --- | --- | --- | --- | --- |
| 1 | `Accept` | `0x1400F91F0` | ProposalId empty (2054); FriendId empty (2058) | seq 6 |
| 2 | `AddFriend` | `0x1400F97E0` | FriendId (2082) | seq 32 (`AddFriend`, `AddFriendToGroup`, `RemoveTentativeFriend`) |
| 3 | `AddFriendToGroup` | `0x1400F9C40` | FriendId (2107), GroupId (2111) | seq 2 |
| 4 | `Block` | `0x1400F9F10` | FriendId (2140); self/ignored (49 `ErrPermission`, 2145) | seq 8 |
| 5 | `CancelProposal` | `0x1400FA150` | ProposalId (2168), FriendId (2172) | seq 10 |
| 6 | `ChangeFriendAlias` | `0x1400FA740` | (2198) | DB `ChangeFriendAlias` direct |
| 7 | `CreateGroup` | `0x1400FAB20` | (2230) | DB `CreateFriendGroup` direct |
| 8 | `Defer` | `0x1400FAD40` | (2267) | DB `UpdateProposal` direct |
| 9 | `DeleteGroup` | `0x1400FAEC0` | GroupId (2297); group 0 (3019 `ErrGroupNotFound`, 2302); 49 (2306) | seq 12 |
| 10 | `DenyProposal` | `0x1400FB140` | ProposalId (2329), FriendId (2333) | seq 13 |
| 11 | `GetRelations` | `0x1400FB910` | 4 `ErrBadData` (2358, 2372) | seq 16; flags `GetCharFriends`, `GetGroups`, `GetAppDataSet`, `GetLocations`, `GetContacts`, `RetryCount` |
| 12 | `GetUserInfo` | `0x1400FBC60` | — | seq 17 |
| 14 | `GetUserOptions` | `0x1400FBCC0` | — | reply from cache: `Reply/IsSuggestionAllowed`, `IsAutoAddAllowed` |
| 15 | `ListGroups` | `0x1400FC070` | — | reply from cache: `Reply` with `type="array"` group list |
| 16 | `MoveGroup` | `0x1400FC4D0` | (2445) | DB `MoveGroup` direct |
| 17 | `PageFriends` | `0x1400FC810` | (2500) | seq 22 |
| 18 | `PageMobileSuggestedFriends` | `0x1400FCDD0` | (2570) | seq 38 |
| 19 | `PageMutualFriends` | `0x1400FD000` | (2598) | seq 20 |
| 20 | `PageRecvProposals` | `0x1400FD290` | (2627) | seq 40 |
| 21 | `PageSentProposals` | `0x1400FD590` | (2669) | seq 39 |
| 22 | `PageSuggestedFriends` | `0x1400FD890` | (2710) | seq 28 |
| 23 | `Propose` | `0x1400FDEE0` | (2734, 2740) | seq 23; `ProposeInfo` element |
| 24 | `Remove` | `0x1400FE870` | (2801) | seq 25 |
| 25 | `RemoveFriendFromGroup` | `0x1400FED70` | FriendId (2859), GroupId (2863) | seq 27 |
| 26 | `RemoveUser` | `0x1400FF040` | — | seq 41 |
| 27 | `Reversed` | `0x1400FF540` | lines 2894, 2899, 2904 | body has `protocol` and `command`; runs the inner `Friend` command (`Accept`, `AddFriend`, `CancelProposal`, `DenyProposal`, `Propose`, `Remove` …) for the other side |
| 28 | `SendMessage` | `0x1400FF760` | (3236, 3241) | seq 31 (`DoMessage`); `AppId`, `Scope`, `ScopeId`, `MessageName`, `Content` |
| 29 | `SetFavorites` | `0x1400FFAC0` | (3292) | DB `ChangeFriendFavorites` direct |
| 30 | `SetUserOptions` | `0x1400FFC60` | — | DB `ChangeUserOptions` direct |
| 31 | `Unblock` | `0x1400FFD70` | FriendId (2924); 49 (2929) | seq 29 |
| 32 | `UpdateGroup` | `0x1401002A0` | (2952, 2956) | DB `ChangeFriendGroupName`; reply `Reply/GroupId/GroupName` |
| 33 | `ContactUpdated` | `0x1400FFFB0` | — | `State` = `added`/`changed`/`deleted` with `FriendPhoneNumber`, `FriendContactName`; update in place |
| 34 | `MobileAccountAssociated` | `0x1400FC1A0` | — | seq 43 (`UserId`, `PlayncUserId`) |
| 35 | `MobileAccountDissociated` | `0x1400FC340` | — | seq 44 |
| 36 | `ChangeUserNameInfo` | `0x1400FA960` | — | seq 35 |
| 37 | `PresInfo` | `0x1400FDAA0` | — | seq 21 (`AppDataSet`, `Status`, `Locations`) |
| 38 | `PrivacyInfo` | `0x1400FDD10` | — | seq 34 (`TargetId`, `PrivacyType`, `Relation`) |
| 39 | `FriendPresInfo` | `0x1400FB730` | — | update in place (`Status`, `AppDataSet`, `Locations`) |
| 40 | `RemoveUserInfo` | `0x1400FF260` | (3543) | seq 42 |
| 49 | `RemoveUserCache` | `0x1400FF210` | — | drop the grid object of the user (`0x140138910`), reply OK |
| 51 | `PageGameFriends` | `0x1400FCB50` | (2539, 2544) | seq 45 (`PageIndex`, `PageSize`, `AppId`) |

Ids 41–48 (`FriendInfo`, `NotifyMessage`, `NotifyProposalDenied`, `NotifyRemoved`, `NotifyUserNameChanged`, `ProposalAccepted`, `ProposalCancelled`, `Proposed`) are events that FriendSrv **sends** to clients (computed command names in `0x140125940` calls), not requests. Id 13 `GetUsersInfo` has no handler (error 52) ❓ (no other switch found).

### 1.5 Sequences (`FriendSequence.cpp`, factory `0x1400DAB50`)

`FriendSeqCreate(type)` makes one sequence object (45 types). Each sequence is a state machine that sends DB and service requests and replies at the end. Vtables found by scan of the `case` code (type: vtable: procs reachable from the slots in 2 steps):

| Type | Vtable | Procs |
| --- | --- | --- |
| 2 | `0x1401ACC58` | `AddFriendToGroup`, `MoveGroup`, `RemoveFriendFromGroup` |
| 5 | `0x1401AC618` | `AddTentativeFriend`, `GetProposeStatus`, `RemoveFriend`, `RemoveFriendChars`, `RemoveFriendFromGroups` |
| 6, 8, 10, 13, 16, 17, 21, 22, 25, 29, 34, 44 | `0x1401AC658`, `0x1401AC758`, `0x1401AC798`, `0x1401AC818`, `0x1401AC9D8`, `0x1401ACA18`, `0x1401ACA58`, `0x1401ACA98`, `0x1401AC918`, `0x1401AC998`, `0x1401ACE38`, `0x1401ACFB8` | no proc in 2 steps (deeper or service calls only) ❓ |
| 12 | `0x1401ACC18` | `AddFriendToGroup`, `DeleteFriendGroup`, `MoveGroup` |
| 19 | `0x1401ACBD8` | `DeleteFriendGroup`, `GetProposalCount`, `GetUserOptions`, `ListFriendGroups`, `MoveGroup` (user load) |
| 20, 28, 38 | `0x1401ACCD8`, `0x1401ACD18`, `0x1401ACD58` | `GetUsersOptions` |
| 27 | `0x1401ACC98` | `MoveGroup`, `RemoveFriendFromGroup` |
| 32, 33 | `0x1401AC6D8`, `0x1401AC718` | `AddFriend`, `AddFriendToGroup`, `RemoveTentativeFriend` |
| 35, 39 | `0x1401ACE78`, `0x1401ACEB8` | `ListProposals` |
| 40 | `0x1401ACEF8` | `ListProposals`, `RemoveFriendUser`, `RemoveProposalUser` |
| 1, 3, 4, 7, 9, 11, 14, 15, 18, 23, 24, 26, 30, 31, 36, 37, 41, 42, 43, 45 | not found by the scan (constructor call, for example type 1 `0x1400CBAC0`, 3 `0x1400CE200`, 23 `0x1400CE160`, 45 `0x1400CD7D0`) | ❓ |

### 1.6 DB procs

`FriendDb`, schema `v2015050401`, 36 proc names in 48 calls, one computed name at `0x1400F075D` (in `0x1400F0700`, only caller `CLoadFriendsSeq::OnProceed` `0x1400E44E0` line 584): the name is the next list step of the load sequence, one of `ListTentativeFriends`, `ListFriends`, `ListFriendChars`, `ListFriendGroupFriends` (after `GetFriendCount` gives `TentativeFriendCount`, `FriendCount`, `FriendCharCount`, `GroupFriendCount`). Constant 1000 in `0x1400F0700` is probably the page size ❓. Full list with call sites: `08-procs.tsv`.

### 1.7 Calls out

Message builder `0x140125940(msg, command, protocol)`; targets found (one call site each unless noted):

| Protocol | Commands | Why |
| --- | --- | --- |
| `Presence` | `SubscribeTrusted`, `Unsubscribe`, `GetUserInfo`, `Forward` | watch friend presence; send friend events through PresenceSrv to the online client |
| `Privacy` | `SubscribeTrusted`, `Unsubscribe`, `DenyUser`, `AllowUser`, `PageDenyUsers` | block/unblock = deny/allow in PrivacySrv |
| `Update` | `SubscribeTrusted`, `Update`, `Forward` | user name change events |
| `MobileFriend` | `AddFriendContact`, `RemoveFriendContact`, `PageFriendContacts`, `PageSuggestedFriends` | contact based friends |
| `Auth` | `GetUserInfo` | user data |
| `Profile` | `GetUserProfileImgUrl` | image URL in friend lists |
| `GameAccount` | `ListUsersAccounts` | game friends (`PageGameFriends`) |
| `AppAuth` | `ListGroups` | app group data |
| `Friend` | `Accept`, `AddFriend`, `CancelProposal`, `ContactUpdated`, `DenyProposal`, `GetRelations`, `GetUserInfo`, `PageFriends`, `PageMobileSuggestedFriends`, `Propose`, `Remove` + 17 computed names (events 41–48) | `Reversed` requests to the FriendSrv that owns the other user; events to clients |
| `Friend.smart` | forward (2 sites, 1.3) | smart users |
| `EventLog` | `LogEvent` (3 sites) | audit log |

### 1.8 Worked example (`Accept`)

Request `Friend/Accept`, route key = UserId `A`, body `<ProposalId>P</ProposalId><FriendId>B</FriendId>`. Handler `0x1400F91F0`: P and B not zero → `Data` = `{ProposalId: P, FriendId: B}`, start seq 6. ProposalId zero → reply `{10, 2, 2054}`; FriendId zero → `{10, 2, 2058}`. Sequence 6 steps ❓ (vtable `0x1401AC658` not decoded).

## 2. PresenceSrv

### 2.1 Start (`PresAppInit` `0x1400C55D0`)

1. `StsSetServerType(1002, 0)`.
2. `StsConfigMonitorAdd(L"Presence.ini", …)`, then `0x1400D7A20` ❓.
3. Name `"Presence"` (`unknown_libname_6_0`). Route key alias `gacct` → `$GameAccount.GetUserIdByAcct` (`0x140136B80`). When `0x1400F0120` returns ≠ 0 (stored in `dword_14024B144`; Auth.ini UniqueProperty ❓): default key type → `$Name.GetIdByUserName`.
4. Provider notify objects (`0x1401372C0`): `EventLog` (`off_1401FB060`), `Update` (`off_1401FB068`), `Gate` (`off_1401FB070`), `Name` (`off_1401FB078`).
5. `0x1400C50F0` ❓ (admin pages), `StsAppCommonStart(0)`.

### 2.2 Config (`Presence.ini`, `0x1400C5720`)

| Section | Key | Default | Call site |
| --- | --- | --- | --- |
| `Limits` | `MaxNotifyRetryCount` | 3 | `0x1400C5749` |
| `Limits` | `MaxWebSessionPresenceCount` | 0 | `0x1400C5770` |
| `General` | `NotiAppDataToPresences` | 0 | `0x1400C5794` |
| `General` | `RemoveAppIdsOnPresMessage` | 0 | `0x1400C57B8` |
| `General` | `MergeLocationsByAppId` | 1 | `0x1400C57DF` |
| `General` | `UseDefaultExternalAccount` | 0 | `0x1400C5803` |
| `LocalLog` | `Use` 0, `InfixFilename` "", `ExtName` `log`, `Period` 0 | — | `0x1400C582F`–`0x1400C58DA` |
| `Validation` | `UserName` | 1 | `0x1400C5954` |

Use of each key ❓ (not traced), except `MaxWebSessionPresenceCount` (Login limit, §2.4). Timers: §2.5. `dbprocs()` fails on this binary (IndexError); PresenceSrv has no DB proc call (§2.6).

### 2.3 Maps

Service maps: `Presence` command map `0x14025D010` (27 ids: 1 AccountCreate, 2 GetUserInfo, 3 Login, 4 Logout, 5 Message, 6 Kick, 7 SetAppData, 8 SetUserData, 9 SetStatus, 10 SubscribeTrusted, 11 Unsubscribe, 12 Whisper, 13 UserInfo, 14 UserLogout, 15 SetWatch, 16 Disconnect, 17 Event, 18 Exec, 19 Flush, 20 Forward, 21 Reversed, 22 SendUserInfo, 23 Talk, 24 RemoveUser, 25 UserIdChanged, 26 XferRequest, 27 XferPresences; id → name `0x1400F26D0`). Subscription map `0x14025D0C0` (6: Subscribe, SubscribeTrusted, Unsubscribe, Update, Forward, Updated; parse user `0x1400F7620`). Status map `0x14025D028` (online, offline, away, kicking). Client maps also present: Auth (110), access (36, `0x14025CF50`), Channel (55, `0x14025CF70`, id → name `0x1400F0B90`), GameAccount (21), Session (13, `0x14025D0A0`).

### 2.4 Per-user dispatch (`CPresUser` vtable `0x14017B2C8`; slot 0 = `0x1400DAD00` dump, slot 1 = `0x1400D7ED0` dispatch)

The vtable starts at `0x14017B2C8` (not `0x14017B2D0`). Slot 0 `0x1400DAD00` writes the user state as XML (`AppDataSet` array of `AppData`, field `CleanupTimeS` from presence +720); it is the admin/dump writer ❓. Slot 1 `0x1400D7ED0` is the command switch. IDA has no code xref to the vtable (indirect use); the caller of slot 1 is the framework user-transaction object (`CUserTxnData<Notify>` string in `0x1400D4E60`) ❓.

`switch` on the command id (`*a3`, jump table `0x1400D7EF7`, cases 2–27). Default: error `{52 ErrBadCommand, module 3, line 3291}`, text `PresUser: invalid command`. Ids 1 `AccountCreate`, 8 `SetUserData`, 10 `SubscribeTrusted`, 11 `Unsubscribe`, 14 `UserLogout`, 23 `Talk` go to the default. A scan of all switches in the binary (`switches()`) finds no other switch on the Presence ids (only the id → name function `0x1400F26D0`). PresenceSrv itself sends `UserLogout` (builder `0x1400F2420`: `Presence/UserLogout` with `UserId`, `GameAccountId`, `Reason`; caller `0x1400D8D70`). `SubscribeTrusted` is in the subscription map (§2.3), parse `0x1400F7470` ❓.

All handler errors use module 3. Field errors are code 10 `ErrBadParam`. Full rows: `08-commands.tsv`.

| Id | Command | Handler | Steps |
| --- | --- | --- | --- |
| 2 | GetUserInfo | `0x1400CE4B0` | Needs `From` (else `{10,3,4287}`). Builds the user info (`0x1400CB3F0`). |
| 3 | Login | `0x1400CEB20` | Checks `Location`, `From`, `clientNetAddress` (`0x1400DE610` type 6601), `AppId` (required). Error texts `login, invalid …` with code 10 (line in a shared path ❓). When the session is a web session (`0x140108BC0` = 2) and the count is over `MaxWebSessionPresenceCount`: `{3005 ErrFlooding,3,3366}`. Reads `ExternalAccount` (bad: `{10,3,3373}`). Copies `UserName`/`LoginName` into user +109 (189 bytes, `0x1400DC470`). Makes a presence (`0x1400F01E0`), then sends an async request object (`0x1400C72F0`, 0x690 bytes) by `0x140114340` with timeout 25000 ms; the reply comes from that object. |
| 4 | Logout | `0x1400CF020` | Needs `Location` (`{10,3,3721}`). Finds the appData of the client appId (log `logout, login had not client appId`, `logout, could not find appData for (%s)`), removes the presence (`0x1400CCAE0`), replies `Reply`. |
| 5 | Message | `0x1400CF2F0` | Needs a full message (`{10,3,4461}`). Wraps the body in a `Presence` message with `protocol`/`command` attributes and an `AppId` list, sends to the watchers; can call Forward `0x1400CDE40`. |
| 6 | Kick | `0x1400CE630` | Reads `GameCode`, `Location`, `ChannelIds`, `AppIds` and two more fields ❓. Kicks matching presences (`0x1400D3940`/`0x1400D3AE0`/`0x1400D3D10`/`0x1400D3F30` → `0x1400D8340`). Replies `Reply` with an `AppIds` array. |
| 7 | SetAppData | `0x1400CFB20` | Needs `Data` (`4326`), `From` (`4328`), valid from-appId (`4330`), valid appId (`4332`). Updater `0x1400F8B30`; failure → `{0,3,4348}` `set app data, updater line %u`. Publishes (`CPresPaper::PublishExtra` `0x1400D85F0`). |
| 9 | SetStatus | `0x1400D0090` | Needs `From` (`4374`). Sets status (`0x14013C1D0`), replies `Reply`. |
| 12 | Whisper | `0x1400D0AD0` | Needs the from header (`4395`). Makes a new guid (fail → `{4 ErrBadData,3,4418}`), sends the message to the target (`0x140101040`), replies `Reply`. |
| 13 | UserInfo | `0x1400D07E0` | Builds a `Message` with the user info and sends it to the watchers. |
| 15 | SetWatch | `0x1400D02B0` | Needs `From` (`4671`) and `Location` (`4673`). Adds a watch under key `Presence` (`0x14013BC20` on user +576). |
| 16 | Disconnect | `0x1400CD440` | Needs `Location` (`4262`). Removes the connection (`0x14013A8D0`), then runs Logout `0x1400CF020`. |
| 17 | Event | `0x1400CD610` | Parses `Update/Object/State` (logs `%u, evt, no body`, `%u, evt, no update elem`, `%u, evt, no target object`). State `changed` with `UserName`/`LoginName` → updates the cached name (`0x1400C5490`, 189 bytes at user +109), then sends `Presence/UserInfo` (13) to the watchers. No error reply. |
| 18 | Exec | `0x1400CDBA0` | Reads `ProcName` and `Params`. There is no proc table: it always replies `{10,3,4812}` `exec %s, unknown procName`. The framework fingerprint name `DbProcBegin` on this address is wrong. |
| 19 | Flush | `0x1400CDCC0` | Logs `GridObjectDestory: %s`, replies `Reply`. |
| 20 | Forward | `0x1400CDE40` | Needs `refer-to` (`4601`) with `protocol` (`4605`), `command` (`4607`) and a valid presence id (`4616`). Forwards the request to that presence. |
| 21 | Reversed | `0x1400CF740` | Needs `refer-to` (`{10,3,4823}`) with `protocol` and `command`. Only protocol `Channel` is known (`0x1400F0E60` name → id, then `0x1400CCE00`). Other cases reply an error with texts `reversed, refer-to missing protocol/command`, `reversed, unknown command` (code/line ❓). |
| 22 | SendUserInfo | `0x1400CF950` | Needs `From` (`4303`) and flags (`4306`). Sends the user info message. |
| 24 | RemoveUser | `0x1400D8BF0` | Calls kick `0x1400D8340` for each presence (PresUser.cpp lines 3771/3773: `RequestKick failed`, `PresKick alloc failed`). |
| 25 | UserIdChanged | `0x1400D0700` | Copies the new id (request +1056, `0x1400F2600`), sends async object `0x1400C6C00` (25000 ms). |
| 26 | XferRequest | `0x1400D1100` | Request +1040 (`0x1400F2530`), sends async object `0x1400C6AA0` (25000 ms). The reply parser `0x1400D64F0` logs `failed to parse xferPresences reply` and rebuilds presences (`0x1400D4080`, reads `CleanupTimeS`). |
| 27 | XferPresences | `0x1400D0EC0` | Writes the presence list into `Reply`. |

### 2.5 Timers

| Class | Vtable | OnTimer | Behaviour |
| --- | --- | --- | --- |
| `CPresCleanup` | `0x14017B310` (instance `0x1401FB120`) | `0x1400D67B0` | Queue of presences with an end time (presence +720, seconds; set from the `CleanupTimeS` field in `0x1400D4080`/`0x1400D6DA0`). If the first entry is not due: return `1000 × (end − now)` ms. Else remove it from the queue, find the user (`0x1400D7DB0` on presence +240) and remove the presence (`0x1400CCAE0`); if no user, log `CPresCleanup::OnTimer, pres user lookup failed`. Then free the entry. Every 20000 entries it returns 1000 ms (yield). Empty queue: 300000 ms. |
| `CReconnectTimer` | `0x14017B2F8` (instance `0x1401FB110`) | `0x1400D68A0` | Heap of users (`qword_1401FB0F8`, count in `qword_1401FB100` high dword) ordered by time (user +80). Returns the time left of the first user. For each due user: pop, then if user +344 is empty or odd, set status (`0x14013C1D0` on user +576) and run `0x1400CC2F0`, `0x1400CC430` (log out the user ❓). Returns −1 (no re-arm) when the heap is empty. |

### 2.6 DB and calls out

No DB proc call (no `DbProcBegin` call form; `Exec` is a stub). Calls out: async request objects through `0x140114340` with timeout 25000 ms (Login `0x1400C72F0`, UserIdChanged `0x1400C6C00`, XferRequest `0x1400C6AA0`); target service ❓ (Gate/Name providers of §2.1). HTTP-style notify `0x1400C4900` builds a `text/xml` request with `userId`, `loginName`, `userName` (error `invalid parameter (all input parameters are null)`) ❓. Publish to channels: `CPresPaper::PublishExtra` `0x1400D85F0`.

### 2.7 Worked example (SetWatch)

Request `Presence/SetWatch` from user U, body with `From` = presence of U and `Location` = L. Handler `0x1400D02B0`: `From` present, `Location` present → adds the watch with key `Presence` to U (+576) and replies `Reply`. With no `Location` → `{10, 3, 4673}`.

## 3. ProfileSrv

ProfileSrv keeps the user profile (gender, legal birthday, profile image URL), user contacts (email, opt-in) and user preferences (language) in `ProfileDb` (schema `v2016082501`). It answers the `Profile` protocol (20 ids). It also has HTTP admin pages under `/profile/`.

### 3.1 Start (`ProfAppInit` `0x1400CAC40`)

1. `StsSetServerType(1034, 0)` (`0x1400CAC54`).
2. Name `"Profile"` (`unknown_libname_6_0`). When `0x1400E08B0` returns ≠ 0 (reads `Auth.ini` `[UserName] UniqueProperty`, default 1): route key default → `$Name.GetIdByUserName`. Key alias `login` → `$Name.GetIdByLoginName` (`0x14011E530`, arg 3002).
3. `0x1400FBCA0(v, 0)` ❓, then the config object `0x1400C4400` → `StsConfigMonitorAdd(L"Profile.ini", cfg)`.
4. `0x1400D1980`: registers the object types `ProfUserProfile`, `ProfTxn` ❓.
5. `0x1400C6AF0`: `StsHttpAddCommands` with pages `/profile/` (`Profile Management`, `Maintenance Commands`, `/profile/get_user_info`, `/profile/get_user_profile`, more ❓). Console handlers `0x1400C5080` / `0x1400C56E0` (ProfConsole.cpp) read `userId`, `genderCode`, `legalBirthday` (errors `Invalid UserId`, `Invalid legalBirthday`).
6. `StsAppCommonStart(0)`.

### 3.2 Config (`0x1400C4400`, ProfConfig.cpp)

Singleton at `qword_140235B90` (0x80 bytes). +56 and +80: two 16-byte base64 constants (`keyhere`; built in, not read from the file). +8 = 60000: timeout in ms, given to `0x1400FBE90("Profile", 60000)` ❓. `cfgkeys()` finds no `Profile.ini` key in the service code; the file is only monitored ❓. Other keys: `Auth.ini` `[UserName] UniqueProperty` (default 1, `0x1400E0931`); framework keys `BlockMask` (7) and `AlwaysReparse` (0) in `0x1400F9A90` ❓.

### 3.3 Commands (`ProfDispatch` `0x1400C85B0`)

Command map `0x140237090` (21 entries: 0 `Invalid`, ids 1–20). `switch` on the id, cases 1–20. Default: `{52 ErrBadCommand, module 2, line 250}`. Handler errors are written into the error record (`a2`): code 10 `ErrBadParam`, module 2, line below, text at +16. Each handler that calls the DB builds the proc call (`DbProcBegin` `0x1400E0970`, database `ProfileDb`, schema `v2016082501`) and sends it with `0x1400FC320(req, obj, 0x7FFFFFFF, 0, 25000, 0)` (timeout 25000 ms); the reply object (ProfData.cpp) writes the reply.

| Id | Command | Handler | Proc | Checks (error line, text) |
| --- | --- | --- | --- | --- |
| 1 | GetUserInfo | `0x1400C9310` | `GetUserProfileAndContact` | — (uses config `0x1400C4400`) |
| 2 | GetUserProfile | `0x1400C9770` | `GetUserProfile` | — |
| 3 | GetUserProfileImgUrl | `0x1400C98E0` | none | Builds `Reply` with `ProfileImgUrl` from config ❓ |
| 4 | CreateUserProfile | `0x1400C8E60` | `AddUserProfile` | 385 `invalid legalbirthday`, 390 `invalid genderCode` |
| 5 | ChangeUserProfile | `0x1400C8960` | `UpdateUserProfile` | 423/446 `invalid legalbirthday` ❓ |
| 6 | RemoveUserProfile | `0x1400CA360` | `RemoveUserProfile` | — |
| 7 | GetUserContact | `0x1400C91A0` | `GetUserContact` | — |
| 8 | GetUserSendEmail | `0x1400C9AD0` | none found ❓ | lines 521/524 ❓ (uses config) |
| 9 | CreateUserContact | `0x1400C8B90` | `AddUserContact` | 549/555 `invalid param` |
| 10 | ChangeUserContact | `0x1400C8750` | via `0x1400D4230` (`UpdateUserContact` ❓) | 575 `the maximum length of an email address is 64` |
| 11 | RemoveUserContact | `0x1400CA150` | via `0x1400C41F0` (`RemoveUserContact` ❓) | 594 `invalid userid` |
| 12 | PageUserContacts | `0x1400C9E00` | `PageUserContacts` | 613 `invalid customerKey`, 618 `invalid pageIndex`/`invalid pageSize` ❓ |
| 13 | PageUserContactChanges | `0x1400C9C20` | `PageUserContactChanges` | 654 `invalid pageIndex`, 659 `invalid pageSize` ❓ |
| 14 | DeactivateUser | `0x1400C9080` | `DeactivateUser` | — |
| 15 | ReactivateUser | `0x1400CA030` | `ReactivateUser` | — |
| 16 | UpdateUserSmsOptin | `0x1400CA480` | via `0x1400D2230` (`GetUserContact`, then update ❓) | 740 `invalid param` (`0x1400CA9C0` check) |
| 17 | GetUserPreference | `0x1400C9600` | `GetUserPreference` | — |
| 18 | CreateUserPreference | `0x1400C8D20` | `AddUserPreference` | — |
| 19 | ChangeUserPreference | `0x1400C8850` | `UpdateUserPreference` ❓ | 817 `the maximum length of an language code is 5` |
| 20 | RemoveUserPreference | `0x1400CA240` | `RemoveUserPreference` | — |

Profile fields (strings `0x14016B198`…): `GenderCode`, `LegalBirthday` (parse `0x1400C83B0`); contact fields `EmailAddress`, `OptInEmail`, `OptInEmailFrom` (parse `0x1400C79B0`); `Email` (`0x1400C7EE0`); `ProfileImgUrl`. Request parse into the request struct ❓ (field table not traced).

### 3.4 Sequences (contact change log)

`0x1400CB6E0` (writes `Reply`) is the common reply step of five multi-step functions: `0x1400CCE00` (switch 0–19, step machine ❓), `0x1400CCF10` (`UpdateUserContact` then `AddUserContactChange`), `0x1400CD8A0` (`UpdateUserPreference`), `0x1400D07D0` (`RemoveUserContact`), `0x1400D09F0` (`AddUserContact` / `UpdateUserContact` / `AddUserContactChange`). Contact changes are logged into `UserContactChanges` (read by id 13, removed by `RemoveUserContactChanges` `0x1400D2179`). Full proc list: `08-procs.tsv` (28 call sites).

### 3.5 State, timers, calls out

No service cache found. `timers()` finds one timer in service range: `0x1400FF160` → OnTimer `0x1400FEBC0` (HTTP request timeout: `Timeout processing request`, html body); this is framework HTTP code ❓. Calls out: route keys to `Name` (`$Name.GetIdByUserName`, `$Name.GetIdByLoginName`) only.

### 3.6 Worked example (CreateUserProfile)

Request `Profile/CreateUserProfile` for user U with `GenderCode` = bad value. Handler `0x1400C8E60` → error `{10, 2, 390}` `invalid genderCode`. With good values → proc `ProfileDb.AddUserProfile` (`0x1400C8F4F`), timeout 25000 ms, reply from the proc result.

## 4. NameSrv

NameSrv keeps user names in `LocalNameDb` and checks and reserves names in `GlobalNameDb` (schema `v2015062301`). It answers the `Name` protocol (11 ids).

### 4.1 Start (`NameAppInit` `0x1400C6880`)

1. `StsSetServerType(113)`.
2. Name `"Name"` (`unknown_libname_6_0`).
3. `0x140100AC0(v, 0)` ❓, `0x140100CB0("Name", 60000)` (timeout 60000 ms ❓), `0x1400CCE30` ❓ (config/timers), `0x1400C4380` ❓.
4. `StsAppCommonStart(0)`.

### 4.2 Config (`0x1400CD320`, section `NameSrvOptions`)

| Key | Default | Call site |
| --- | --- | --- |
| `MasterDatacenter` | "" (string; matched against a datacenter list, `0x1400D01A0`) | `0x1400CD3AA` |
| `SessionEffectiveUntil` | 60 | `0x1400CD459` |
| `SessionRecoveryInterval` | 60000 | `0x1400CD497` |
| `DeleteExpiredNamesInterval` | 60000 | `0x1400CD4D7` |
| `DeleteExpiredNamesCounts` | 100 | `0x1400CD517` |
| `CompletedSessionEffectiveUntil` | ❓ | `0x1400CD538` |

Unit of each value ❓ (intervals probably ms, "EffectiveUntil" probably seconds).

#### 4.2.1 Config object and other sections

The config object is at `0x1401DE140` (vtable `off_1401DE140`, monitored for `Name.ini` by `0x1400CCE30`). The loader `0x1400CD320` writes these fields. Handlers read them as globals:

| Global | Field | Key | Check |
| --- | --- | --- | --- |
| `dword_1401DE148` | +8 | `MasterDatacenter`, converted to a datacenter id (field +4 of a 72-byte datacenter record) | 0 → log `NameSrv: MasterDatacenter is invalid.` |
| `dword_1401DE14C` | +12 | `SessionEffectiveUntil` | too short → log, set 60 |
| `dword_1401DE150` | +16 | `SessionRecoveryInterval` (ms, timer period, §4.5) | too short → log, set 60000 |
| `dword_1401DE154` | +20 | `DeleteExpiredNamesInterval` | too short → log, set 60000 |
| `dword_1401DE158` | +24 | `DeleteExpiredNamesCounts` | — |
| `dword_1401DE15C` | +28 | `CompletedSessionEffectiveUntil` (default = a local value ❓) | — |
| `dword_1401DE160` | +32 | `DisableLocalName.<node name>` (`0x1400C7AC0`, default 0) | — |
| `dword_1401DE164` | +36 | `DisableLocalNameDb.<node name>` (default 0) | — |

The "too short" limits are not read ❓. Node name comes from `GetNodeInfo` (log `NameSrv: GetNodeInfo failed`).

- Section `NameTypes` (`0x1400CD320` line 179): each entry is `<type name> = <type id>, <pending secs>`. The loader keeps the types in two maps: by id (`unk_1401DE168`, object +40, record 0x50 bytes, +72 = pending secs) and by name (`NameType` object, 0x40 bytes). If an entry is bad, it logs `NameSrv: NameType(%s) config is invalid`.
- Section `ProcNames` (line 279): each entry is `<proc name> = <type name>, <flag>`. Map at object +96 (`qword_1401DE1C0` buckets, `dword_1401DE1D0` mask). Record `ProcNameConfig` (0xC0 bytes): +56 name, +184 = type id of `<type name>`, +188 = flag. The loader always adds a built-in entry `GetIdByUserId` {type 0, flag 1}. Logs: `NameType(%s) is not supported`, `NameType for (%s) is invalid`, `ProcName (%s) exists`.

#### 4.2.2 Other start work

- `0x1400CCE30`: registers the transaction stats names `ModifyNamesTxn` (`off_1401DE1D8`), `CheckNamesTxn`, `GetNameInfoTxn`, `ModifyBanWordTxn`, `ExecTxn`; monitors `Network.ini`, `StServer.ini` (object `off_1401DE118`) and `Name.ini` (config object); makes a pool `0x1400F1EF0(4096, 0x4000)`; ends with `0x140115270(5, &unk_1401DE080)` ❓ (counters `dword_1401DE090`, `dword_1401DE0D8` are in this block).
- `0x1400C4380` (`StsHttpAddCommands`): HTTP pages `/name` (`Name`), `/test` (`Test`) with `/add_name`, `/remove_name`, `/check_name`, `/change_name`, `/change_status`, `/change_user`, `/get_name_info`, `/add_ban_word`, `/remove_ban_word`, and `Session` pages `/list_expired_sessions`, `/list_error_sessions` (procs `ListExpiredSessions` `0x1400C601B`, `ListErrorSessions` `0x1400C5ECB`).

### 4.3 Commands

Command map `0x14022F0E0` (11): 1 AddNames, 2 AddBanWord, 3 AddUser, 4 ChangeNames, 5 ChangeStatus, 6 ChangeUser, 7 CheckNames, 8 GetNamesInfo, 9 RemoveBanWord, 10 RemoveNames, 11 Exec. `table()` finds no handler. All 11 ids have a handler.

Entry `NameOnMessage` `0x1400C6970`: if the id (`dword_14022DA08`) is 11, it calls `NameExec` `0x1400CC6E0`; else `NameDispatch` `0x1400CCF30`. If the handler returns 0, it sends the error record with `ReplyError`. `0x1400CA860` is not a dispatcher: it is the field reader for `SessionData` (`UserDatacenter`, `UserId`, `EffectiveUntilSecs`, `NameStatus`, `PendingSecs`, `Preemptive`, `Pending`, `NewNameValue`), called by `0x1400CA5E0` and `0x1400CAB70`.

`NameDispatch` `0x1400CCF30`, switch at `0x1400CCF51`:

| Id | Command | Handler |
| --- | --- | --- |
| 1, 4, 5, 6, 10 | AddNames, ChangeNames, ChangeStatus, ChangeUser, RemoveNames | `NameModify` `0x1400C9C70` |
| 2, 9 | AddBanWord, RemoveBanWord | `NameBanWord` `0x1400C98F0` |
| 3 | AddUser | `NameAddUser` `0x1400C7CC0` |
| 7 | CheckNames | `NameCheck` `0x1400C7ED0` |
| 8 | GetNamesInfo | `NameGetInfo` `0x1400C8FD0` |
| other | — | `{52 ErrBadCommand, 2, 2911}` |

Error records are `{code, module 2, line}`. Codes: 4 `ErrBadData`, 10 `ErrBadParam`, 3030 `ErrBadConfig`, 52 `ErrBadCommand`. "Local DC" = the result of `0x140122B00` (IDA name `GeoIP_num_countries_18` is wrong; it returns the datacenter id of this server ❓). "Master" = `dword_1401DE148`. "Forward to master" = copy the request (`0x1400EE930`), set the route `id` = master DC with mode `random` (`0x1400E4E80`, `0x1400F6540`) and send it (`0x140122C40`) with the request source (`a2+4`); the handler returns 1 and does not reply itself.

All DB calls use schema `v2015062301` and the framework async send `0x1401010C0(req, obj, 25000, 0)` (timeout 25000 ms).

**`NameModify` `0x1400C9C70` (ids 1, 4, 5, 6, 10)**
1. Master is 0 → `{?, 2, 802}` (code not read ❓).
2. `DisableLocalName` (`dword_1401DE160`) ≠ 0 → `{3030, 2, 807}`.
3. Id 4: calls request vslot +104 ❓.
4. Request body (vslot +96) missing → `{10, 2, 818}`.
5. `UserDatacenter` of the request (`0x1400CAD00`) is 0 → `{4, 2, 824}`.
6. User DC = local DC (local path, two-phase session):
   - If `DisableLocalNameDb` (`dword_1401DE164`) ≠ 0: make a session object (`0x1400C69E0`, type 1, the id, built-in record `xmmword_140166580`) and complete it at once (no `LocalNameDb` call) ❓.
   - Else read `Names/Name` items. For each: parse `NameType`/`NameValue` (`0x1400E45C0`, fail → `{10, 2, 863}`), build `SessionData` (`0x1400CAB70`, fail → `{10, 2, 869}`), write `Name{NameType, NameValue, SessionData}` to the XML. Missing `Names`/`Name` → `{10, 2, 848}`/`{10, 2, 853}` ❓ (order not checked).
   - `SessionEffectiveUntil` is 0 → `{3030, 2, 885}`. Make a new session id (`0x1400D2FB0`); fail → `{4, 2, 891}`.
   - `LocalNameDb.CreateSession(session id, xml, SessionEffectiveUntil)` (`0x1400CA203`); reply object from `0x1400C69E0`. The reply path (`0x1400C7730`) sends the change to `GlobalNameDb` on the master, or forwards it to the master DC, and replies with `StsRouteReply` ❓.
7. User DC ≠ local DC: `++dword_1401DE0D8`. If local DC is master: build the names param (`0x1400CA5E0`, fail → `{10, 2, 923}`), call `GlobalNameDb.<command name>` (proc name = `0x1400E43D0(id)`, the command name of the id, for example `AddNames`), reply object `CModifyNamesTxn` (type 4, id). Else → `{4, 2, 946}`.

**`NameBanWord` `0x1400C98F0` (ids 2, 9)**
1. Master is 0 → `{3030, 2, 1164}`. Body missing → `{10, 2, 1170}`.
2. `NameType` not in the type map → `{10, 2, 1176}`. `BanWord` empty → `{10, 2, 1182}`.
3. Local DC ≠ master: `++dword_1401DE0D8`, forward to master, return 1.
4. Else `GlobalNameDb.AddBanWord` (id 2) or `RemoveBanWord` (id 9) with (type id, ban word); other id → `{52, 2, 1194}`. Reply object 8 bytes (`ModifyBanWordTxn` ❓).

**`NameAddUser` `0x1400C7CC0` (id 3)**
1. Body (vslot +96); names param `0x1400CA5E0`; fail → `{10, 2, 956}` or `{10, 2, 962}` ❓ (which is which not read).
2. `LocalNameDb.AddUser(param)` (`0x1400C7E0D`), reply object `CModifyNamesTxn` (type 4, built-in record `xmmword_140166580`).

**`NameCheck` `0x1400C7ED0` (id 7)**
1. Master is 0 → `{3030, 2, 988}`. Local DC ≠ master → forward to master.
2. Body missing → `{10, 2, 995}`; `Names` missing → `{10, 2, 1001}`; `Name` missing → `{10, 2, 1006}` ❓ (from line order).
3. For each `Name`: `NameType` must be in the type map and `NameValue` not empty, else stop the loop. Read `Administrator`, `Preemptive`. Write `Name{NameType, NameValue, PendingSecs (type +72, only if ≠ 0), Administrator (if ≠ 0), Preemptive (if set)}`.
4. `GlobalNameDb.CheckNames(xml)` (`0x1400C82D2`); reply object 8 bytes (`CheckNamesTxn` ❓). Proc begin fail → `{10, 2, 1020}` ❓.

**`NameGetInfo` `0x1400C8FD0` (id 8)**
1. Master is 0 → `{3030, 2, 1082}`. Local DC ≠ master → forward to master.
2. Body missing → `{10, 2, 1089}`; `Names`/`Name` missing → `{10, 2, 1095}`/`{10, 2, 1100}` ❓.
3. For each `Name` that parses (`0x1400E45C0`) and has a known type: write `Name{NameType, NameValue}`.
4. `GlobalNameDb.GetNamesInfo(xml)` (`0x1400C92D0`); reply object 8 bytes. Fail → `{10, 2, 1112}` ❓.

**`NameExec` `0x1400CC6E0` (id 11)**
1. Master is 0 → `{3030, 2, 2921}`. Body missing → `{10, 2, 2927}`. `ProcName` empty → `{10, 2, 2933}`. `Params` missing → `{10, 2, 2939}`. `UserDatacenter` is 0 → `{4, 2, 2945}`.
2. User DC = local DC: if `DisableLocalName` or `DisableLocalNameDb` is set, end the `CExecTxn` (type 3) at once with record line 2959 ❓ and return. Else `++dword_1401DE090`, database `LocalNameDb`, txn type 3.
3. User DC ≠ local DC: `++dword_1401DE0D8`. Local DC ≠ master → `{4, 2, 2980}`. Else database `GlobalNameDb`, txn type 4.
4. Look up `ProcName` in the `ProcNames` map; not found → `{3030, 2, 2990}`. The client does not choose the proc name. The proc is: `GetIdById` if the entry type id is 0; `GetIdByName` if type id ≠ 0 and flag = 0; `GetNameById` if type id ≠ 0 and flag ≠ 0. Parameters: type id (int) if ≠ 0, else an empty string ❓ (`WindowName` reference); then the `Params` text (`0x1400C8750`).
5. Send with reply object `CExecTxn` (type 3 or 4), timeout 25000 ms.

#### 4.3.1 Session reply handlers (not complete ❓)

- `0x1400C7730` (called from `0x1400CEE20`): after `CreateSession`; if local DC is master, calls `GlobalNameDb`, else forwards to master (`id`, `random`). Replies with `StsRouteReply`. Error lines 1727, 1737, 1744, 1775.
- `0x1400CB830` (called from `0x1400CEE50`): reads the reply `Names/Name`; builds `Request/Names (Type=array)`; uses `NewUserCenter`; sends to another DC or calls `LocalNameDb.DeleteSession` (`0x1400CBEA2`). On each error path it sets the recovery timer `off_1401DE0F8` to `SessionRecoveryInterval`.
- `0x1400C9470` (from `0x1400CC000`): change of user datacenter (`NewUserDatacenter`), error line 1881.
- `0x1400CC200`, `0x1400CF910`: read/write a session record `ReplyType`, `Command`, `SessionId`, `Names`. Error lines 1637–1659.

#### 4.3.2 Worked example (CheckNames, id 7)

Request on the master DC, `Name.ini` has `NameTypes: CharName = 1, 300`:
`<Request><Names><Name><NameType>1</NameType><NameValue>Hero</NameValue></Name></Names></Request>`.
`NameCheck` writes `<Names><Name NameType=1 NameValue="Hero" PendingSecs=300/></Names>` and calls `GlobalNameDb.CheckNames` (`v2015062301`). The reply object returns the DB result. On a non-master DC the same request goes to the master DC unchanged. If `NameType` is 7 (not configured), the loop stops at that name ❓ (empty list sent).

### 4.4 DB procs (from `dbprocs()`, 22 call sites)

`LocalNameDb`: `AddUser` `0x1400C7E0D`, `ChangeUser` `0x1400C74BC`/`0x1400C757F`, `ChangeSessionStatus` `0x1400C86FC`, `CreateSession` `0x1400CA203`, `DeleteSession` `0x1400CBEA2`/`0x1400CE69A`, `DeleteCompletedSessions` `0x1400CEF75`, `GetExpiredSession` `0x1400CF057`, `ListErrorSessions` `0x1400C5ECB`, `ListExpiredSessions` `0x1400C601B`. `GlobalNameDb`: `CheckNames` `0x1400C82D2`, `GetNamesInfo` `0x1400C92D0`. `DeleteExpiredReservedNames` `0x1400CF265` and `DeleteExpiredPendingNames` `0x1400CF2F5` are sent by `0x1400CF1F0(db, schema)` with `StsRouteSend` (no reply object); `0x1400CEEC0` calls it with `GlobalNameDb` if local DC is master, else with `LocalNameDb`. The int parameter is probably `DeleteExpiredNamesCounts` ❓. Computed proc names: `0x1400CCC36` ❓; `0x1400C7972`, `0x1400CE200`, `0x1400C9AF7`, `0x1400CA4B1` (Global) and `0x1400CBE4C`, `0x1400C9876` (Local) use `0x1400E43D0(id)` = command name of the id (`AddNames`, `RemoveNames`, `CheckNames`, `ChangeNames`, `ChangeStatus`, `ChangeUser`, `AddUser`, …; cases 1–11) ❓ (per-site value not read). `NameExec` calls `GetIdById`/`GetIdByName`/`GetNameById` (§4.3) in `LocalNameDb` or `GlobalNameDb`.

### 4.5 Timers (from `timers()`)

| Timer object | OnTimer | Period | Set at |
| --- | --- | --- | --- |
| `off_1401DE0F8` | `0x1400CEFE0`: `LocalNameDb.GetExpiredSession`, reply object `CRecoverSessionTxn` (sent with `SendToProviderTxn`) | `dword_1401DE150` = `SessionRecoveryInterval` (verified, config object +16); 0 at `0x1400CDC5B`, `0x1400CEE97` | `0x1400CE721`, `0x1400CADFB`, `0x1400CB172`, `0x1400CB673`, `0x1400CB88B`, `0x1400CB937`, `0x1400CBD45`, `0x1400CBF58` (error paths of the session reply handlers) |
| `off_1401DE108` | `0x1400CEEC0`: `DeleteExpiredReservedNames` + `DeleteExpiredPendingNames` (Global on master, else Local), then `LocalNameDb.DeleteCompletedSessions` (`StsRouteSend`, no reply) | 0 at start; later period `DeleteExpiredNamesInterval` ❓ | `0x1400CDC6C` |

State: counters `dword_1401DE090` (local Exec calls) and `dword_1401DE0D8` (calls that go to the master / global path). No in-memory name cache: all name state is in the two DBs.

### 4.6 Calls out

- To the NameSrv of the master DC: `CheckNames`, `GetNamesInfo`, `AddBanWord`, `RemoveBanWord` (when this DC is not master) and session steps (`0x1400C7730`, `0x1400CB830`), route `id` = DC id, mode `random`.
- No calls to other service types.

## Framework gaps

Facts that `00-framework.md` does not give. Addresses are per binary (FriendSrv / PresenceSrv / NameSrv / ProfileSrv).

| Fact | Address | Note |
| --- | --- | --- |
| Route key alias registration: key type → `$<Service>.<Proc>` (for example `userid` → `$Name.GetIdByUserId`, `gacct` → `$GameAccount.GetUserIdByAcct`); the username alias is added only when `Auth.ini [UserName] UniqueProperty` ≠ 0 (default 1); lookup fail error 3002 `ErrAccountNotFound` | FriendSrv `0x14015AF30`, PresenceSrv `0x140136B80` | same form as RoleSrv (`06` §3.1) |
| Provider notify object registration (one object for each service name whose events the service gets) | FriendSrv `0x14015B670`, PresenceSrv `0x1401372C0` | |
| Async DB/service send with a reply object and a timeout of 25000 ms | FriendSrv `0x140114340`, ProfileSrv `0x1400FC320(req, obj, 0x7FFFFFFF, 0, 25000, 0)`, NameSrv `0x1401010C0(req, obj, 25000, 0)` | |
| Service name with a timeout of 60000 ms, called at start | ProfileSrv `0x1400FBE90("Profile", 60000)`, NameSrv `0x140100CB0("Name", 60000)` | meaning of the timeout ❓ |
| Transaction stats name registration | NameSrv `0x1401010F0(name, obj)` | names `ModifyNamesTxn`, `CheckNamesTxn`, `GetNameInfoTxn`, `ModifyBanWordTxn`, `ExecTxn` |
| Local datacenter id | NameSrv `0x140122B00` | IDA lib name `GeoIP_num_countries_18` is a wrong FLIRT match |
| Forward a request to another DC: copy (`0x1400EE930`), route `id` + `random` (`0x1400E4E80`, `0x1400F6540`), send (`0x140122C40`) | NameSrv | |
| Fire-and-forget DB call (no reply object) with `StsRouteSend` after `DbProcBegin` | NameSrv `0x1400CF1F0`, `0x1400CEEC0` | |

## Differences from existing specs

- `Docs/Platform/Sts/10-stservers-original.md` "Databases" lists only `GlobalNameDb` for NameSrv. NameSrv also uses `LocalNameDb` (same schema `v2015062301`) for sessions, `AddUser`, `ChangeUser` and local `Exec` (§4.4).
- The same file puts FriendDb under "Others … `dbo` or per-exe pin". FriendSrv pins schema `v2015050401` (§1.6); ProfileSrv pins `ProfileDb` `v2016082501` (§3).
- `Docs/Platform/StServers/16-play-path.md` says PresenceSrv is not analysed. §2 now gives its 27 commands; `Login` and the other ids are in §2.4 and `08-commands.tsv`.
- `Docs/Platform/StServers/13-routesrv.md` lists the forward scope as needing NameSrv. NameSrv forwards to the master DC itself (§4.6); it does not use the RouteSrv forward scope ❓.
