# 07 — ExAuthSrv, ExAuthnSrv, IpCheckSrv (RE reference)

Summary:
1. IpCheckSrv (type 1015) keeps one grid object `CIpData` per IP address. It answers the `IpCheck` protocol (22 ids, 11 with a handler): connection flood check, black/white lists and a per-IP request limit. Data is in `MasterDb` (schema `v2015102301`). See §3.
2. ExAuthSrv (type 1036, `ExAuth.ini`) links external provider accounts to user ids in `AssociationAcctDb` (`v2016042101`). It serves 20 `ExAuth` ids and runs a 28-type txn state machine (`CExAuthTxn`). See §1.
3. ExAuthnSrv (type 1118, `ExAuthn.ini`) serves 2 `ExAuthn` ids (AuthenticateToken, RequestToken) and checks provider tokens over HTTP(S) with one txn class per provider. See §2.
4. All three link the framework of `00-framework.md`. ExAuthnSrv also links the member library (`09-member-grade-level.md` §0). This chapter gives only the service code.
5. Long lists: `07-commands.tsv`, `07-procs.tsv`.

Sources: the `.i64` copies in `ida/lanes/st/` (build 381). All addresses are in the binary of the section. Error records are `{code, module, line}` (framework 3.1); code names are from `errcodes.tsv`. ❓ = not verified in IDA.

## Quick find

| Question | Function | Binary, address | Section |
| --- | --- | --- | --- |
| IpCheckSrv start (type 1015, grid, INI) | `IpCheckAppInit` | IpCheckSrv `0x1400C8C20` | 3.1 |
| IpCheckSrv `IpCheck.ini` keys | `IpCheckConfigLoad` | IpCheckSrv `0x1400C8CB0` | 3.2 |
| IpCheckSrv message entry (dirty → reload) | `IpCheckOnMessage` | IpCheckSrv `0x1400C8E50` | 3.3 |
| IpCheckSrv command switch | `IpDataDispatch` | IpCheckSrv `0x1400C86A0` | 3.3 |
| Flood check (token bucket) | `RateCharge` | IpCheckSrv `0x1400ED140` | 3.5 |
| Request limit algorithm | `IpLimitHit` | IpCheckSrv `0x1400C8A60` | 3.6 |
| IP DB txn reply switch | `CIpTxn` slot 4 | IpCheckSrv `0x1400CB0F0` | 3.7 |
| SetDirty flush timer | `CIpData` timer slot 0 | IpCheckSrv `0x1400C7DE0` | 3.8 |

| ExAuthSrv command switch | `ExAuthDispatch` | ExAuthSrv `0x1400D0B80` | 1.2 |
| ExAuthSrv provider code → option object | `ProviderFind` | ExAuthSrv `0x1400CA1B0` | 1.2 |
| ExAuthSrv txn type switch | `CExAuthTxn` slot 1 | ExAuthSrv `0x1400D3210` | 1.3 |
| ExAuthSrv request-id continuation | `CExAuthTxn` slot 6 | ExAuthSrv `0x1400D2360` | 1.3 |
| ExAuthSrv write procs (vtable slots) | `CExAuthTxnData` `+0x108`…`+0x138` | ExAuthSrv `0x1401734D0` | 1.4 |
| ExAuthnSrv command switch | `CExAuthnSocketNotify` slot 0 | ExAuthnSrv `0x1400DE650` | 2.2 |
| ExAuthnSrv provider factory | provider switch | ExAuthnSrv `0x1400DD860` | 2.2 |
| ExAuthnSrv reply builder | `Reply {ExternalAccountKey, …}` | ExAuthnSrv `0x1400DE0F0` | 2.2 |

## 1. ExAuthSrv

### 1.1 Role, start and config

ExAuthSrv links external accounts (provider accounts such as steam, google, apple, guest) to NC user ids. It keeps the links in `AssociationAcctDb`. It sends the provider token check to ExAuthnSrv (`ExAuthn.AuthenticateToken`) and asks other services for names, tokens, bans and member links (§1.6).

App init `0x1400D65B0`:
1. Seeds the random generator with `timeGetTime() ^ x ^ GetCurrentProcessId()`.
2. `StsSetServerType(1036, 0)` at `0x1400D65E6`.
3. Registers the service map `ExAuth` (`unknown_libname_6_0("ExAuth", 1, …)`).
4. Registers route key aliases with the StsRoute function `0x140123D80(service, 0, key, alias, 3002 ErrAccountNotFound, …)` (StsRoute.cpp): `ExAuth`/`userid` → `$Name.GetIdByUserId` (in init), then in `0x1400D67A0` one alias per provider code: `facebook`, `tencent`, `tencent_lin`, `innova`, `shanda`, `steam`, `gamania_lin`, `google`, `guest`, `apple`, `garena` → `$Name.GetIdBy<Provider>`; `daum`, `tooniland` → `$ExAuth.GetIdByDaum` / `$ExAuth.GetIdByTooniland` (these two are served by ExAuthSrv itself through `Exec`, §1.2 id 11). The aliases are registered for the services `Auth` and `ExAuth`.
5. `0x1400C5000`: INI file name `ExAuth.ini`. `0x1400CE770`: fills a table of 40 country strings (`%d`) ❓ use. `0x1400D51E0`: HTTP test pages (`StsHttpAddCommands(…, 10, 1)`): `/test/`, `/test/authenticate_token`, `/test/associate_my_external_account`, `/test/associate_external_account`, `/test/dissociate_external_account`, `/test/page_external_account_changes`, `/test/delete_user`, `/test/get_external_account_info`, `/test/get_external_accounts`, `/test/link_my_external_game_account`.
6. Main loop `0x140127100(0)`.

Config object `CExAuthConfig` (vtable `0x1401718E0`, load `0x1400C56E0`). Provider sections are read by provider option objects (vtable slot 1 at `+8`): `FacebookOptions`, `InnovaOptions`, `ShandaOptions`, `SteamOptions`, `TencentOptions`, `TencentLinOptions`, `DaumOptions`, `ToonilandOptions`, `GamaniaLinOptions`, `GoogleOptions`, `AppleGameCenterOptions`, `GarenaOptions`.

- `StsSetServerType` is called once, at `0x1400D65E6` (type 1036).
- Service map `ExAuth` `0x1402574D0` (20 ids). Other service maps in the binary: provider codes `0x1402574F0` (1 facebook, 2 tencent, 3 tencent_lin, 4 innova, 5 shanda, 6 steam, 7 daum, 8 tooniland, 9 gamania_lin, 10 google, 11 guest, 12 apple, 13 global, 14 ncsoft, 15 garena), `ExFriend` ❓ `0x140257528` (7: PageExFriends … Flush; no handler found), `0x140257508` (0 Invalid, 1 AuthenticateToken, 2 RequestToken). The other maps are client maps of other protocols (Auth, AppAccess, Token, IpCheck `0x1402575B8`, …) ❓ (use not checked).
- Config keys (`cfgkeys()`, service functions; file `ExAuth.ini`). Offsets are in `CExAuthConfig`: `+12` RetryLimit, `+16` RetryIntervalMs, `+20` CountryCode, `+24` UseMemberSrv (bool), `+48` AllServiceAppGroupId. `EventLog/UseTimeout` = 0 sets a flag ❓ (use not read). "(per provider)" = section `<Provider>Options`:

| Section | Key | Default | Reader |
| --- | --- | --- | --- |
| `General` | `DefaultUserName` | `Nickname` | `0x1400C56E0` |
| `General` | `RetryLimit` | 10 | `0x1400C56E0` |
| `General` | `RetryIntervalMs` | 100 | `0x1400C56E0` |
| `General` | `CountryCode` | 0 | `0x1400C56E0` |
| `General` | `UseMemberSrv` | 0 | `0x1400C56E0` |
| `General` | `AllServiceAppGroupId` | 0 | `0x1400C56E0` |
| `EventLog` | `UseTimeout` | 0 | `0x1400C56E0` |
| (per provider) | `NameQueryGameCode` | `unk_140171968` ❓ (string not read) | `0x1400C55E0` |
| (per provider) | `AssociationAlwaysExist` | 0 | `0x1400C55E0`, `0x1400C51C0`, `0x1400C5190` |
| `FacebookOptions` | `AppGroupCodes` | none | `0x1400C51C0` |
| `GlobalPreferredServices`, `InstantGlobalForwards` | `Service` (list) | — | `0x14011FAE0` (framework-like ❓) |

