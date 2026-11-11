# 15 — AuthSrv: client login and token (original StServers)

Source: AuthSrv.exe 1.20.3350, IDA copy `ida/lanes/auth-1/AuthSrv.exe.i64`. No PDB.
Scope: the path that CliGate uses (LoginStart, KeyData, LoginFinish, RequestToken).
CliGate side: [11-cligate-protocol.md](11-cligate-protocol.md) §4, §9, §10, §11.

## 1. Dispatch

| Item | Fact | Source |
| --- | --- | --- |
| Command name table | id → name, protocol `Auth`. Same ids as CliGate §2. | sub_140110A30 |
| Ids in scope | 21 KeyData, 22 LoginStart, 23 LoginFinish, 30 RequestGameToken, 31 ConsumeGameToken, 32 RequestToken | sub_140110A30, table 0x1401AE1E0 |
| Parse | The request body is parsed into one request record (command id at offset 0) | sub_140110FC0, sub_1400D7710 |
| Parse error | text `AuMain: invalid message, parser line %u` | sub_1400D7130 |
| Pre-checks | sub_1400C94A0, sub_1400C9180, sub_1400DF820 run before the handler ❓ (not decoded) | sub_1400D7040 |
| Local or forward | If the request has a user and the user is not local (grid check), AuthSrv forwards the request to the owner with timeout 25 000 ms | sub_1400D68F0, sub_1400ECF00 |
| Handler switch | on the command id | sub_1400F3770 |

| Id | Command | Handler |
| --- | --- | --- |
| 21 | KeyData | sub_1400E5B30 |
| 22 | LoginStart | sub_1400E6910 |
| 23 | LoginFinish (also id 27) | sub_1400E6380 |
| 30 | RequestGameToken | sub_1400E7F50 |
| 31 | ConsumeGameToken | sub_1400E3D70 |
| 32 | RequestToken | sub_1400E8190 |

### 1.1 DB transaction record `CAuLoginTxn` (vtable 0x1401AA368)

| Offset | Field | Source |
| --- | --- | --- |
| +8 | reply id (the request id of the caller) | sub_1400E6910 |
| +12 | state (selects the reply handler) | sub_1400E6910 |
| +16 | command id | sub_1400E6910 |
| +24 | UserId (16 bytes) | sub_1400E6910 |
| +40 | SessionId (16 bytes) | sub_1400E6910 |

| Vtable slot | Use | Source |
| --- | --- | --- |
| +24 | dump: `ReplyId`, `ReplyType`, `Command`, `UserId`, `SessionId`, `SecAuthType` | sub_1400F69C0 |
| +32 | DB reply OK: `switch (state − 3)` | sub_1400F1FE0 |
| +40 | DB reply error: `switch (state)`; default sends the DB error unchanged to the caller | sub_1400F0B90 |

## 2. Login session record (0x210 bytes)

Made by sub_1400EE7C0 → sub_1400DD4E0. Kept in a list at service +224 (link offset at service +208).

| Offset | Field | Source |
| --- | --- | --- |
| +16 | SessionId: GUID. If GUID make fails: 16 random bytes. | sub_1400DD4E0 (sub_140103E20, sub_14010DBB0) |
| +32 | UserId (16 bytes) | sub_1400DD4E0 |
| +48, +64, +80 | copied from the request record (+304, +96, +112) ❓ (client address and app data) | sub_1400E6910 |
| +128 | SRP server object `CNpSrpServer` (wraps `CSrpServer`), random source sub_14010DBB0 | sub_1400DD4E0, sub_14012F880, sub_14012F230 |
| +136 | LoginName, max 65 bytes | sub_1400E6910 |
| +380 | validated flag (0 → log text `NOT VALIDATED`) | sub_1400E5B30 |
| +472 | Roles: u32 array | sub_1400DED50 |

Session delete: sub_1400F3F40 (unlink and free).

## 3. LoginStart (id 22)

### 3.1 Request (sub_1400E6910)

