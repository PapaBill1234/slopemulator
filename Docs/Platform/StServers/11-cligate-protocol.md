# 11 — CliGate client protocol (original StServers)

Source: `CliGate.exe` (NP 1.20.3350, x64, no PDB), IDA 9.5 decompile
`BNSX-files/decomp/CliGate.c`. Addresses are image addresses (base 0x140000000).
Facts not checked against a live run are marked ❓.

## 1. Wire format

| Item | Format | Source |
| --- | --- | --- |
| Request line | `POST /<Protocol>/<Command> STS/1.0\r\n` (`HTTP/1.1` when the conn is HTTP) | sub_1400F1640 |
| Reply line | `STS/1.0 <status> <reason>\r\n` | sub_1400F2A80 |
| Default reasons | <200 `Informational`, <300 `OK`, <400 `Redirection`, <500 `Bad Request`, else `Server Error` | sub_1400F2A80 |
| Error reply | status 400, reason = error name (table below), body `<Error .../>` | sub_14012B940 |
| Sequence header `s` | `%u` + `R` (final) or `r` (partial); same as Rust `Transaction` | sub_1400F2790 |
| Error body | `<Error code= server= module= line= text=/>`; zero fields are left out | sub_140126380 |
| Error `server` | when 0, filled with the local server id (u16) | sub_14012B940 → sub_14012EC10 |

Error struct used by every handler: `{code u32, server u32, module u32, line u32, text*}`.
CliGate handlers use `module=1` and `line` = a source line number (for example 1293, 1336).

### Error-code names

`errcodes.tsv` (same folder) holds all 1550 `code → name` pairs, read from the
tables at 0x14027F170 (120 entries) and 0x14027F900. Common codes:

| Code | Name | Used by CliGate for |
| --- | --- | --- |
| 0 | Success | |
| 4 | ErrBadData | bad backend reply |
| 10 | ErrBadParam | missing body / field |
| 42 | ErrTimeout | backend timeout (logs the reply name) |
| 49 | ErrPermission | command not allowed in this state |
| 3030 | ErrBadConfig | cannot bind client app info |

## 2. Auth command enum

The table at 0x1401A35F0 (110 pairs `{name*, id}`) maps the `Auth` command name to an id
(sub_1400E4780). The ids are used by CliGate, AuthSrv and the route layer.

| Id | Command | Id | Command | Id | Command |
| --- | --- | --- | --- | --- | --- |
| 18 | GetMyUserInfo | 21 | KeyData | 22 | LoginStart |
| 23 | LoginFinish | 30 | RequestGameToken | 32 | RequestToken |
| 35 | VerifyMobiOtp | 36 | VerifySecondPwd | 37 | GetSecurityCardCodeIndex |
| 38 | VerifySecondaryAuth | 46 | CreateAccount | 58 | TokenKeyData |
| 96 | LoginTokenStart | 97 | LogoutMyClient | 98 | RequestCryptKey |
| 99 | WaitForAllowedLogin | | | | |

The full list (1 AccessTest … 110 Flush) is in the IDA table; the port copies it as one Rust enum.

## 3. CliConn dispatcher (CliGate.c line ~144380)

```
request ──► bind client app info (fail → 3030)
        ──► Auth/LoginStart or LoginTokenStart: reset session first (sub_1400D4390)
        ──► GameAccount cmd 3 ───────────► sub_1400D0500
        ──► Auth switch (below)
        ──► SecondaryAuth 19, IpAuthz, Profile 7, Mail 1, Gate/LookupGeolocation,
            Slot 4, Purchase, PurchaseLimit 23, VirtualCurrency 3, Auction …
        ──► otherwise: forward to the route grid (sub_14011A3A0), reply via CCliConn::ClientReply
```