Heartbeat keys (`0x14010F670`) and `BlockMask`/`AlwaysReparse` (`0x1400FE9B0`) are as in `09` §0.3/§0.5.

### 1.2 Dispatch

`ExAuthDispatch` `0x1400D0B80`: switch on the request id (`*a1`). Default: reply `{52 ErrBadCommand, server 0, module 1, line 3917}`, log `ExAuthConn: invalid command request`. Handlers (`ExAuthConn.cpp`):

Request fields (offsets in the parsed request `a1`): `+0` request id, `+4` route connection id (reply target), `+660` UserId (64-bit, also `a1[165..166]`), `+680` second UserId (`a1[169..170]`, used by id 16), `+704` AuthProviderCode (string), `+712` AuthnToken, `+720` AppId, `+728`…`+784` external account fields (Key `+736`, Name `+744`, then data fields copied into the txn by vtable slots 120…208), `+800` exGameAccountKey (Exec), `+820` page parameters (id 19). Field names come from the reply/request strings; the parse function is framework (`00-framework.md` request parse) ❓ field-by-field map not decoded.

Errors are `{code, server 0, module 1, line}` (ExAuthConn.cpp). Codes: 10 `ErrBadParam`, 3327 `ErrAuthProviderNotFound` (reply by `StsRouteReply` when `ProviderFind` `0x1400CA1B0` returns 0). `ProviderFind(code, connId, reqId, appId, …)` is a switch on the provider code (1–15, §1.1 map `0x1402574F0`) that returns the provider option object of `CExAuthConfig` or 0.

| Id | Command | Handler | Checks in order (code@line) | Then |
| --- | --- | --- | --- | --- |
| 1 | AuthenticateToken | `0x1400CB9B0` | AuthProviderCode missing 10; provider is `plaync` 10@4016; AuthnToken missing 10@4021; AppId missing 10@4026; ProviderFind 3327@4039 | build `ExAuthn.AuthenticateToken {AuthProviderCode, AuthnToken}` (`0x1400E6740`→`0x1400E67A0`), new `CAuthenticateTokenTxn` (`0x1400C5B30`, 0x428 bytes, keeps token and provider), `SendToProviderTxn(msg, txn, 25000 ms, 0)` |
| 2 | AuthenticateTokenInternal | `0x1400CBCC0` | external account key missing 10@4062; ProviderFind 3327 | copies account fields into a txn data object. If `+776` (name) is set: name query (`0x1400C87C0`, `Names[] {Name, NameType}`) with txn type 4. Else: proc `ListExGameAccountsByUserId(UserId)`, txn type 9 |
| 3 | AssociateExternalAccount | `0x1400CAF60` | ProviderFind 3327@4124; UserId 10@4129; ExternalAccountKey 10@4134; ExternalAccountName 10@4139; AppId 10@4144; ProviderFind 3327@4165 (line 4148 is the default error record) | the provider object gets ExternalAccountKey `+736` (slot `+120`), ExternalAccountName `+744` (slot `+128`), ExternalAccountData `+752` (slot `+136`); if `+760` is not empty, slot `+176` (`0x1400D4350`); then slot `+32` `0x1400CE8F0`: sends `AppAuth.GetAppGroupCode {AppId}` (`0x1400E4730`) with a new txn type 0. Not a forward: `0x1400C6600` only copies the 3327 error record |
| 4 | AssociateMyExternalAccount | `0x1400CB650` | ProviderFind 3327@4177; provider `guest` 10@4182; UserId 10@4187; AuthnToken 10@4192; AppId 10@4197; ProviderFind 3327@4211 | as id 1 (`CAuthenticateTokenTxn` → ExAuthn) |
| 5, 6 | Associate(My)ExternalAccountInternal | `0x1400CB280` | AuthProviderCode 3327@4234; UserId 10@4239; ExternalAccountKey 10@4244; AppGroupCode 10@4249; ProviderFind 3327@4264 | txn type 5 |
| 7 | GetExternalAccountInfo | `0x1400CCE60` | UserId 10@4291 | proc `ExAcctGetExAcctByUserIdAndAuthProviderCode` (code given) or `ExAcctGetExAcctByUserId`; txn type 6 (`0x1400CD0C0` = the AppId path: `AppAuth.GetAppGroupCode`, txn type 9) |
| 8 | DissociateExternalAccount | `0x1400CC250` | UserId 10@4336; AuthProviderCode 10@4341; AppId 10@4346 | sends `ExAuth.DissociateExternalAccountInternal {…, AppId}` (`0x1400E5700`) to the user's own ExAuth route |
| 9 | DissociateExternalAccountInternal | `0x1400CC4C0` | UserId 10@4370; AuthProviderCode 10@4375; AppId 10@4380; ProviderFind 3327@4394 | sends `AppAuth.GetAppGroupCode {AppId}` (`0x1400E4730`) |
| 10 | GetExternalAccounts | `0x1400CD380` | ExternalAccountName missing 10@4415; name invalid 10@4428; AuthProviderCode invalid 10@4458/4462 | input array `ExternalAccountNames[] {ExternalAccountName}`; proc `ExAcctGetExAcctsByExAcctNameAndAuthProviderCode` or `ExAcctGetExAcctsByExAcctName`; txn type 19 |
| 11 | Exec | `0x1400CC780` | ProcName missing 10@3936; Param missing 10@3941; ProcName not in list 10@3956 (`invalid procName(%s)`); Param invalid 10@3966/3994; exGameAccountKey invalid 10@3987 | allowed ProcName: `GetIdByDaum`, `GetIdByTooniland`, `GetIdByFacebook`, `GetIdByGoogle` (→ provider lookup `0x1400E6660`), `GetLastLinkedIdByExGameAccountKey` (proc of the same name). This is the target of the `$ExAuth.GetIdBy*` route aliases |
| 12 | RequestGuestToken | `0x1400CE290` | AppId missing 3327@4487 | sends `Token.Instant.RequestToken {id, random, AppId, EffectiveUntilSecs}` (`0x1400EAB00`); txn type 7 |
| 13 | RequestExAccessToken | `0x1400CDE60` | UserId 10@4510; AppId 10@4515; AuthProviderCode 10@4520 | sends `ExAuth.RequestExAccessTokenInternal` (`0x1400E5CA0`) |
| 14 | RequestExAccessTokenInternal | `0x1400CE0D0` | UserId 10@4540; AppId 10@4545; AuthProviderCode 10@4550; ProviderFind 3327@4564 | ❓ next step not read |
| 15 | UpdateExAccessToken | `0x1400CE410` | UserId 10@4574; AppId 10@4579; AuthProviderCode 10@4584; ProviderFind 3327@4598 | as id 1 (`CAuthenticateTokenTxn` → ExAuthn) |
| 16 | UpdateExAccessTokenInternal | `0x1400CE6C0` | the two UserIds differ 10@4621 (`userId mismatched`) | calls id 2 handler `0x1400CBCC0` |
| 17 | DeleteUser | `0x1400CC120` | none | proc `ExAcctDeleteExAcct(UserId)`; txn type 21 |
| 18 | DeleteLocalUser | `0x1400CC000` | none | proc `DeleteLinkGameAccounts(UserId)`; small txn (0x30 bytes) ❓ class |
| 19 | PageExternalAccountChanges | `0x1400CDC50` | parameter missing 10@4680; PageSize > 20 10@4683 (`page size is over max`) | proc `ExAcctListExAcctChanges` (params: 64-bit, int, int via `0x1400E4950`); txn type 22 |
| 20 | LinkMyExternalGameAccount | `0x1400CD8D0` | AuthProviderCode 3327@4711; provider `guest` 10@4716; UserId 10@4721; AuthnToken 10@4726; AppId 10@4731; ProviderFind 3327@4745 | as id 1 (`CAuthenticateTokenTxn` → ExAuthn) |

