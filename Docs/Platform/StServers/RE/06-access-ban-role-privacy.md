# 06 — AccessSrv, BanSrv, RoleSrv, PrivacySrv (RE reference)

Summary:
1. AccessSrv: see §1.
2. BanSrv: see §2.
3. RoleSrv keeps roles (id, name, type) and gives roles to users and to IP address ranges inside app groups of type 5. It answers the `Role` protocol (11 commands) and uses `RoleDb`.
4. PrivacySrv: see §4.
5. All four services use the framework of `00-framework.md` without change. This chapter gives only the service code.

Sources: the `.i64` copies in `ida/lanes/st/` (build 381). All addresses are in the binary of the section. Error records are `{code, module, line}` (framework 3.1); code names are from `errcodes.tsv`. Long lists: `06-commands.tsv`, `06-procs.tsv`. ❓ = not verified in IDA.

## Quick find

| Question | Function | Binary, address | Section |
| --- | --- | --- | --- |
| RoleSrv start (server type, key alias, admin pages) | `RoleAppInit` | RoleSrv `0x1400C84E0` | 3.1 |
| RoleSrv request parse (fields → global record) | `RoleParseRequest` | RoleSrv `0x1400D6E30` | 3.2 |
| RoleSrv command switch (ids 1–11) | `CRoleConn::OnRequest` | RoleSrv `0x1400C66B0` | 3.3 |
| RoleSrv DB/AppAuth reply switch (txn type 1–5) | `CRoleTxn::OnSuccess` | RoleSrv `0x1400C6880` | 3.4 |
| RoleSrv role check on login (`Inquiry`) | `RoleInquiry` + `RoleOnAppGroups` | RoleSrv `0x1400C50C0` / `0x1400C6150` | 3.3, 3.4 |

## 1. AccessSrv

Blocked (2026-11-12): IDA cannot open `AccessSrv.exe.i64` (old worker, timeout). See `Docs/Plan/handoffs/st-06.md`.

## 2. BanSrv

Blocked (2026-11-12): IDA cannot open `BanSrv.exe.i64` (old worker, timeout).

## 3. RoleSrv

### 3.1 Role, start and config

RoleSrv stores roles and role grants in `RoleDb` (schema `v2016072601`). A grant gives a role to a user (UserId GUID) or to an IPv4/IP text range, inside one app group. The service asks AppAuthSrv (`AppAuth` protocol) for the app group data. Callers: GM tools and the admin pages (`/role/...`) for the change commands; services and gates call `Inquiry` to get the roles of a user at login ❓ (no caller found in the four binaries of this slice).

Start `RoleAppInit` `0x1400C84E0` (`CApplication` slot 0):
1. `StsSetServerType(1121, 0)`; name `"Role"` (`unknown_libname_6_0`).
2. Protocol notify `CRoleProviderNotify` (`qword_140226888`) is added for protocol `"EventLog"` with `0x140113E50` (StsRoute.cpp, shared with 16 binaries) ❓ use.
3. `0x140113910("Role", 0, "login", "$Name.GetIdByLoginName", 3002, 4, 0)` (StsRoute.cpp, shared with 10 binaries): adds a 0x40-byte record to the list at `off_1401D9058`. Meaning ❓: a route key alias. A `Role` request with key type `login` is first resolved by `Name/GetIdByLoginName`; error 3002 is used when the name is not found.
4. Connection object `CRoleConn` (`qword_140226878`) is registered with `0x1400F0F40` (framework service register).
5. `StsAppCommonStart(0)`, then `0x1400C81A0`: admin HTTP pages `/role/` ("Role Management Page"), `/role/create_role`, `/role/list_roles`, `/role/delete_role`, `/role/add_user_role`, `/role/remove_user_role`, `/role/add_netaddress_role`, `/role/remove_netaddress_role`, `/role/list_user_roles`, `/role/list_netaddress_roles`, `/role/inquiry` (one page per command).

Config: RoleSrv reads no own keys. `cfgkeys()` finds only framework keys (`StServer.ini`, `[General]` route keys, TLS, logging). No service timer (`timers()`: the only service call is `0x140144086` with ms −1 = remove).

The binary also has the `AppAuth` name map (`0x1402276B0`, 49 ids, `RegisterApp` … `ListRegionalAppGroups`). Only its static initializer and `atexit` use it: RoleSrv does not handle `AppAuth` commands.

### 3.2 Request parse (`RoleParseRequest` `0x1400D6E30`)

OnMessage `0x1400C6810` calls the parse with the global request record `R = 0x140226510`. Parse fail → error reply (framework 3.2). Error lines: unknown command name 313; bad body 325, 336, 351.