| Auth id | Handler | Reply handler | Notes |
| --- | --- | --- | --- |
| 18, 20 | sub_1400CFA50 | | only before login (conn+300 = 0) |
| 21 KeyData | sub_1400D0130 | KeyDataReply sub_1400D2E60 | needs session (conn+328); `ErrPermission` after login |
| 22 LoginStart | sub_1400D09E0 | LoginStartReply sub_1400D4030 | LoginName max 64 chars |
| 23 LoginFinish | sub_1400D07B0 | LoginFinishReply sub_1400D3B20 → LoginFinishGateReply sub_1400D35C0 | needs flag conn+304 bit 2 (set by KeyData Complete) |
| 30 RequestGameToken | sub_1400D1940 | | |
| 32 RequestToken | sub_1400D1B10 | | |
| 35, 36, 38 | sub_1400D2380 | | secondary auth |
| 37 | sub_1400CFBA0 | | |
| 46 CreateAccount | sub_1400CF3E0 | CreateAccountReply sub_1400CF120 | needs salt + verifier |
| 58 TokenKeyData | sub_1400D1ED0 | TokenKeyDataReply sub_1400D55A0 | |
| 96 LoginTokenStart | sub_1400D0B60 | (local reply) | |
| 97 LogoutMyClient | sub_1400D1150 | LogoutReply sub_1400D4600 | |
| 99 WaitForAllowedLogin | sub_1400D27A0 | | |

All forwarded calls use sub_1400CEF30 with a 25 000 ms timeout (0x61A8).

Note on ids 18 and 20 (`ida/StServers/CliGate.exe`, sub_1400CFA50, read 11 Nov 2026): the handler needs the login session string (conn+328 ≠ 0). Else ErrPermission 49, module 1, line 2331. With a session it removes header `p`, adds the session data (sub_1400D29E0, conn+248) and forwards with reply callback `ClientReply` (sub_1400CED50). The table text "only before login" is thus not exact: the condition is "login session present". ❓ The game client sends `Auth/GetMyUserInfo` after LoginFinish, when the session string is free. Which id that request has in this switch is not verified. The Rust port forwards `GetMyUserInfo` for a logged-in user (it is in `Gate.ini [Commands.CliGate]`).

`GameAccount` command 3 (`ListMyAccounts`, sub_1400D0500): a `CCliConnTxn` with reply callback sub_1400D3340, then the generic gate path (sub_14011A3A0).

## 4. Login sequence

```
Client                 CliGate                         AuthSrv
  │ Auth/LoginStart       │                               │
  │ <LoginName>           │ Auth/LoginStart               │
  │──────────────────────►│ <LoginName><NetAddress>──────►│
  │                       │◄──── <Session><KeyData>…      │ (userref, session saved conn+328)
  │◄── <Reply><KeyData>   │                               │
  │ Auth/KeyData          │ Auth/KeyData (t: header =     │
  │──────────────────────►│ userref) <Session><KeyData>──►│
  │                       │◄── <KeyData>[<Complete/>      │
  │                       │     <SessionKey>base64]       │
  │◄── <Reply><KeyData>   │ (sent in clear, then RC4 on)  │
  │ Auth/LoginFinish      │ Auth/LoginFinish + Session ──►│
  │──────────────────────►│◄── Licenses, Roles, PcCafe…   │
  │                       │ CCliConnTxn → gate reply      │
  │◄── reply              │ conn+300 = login id           │
```

| Step | Facts | Source |
| --- | --- | --- |
| LoginStart failure | 3 failures (conn+296) close the connection | sub_1400D2E60 |
| KeyData `Complete` | needs `SessionKey`; CliGate Base64-decodes it, builds the crypt filter (sub_140107980), sends the reply, then installs the filter (vtable+88) | sub_1400D2E60 |
| Log lines | `LOGIN`, `LOGIN-FAILED`, `LOGOUT`, `DISCONNECT` with userref, address, login name | sub_1400D2E60, sub_1400D4390 |
| Reset | LoginStart / LoginTokenStart clear session, userref, random buffers | sub_1400D4390 |

## 5. LoginTokenStart (id 96) — local reply