Helpers: `0x1400DA9A0` = `operator new` (hypothesis of stage 1 is now verified: it allocates the txn objects); `0x1400C5BE0(obj, type, connId, data, 0)` = `CExAuthTxn` constructor (vtables `0x140173250` / `0x140173288`, type at `+40`, data at `+48`, connId at `+56`); `0x1400E47B0` = `DbProcBegin(msg, …, "AssociationAcctDb", "v2016042101", proc, …)`; param writers `0x1400E4900` (64-bit), `0x1400E4920` (string), `0x1400E4950` (int); end `0x1400E4860`.

### 1.3 Txn steps (`CExAuthTxn`)

`CAuthenticateTokenTxn` (vtable `0x140173310`): result slot `0x1400D1730` replies 3301 `ErrTokenNotFound` on a missing token, else continues into `CExAuthTxn` slot 6 `0x1400D2360`. `0x1400D2360` switches on the **request id** (1, 3, 4, 7, 9, 14, 15, 20 start a next step; 2, 5, 6, 8, 10–13, 16–19 reply at once with `StsRouteReply`). Its errors: 3734 `ErrAppGroupNotFound`@3215/3219 (no `AppGroupCode`), 3375 ❓ name not in `errcodes.tsv`. Next steps: id 1 → re-send to provider (`SendToProviderTxn(…, 1)`); id 7 → proc `ListExGameAccountsByUserId`, txn type 9 (copies `+60`/`+68`); id 14 → proc `GetExternalTokenData`; id 20 → txn type 20; other branches create types 16, 27.

`CExAuthTxn` slot 1 `0x1400D3210` switches on the txn type (`+40`). Type → step function (what it reads / sends / errors code@line):

| Type | Step | Reads / sends | Errors |
| --- | --- | --- | --- |
| 0, 1 | `0x1400C7760` | `UserId`; builds `%s@plaync.com`; log `not your access token` | — |
| 2 | `0x1400C6BC0` | reply or re-send | — |
| 3 | `0x1400D3D10` | on 3044 `ErrInProgress` and retry count `+268` < RetryLimit: `StsTimerSet(+24, RetryIntervalMs)` (retry); else `Auth.ChangeUserName {UserNameOld, …, "null password"}` (`0x1400E41A0`) | 1905@? ❓ |
| 4 | `0x1400C95C0` | name query result → next send | — |
| 5 | `0x1400D0030` | `Names[] {Name}`, `UserCenter`, `UserId`; sends `ExAuth.GetExternalAccountInfo {AuthProviderCode}` (`0x1400E58C0`) | 3003 `ErrAccountExists`@2021 |
| 6 | `0x1400CF250` | reply `{UserId, AuthProviderCode, ExternalAccountKey, ExternalAccountName, ExternalAccountData, Created}` | — |
| 7 | `0x1400D40A0` | reply `{TokenId, AuthnToken}` (guest token) | — |
| 8 | `0x1400CFC40` | `AppGroupCode`, `AuthProviderCode` | 7606 `ErrCannotDissociate`@2559 |
| 9 | `0x1400D0E40` | `AuthProviderCode`, `AppGroupCode`, `ExternalUserAppKey`; proc `ExAcctGetExAcctByUserId` | 4 `ErrBadData`@2348, 0@2324 |
| 10 | `0x1400D4A00` | proc `ListExGameAccountsByUserId` | — |
| 11 | `0x1400C70E0` | reply or send | — |
| 12 | `0x1400C6E40` | `created`, `guest` | — |
| 13 | `0x1400CEC60` | `ExternalAccounts[] {AuthProviderCode, ExternalAccountKey}` | 4@2581, 4@2587 |
| 14 | `0x1400CABD0` | sends `ExFriend.RemoveExFriends {AuthProviderCode, …}` (`0x1400E6970`) for non-guest | — |
| 15 | `0x1400CAD40` | `AuthProviderCode`, `ExternalAccountKey` | — |
| 16 | `0x1400D0680` | `IsLoginNameValidated`; proc `ListExGameAccountsByUserId` | 4@2708, 3003@2723 |
| 17 | `0x1400C9B60` | `UserId`, `UserCenter`; sends `Auth.GetUserInfo` (`0x1400E4520`) | — |
| 18 | `0x1400CA690` | reply or send | — |
| 19 | `0x1400CF7A0` | reply `ExternalAccounts[] {UserId, UserDatacenter, AuthProviderCode, ExternalAccountKey, ExternalAccountName, ExternalAccountData}` | — |
| 20 | `0x1400D4880` | reply / send | — |
| 21 | `0x1400CA8A0` | DeleteUserReply: log `Account deleted. userId(%s), authProviderCode(%s), exAcctKey(%s)`; next send | — |
| 22 | `0x1400D35A0` | reply `{PageSize, PageIndex, TotalCount, TotalPages, ExternalAccounts[]}` from `Set/Row` | 4@2834, 4@2840 |
| 23 | `0x1400D0430` | `UserId`, `UserStatus`; sends `Access.GetBans {AppGroupId}` (`0x1400E4650`) | 4@2921, 4@2937 |
| 24 | `0x1400CEB10` | `Count`; sends `Member.CheckLink {UserId}` (`0x1400E6CB0`) | 4@2986 |
| 25 | `0x1400C9740` | `LoginName`; sends `ExAuth.AssociateMyExternalAccountInternal` (`0x1400E5050`) | — |
| 26 | `0x1400C7240` | `linked`; log `Silent association for login. %s` | 4@3071 |
| 27 | `0x1400C7450` | reply / send | 4@3109 |

Creators of types. New txn (`0x1400C5BE0` or inline): 4, 9 id 2; 5 id 5/6; 6 id 7; 9 `0x1400CD0C0`; 19 id 10; 21 id 17; 22 id 19; 7 id 12; 0, 9, 16, 20, 27 `0x1400D2360`; 0 `CExAuthTxnData` slot `+32` `0x1400CE8F0` (id 3 and other associate paths, after `AppAuth.GetAppGroupCode`); 17 `0x1400C9A60` (called by types 0/1 and 17). In-place change (the step writes the next type to `+40` and sends the next request with the same txn):

| From step (type) | Sets type |
| --- | --- |
| `0x1400C7760` (0) | 1 |
| `0x1400C6BC0` (2) | 11 |
| `0x1400C95C0` (4) | 10 |
| `0x1400D0030` (5) | 12 or 13 |
| `0x1400CFC40` (8) | 14 |
| `0x1400D0E40` (9) | 6 |
| `0x1400D4A00` (10) | 9 |
| `0x1400C70E0` (11) | 12 |
| `0x1400C6E40` (12) | 14 |
| `0x1400CEC60` (13) | 2 |
| `0x1400CABD0` (14) | 15 |
| `0x1400CAD40` (15) | 3 |
| `0x1400D0680` (16) | 8 |
| `0x1400C9B60` (17) | 18 or 23 |
| `0x1400CA690` (18) | 2 |
| `0x1400CA8A0` (21) | 3 |
| `0x1400D0430` (23) | 24 |
| `0x1400CEB10` (24) | 25 |
| `0x1400C9740` (25) | 26 |

`CExAuthTxnData` (vtable `0x1401734D0`; provider subclasses such as `CFacebookTxnData` `0x140173650`, `CTencentTxnData` `0x1401737D0` use the same slots): `+8` `0x1400D13C0`, `+32` StartAssociate `0x1400CE8F0`, `+40` `0x1400CEFE0`, `+48` `0x1400CEFB0`, `+120`/`+128`/`+136` set key/name/data, `+176` `0x1400D4350`, `+264`…`+312` write procs (§1.4).

### 1.4 DB procs (`AssociationAcctDb`, schema `v2016042101`, `dbprocs()` 22 calls)

`ListExGameAccountsByUserId` (5 calls: `0x1400CBCC0`, `0x1400D2360`, `0x1400D0680`, `0x1400D4A00`, +1 ❓), `ExAcctGetExAcctByUserId` (`0x1400CCE60`, `0x1400D0E40`), `ExAcctGetExAcctByUserIdAndAuthProviderCode` (`0x1400CCE60`), `ExAcctGetExAcctsByExAcctNameAndAuthProviderCode`, `ExAcctGetExAcctsByExAcctName` (`0x1400CD380`), `GetLastLinkedIdByExGameAccountKey` (`0x1400CC780`), `ExAcctDeleteExAcct` (`0x1400CC120`), `DeleteLinkGameAccounts` (`0x1400CC000`), `ExAcctListExAcctChanges` (`0x1400CDC50`), `GetExternalTokenData` (`0x1400D2360`). The fifth `ListExGameAccountsByUserId` call is `0x1400CD0C0` (`0x1400CD2B7`).