| Offset in R | Field (XML element) | Type | Default when absent |
| --- | --- | --- | --- |
| +40 | request header copy (`f` header, `0x1400E6D70`) | struct | — |
| +64 | caller net address (in the header copy) ❓ | string | — |
| +248 | caller app id (in the header copy) ❓ | GUID | — |
| +692 | command id (map `0x140227710`) | int | — |
| +696 | message vtable +16 value ❓ | int | — |
| +700 | txn id for the reply | int | — |
| +704 | `AppGroupId` | int | 0 |
| +708 | `AppId` | GUID | R+248 (caller app) |
| +728 | `ClientNetAddress` | string | R+64 (caller address) |
| +752 | `FirstNetAddress` | string | empty |
| +776 | `LastNetAddress` | string | empty |
| +800 | `RoleId` | int | 0 |
| +808 | `RoleName` | string | empty |
| +832 | `RoleType` | int | 0 |
| +836 | `UserId` | GUID | text before the first `;` of the `t` header (route key) |

Address rule (`0x1400C46E0` and others): text with exactly 3 dots is type 1 (IPv4). It is parsed with `0x1400CAA90` and written as `%03d.%03d.%03d.%03d` (zero-padded, so `10.0.0.1` → `010.000.000.001`). Other text is type 2 and is sent as given. App group list text: `%u.` for one group (for example `7.`).

- `quirk_role_ipv4_zero`: when an IPv4 text with 3 dots does not parse (`0x1400CAA90` returns 0), the formatted text stays empty and an empty string goes to the proc (`0x1400C46E0` line 74).
- `quirk_role_last_plus_one`: `AddNetAddressRoles` and `RemoveNetAddressRoles` send `LastNetAddress + 1` (an exclusive end). When the last address is `255.255.255.255` (−1) the code does not add 1 (`0x1400C46E0` lines 106–110).

### 3.3 Commands (`CRoleConn::OnRequest` `0x1400C66B0`)

Switch on R+692. Default (id 0 or unknown): reply `{51 ErrNotImplemented, 2, 130}`. All parameter errors are `{10 ErrBadParam, 2, line, text}`. All DB calls: `DbProcBegin(…, "RoleDb", "v2016072601", proc, 0)`, then `SendToProviderTxn(msg, CRoleTxn, 25000, 0)`. Txn types are in 3.4.

| Id | Command | Handler | Checks (error line, text) | Action |
| --- | --- | --- | --- | --- |
| 1 | AddNetAddressRoles | `0x1400C46E0` | AppGroupId, RoleId, First, Last not empty (248 `invalid parameter`); First and Last same address type (260 `invalid parameter`) | proc `AddNetAddressRoles(AppGroupId, RoleId, First, Last+1, type)`; txn 1 |
| 2 | AddUserRole | `0x1400C4B00` | AppGroupId ≠ 0 and UserId ≠ 0 (180 `invalid appgroupid or userid`) | `AppAuth/GetGroupInfo(AppGroupId, Apps)` (`0x1400D5B20`); txn 4 keeps UserId, RoleId, AppGroupId |
| 3 | CreateRole | `0x1400C4C60` | RoleId ≠ 0 and RoleName not empty (136 `missing roleId or roleName`) | proc `CreateRole(RoleId, RoleName, RoleType)`; txn 1 |
| 4 | DeleteRole | `0x1400C4DF0` | RoleId ≠ 0 (152 `missing roleId`) | proc `DeleteRole(RoleId)`; txn 1 |
| 5 | DeleteUser | `0x1400C4F40` | UserId ≠ 0 (366 `userid`) | proc `RemoveUserRolesByUserId(UserId)`; txn 1 |
| 6 | Inquiry | `0x1400C50C0` | AppId ≠ 0 (349 `app`); UserId ≠ 0 (350 `userid`); ClientNetAddress not empty (352 `netaddress`) | `AppAuth/ListAppGroups(AppId, AppGroupType=5)` (`0x1400D5BC0`); txn 5 keeps UserId and the address |
| 7 | ListNetAddressRoles | `0x1400C52F0` | AppId or AppGroupId given (311 `missing app`); ClientNetAddress not empty (314 `missing netaddress`) | AppGroupId = 0: `ListAppGroups(AppId, 5)`, txn 5. Else proc `ListNetAddressRoles("N.", address, type)`; txn 3 |
| 8 | ListUserRoles | `0x1400C5780` | AppId or AppGroupId given (211, text ❓) | AppGroupId = 0: `ListAppGroups(AppId, 5)`, txn 5. Else proc `ListUserRoles(UserId, "N.")`; txn 3 |
| 9 | ListRoles | `0x1400C5640` | none | proc `ListRoles(RoleType)`; RoleType 0 → NULL param (`0x1400D5E20`); txn 2 |
| 10 | RemoveNetAddressRoles | `0x1400C59D0` | same as id 1 (281 missing, 293 type mismatch) | proc `RemoveNetAddressRoles(AppGroupId, RoleId, First, Last+1, type)`; txn 1 |
| 11 | RemoveUserRole | `0x1400C5DF0` | none | proc `RemoveUserRole(UserId, AppGroupId, RoleId)`; txn 1 (type value ❓) |