| Step | Fact |
| --- | --- |
| 1 | `<LoginName>` is required (request +1016). Else ErrBadParam 10 `missing login name`. |
| 2 | Make a login session (§2). Fail → code 4 `session create failed.` |
| 3 | Copy LoginName to session +136 (max 65 bytes). |
| 4 | If the user center (service +28) = own datacenter (sub_140157AF0): DB call `Exec` `AcctLoginInfoById` on logical DB `off_1402322A8` with schema version `v2016102001`, param = UserId (sub_140112220, sub_140112570, sub_1401124D0). Else: forward to the user datacenter (sub_14010F7D0). |
| 5 | Send with a `CAuLoginTxn`, state 7, command id from the request, SessionId = session +16, timeout 25 000 ms, route flag 287 (sub_140135670). |

### 3.2 DB reply, state 7 (sub_1400EDE00)

| Step | Fact |
| --- | --- |
| 1 | Find the session by txn SessionId. Not found → code 3101 `login start reply, session not found`. |
| 2 | Take the first row (sub_140112500). No row → `login start reply, missing row`; session deleted. |
| 3 | Parse the row (sub_1400D0410). Bad user ref → `AuthSrv: invalid user ref in row element`. Bad row → `login start reply, invalid loginname`; session deleted. |
| 4 | SRP start (sub_1400D1170): read row columns `Salt` and `Verifier` as binary (row vtable +224). If one is empty → code 3317, line 563, `no salt verifier`. Call SRP `Start(out keyData, loginName, salt, verifier)` (srp vtable +8). Fail → code 4, line 571, `start key exchange fail`. On error the session is deleted. |
| 5 | Store the row data on the session (sub_1400F57B0, sub_1400F5630) ❓. |
| 6 | Reply (line 4189): `<Reply><Session>GUID</Session><UserId>GUID</UserId><UserName>row UserName</UserName><UserCenter>row UserCenter</UserCenter><KeyData>…</KeyData></Reply>` (KeyData writer sub_1400F72D0). |

### 3.3 UserId in the STS `t` header

| Item | Fact | Source |
| --- | --- | --- |
| `t` parse | Form 2: GUID text, then optional `;params`. The GUID is the UserId. Form 3: text `kind:value`. Then UserId = zero and the text is kept (+24, value after `:` at +32). | sub_1401191A0 |
| Check | `t` parse fails → ErrBadParam 10, module 5, line 166 `AuMain: invalid to header`. UserId zero → ErrBadParam 10, module 5, line 170 `AuMain: missing userId`. | sub_1400D7710 |
| Result | AuthSrv does not resolve a name to a UserId itself. Each request (also LoginStart) must arrive with a GUID `t`. | sub_1400D7710 |
| Resolver table | At start AuthSrv registers, for protocol `Auth`: `login` → `$Name.GetIdByLoginName`, (one more kind, name not read) → `$Name.GetIdByUserName`, `gacct` → `$GameAccount.GetUserIdByAcct`, `userid` → `$Name.GetIdByUserId`. Each has the value 3002 and a flag (4, or 0 for `userid`). | sub_1400D7840, sub_140157E70 |
| Table use | The table is joined into one comma-separated text (sub_140156870) and used by the STS service code (sub_14014F470). | sub_140156870 |

So a caller sends LoginStart with `t=login:<LoginName>`, and a component before AuthSrv (the router side) calls `Name.GetIdByLoginName` and changes `t` to the GUID form ❓. The resolver is not in AuthSrv. The meaning of 3002 and of the flag is not decoded ❓. For the Rust port: the handler gets the UserId as an input (see the Rust card). A test resolver can map LoginName to UserId.

## 4. SRP-6a server (`CSrpServer`, vtable 0x1401BA5C8)

### 4.1 Group, hash and number format

| Item | Fact | Source |
| --- | --- | --- |
| N | 1024-bit. The value is equal to the RFC 5054 1024-bit group (first bytes `EEAF0AB9`). Stored as 32 big-endian u32 words at 0x1401BA540. | sub_14012DF20 |
| g | 2 | sub_14012DF20, sub_14012F450 |
| Hash H | SHA-256 (standard IV, standard digest byte order) | sub_14011E6B0, sub_14011ECA0 |
| H over many items | one SHA-256 over the items, in order, without separators | sub_14011E6B0 |
| Big number to bytes (`le(x)`) | little-endian, length = 4 × used u32 words (top zero words removed, so the length is always a multiple of 4) | sub_14017CD90 |
| Bytes to big number | little-endian | sub_14017CCC0 |
| Digest to number (`hnum(d)`) | reverse the order of the eight 4-byte groups of `d`, then read little-endian. (This is not the big-endian read of `d`.) | sub_14012DF20, sub_14012E960 |
| k | `hnum(H(le(N), le(g)))`. `le(N)` = 128 bytes, `le(g)` = `02 00 00 00`. | sub_14012DF20 |
| HNG | `H(le(N)) XOR H(le(g))`, 32 bytes, raw digest bytes | sub_14012DF20 (xmmword_1402955C0) |
| I | login name, ASCII `A`–`Z` changed to lower case, max 64 chars | sub_14012FA30, sub_140101830 |
| HI | `H(I)` (I without the end zero) | sub_14012F450 |