Write procs. Each is a `CExAuthTxnData` vtable slot (same slot in all provider subclasses). The step functions call the slot. Parameter kinds are in call order (i64 = `0x1400E4900`, str = `0x1400E4920`); the parameter names are ❓ (the order of the request fields is not mapped).

| Proc | Wrapper | Slot | Params | Called from (step, txn type) |
| --- | --- | --- | --- | --- |
| `ExAcctAssociate` | `0x1400C8350` | `+0x108` | str, str, str, str, i64, str | `0x1400C6C9D` in `0x1400C6BC0` (2) |
| `ExAcctAssociateApp` | `0x1400C8180` | `+0x110` | i64, str, str, str, str | `0x1400C71E6` in `0x1400C70E0` (11); `0x1400D015F` in `0x1400D0030` (5); `0x1400D11C2` in `0x1400D0E40` (9) |
| `ExAcctDissociate` | `0x1400C8BA0` | `+0x118` | i64, str | `0x1400CAC60`, `0x1400CACDB` in `0x1400CABD0` (14) |
| `ExAcctDissociateApp` | `0x1400C8AF0` | `+0x120` | i64, str, str | `0x1400C6F75` in `0x1400C6E40` (12); `0x1400CFE26` in `0x1400CFC40` (8) |
| `ExAcctUpdateExAcct` | `0x1400C94F0` | `+0x128` | str, str, str, str | `0x1400D11E7` in `0x1400D0E40` (9) |
| `ExAcctUpdateExAcctByUserId` | `0x1400C9430` | `+0x130` | i64, str, str, str, str | `0x1400C966A` in `0x1400C95C0` (4) |
| `LinkExGameAccount` | `0x1400C9010` | `+0x138` | i64, str, str, str, str | `0x1400C7020` (12), `0x1400C738F` (26), `0x1400C7578` (27), `0x1400C7BF6` (0/1), `0x1400D2940` (`0x1400D2360`), `0x1400D493F` (20) |

### 1.5 State and timers

- No service cache. State is per txn only.
- Only service timer: txn type 3 retry (`0x1400D3D10`), period = `RetryIntervalMs` (default 100 ms), at most `RetryLimit` (default 10) tries, on reply 3044 `ErrInProgress`.
- Provider sends use a 25000 ms timeout.

### 1.6 Calls out

| Target | Command | Builder | When |
| --- | --- | --- | --- |
| ExAuthnSrv | `ExAuthn.AuthenticateToken {AuthProviderCode, AuthnToken}` | `0x1400E67A0` | ids 1, 4, 15, 20 |
| ExAuthSrv (route by user) | `ExAuth.DissociateExternalAccountInternal`, `RequestExAccessTokenInternal`, `GetExternalAccountInfo`, `AssociateMyExternalAccountInternal` | `0x1400E5700`, `0x1400E5CA0`, `0x1400E58C0`, `0x1400E5050` | ids 8, 13; types 5, 25 |
| AppAuthSrv | `AppAuth.GetAppGroupCode {AppId}` | `0x1400E4730` | id 9, `0x1400CD0C0`, `0x1400CE8F0` |
| TokenSrv | `Token.Instant.RequestToken` | `0x1400EAB00` | id 12 |
| AuthSrv | `Auth.GetUserInfo`, `Auth.ChangeUserName` | `0x1400E4520`, `0x1400E41A0` | types 17, 3 |
| AccessSrv | `Access.GetBans {AppGroupId}` | `0x1400E4650` | type 23 |
| MemberSrv | `Member.CheckLink {UserId}` | `0x1400E6CB0` | type 24 |
| ExFriend | `ExFriend.RemoveExFriends` | `0x1400E6970` | type 14 (this explains the client map `0x140257528`; it is a client map, not served) |
| NameSrv | name query `Names[] {Name, NameType}` | `0x1400C87C0` | id 2 |

### 1.7 Worked example (AuthenticateToken, id 1)

Request `{AuthProviderCode "steam", AuthnToken "<ticket>", AppId "<app>"}`. Checks pass; `ProviderFind("steam")` (code 6) returns the `SteamOptions` object. ExAuthSrv sends `ExAuthn.AuthenticateToken {AuthProviderCode "steam", AuthnToken "<ticket>"}` with a 25000 ms timeout. If ExAuthnSrv gives no token, the reply is `{3301 ErrTokenNotFound}`. If it succeeds, step `0x1400D2360` case 1 continues and the reply carries the linked account (`UserId`, `ExternalAccountKey`, …). With `AuthProviderCode "plaync"` the reply is `{10 ErrBadParam, module 1, line 4016}`.

## 2. ExAuthnSrv

### 2.1 Role, start and config

ExAuthnSrv checks a provider token (steam ticket, google token, …) against the external provider over HTTP(S) and returns the external account. Its only known caller is ExAuthSrv (`ExAuthn.AuthenticateToken`, §1.6). The member library code is in `09-member-grade-level.md` §0.

App init `0x1400DF1F0`:
1. `StsSetServerType(1118, 0)` at `0x1400DF1FB`.
2. Registers the service map `ExAuthn` (`unknown_libname_6_0("ExAuthn", 0, …)`): `0 Invalid`, `1 AuthenticateToken`, `2 RequestToken` (same map as ExAuthSrv `0x140257508`).
3. `StsRouteStart(0x1400DE0E0(), 0, 0)`: the route factory `off_1401FB028` (vtable `0x140173BB0`, slot 0 `0x1400DDBD0`) creates one `CExAuthnSocketNotify` (vtable `0x140173B68`) per connection.
4. `0x1400DEAE0`: HTTP test pages (`StsHttpAddCommands`): `/test/`, `/test/authenticate_token`, `/test/request_token`.
5. `0x1400DB070`: INI file name `ExAuthn.ini`. `0x14011AF40` is the HTTP client start (not the main loop): it starts the 1000 ms tick `off_14021DA20` (§2.3).

The maps that `table()` gives (`0x140260660` Auth with 80 ids, `0x1402606F8` 41 ids, `0x140260718` Token, `0x140260680` provider codes) are client maps built by static initializers (`0x1400D0DD0`, `0x1400D0ED0`). ExAuthnSrv does not serve them.

Config (`CExAuthnConfig`, vtable `0x140173178`, load `0x1400DBD80`): `HttpConn/Connections` = 1 (`+8`), `HttpConn/Timeout` = 10000 ms. Then one provider config object per section: `FacebookOptions`, `GoogleOptions`, `InnovaOptions`, `DaumOptions`, `ToonilandOptions`, `ShandaOptions`, `TencentOptions`, `TencentLinOptions`, `AppleGameCenterOptions`, `SteamOptions`, `GamaniaLinOptions`, `GarenaOptions` (config classes `CFacebookConfig` `0x1401730B8`, `CGoogleConfig` `0x1401730D0`, `CTencentConfig` `0x1401730A0`, `CInnovaConfig` `0x1401730E8`, `CShandaConfig` `0x140173100`, `CAppleGameCenterConfig` `0x140173118`, `CSteamConfig` `0x140173130`, `CGamaniaConfig` `0x140173148`, `CGarenaConfig` `0x140173160`). Keys in the provider readers (`cfgkeys()`; the section of each reader is ❓ not mapped):

| Reader | Keys (default) |
| --- | --- |
| `0x1400DB430` (common HTTP options) | `Enabled` (0), `Url` (empty), `ConfigPath` (2 reads), `ProxyServerEnabled` (0), `ProxyServerAddress`, `SslMethod` (`TLSv1`), `RequestProtocol` (`HTTPS`) |
| `0x1400DB790` | `HttpMethod` (`GET`), `UriPrefix` (`stage`) |
| `0x1400DB1B0` | `Domains` (list), `Expiration` (0) |
| `0x1400DBB60` | `WebApiKey` (value: `keyhere`), `ResponseType` (`json`) |
| `0x1400DB960` | `Certificate` (value: `keyhere`) |
| `0x1400DBC90` (Tencent net bar) | `UseNetBarLevel`, `NetBarSigType`, `NetBarEncodeType`, `IgnoreNetBarLevelError` (all 0) |