| Step | Detail |
| --- | --- |
| Input | `<ClientRand>` Base64, must decode to 32 bytes, else ErrBadParam line 1456 |
| ServerRand | 32 bytes: word 0 = sub_1400D9CE0(time) ❓, bytes 4..31 random, then XOR with ClientRand bytes 4..31 (sub_140108330) |
| Keys | needs the loaded server certificate (off_1402471B0: public key at +32/+44, signature at +56/+68); missing → code 4 line 1466 |
| Reply | `<Reply><ServerRand/><ServerPublicKey/><ServerSignature/></Reply>` |

## 6. Crypt filters

| Function | Builds | Detail |
| --- | --- | --- |
| sub_140107980(key, len) | CCryptFilter | two `Crypt::CKeyRc4` (send, recv) with the same key; plain RC4 KSA, no drop-N (vtable+16 is a no-op) |
| sub_1401078F0(k1, k2, len) | CCryptFilter | RC4 with separate send and receive keys |
| sub_1401077D0(ctx, is_server, cfg) | CTlsFilter | OpenSSL TLS filter; log `SocketCrypt [...] failed tls filter initialize` |

The KeyData session key feeds sub_140107980, so the client link is RC4 with one shared key, as in `rc4.rs`.

## 7. Connection identity (`Sts/Connect`, CliGate.c ~line 204990)

Every STS socket first says who it is. Two forms exist.

| Form | Fields |
| --- | --- |
| Header on `POST /Sts/Connect`: `X-Sts-Connect: <ConnType>; product=; index=; address=; deployment=; epoch=; notify=; version=; process=; program=; build=` | all numeric except `address` |
| Body `<Connect>` | `ConnType`, `Process`, `ProductType`, `AppIndex`, `Deployment`, `NotifyFlags`, `VersionFlags`, `Address`, `Program`, `Build`, `Epoch` |
| No `X-Sts-Connect` header | game client: ConnType 300, or 301 with `Authorization`; 302 for `Upgrade: websocket` |

Required: ConnType, Process, Program, Build (Program −1 → own program id). A bad `address` rejects the connection.

## 8. Open items

| Item | Status |
| --- | --- |
| Certificate loader sub_1400D6030 | Out of scope. Only LoginTokenStart (§5) uses it. The SRP login (§4) does not. |
| LoginFinishGateReply body | Closed, see §10 |
| RequestToken path | Closed, see §11 |

## 9. Forwarding to AuthSrv (route bus)

### 9.1 Send path

```
CliConn handler ─► build new frame (Auth/<Cmd>, own body) ─► sub_1400CEF30
  ─► new CCliConnTxn (0x50 bytes) ─► sub_140114980 ─► sub_140114A10 (StsGate send)
  ─► route link qword_1402AF250 vtable+32 (with txn, timeout) ─► RouteSrv ─► AuthSrv
```

| Item | Fact | Source |
| --- | --- | --- |
| Frame | CliGate does not forward the client frame. It builds a new request with its own body. | sub_1400E3D90, sub_1400E3B30 |
| Txn record | `CCliConnTxn`: +40 conn pointer, +48 client request id, +64 reply callback (pointer to member), linked into the conn list at conn+72 | sub_1400CEF30 |
| Timeout | 25 000 ms for each Auth call (0x61A8). StsGate also uses 25 000 when the caller gives 0 and no txn. | sub_1400D09E0, sub_140114A10 |
| Route check | StsGate looks up protocol and command in the route table for the conn program (conn+388). Unknown → ErrBadParam 10, module 8000, line 2405, text `StsGate: unknown protocol or command` | sub_1400FAFA0, sub_140114A10 |
| Client id lost | ErrBadParam 10, module 8000, text `StsGate: clientId not found, %u` | sub_140114A10 |
| Header `f` | If the frame has no `f` header with `appid`, StsGate adds `f` from the client app info (socket conn+424) | sub_140114A10 |
| NetAdjust | Protocol `Auth`: command id 1 (AccessTest) gets a local reply `NetAdjust: access allowed` (module 8009, line 1646). Other Auth commands go on. | sub_14017E990 |

### 9.2 IP check gate (socket conn+1108)