### 4.2 State and steps

| State (+8) | Meaning | Source |
| --- | --- | --- |
| 3 | new / done | sub_14012F230, sub_14012E960 |
| 1 | Start done, wait for client key data | sub_14012F450 |

**Start** (vtable +8, sub_14012F450; wrapper sub_14012FA30):

1. Fail (return 0) if I is empty, or the salt is not 8 bytes, or the verifier is not 128 bytes.
2. s = salt (8 bytes, little-endian). v = verifier (128 bytes, little-endian).
3. b = 32 random bytes (random source of §2), little-endian.
4. B = (k·v + g^b) mod N.
5. Out key data = `u32le len(le(s))` ‖ `le(s)` ‖ `u32le len(le(B))` ‖ `le(B)`.

**Step** (vtable +16, sub_14012E960). Input is the client key data.

1. Needs state 1. Else fail.
2. Read `u32le lenA` ‖ A (lenA bytes, little-endian) ‖ `u32le 32` ‖ M1 (32 bytes). All input must be used. A must be ≠ 0 and < N. Else fail.
3. u = `hnum(H(le(A), le(B)))`.
4. S = (A · v^u)^b mod N.
5. K = `interleave(le(S))` (§4.3), 64 bytes.
6. M = H(HNG, HI, le(s), le(A), le(B), K). Compare with M1. Not equal → fail.
7. M2 = H(le(A), M, K). Out key data = `u32le 32` ‖ M2. Complete = 1. State = 3.

**SessionKey** (vtable +24, sub_14012F020): K, 64 bytes.

### 4.3 interleave (sub_14012F030)

Input: `T = le(S)`, length n.

1. While n > 4 and `T[0]` ≠ 0: remove `T[0]`, n −= 1. (The code removes **non-zero** first bytes. Copy this exactly. The client uses the same code ❓.)
2. h = n / 2 (integer). `E[j] = T[n−1−2j]`, `F[j] = T[n−2−2j]`, for j = 0 … h−1.
3. G = H(E), Hf = H(F).
4. `K[2j] = G[j]`, `K[2j+1] = Hf[j]`, j = 0 … 31.

### 4.4 Rust test vector

No real client trace is available ❓. The Rust test makes its own vector: a
client side that uses §4.1–§4.3 (fixed b, fixed a, salt, password-derived x
❓ — the verifier formula of the account DB is not in AuthSrv). The server
must accept the client M1, return M2 that the client accepts, and both sides
must have the same K.

## 5. KeyData (id 21, sub_1400E5B30)

| Step | Fact |
| --- | --- |
| 1 | Find the session by request `<Session>` (request +840). Not found → ErrBadSession 3101 `login session not found.` |
| 2 | `<KeyData>` (request +992, length +1000) is required. Else ErrBadParam 10 `missing keydata.` |
| 3 | Each char must be Base64 (`A–Z a–z 0–9 + / =`). Else ErrBadParam 10 `invalid keydata.`; session deleted. |
| 4 | Decoded length bound 3·(len/4) = 0 → ErrBadParam 10 `invalid keydatalen.`; session deleted. |
| 5 | Base64 decode (sub_14011BD80). SRP Step (§4.2). |
| 6 | Step fails → session deleted, error ErrBadPasswd 11, module 6, line 7483, no body. Log `LOGIN-FAILED, <login name>`. |
| 7 | Step OK, but session +380 = 0 → session deleted, error ErrNeedEmailValidated 3320, module 6, line 7487. Log `NOT VALIDATED, <login name>`. |
| 8 | Step OK → reply code 0, module 6, line 7500: `<Reply>[<KeyData>Base64</KeyData>][<Complete/><SessionKey>Base64 K</SessionKey>]</Reply>`. KeyData only if out data is not empty. Complete and SessionKey only when the step completes. The session stays for LoginFinish. Log `LOGIN, <login name>` when complete. |