### 2.2 Dispatch and commands

`CExAuthnSocketNotify` slot 0 `0x1400DE650`: parses the message (`0x1401004F0`). Parse error → reply with the parse error. Id 1 → `0x1400DDC50`, id 2 → `0x1400DDEA0`, else reply `{52 ErrBadCommand, server 0, module 2, line 163}` (`StsRouteReply` = `0x14013D770`). Errors are `{code, 0, module 2, line}`.

| Id | Command | Handler | Request | Checks in order (code@line) |
| --- | --- | --- | --- | --- |
| 1 | AuthenticateToken | `0x1400DDC50` | `AuthProviderCode` (`+8`), `AuthnToken` (`+16`), `AppId` (16 bytes at `+40`), value at `+708` | AuthProviderCode missing → 3327 `ErrAuthProviderNotFound` (no line, `0x14011E510`); AuthnToken missing → 10 `ErrBadParam`@172; AppId all zero → 10@175; no provider object → 3327@184 |
| 2 | RequestToken | `0x1400DDEA0` | `AuthProviderCode`, `ExternalUserAppToken`, `AppId` | as id 1: 3327; 10@202; 10@205; 3327@214 |

After the checks both handlers call the provider factory `0x1400DD860(out, connId, reqId, providerCode, AppId)`. It switches on the provider code and returns a `shared_ptr` to a new provider txn data object (`C<Provider>TxnData`, base `CExAuthnTxnData` vtable `0x140173AD0`). Id 1 (`0x1400DDC50` lines 96–99): calls slot `+80` with the request value at `+708` (only `CGuestTxnData` has a body, `0x1400EBB20`; the others are the empty function `0x14015A930`), then slot `+8` = AuthenticateToken(errOut, AuthnToken). If `errOut` has a code, the handler replies it at once. Id 2 calls slot `+24` = RequestToken. Default slots `+16`/`+24`/`+32` = `0x1400DE8D0`: returns 51 `ErrNotImplemented`. Slot `+48` handles the provider response (HTTP reply or `AppAuth` reply).

Provider txn vtables (slots `+0` dtor, `+8` AuthenticateToken, `+16`, `+24` RequestToken, `+48` response):

| Class | Vtable | `+8` | `+16` | `+24` | `+48` |
| --- | --- | --- | --- | --- | --- |
| `CAppleGameCenterTxnData` | `0x1401727B0` | `0x1400D8680` | `0x1400D8890` | 51 | `0x1400D8970` |
| `CDaumTxnData` | `0x140172C10` | `0x1400D9F40` | 51 | 51 | `0x1400DA010` |
| `CFacebookTxnData` | `0x1401744B8` | `0x1400DF670` | `0x1400DF7C0` | `0x1400E1080` | `0x1400DFE80` |
| `CGamaniaTxnData` | `0x140174A48` | `0x1400E1610` | 51 | 51 | `0x1400E19F0` |
| `CGarenaTxnData` | `0x140175390` | `0x1400E3B70` | `0x1400E3CB0` | 51 | `0x1400E4550` |
| `CGoogleTxnData` | `0x140175AC8` | `0x1400E82F0` | `0x1400E84C0` | `0x1400EACA0` | `0x1400E8EF0` |
| `CGuestTxnData` | `0x1401761C0` | `0x1400EB570` | 51 | 51 | `0x1400EB7B0` |
| `CInnovaTxnData` | `0x1401766E8` | `0x1400EC9E0` | 51 | 51 | `0x1400ECB90` |
| `CShandaTxnData` | `0x140176C90` | `0x1400EE620` | 51 | 51 | `0x1400EE820` |
| `CSteamTxnData` | `0x140177650` | `0x1400F03F0` | 51 | 51 | `0x1400F08C0` |
| `CTencentTxnData` | `0x140177C18` | `0x1400F1640` | 51 | 51 | `0x1400F1900` |
| `CToonilandTxnData` | `0x140177DD0` | `0x1400F1FA0` | 51 | 51 | `0x1400F2080` |

Only facebook and google implement RequestToken (id 2). For all other providers id 2 replies 51 `ErrNotImplemented`.

Reply (ids 1 and 2): one shared builder `0x1400DE0F0` writes `Reply {ExternalAccountKey, ExternalAccountName, ExternalAccountData, ExternalUserAppKey, ExternalUserAppToken, ExternalAccountContact}`. Its 12 callers are the provider result functions (`0x1400D8990` apple, `0x1400D9F40` daum, `0x1400DFEF0` facebook, `0x1400E1A00` gamania, `0x1400E3CB0` garena, `0x1400E9410` google, `0x1400EB9C0` guest, `0x1400ED030` innova, `0x1400EE830` shanda, `0x1400F08D0` steam, `0x1400F1640` tencent, `0x1400F1FA0` tooniland). Which provider value goes into which reply field is ❓ not decoded per provider.

Provider checks (strings in the slot `+8`/`+48` call trees; `<secret>` values are `keyhere` in config):

| Provider | How the token is checked | Request / response fields |
| --- | --- | --- |
| facebook | sends `AppAuth.GetExternalAppInfo {AppId}` (reads `AppGroupCode`, `AuthProviderCode`), then HTTPS `GET /v2.8/me?fields=id,email,name,token_for_business&access_token=%s[&appsecret_proof=%s]` (`0x1400E0CC0`). RequestToken: `/v2.8/oauth/access_token?grant_type=fb_exchange_token…` (`0x1400DF990`), `/v2.8/oauth/client_code?…` (`0x1400DFBE0`) | parsed: `Error`, `AccessToken`, `TokenForBusiness`, `Email` (`0x1400E09A0`), `Token` |
| google | `AppAuth.GetExternalAppInfo {AppId}`, then `POST /oauth2/v3/token?alt=json` (`0x1400EA530`, `0x1400EA7E0`; body `code`, `redirect_uri`), `/oauth2/v3/tokeninfo?alt=json` with `id_token=%s` (`0x1400EAF30`), `/oauth2/v3/userinfo?alt=json&access_token=%s` (`0x1400EAA50`) | `Error`, `AccessToken`, `RefreshToken`, `IdToken` (`0x1400E9E40`), `Email` |
| apple | `AppAuth.GetExternalAppInfo {AppId}`; HTTPS `GET` of `PublicKeyUrl`; signature check | token fields `PublicKeyUrl`, `Signature`, `Salt`, `Timestamp`, `PlayerId`, `UserName` (`0x1400D91F0`) |
| steam | token parse (`CSteamTxnData::ParseToken`: steam id + ticket length check), then `GET /ISteamUserAuth/AuthenticateUserTicket/v1/?key=%s&format=%s&appId=%d&ticket=%s` (`0x1400F0590`; key = `keyhere` from config) | `Result`, `Steamid` (`0x1400F0B90`), `Errorcode`, `Errordesc` (`0x1400F0E50`), `Error`, `Response`, `Params` (`0x1400F1160`) |
| gamania_lin | token size check (`gamania authnToken size : %d, max size : %d`), `GET`/`POST` `/stage/api/ServiceAccount?token=%s` or `/openid/api/ServiceAccount?token=%s` (`0x1400E17C0`; prefix from `UriPrefix`) | `Error`, `ServiceCode`, `ServiceRegion`, `ServiceAccountID`, `ServiceAccountSN`, `DisplayName` (`0x1400E1B90`) |
| innova | Thrift-like client | `accountId`, `password`, `address` (`0x1400ED190`); `access_token`, `expiration_date`, `message`, `sign_in_request`/`sign_in_response` {`account_id`, `sign_in_code`, `long_lived`, `user_ip`, `token`} (`0x1400ED970`…`0x1400EE020`) |
| shanda | Shanda library (`ShandaLib`), `VerifyServer.ReserveNotify` | token fields `sdSessionId`, `sdAppId`, `sdAreaId` (`0x1400EE980`), result `sndaId` (`0x1400EEA70`) |
| tencent, tencent_lin | local signature check `tsigapi_checkLogin` (DLL); net bar string `NetBarSigType=%u;NetBarLevel=%u;CreditRank=%u;TeamRank=%u;Vip=%u;…` | `qqid`, `appId`, signature lengths |
| daum, tooniland | local token parse only (`ParseAuthnToken`: key, name, data, data length checks); no HTTP | — |
| guest | local; builds `;appid=%s` and `;client=%s` with `127.0.0.1` | `UserMarketCode` (`0x1400EB9C0`) |
| garena | ❓ no strings in its call tree (`0x1400E3B70`) | — |