The first request of each client socket goes out wrapped in a
`CStsGateIpCheckTxn` (sub_140108CD0). StsGate adds `t` = socket conn+1080 to it.

| State | Meaning | Next request | Source |
| --- | --- | --- | --- |
| 0 | no check yet | wrap in IpCheck txn, send, state → 1 | sub_140114A10 |
| 1 | check pending | ErrTransPending 3006, module 8000, line 2496 | sub_140114A10 |
| 2 | allowed | normal send (vtable+32 with txn, vtable+40 without) | sub_140114A10 |
| 3 | flooding | route link vtable+0 (client id), ErrFlooding 3005, line 2513 | sub_140114A10 |
| 4 | IP not allowed | route link vtable+8 (client id), ErrNotAllowedIp 3721, line 2521 | sub_140114A10 |

The IpCheck reply sets state 2, 3 or 4 ❓ (the reply handler is not decoded).
Rust phase 1: no IP check. Start every socket in state 2.

### 9.3 Target header `t` per command

| Command | `t` | Source |
| --- | --- | --- |
| LoginStart | none. The protocol `Auth` selects the AuthSrv provider. | sub_1400E3D90 |
| KeyData, and later calls with a user | `<center>$<UserId>`. `center` = own datacenter (dword_1402B02C8, from StServer.ini, log `StsRoute: Datacenter changed`). `UserId` = GUID as `%08X-%04X-%04X-%02X%02X-%02X%02X%02X%02X%02X%02X`. | sub_1400E3B30, sub_1400E91D0, sub_1400E8DF0, sub_1400FC170 |

The userid key form matches `[InstantGlobalForwards]` (Auth and ExAuth by userid):
RouteSrv sends the key to the AuthSrv that owns the user. The `t` form is
`Target::Key` in `sts-core` route.rs.

The user reference comes from the LoginStart reply body: `<UserId>` (GUID),
`<UserCenter>` (u32), `<UserName>` (sub_1400FC8F0 with prefix `User`). CliGate
keeps UserId at conn+248, UserCenter at conn+240, UserName at conn+176 (64 chars)
(sub_1400D4030). If UserId or UserCenter is missing:
ErrBadParam line 1302 `CliConn: login start, invalid userref`.

### 9.4 Reply path

| Item | Fact | Source |
| --- | --- | --- |
| Dispatch | The txn reply calls the callback with (conn, txn, error, reply frame). Reply frame is null on error. | sub_1400D53C0 |
| Final | If error code ≠ 0xFFFFFFFF, the txn clears its conn pointer (one reply only) | sub_1400D53C0 |
| Conn gone | The txn conn pointer is 0, so the reply is dropped | sub_1400D53C0, sub_1400CE490 |
| Client id | The reply goes to the client with the client request id from txn+48 (`ClientReply`, conn+8 vtable+152) | sub_1400D4030 |
| Error | The backend error goes to the client unchanged (code, server, module, line, text) | sub_1400D4030 |
| Timeout | Code 42: CliGate also sends a report message with the user, the address, tag `Cli` and the reply name (for example `CCliConn::LoginStartReply`) | sub_1400D5410 |
| Bad reply | No body → ErrBadParam module 1 line 1293; no `Session` → 1297; no `KeyData` → 1306 | sub_1400D4030 |
| LoginStart OK | conn+328 = Session string. Client gets `<Reply><KeyData>…</KeyData></Reply>`, status 200 | sub_1400D4030 |
| LoginStart error | Session cleared; conn+296 += 1; at 3 the client socket closes (vtable+40) | sub_1400D4030 |

## 10. LoginFinish chain (3 backend calls)

LoginFinish makes three calls in sequence. The client gets one reply, from step 3.

```
Client Auth/LoginFinish
 └► step 1  Auth/LoginFinish           t=<dc>$<UserId>  ─► AuthSrv      reply cb sub_1400D3B20
     └► step 2  Access/GetAppUserRights t=0$<UserId>    ─► rights svc ❓ reply cb sub_1400D2A90
         └► step 3  Presence/Login      (StsGate login) ─► Presence ❓   reply cb sub_1400D35C0
             └► client reply <Reply>… (§10.4)
```