CliGate uses `SessionKey` for the RC4 filter (11 §4).

## 6. LoginFinish (id 23 and id 27, sub_1400E6380)

| Step | Fact | Source |
| --- | --- | --- |
| 1 | Find the session by request `<Session>`. | sub_1400E6380 |
| 2 | Path in scope: no SecondaryAuthSrv (dword_140283180 = 0) and no PC-cafe data. Then call the finish routine and delete the session. | sub_1400E6380, sub_1400F4250, sub_1400F3F40 |
| 3 | Reply elements, in this order: `LoginName`, `UserId`, `UserName`, `UserCenter`, `UserStatus`, `Roles`, `PcCafe/PcCafeCode` (only if set), `PasswordUntil` (only if set), `ExternalAccounts`. | sub_1400F4250 |

The error code when the session is not found is not verified for this handler ❓. KeyData (§5) uses 3101 for the same case.

## 7. Tokens

### 7.1 Token lifetime (config, sub_1400D6CF0)

| INI section | Key | Default | Stored as | Source |
| --- | --- | --- | --- | --- |
| `TokenTimeout` | `App` | 1800 s | 1 800 000 ms (dword_140283870, read by sub_1400D68D0) | sub_1400D6CF0 |
| `TokenTimeout` | `Game` | 300 s | 300 000 ms (dword_140283874) | sub_1400D6CF0 |
| `TokenType` | `App`, `Game` | 0 | dword_140283890, dword_140283894 ❓ (use not decoded) | sub_1400D6CF0 |

### 7.2 RequestToken (id 32, sub_1400E8190)

The handler gets the user record (arg 1) and the UserId (arg 2). The caller finds the user record ❓ (not decoded).

| Step | Fact | Source |
| --- | --- | --- |
| 1 | User record +164 (UserStatus) must be 1. Else ErrPermission 49 `Invalid UserStatus`. | sub_1400E8190 |
| 2 | Request AppId (request +792, 16 bytes) must not be all zero. Else ErrBadParam 10 `missing appId`. | sub_1400E8190 |
| 3 | Make the app token (§7.3) with AppId, request +304 (issuer AppId ❓), request +96 (client address, IPv4), request +832 (text ❓). Fail → code 4 `app token create failed`. | sub_1400E8190, sub_1400E07A0 |
| 4 | Token text = `<UserId GUID text>:<token GUID text>` (format `%s:%s`, max 75 bytes). GUID text as §8.1. | sub_1400E8190, sub_140128270 |
| 5 | Reply code 0 (line 8114): `<Reply><AuthnToken>token text</AuthnToken></Reply>`. | sub_1400E8190 |

### 7.3 App token store (`CAppToken`, 0x1B0 bytes)

| Step | Fact | Source |
| --- | --- | --- |
| 1 | Each user record has a list of app tokens (head at user +296, link offset at user +280). | sub_1400E07A0 |
| 2 | If a token with the same AppId (token +308) exists: log kind 3 (`OVERWRITTEN`), unlink it and delete it. There is one token for each (user, AppId). | sub_1400E07A0 |
| 3 | Token id = new GUID. If GUID make fails: 16 random bytes (as §2). | sub_1400E07A0 (sub_140103E20, sub_14010DBB0) |
| 4 | Make the record, add it to the user list, start a timer with the app lifetime (§7.1), log kind 0 (`CREATE`). | sub_1400E07A0, sub_140144EB0 |

Record fields (sub_1400D95B0):

| Offset | Field |
| --- | --- |
| +32 | UserId (16 bytes) |
| +48 | token GUID as text (max 257 bytes) |
| +308 | AppId (16 bytes) |
| +324 | issuer address text (max 55 bytes) |
| +380 | expiry tick = `timeGetTime()` + app lifetime ms |
| +384 | `TokenInfo` sub-object |
| +392 | value from request +304 (16 bytes) |
| +408 | copy of request +832 text |