| Code | Provider | Factory |
| --- | --- | --- |
| 1 | facebook | `0x1400DCBD0` |
| 2, 3 | tencent, tencent_lin | `0x1400DD360` (with net bar data) |
| 4 | innova | `0x1400DD090` |
| 5 | shanda | `0x1400DD180` |
| 6 | steam | `0x1400DD270` |
| 7 | daum | `0x1400DCAE0` |
| 8 | tooniland | `0x1400DD460` |
| 9 | gamania_lin | `0x1400DCCC0` |
| 10 | google | `0x1400DCEB0` |
| 11 | guest | `0x1400DCFA0` (`CGuestTxnData` slot `+80` = `0x1400EBB20`: local, no HTTP ❓) |
| 12 | apple | `0x1400DC9F0` |
| 13 global, 14 ncsoft | none | no case → 3327 |
| 15 | garena | `0x1400DCDC0` |

Txn data classes (shared_ptr, `std::_Ref_count_obj<C…TxnData>`): `CFacebookTxnData` `0x1401744B8` (slot 3 `0x1400E1080`), `CGoogleTxnData` `0x140175AC8` (slot 3 `0x1400EACA0`), `CGuestTxnData` `0x1401761C0`, `CSteamTxnData` `0x140177650`, `CTencentTxnData` `0x140177C18`, `CInnovaTxnData` `0x1401766E8`, `CShandaTxnData` `0x140176C90`, `CDaumTxnData` `0x140172C10`, `CToonilandTxnData` `0x140177DD0`, `CGamaniaTxnData` `0x140174A48`, `CGarenaTxnData` `0x140175390`, `CAppleGameCenterTxnData` `0x1401727B0`; HTTP notify classes `CFacebookNotify` `0x140174518`, `CGoogleNotify` `0x140175B30`, `CGamaniaNotify` `0x140174AA8`, `CAppleGameCenterNotify` `0x140172890`. Innova uses a Thrift-like client (`Innova::Services::APIException`, `ProtocolException`, `InvalidArgumentException`).

❓ Not decoded: per provider, which response field becomes `ExternalAccountKey` / `ExternalAccountName` / `ExternalAccountData`; the garena check.

### 2.3 DB procs, state, timers, calls out

- DB procs: none. `dbprocs()` returns 0 rows on this binary.
- State: one provider txn data object per request (`shared_ptr`, freed after the reply); HTTP connection pool `HttpConn/Connections` with `HttpConn/Timeout`.
- Timers: no timer in the provider code. `timers()` gives 25 rows in non-framework functions; all are in the shared HTTP client library (`StsHttpCli`) and the counters page: idle/timeout lists `off_14021D9C8` (period `dword_14021D9E0`, set at `0x1401186B0`/`0x14011AE30`) and `off_14021DA00` (`dword_14021DA18`, `0x1401198C0`), a 1000 ms tick `off_14021DA20` started at `0x14011AF40` (callback `0x140118A20`, 30000 ms check), and a 1000 ms timer at `0x1401403B0` (`/sts/counters`). See "Framework gaps".
- Calls out: `AppAuth.GetExternalAppInfo {AppId}` → AppAuthSrv (facebook, google, apple; reads `AppGroupCode`, `AuthProviderCode`). External provider HTTP(S) servers (§2.2 table).

### 2.4 Worked example (AuthenticateToken, id 1)

ExAuthSrv sends `{AuthProviderCode "steam", AuthnToken "<ticket>", AppId <16 bytes>}`. ExAuthnSrv checks the 3 fields, creates the steam txn (`0x1400DD270`) and sends the HTTP check (Steam Web API; `WebApiKey` = `keyhere` ❓ which reader Steam uses). With `AuthProviderCode "ncsoft"` (14) the reply is `{3327, module 2, line 184}`.

## 3. IpCheckSrv

### 3.1 Role and start (`IpCheckAppInit` `0x1400C8C20`)

IpCheckSrv tells the gates and the auth services if an IP address can connect or send a request. It keeps one grid object per IP address (`CIpData`, 0x68 bytes, constructor `0x1400C6560`). Callers: services that send `IpCheck.*` requests (the IpCheck message builders `0x1400E12E0`–`0x1400E2680` are a shared library; ExAuthSrv has the map `0x1402575B8` `CheckIp` ❓ caller list not decoded) and the admin HTTP pages of IpCheckSrv itself.

Start, in order:
1. `StsSetServerType(1015, 0)`.
2. `unknown_libname_6_0("IpCheck", 0, …)` (route protocol name).
3. Grid start `0x1400FF140`, then `0x1400FF330("IpCheck", 28800000)`: grid object life time **8 h** (same role as `GridObjectTimeoutSet`, `09` §0.3).
4. `StsConfigMonitorAdd(L"IpCheck.ini", …)` → `IpCheckConfigLoad` (3.2).
5. `0x1400C9A60` (`IpTxn` class register), `0x1400C4040` (admin HTTP pages), `StsAppCommonStart(0)`.

Admin HTTP pages (`0x1400C4040`): `/addresses`, `/test/` (`add_black_ip`, `add_white_ip`, `delete_black_ip`, `delete_white_ip`, `list_black_ips`, `list_white_ips`, `page_black_ips`, `page_white_ips`, `check_ip`), `/ip_request_limit/` (`add_ip_request_limit`, `change_ip_request_limit`, `delete_ip_request_limit`, `list_ip_request_limits`, `page_ip_request_limits`, `reset_cache_ip_limit`). Page handlers `0x1400C45C0`–`0x1400C6080` (`IpConsole.cpp`) read `IpAddress` or `StartIpAddress`/`EndIpAddress` (default `0.0.0.0`), `MaxRequestCount`, `UnitTime`, `ProhibitTimePeriod`, `PageIndex`, `PageSize`.

### 3.2 Config (`IpCheck.ini`, `IpCheckConfigLoad` `0x1400C8CB0`)

Live reload (framework 2.8): every change applies at once.

| Section | Key | Default | Global | Use |
| --- | --- | --- | --- | --- |
| `FloodingCheck` | `action` | `log` | `dword_1401DB0E0` | Connect flood action: `ignore` 0, `log` 1, `error_reply` 2 (table `0x1401DB100`). Unknown text: value is not changed. |
| `SetDirtyInterval` | `interval` | 1000 | `dword_14022A838` | ms between two SetDirty sends (3.8) |
| `DefaultLimit` | `mrc` | 0 | `dword_14022A83C` | default max request count |
| `DefaultLimit` | `ut` | 0 | `dword_14022A928` | default unit time (s); 0 = no limit |
| `DefaultLimit` | `ptp` | 0 | `dword_14022A92C` | default prohibit time period (s) |
| `limit` | `action` | `log` | `dword_1401DB0E4` | CheckIpLimit action: `ignore` 0, `log` 1, `ip` 2 (table `0x1401DB130`) |

The start values of `dword_1401DB0E0`/`dword_1401DB0E4` before the first load ❓ (data section values not read).

### 3.3 Parse and dispatch

- Parse `0x1400C8F90` → `0x1400E29F0` fills the global request record `unk_14022A840` (+8 = id). Fields: `NetAddress`, `StartNetAddress`, `EndNetAddress`, `PageIndex`, `PageSize`, `MaxRequestCount`, `UnitTime`, `ProhibitTimePeriod`. Fail: `ReplyError` with text `parsing ip check message`.
- `IpCheckOnMessage(conn, obj, …, txn)` `0x1400C8E50`, switch on the id:
  - Ids 1, 5–9, 14–18, 21, 22: no object → reply `{4 ErrBadData, module 3, line 218}` text `bad server data`. Object with `Dirty` (+68) = 0, or id 15 → `IpDataDispatch`. Else (dirty) → `IpDataReload` `0x1400C75C0`: proc `ListNetAddressStatuses` again (txn type 4), and the request waits on the object (`0x1400FF470`) ❓.
  - Other ids (0, 2–4, 10–13, 19, 20): `0x1400C9880` → reply `{52 ErrBadCommand, module 4, line 382}`. So **`LoginSuccess`, `LoginFailure`, `Logout`, `List*`, `Page*` are not served over the route**; the `List*`/`Page*` data is only on the admin pages (txn types 2, 3, 7, 8).