### 10.1 Step 1, Auth/LoginFinish (sub_1400D07B0, reply sub_1400D3B20)

| Item | Fact |
| --- | --- |
| Precheck | conn+300 = 0 (not logged in) and conn+304 bit 2 (KeyData complete). Else ErrPermission 49, module 1, line 1681 |
| Precheck | Session (conn+328) present. Else ErrPermission 49, line 1686 |
| No body | ErrBadParam 10, text `CliConn: login finish, missing body` |
| Frame | The client frame is used. CliGate removes the client headers `t`, `f`, `p`, sets body child `<Session>` = conn+328, copies the frame, adds `t` = `<dc>$<UserId>` (§9.3) |
| Reply error | Session cleared; error to client unchanged |
| Reply has `<AuthType>` | Secondary auth needed. Client gets `<Reply><AuthType/>[<Status/>]</Reply>` (module 1, line 1777). The login stops here. |
| Reply no body | ErrBadParam line 1763 `CliConn: login finish reply auth, missing body`; session cleared |
| Reply OK | CliGate keeps: `<Licenses>` (copied to conn+560), `<Roles><RoleId>…` (u32 list), `<PcCafe><PcCafeCode>` (32 chars), `<PasswordUntil>` (32 chars), `<UserStatus>` (u32). Then step 2. |

### 10.2 Step 2, Access/GetAppUserRights (sub_140117B40, sub_14016F460, reply sub_1400D2A90)

| Item | Fact |
| --- | --- |
| Request | `POST /Access/GetAppUserRights`, `t` = `0$<UserId>` (center 0 = local) |
| Body | `<Request><AppId>GUID</AppId><AppSecret></AppSecret><RedirectUri></RedirectUri></Request>`. AppId = Game.ini `[Configurations]` appId for the StServer.ini ConfigName (sub_1400D67F0). AppSecret and RedirectUri are empty strings. |
| Reply error | error to client unchanged |
| No txn record | ErrBadData 4 to the client |
| No body | ErrBadParam line 2200 `CliConn: missing body` |
| No `<AccessMask>` | ErrBadParam line 2204 `CliConn: missing accessMask` |
| OK | AccessMask = strtoi64 (base 0). CliGate stores it on the gate user (sub_140119F70). Then step 3. |

### 10.3 Step 3, Presence/Login (sub_140118010, builder sub_1400E7600)

CliGate makes a `StsGateUserConn` record for the user (0x608 bytes) and sends
`POST /Presence/Login`. The body is `<Request>` with: `UserStatus`, `Location`,
`User…`, `Alias`, `GameCode`, `WorldCode`, `GameAccountId`, `ClientNetAddress`,
`Status`/`AppDataSet`/`Aliases`/`Channels`/`Groups`/`Locations`/`ExternalAccounts`
flags, `ExternalAccount{AuthProviderCode, ExternalAccountKey, ExternalAccountName,
ExternalAccountData, ExternalUserAppKey}`, `AppId`, `AppSecret`, `RedirectUri`,
`Roles type="array"{RoleId}`, `PcCafe{PcCafeCode}`. The exact field values are ❓.

Rust phase 1: no Presence service exists. Do step 3 locally (no request) and
send the client reply of §10.4 with an empty gate body.

### 10.4 LoginFinishGateReply (sub_1400D35C0): client reply body

| Order | Element | Source of value |
| --- | --- | --- |
| 1 | `<UserId>`, `<UserCenter>` (prefix `User`, sub_1400FDDB0) | conn+248, conn+240 |
| 2 | `<Roles type="array">` with one `<RoleId>` per role | gate user roles (sub_14011A100, user +1480) |
| 3 | `<PasswordUntil>` if not empty | step 1 record +73 |
| 4 | each child of the Presence/Login reply body, except `UserCenter` and `UserId` | step 3 reply |
| 5 | raw XML from conn+600 if not empty (the stored Licenses ❓) | conn+600 |