Request fields for tokens (parser sub_140110FC0):

| Request offset | Element | Note |
| --- | --- | --- |
| +760 | `AccountAlias` | text |
| +792 | `AppId` | GUID. If it is not given or is zero, the parser copies request +304 into it. | 
| +984 | `GameCode` | text |

### 7.4 RequestGameToken (id 30, sub_1400E7F50)

| Step | Fact | Source |
| --- | --- | --- |
| 1 | User record +164 (UserStatus) must be 1. Else ErrPermission 49 `invalid user status`. | sub_1400E7F50 |
| 2 | `GameCode` (+984) and `AccountAlias` (+760) are both required. Else ErrBadParam 10 `missing parameter`. | sub_1400E7F50 |
| 3 | Make the STS request `GameAccount/ListAccounts` with the UserId (user +12) and body `<Request><GameCode>…</GameCode></Request>`. | sub_1400E7F50, sub_140114360 |
| 4 | Make a `CAuTxn` (0x160 bytes, state 19). Copy GameCode (max 16 bytes) to txn +104, AccountAlias (max 65 bytes) to txn +120, client address text (max 55 bytes) to txn +185. Txn +312 = request +832 text. | sub_1400E7F50, sub_1400DD6A0 |
| 5 | Send with timeout 25 000 ms (sub_140135670, flag value 3103 ❓). | sub_1400E7F50 |

Reply of `ListAccounts`, state 19 (`CAuTxn` vtable 0x1401AA238, slot +32 sub_1400F22C0 → sub_1400F4A40):

| Step | Fact |
| --- | --- |
| 1 | No reply body → code 4 `request game token reply, missing data`. |
| 2 | Find the first `GameAccount` child whose `Alias` is equal to txn AccountAlias (byte compare, sub_1401014B0, case-sensitive ❓). |
| 3 | No match, or no `GameAccountId` → code 4 `request game token, missing game account`. Empty `GameAccountId` → code 4 `request game token, invalid data`. |
| 4 | Make the game token (§7.5). Fail → code 4 `request game token, create token failed`. |
| 5 | Reply code 0 (line 1376): `<Reply><Token>GUID text</Token></Reply>`. The token is the game token GUID (token +32), GUID text as §8.1 (sub_140129F90 → sub_140128270). |

### 7.5 Game token store (`CGameToken`, 0xC0 bytes)

| Step | Fact | Source |
| --- | --- | --- |
| 1 | Make the record (sub_1400D9790). Add it to a list on the user record (head at user +264, link offset at user +256). | sub_1400E0A10 |
| 2 | Start a timer with the game lifetime (§7.1, sub_1400D68E0). | sub_1400E0A10, sub_140144EB0 |
| 3 | There is no "replace the old token" step (different from §7.3). | sub_1400E0A10 |

| Offset | Field | Source |
| --- | --- | --- |
| +32 | token: new GUID. If GUID make fails: 16 random bytes. | sub_1400D9790 |
| +48 | GameCode (max 16 bytes) | sub_1400D9790 |
| +64 | GameAccountId text (heap copy, length at +76) | sub_1400D9790 |
| +104 | client address text (max 55 bytes) | sub_1400D9790 |
| +160 | expiry tick = `timeGetTime()` + game lifetime ms | sub_1400D9790 |
| +168 | copy of request +832 text (length at +180) | sub_1400D9790 |

ConsumeGameToken (id 31) uses this store. See §7.7.

### 7.7 ConsumeGameToken (id 31, sub_1400E3D70)

Source: `ida/StServers/AuthSrv.exe`, read 11 Nov 2026. The handler gets the user record of the `t` header (arg 1).