- `IpDataDispatch(obj, req, txn)` `0x1400C86A0`: 1 → Connect, 5 → Disconnect, 6–9 and 16–18 → `IpAdminEdit`, 14 → CheckIp, 15 → set `Dirty` = 1 (no reply), 21 → CheckIpLimit, 22 → ResetCacheIpLimit. Default: `{52, module 2, line 174}`.
- New object (`0x1400C7CD0`): when the request record +40 ≠ 0 → `0x1400C7790` (proc `ListNetAddressStatuses`, txn type 0) ❓ (meaning of +40 not decoded); else make an empty `CIpData`, register it (`0x1400FF7C0`) and dispatch.

### 3.4 `CIpData` layout (`0x1400C6560`, status writer `0x1400C7950`)

| Offset | Field | Note |
| --- | --- | --- |
| +8 | vtable `CIpData` `0x140163858` (slot 2 `0x1400C87F0` writes status XML) | timer vtable `0x140163878` at +8 for `StsTimerSet` (slot 0 = `0x1400C7DE0`) ❓ two vtables share +8 in the decompile |
| +24 | `Charge` (flood bucket time, ms) | 3.5 |
| +28 | `Connections` | Connect +1, Disconnect −1 |
| +32 | `MaximumRequestCount` (limit[0]) | 3.6 |
| +36 | `UnitTime` s (limit[1]) | |
| +40 | `OriginProhibitTimePeriod` s (limit[2]) | |
| +44 | `TotalRequestCount` (limit[3]) | |
| +48 | `ProhibitTimePeriod` s (limit[4]) | |
| +52 | `FinishTime` (limit[5], unix s) | end of the count window |
| +56 | `limitFinishTime` (limit[6], unix s) | end of the block |
| +60 | `ReachUnitTime` (limit[7], s) | |
| +64 | `LookupIpInDb` (limit[8]) | 1 = a limit row is in the DB |
| +68 | `Dirty` | 1 = reload from DB before the next request |
| +72 | `BlackStatus` | DB `NetAddressStatus` = 2 |
| +76 | `WhiteStatus` | DB `NetAddressStatus` = 1 |
| +80/+92 | SetDirty range list (8-byte `{start, end}` IPv4 numbers), count at +92 | 3.8 |

Time source: `GetSystemTimeAsFileTime` `0x1400D0240` → unix seconds `0x1400D01A0`.

### 3.5 Commands

Reply on success: `{0, module 2, line N}` with an empty `<Reply/>` body. All errors use module 2.

| Id | Command | Handler | Steps | Errors (line) |
| --- | --- | --- | --- | --- |
| 1 | Connect | `0x1400C6CD0` | If the caller value `0x1401108B0` < 1 ❓ (reads +32 of the current connection record; probably a trust level): (a) `BlackStatus` ≠ 0 and `WhiteStatus` = 0 → `ErrNotAllowedIp`; (b) `RateCharge(+24, 1000, 120000)` fails → action 1: log `flooding detected from %s, reply to %s` and go on; action 2: `ErrFlooding`; action 0: go on. Then `Connections` + 1, reply OK. | 3721 (181), 3005 (206); OK 219 |
| 5 | Disconnect | `0x1400C6F20` | `Connections` = 0 → `ErrConnNotFound`; else − 1, reply OK | 3032 (227); OK 237 |
| 6, 7, 8, 9 | AddBlackIp, AddWhiteIp, DeleteBlackIp, DeleteWhiteIp | `IpAdminEdit` `0x1400C7070` | Check range (`0x1400C96A0`), proc `AddNetAddress`/`DeleteNetAddress` (4 params), `CIpTxn` type 1 | 10 (bad range / bad IP) |
| 14 | CheckIp | `0x1400C6770` | `WhiteStatus` ≠ 0 or `BlackStatus` = 0 → OK; else `ErrNotAllowedIp` | 3721 (246); OK 251 |
| 15 | SetDirty | inline | `Dirty` = 1; no reply | — |
| 16, 17, 18 | AddIpRequestLimit, ChangeIpRequestLimit, DeleteIpRequestLimit | `IpAdminEdit` | procs with 6 / 6 / 3 params, `CIpTxn` type 1 | 10 |
| 21 | CheckIpLimit | `0x1400C68C0` | 3.6 | 10 (655), 3779 (677); OK 674 / 685 |
| 22 | ResetCacheIpLimit | `0x1400C74E0` | `0x1400C8B60`: `limitFinishTime` = `ReachUnitTime` = 0, `TotalRequestCount` = 0, `ProhibitTimePeriod` = `OriginProhibitTimePeriod`; reply OK | OK 698 |
| 0, 2–4, 10–13, 19, 20 | Invalid, LoginSuccess, LoginFailure, Logout, ListBlackIps, ListWhiteIps, PageBlackIps, PageWhiteIps, ListIpRequestLimits, PageIpRequestLimits | none | `ErrBadCommand` | 52 (module 4, 382) |

`RateCharge(t, cost, cap)` `0x1400ED140` (token bucket): `now = timeGetTime()`; if `now > t` then `t = now`; if `t + cost − now ≥ cap` then `t += 500`, fail; else `t += cost`, pass. With cost 1000 and cap 120000 an IP can make about 120 Connects in a burst, then 1 per second.

`IpAdminEdit` `0x1400C7070`:
1. `0x1400C96A0` checks the range: `StartNetAddress` and `EndNetAddress` valid and `end − start ≤ 0x1000000`; or `NetAddress`, where a last `*` is changed to `0` ❓ (exact wildcard rule not decoded). Fail → `{10, module 2}`.
2. Both ends padded with `0x1400E2870` to `%03u.%03u.%03u.%03u` (fail: `{10}`).
3. Params (type 2 = text, 0 = int): `StartNetAddress`, `EndNetAddress`, int 1 ❓ (constant, meaning not decoded), then: Add/Delete IP → status from `0x1400C93D0` (black ids 6, 8 → 2; white ids 7, 9 → 1); Add/Change limit → `MaxRequestCount`, `UnitTime`, `ProhibitTimePeriod`; Delete limit → none.
4. Proc name from `0x1400C9350`. `0x1400E0C30(msg, "MasterDb", proc, n, params, "v2015102301")`, send `0x1400FF800(msg, CIpTxn(type 1, id), key 0x7FFFFFFF, 0, 25000, 0)`.
5. Reply (`IP_PROCESS`, `0x1400C8360`): reply OK (line 473) to the caller, add `{start, end}` to the SetDirty list, `StsTimerSet(obj+8, SetDirtyInterval, 1)`, log `Ip status; %s; %s - %s;`. Bad IP in the DB reply: `{…, module 2, line 465}` `bad ip address`.

### 3.6 CheckIpLimit (`0x1400C68C0`, `IpLimitHit` `0x1400C8A60`)

1. If `LookupIpInDb` = 0 and the limit differs from `DefaultLimit` (mrc, ut, ptp): copy the defaults (`ProhibitTimePeriod` = `OriginProhibitTimePeriod` = ptp) and clear the counters (`0x1400C8B50`). So IPs with no DB row follow the live INI defaults.
2. Action (`limit.action`) 0: reply OK (685), no count.
3. Action 1 or 2: `IpLimitHit`:
   - `UnitTime` = 0 → pass.
   - `TotalRequestCount` + 1. First request: `FinishTime = now + UnitTime`.
   - `TotalRequestCount > MaximumRequestCount`:
     - If `limitFinishTime` = 0: `ReachUnitTime = UnitTime + now − FinishTime` (seconds since the first request), `limitFinishTime = now + ProhibitTimePeriod`.
     - If `UnitTime ≤ ReachUnitTime` (the requests were slow): `ProhibitTimePeriod −= ReachUnitTime / UnitTime` (not below `OriginProhibitTimePeriod` or 0); clear `limitFinishTime`, `ReachUnitTime`, `TotalRequestCount`; pass.
     - Else if `limitFinishTime > now`: **fail** (blocked; counters stay).
     - Else (block ended): `ProhibitTimePeriod += UnitTime / ReachUnitTime` (`0x1400C8B70`, only when `ReachUnitTime` ≠ 0); clear the three fields; pass.