| Item | Fact |
| --- | --- |
| Logged in | If the error code ≤ 0: conn+300 = timeGetTime() (login time, non-zero = logged in) |
| Session | conn+328 is freed in all cases |
| Error | error to client unchanged, no body |
| Status | 200 with `s: <id>R` |

## 11. RequestToken (32) and RequestGameToken (30)

Both use the same handler code (sub_1400D1B10 and sub_1400D1940 are equal).

| Step | Fact | Source |
| --- | --- | --- |
| 1 | Refresh the client app info of the gate conn (arg 130) ❓ | sub_1401178E0 |
| 2 | If the login used an external account (conn+408 ≠ 0): replace body `<AuthProviderCode>` with conn+440 | sub_1400D1B10 |
| 3 | One-way request (no request id): forward without txn | sub_14011A630 |
| 4 | Else: new CCliConnTxn with callback `ClientReply` (sub_1400CED50); forward the client frame through the generic gate path | sub_14011A3A0 → sub_140113F90 |

### 11.1 Generic gate path (sub_140113F90), user conn

| Check | Result | Line |
| --- | --- | --- |
| client id unknown | ErrBadParam 10 `StsGate: clientId not found, %u` | 2560 |
| unknown protocol/command, user conn | ErrPermission 49, text `permission denied '%s/%s', not logged in` (or `, game server`, or `, <user name>`) | 2603 |
| rate limit (conn+1104, 270000) ❓ | ErrFlooding 3005 `request flooding, …`; route link vtable+0 (client id) | 2610 |
| not logged in (conn+1116 = 0) | ErrPermission 49 | 2628 |
| client `f` header does not match the user | ErrBadParam `StsGate: invalid from header` | 2685 |
| `f` without user name | ErrBadParam `StsGate: incomplete from header` | 2689, 2697 |
| `f` name not the game prefix | ErrBadParam `StsGate: masquerading as non-game user, %s` | 2711 |

Header changes before send:
1. Remove the client headers `p`, `v`, `r`.
2. Set `f` = the logged-in user (UserId, center, alias or user name) (sub_1400E9650).
3. Add the client app info (`appid`) to `f` (sub_1400FCFB0).
4. If `t` has no center: resolve the user name with `$Name.GetUserNameById` or
   `$Name.GetIdByUserId`, then set `t` again ❓ (sub_1400E54C0).
5. Send with txn and 25 000 ms (vtable+32), or without txn (vtable+40).

### 11.2 ClientReply (sub_1400CED50)

| Item | Fact |
| --- | --- |
| Error | error to client with id txn+48 |
| OK | Remove header `t` from the backend reply. Copy the full reply (status, headers, body) to the client with id txn+48. |
| Partial | If the backend reply is not final, the client gets it with code 0xFFFFFFFF (partial, `s: <id>r`); the txn stays open |
| Timeout 42 | report message `CCliConn::ClientReply` (§9.4) |

Real client trace: RequestToken is sent two times (two AppIds) after
LoginFinish. AuthSrv makes the token (card auth-1).

## 12. Session lifecycle

### 12.1 Session states

The client conn keeps its state in three fields. There is no state enum.

| State | conn+328 Session | conn+304 bit 2 | conn+300 login time | Allowed Auth commands | Source |
| --- | --- | --- | --- | --- | --- |
| Connected | empty | 0 | 0 | LoginStart, LoginTokenStart, 18, 20, CreateAccount | sub_1400CFA50, §3 |
| LoginStarted | set | 0 | 0 | KeyData | sub_1400D4030, sub_1400D0130 |
| KeyDataComplete | set | 1 | 0 | LoginFinish | sub_1400D2E60, sub_1400D07B0 |
| LoggedIn | freed | 1 | timeGetTime() (≠ 0) | RequestToken, RequestGameToken, LogoutMyClient, generic gate path | sub_1400D35C0, §11 |
| Closed | — | — | — | none | sub_1400D4390 |

LoginStart or LoginTokenStart in any state first does Reset (§12.2), so the
state goes back to Connected (§3).