Note: `RemoveUserRole` does no parameter check (`quirk_role_remove_no_check`): zero ids go to the proc.

### 3.4 Reply handling (`CRoleTxn`)

`CRoleTxn` (0x2E0 bytes, vtable slots at `0x14015F100`): +24 command id, +28 caller txn id, +32 txn type, +36 header copy, +688 UserId, +704 AppGroupId, +708 RoleId, +712 net address text. Constructors: `0x1400C4000` (plain), `0x1400C4070` (+UserId), `0x1400C40F0` (+UserId, RoleId, AppGroupId), `0x1400C4190` and `0x1400C4220` (+address), `0x1400C42B0` (copy of an other txn).

OnSuccess `0x1400C6880` (slot +0x18) switches on the txn type:

| Type | Function | Behaviour |
| --- | --- | --- |
| 1 | `0x1400C6A40` | Pass the DB reply body to the caller unchanged. |
| 2 | `0x1400C6C50` | Reply `<Reply><Roles type="array"><Role>` with `RoleId`, `RoleName`, `RoleType`, `Registered` per row. |
| 3 | `0x1400C6AF0` | Reply `<Reply><Roles type="array">` with one `RoleId` per row. |
| 4 | `0x1400C5F70` | GetGroupInfo reply. No data → `{10, 2, 538, "appgroup info reply, missing data"}`. `AppGroupType` 0 → `{3734 ErrAppGroupNotFound, 2, 542}`; ≠ 5 → `{3734, 2, 545}`. Else proc `AddUserRole(UserId, AppGroupId, RoleId)`, txn 1. |
| 5 | `0x1400C6150` | ListAppGroups reply. No rows element → `{?, 2, 562, "missing row"}`. `0x1400C4440` joins the `AppGroupId` values (`,` between, `.` at the end ❓). No group → reply empty `Roles` array (no error). Else by command id: 7 → `ListNetAddressRoles(groups, address, type)`; 8 → `ListUserRoles(UserId, groups)`; 6 → `GetLoginRoles(groups, address, type, UserId)`; all with txn 3. Other id → error (lines 609/611 ❓). |
| other | — | error line 471 `bug` ❓ |

OnError `0x1400C6640` (slot +0x20): when the code is 42 (ErrTimeout), log through `0x1400C6F10("Role", command name, UserId)`; then send the error record to the caller unchanged.

Worked example (`Inquiry`): request `Role/Inquiry` with `AppId = {A}`, `UserId = {U}`, `ClientNetAddress = "10.1.2.3"`. RoleSrv sends `AppAuth/ListAppGroups {AppId={A}, AppGroupType=5}`. Reply rows `AppGroupId` 7 and 9 → list `7,9.` ❓. RoleSrv calls `RoleDb.GetLoginRoles("7,9.", "010.001.002.003", 1, {U})` and replies `<Reply><Roles type="array"><RoleId>3</RoleId>…`.

### 3.5 State, timers, calls out

- State: one global request record (R, 3.2). It is safe because each process has one Compute thread (framework 2.6). No cache, no timer.
- Calls out: `AppAuth/GetGroupInfo` (AddUserRole), `AppAuth/ListAppGroups` (Inquiry, ListNetAddressRoles, ListUserRoles). Timeout 25000 ms each. DB: `RoleDb` only.

## 4. PrivacySrv

Blocked (2026-11-12): IDA cannot open `PrivacySrv.exe.i64` (old worker, timeout).

## Framework gaps

- `0x140113910` (StsRoute.cpp, 10 binaries): adds a route key alias record (protocol, key type, resolver `$Proto.Command`, error code, 4, extra). RoleSrv: `("Role", "login", "$Name.GetIdByLoginName", 3002, 4)`. ❓ how RouteSrv/the client uses it.
- `0x140113E50` (StsRoute.cpp, 16 binaries): adds a provider notify object for an other protocol (RoleSrv: `"EventLog"`).

## Differences from existing specs

(Filled at the end.)