| Step | Fact | Source |
| --- | --- | --- |
| 1 | No body → ErrBadParam 10, module 6, line 7933, `missing body`. | sub_1400E3D70 |
| 2 | Token (request +312, 16 bytes) all zero → ErrBadParam 10, line 7940, `missing token`. | sub_1400E3D70 |
| 3 | `GameCode` (+984) empty → ErrBadParam 10, line 7947, `missing game code`. | sub_1400E3D70 |
| 4 | Find the token in the game token list of the user (user +272): the GUID must be equal and the GameCode must be equal (sub_1401014B0). | sub_1400E9000 |
| 5 | Not found → error 3010, module 6, line 8038, no text. Log `failed to authenticate game token - user:%s, from:%s, error:%u(%u), app:%s, game:%s, token:%s` with sub-code 1. | sub_1400E3D70 |
| 6 | Address check: token address (+104) against request `ClientNetAddress` (+120), kind `game`. Equal → pass. Not equal → log `consume %s token from wrong location[u:%s,rip:%s,cip:%s]`; it still passes when GeoIP has no country data ❓ (the full GeoIP rule is not decoded). Fail → error 3303, sub-code 3, line 8038. | sub_1400F63B0 |
| 7 | Pass → log `game token is authenticated - user:%s, from:%s, app:%s, game:%s, token:%s`. | sub_1400E3D70 |
| 8 | If request +304 (16 bytes, issuer AppId ❓) is not zero: a second bus request with a `CAuTxn` state 20, timeout 25 000 ms ❓ (target not decoded). Else reply at once. | sub_1400E3D70 |
| 9 | Reply (sub_1400F3FB0): `<Reply>` with `GameAccountId` (token +64), `LoginName` (user +32), `UserId` (user +12), `UserName` (user +97), `UserStatus` (user +164 if user flag bit 3, else ❓), `UserCenter` (user +28), `Roles` (`type="array"`, `RoleId` list), and `TokenInfo/AuthProviderCode` only if the token text +168 is set. | sub_1400F3FB0 |
| 10 | The token is unlinked and deleted after a found token, in the pass and the fail case. One use only. | sub_1400F3EE0 |

Rust port: steps 1–7, 9, 10. Step 8 and the GeoIP rule are not ported ❓.

### 7.6 App token log (§7.3)

Log line (sub_1400DA5E0): `APPTOKEN,<kind>,UserId,…,AppId,…,Token,…,IssuerAppId,…,IssuerAddress,…,ExpireTimeMs,…`. Kinds 0–8: `CREATE`, `REVOKE`, `TIMEDOUT`, `OVERWRITTEN`, `VERIFY_SUCCEED`, `VERIFY_INVALID`, `VERIFY_WRONGLOCATION`, `REVOKE_ALL`, `VERIFY_NOTFOUND`. Other values: `UNDEFINED`. The case-to-name order comes from the string order in the function ❓.

## 8. DB calls

### 8.1 `AcctLoginInfoById` (LoginStart, §3.1 step 4)

| Item | Fact | Source |
| --- | --- | --- |
| Logical DB | `AcctDb` (off_1402322A8) | sub_1400E6910 |
| Schema version | `v2016102001` | sub_1400E6910 |
| Proc table | `procs.tsv` row 18 | — |
| Params | one param `<G>` = UserId as GUID text `%08X-%04X-%04X-%02X%02X-%02X%02X%02X%02X%02X%02X` (upper case). Byte order: bytes 0–3 as u32le, 4–5 as u16le, 6–7 as u16le, then bytes 8–15 in order. Example: bytes `00..0F` → `03020100-0504-0706-0809-0A0B0C0D0E0F`. | sub_140112570, sub_140128270 |
| Rows | the reply uses the first row only (§3.2 step 2) | sub_140112500 |

Row columns:

The column names come from the handoff of auth-1 and from the readers named below. The types are from the use in AuthSrv, except Salt and Verifier (sizes from §4.2). The other types are not verified ❓.

| Column | Type | Use | Source |
| --- | --- | --- | --- |
| `UserId` | GUID | user ref | sub_1400D0410 |
| `UserCenter` | int | reply `UserCenter` | sub_140128A20 |
| `UserName` | string | reply `UserName` | sub_140128A20 |
| `Salt` | Base64 → 8 bytes | SRP s (§4.2) | sub_14012CA30, sub_1400D1170 |
| `Verifier` | Base64 → 128 bytes | SRP v (§4.2) | sub_14012CA30, sub_1400D1170 |
| `PasswordUntil` | time | LoginFinish `PasswordUntil` | sub_1400F57B0 |
| `ValidateType` | int | validation mode | sub_1400F5630 |
| `Validated` | bool | session +380 (§5 step 7) | sub_1400F5630 |
| `Roles/RoleId` | u32 list | session +472 (§2) | sub_1400F57B0, sub_1400DED50 |