4. Fail and action 1: log `limit detected from %s, Rut : %d, TotalRc : %d, (Mrc : %d, Ut : %d,  Ptp : %d), %d`, reply OK (685). Before the log the IP text is made with `0x1400E2870`; fail → `{10, module 2, line 655}`.
5. Action 2: pass → OK (674); fail → `{3779 ErrLimitOverMaxRequestCount, module 2, line 677}`.

| Quirk | Address | What the code does | Value | Port |
| --- | --- | --- | --- | --- |
| `quirk_iplimit_int_round` | `0x1400C8A60`, `0x1400C8B70` | `(float)(a / b) + 0.5` is done after an integer divide, so the +0.5 has no effect | floor of `a / b` | keep value |
| `quirk_ip_massterdb_log` | `0x1400CA380` | timeout log for `GetNetAddressRequestLimit` names the DB `MassterDb` | log text only | fixed (log text is not wire data) |

### 3.7 DB txn (`CIpTxn`, vtable `0x140163F20`, `IpTxn.cpp`)

`CIpTxn(obj, type, id)` `0x1400C9100` (0x28 bytes; +8 type, +36 command id). Load txn `0x1400C90D0(type, ip)`. Types (`IPTXN_REPLY_*` names at `0x140162FF8`):

| Type | Name | Reply handler | Use |
| --- | --- | --- | --- |
| 0 | `CHECK_IP_ADDRESS` | `0x1400CB240` | first load of a new IP: `NetAddressStatus` rows, then `GetNetAddressRequestLimit`, then a new `CIpData` |
| 1 | `IP_PROCESS` | `0x1400C8360` | Add/Delete/Change result (3.5) |
| 2 | `LIST_IPS` | `0x1400CB9D0` | admin page |
| 3 | `PAGE_IPS` | `0x1400CC3F0` | admin page |
| 4 | `GET_STATUS` | `0x1400C8090` | reload of a dirty object: `NetAddressStatus` 1 → `WhiteStatus`, 2 → `BlackStatus`, `Dirty` = 0, then proc `GetNetAddressRequestLimit` (type 6). Bad reply: `{…, 2, 382}` `bad db reply`; pad fail: line 397 |
| 5 | `CLIENT` | `0x1400CC9C0` | ❓ |
| 6 | `GET_REQUEST_LIMIT` | `0x1400C7FB0` | row found: `MaximumRequestCount = LimitCount`, `UnitTime = LimitDuration / 1000`, `OriginProhibitTimePeriod = ProhibitTimePeriod = ForbiddenDuration / 1000`, `LookupIpInDb = 1`; no row: `LookupIpInDb = 0` |
| 7 | `LIST_IP_REQUEST_LIMITS` | `0x1400CB580` | admin page |
| 8 | `PAGE_IP_REQUEST_LIMITS` | `0x1400CBD80` | admin page |

- Slot 4 (OnReply) `0x1400CB0F0`: types 1, 4, 6 go to the object (`0x1400C7EE0`; no object → line 823 `on transaction: invaild data`); other types to `0x1400C9950`. Bad type: line 836 / 363 / 721 `invalid ip (check) txn process`.
- Slot 5 (OnError) `0x1400CA380`: on `ErrTimeout` (42) logs the DB and proc (`0x1400C9B00`). Then types 1 and 2 (and every type with a bit other than 1, 2): reply the error to the caller; types 0, 4, 6: `0x1400FF3F0(err, 0)` (fail the waiting grid requests) ❓.
- DB units: the DB keeps durations in ms; `CIpData` keeps seconds.

### 3.8 SetDirty flush timer (`0x1400C7DE0`)

After an admin edit the object sends `IpCheck.SetDirty` (`0x1400E2680`, one-way `StsRouteSend`) so that every `CIpData` in the range reloads on its next request. Each tick takes the first `{start, end}` of the list:
- `end − start ≤ 1500`: send SetDirty for the range, remove it; empty list → return −1 (stop), else return `SetDirtyInterval`.
- Else: send SetDirty for the first part, `start += 1501`, return `SetDirtyInterval`.
So a large range takes `ceil(size / 1501)` ticks of `SetDirtyInterval` ms (default 1000).

### 3.9 DB procs (`MasterDb`, schema `v2015102301`, builder `0x1400E0C30`, all 25000 ms)

| Proc | Params | Call site |
| --- | --- | --- |
| `ListNetAddressStatuses` | padded IP | `0x1400C75C0` (type 4), `0x1400C7790` (type 0) |
| `GetNetAddressRequestLimit` | padded IP | `0x1400C8090`, `0x1400CB240` |
| `AddNetAddress`, `DeleteNetAddress` | start, end, 1, status | `0x1400C7070` |
| `AddNetAddressRequestLimit`, `ChangeNetAddressRequestLimit` | start, end, 1, mrc, ut, ptp | `0x1400C7070` |
| `DeleteNetAddressRequestLimit` | start, end, 1 | `0x1400C7070` |
| `ListNetAddresses`, `PageNetAddresses` | range (+ page) | `0x1400CA730`, `0x1400CAD90` (admin pages) |
| `ListNetAddressRequestLimits`, `PageNetAddressRequestLimits` | range (+ page) | `0x1400CA430`, `0x1400CAA50` (admin pages) |

### 3.10 Calls out

- `IpCheck.SetDirty` to the IpCheck grid (itself, all owners of the range) from the flush timer (3.8).
- `MasterDb` procs (3.9). No call to other services.

### 3.11 Worked example (Connect)

Request `IpCheck.Connect`, `NetAddress` = `10.1.2.3`. Object: `BlackStatus` 0, `WhiteStatus` 0, `Charge` = now − 5000, `Connections` 4, `FloodingCheck.action` = `log`. `RateCharge`: `t` = now; `now + 1000 − now = 1000 < 120000` → pass, `Charge` = now + 1000. `Connections` = 5. Reply `{code 0, module 2, line 219}` with `<Reply/>`.

## Framework gaps

| Fact | Binary, address | Note |
| --- | --- | --- |
| StsRoute key alias registration `(service, 0, key, alias, notFoundCode, …)` (`StsRoute.cpp`) | ExAuthSrv `0x140123D80` | §1.1 step 4; not in `00-framework.md` |
| HTTP client library (`StsHttpCli`): timeout lists `off_14021D9C8` (period `dword_14021D9E0`, set `0x1401186B0`, `0x14011AE30`) and `off_14021DA00` (`dword_14021DA18`, `0x1401198C0`); 1000 ms tick `off_14021DA20` started at `0x14011AF40`, callback `0x140118A20` (30000 ms check) | ExAuthnSrv | shared with 2–3 binaries |
| `/sts/counters` page timer, 1000 ms | ExAuthnSrv `0x1401403B0` | shared with 9 binaries |
| Grid server API used by IpCheckSrv: start `0x1400FF140`; protocol object life time `0x1400FF330(name, ms)`; request waits on object `0x1400FF470` ❓; register object `0x1400FF7C0`; DB txn send `0x1400FF800(msg, txn, key, 0, 25000, 0)`; fail waiting requests `0x1400FF3F0(err, 0)` ❓ | IpCheckSrv | old framework build; compare with `GridTxnSend` `00` §2.3 |
| Proc builder `(msg, db, proc, n, params, schema)` | IpCheckSrv `0x1400E0C30` | other argument order than `DbProcBegin` |
| Caller trust value (connection record `+32`) | IpCheckSrv `0x1401108B0` ❓ | §3.5 Connect |
| `StsRouteStart` with a socket-notify factory | ExAuthnSrv `0x1400DE0E0` | same pattern as `00` §2 HelloSrv; not a gap |

## Differences from existing specs

- `Docs/Platform/Sts/10-stservers-original.md` line 114 names the IpCheckSrv database `PlatformMasterDb`. The proc calls use the DB name `MasterDb` (§3.9, `0x1400E0C30`). The config maps the name; the schema `v2015102301` is the same.
- `11-cligate-protocol.md` line 192: the IpCheck reply states of the gate are ❓ there. §3.5 gives the codes that IpCheckSrv can reply (3721, 3005, 3032, 3779).