### 12.2 Reset (sub_1400D4390)

| Step | Fact |
| --- | --- |
| 1 | Write one log line with format `%s, %s, %S, %s`: tag, UserId, client address, login name |
| 2 | Tag = `LOGOUT` if conn+300 ≠ 0 (logged in), else `DISCONNECT` |
| 3 | Clear conn+300, UserId (conn+248), UserCenter, names, Session and the random buffers |
| 4 | Clear flag conn+304 bit 2 |
| 5 | If the conn was logged in: log out the gate user (StsGate user record, §10.3) and give the socket a new client id ❓ (the exact call is not decoded) |

Reset does not close the socket. The caller closes it.

### 12.3 Client Auth/LogoutMyClient (97) (sub_1400D1150)

| Condition | Result |
| --- | --- |
| Logged in (conn+300 ≠ 0) | Reset (§12.2), reply 200 OK with no body, then close the socket (vtable+72 ❓). Line 1733 |
| Not logged in | ErrUserNotOnline 3007, module 1, line 1736. The socket stays open |

CliGate does not send LogoutMyClient to AuthSrv ❓ (no forward call found in sub_1400D1150).

### 12.4 Bus request `Cli/Logout` (sub_1400D6A00 → sub_1400CEB30 → sub_1400D0EE0)

A backend service (for example AuthSrv for a duplicate login) sends `Cli/Logout` to CliGate.

| Item | Fact |
| --- | --- |
| Body | `<ClientId>` (u32), `<UserId>` (GUID), optional `<Force>` (string `Force` used in sub_1400D0EE0) |
| No match | No conn has this ClientId and UserId: ErrClientNotFound 3018 |
| `<Force>` present | Reset (§12.2), close the socket. Reply OK to the requester ❓ |
| No `<Force>` | CliGate sends a request to the game client (`t` = user). On the client reply (LogoutReply sub_1400D4600): reply to the requester, Reset, close the socket |
| Client reply timeout | Report message `CCliConn::LogoutReply` (§9.4) |
| Presence command 14 | Same path as `Cli/Logout` ❓ (command name not decoded) |

### 12.5 Client idle timeout

| Item | Fact | Source |
| --- | --- | --- |
| Config | `[IdleTimeout]` value in seconds, read at config load | sub_1400D6030 → sub_1400CF050 |
| Unit | seconds × 1000 = ms | sub_1400D5FB0 |
| Default and minimum | 300 000 ms (300 s). A smaller value becomes 300 000 | sub_1400D5FB0 |
| Log | `The idle timeout of ClientManager(%s) : %u seconds` and `The idle timeout of Old ClientManager(%s) : %u seconds` | sub_1400CF050 |
| Disconnect | The socket layer closes a socket with no traffic for the timeout. Log `StsSocket::%u, Disconnect - idle timeout for (%s)` | sub_140138A90 |
| Skip | If cfg `DisableIdleDisconnect` is set, log `StsSocket::%u, Disconnect skipped by cfg(DisableIdleDisconnect) - idle timeout for (%s)` and do not close ❓ (cfg source not found) | sub_140138A90 |
| On close | Socket close calls Reset (§12.2): log tag `LOGOUT` or `DISCONNECT` | sub_1400D4390 |

### 12.6 AuthSrv lost

No special code was found ❓. Open txns end with ErrTimeout 42 after 25 000 ms
(§9.1) and the client gets the error (§9.4). The client session stays open.
Rust phase 1: same behaviour. When the route client reports `ProviderLost` for
`Auth`, do nothing more.

### 12.7 Log strings

| Log | Source |
| --- | --- |
| `LOGIN`, `LOGIN-FAILED` | sub_1400D2E60 |
| `LOGOUT`, `DISCONNECT` (format `%s, %s, %S, %s`) | sub_1400D4390 |
| `The idle timeout of ClientManager(%s) : %u seconds` | sub_1400CF050 |
| `StsSocket::%u, Disconnect - idle timeout for (%s)` | sub_140138A90 |
