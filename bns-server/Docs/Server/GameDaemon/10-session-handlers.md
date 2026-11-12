# 10 — Session receive handlers, build 381.8867

## Scope and notation

This file extends docs 04–07. It does not replace their limits.
This is a static specification. No client test ran.
`done` means that all game-logic paths for this receive operation were read.
It does not mean that a Rust port or a client test passed.
`partial` means that the listed paths still need research.
Build 348.8349 supplies names only. Build 381 supplies behavior and command IDs.
Addresses without a build label refer to build 381.
Full formats include command `h`. They exclude the outer size field.
Scalars use little-endian order. `h/E/u` are 2 bytes. `i` is 4 bytes.
`q` is 8 bytes. `c/Q` are 1 byte. Preserve raw bits where the sign is unknown.
`b/K` are nonnegative signed-16 byte counts followed by bytes.
`s` is counted UTF-16LE. `C` is a 2-byte repeat count, not a 1-byte scalar. Shared reader `0x14069DA00` consumes two bytes and returns their raw ushort bits. Callers can interpret the result as signed. A missing count returns low-16 `0xFFFF`. Semicolon ends a repeated group; it adds no wire byte. See docs 04–05 for other codec limits.
A handler that does not decode a body does not prove an exact-length check.
Do not add a trailing-byte rejection to such a handler.
PC routes require the dispatcher to resolve an authenticated PC.
Any-state routes do not have that requirement. See doc 09 for dispatch checks.
A reference release is not a socket close.
Use doc 07, part 4 for the conditional user-manager kick boundary.
Do not apply its BoardGacha kick rule to these handlers.
No runtime identity, address, password or key value is included here.

## 16 — G_PingReqCmd — done

- **381:** `0x140370210`. **348:** `HandleGame_PingReq`, `0x14036DF20`.
- **Wire:** `h` in the existing client constructor. The server does not decode the body.
- **Checks:** Any-state dispatch. No handler state, range, ownership, cooldown or table check.
- **State:** Write a debug log with the session ID. Return true.
- **Output/failure:** No reply, broadcast or kick. No daemon or DB call.

## 19 — G_GameKeyAuthReqCmd — done

- **381:** `0x14001AFE0`. **348 name:** `-`.
- **Wire:** No format decoder. Body bytes are unused. The command name does not prove an authentication body.
- **Checks:** Unauthenticated dispatch only. No checks in the target.
- **State/output:** Return true. No authentication change, reply, broadcast, failure, kick, daemon call or DB call.

## 22 — G_NPTokenAuthReqCmd — done

- **381:** `0x14001AFE0`. **348 name:** `-`.
- **Wire:** No format decoder. Body bytes are unused.
- **Checks:** Unauthenticated dispatch only. No checks in the target.
- **State/output:** Return true. No token processing, state change, reply, broadcast, failure, kick, daemon call or DB call.

## 34 — G_QueryWaitingNumberReqCmd — done

- **381:** `0x14001AFE0`. **348 name:** `-`.
- **Wire:** No format decoder. Body bytes are unused.
- **Checks:** PC dispatch only. No checks in the target.
- **State/output:** Return true. No queue lookup, state change, reply, broadcast, failure, kick, daemon call or DB call.

## 38 — G_LeaveWorldReqCmd — done

- **381:** `0x14001AFE0`. **348 name:** `-`.
- **Wire:** No format decoder. Body bytes are unused.
- **Checks:** PC dispatch only. No checks in the target.
- **State/output:** Return true. It does not leave the world. No reply, broadcast, failure, kick, daemon call or DB call.

## 307 — G_ClientConfigReqCmd — done

- **381:** `0x14001AFE0`. **348 name:** `-`.
- **Wire:** No format decoder. Body bytes are unused.
- **Checks:** PC dispatch only. No checks in the target.
- **State/output:** Return true. No configuration update, reply, broadcast, failure, kick, daemon call or DB call.

## 414 — G_CheckRoundTripTimeReqCmd — done

- **381:** `0x14039CCD0`. **348:** `HandleGame_CheckRoundTripTimeReq`, `0x14039C8E0`.
- **Wire:** `hqq`: command, opaque data1, opaque data2.
- **Checks:** PC dispatch. Decode through `0x14069F410`. No other checks.
- **State:** No game-state change.
- **Output:** On decode success, send 415, `hqq`, to the requesting PC. Echo both q values unchanged.
- **Failure:** On decode failure, log only. Return true on both paths. No kick or broadcast.
- **Dependencies:** No table, daemon or DB call.

## 416 — G_WriteLatencyLogReqCmd — done

- **381:** `0x14039CD70`. **348:** `HandleGame_WriteLatencyLogReq`, `0x14039C980`.
- **Wire:** `hQiiiii`: command, type byte, min, max, average, count, client-address bits.
- **Checks:** PC dispatch, then decode. No numeric bounds or table lookup.
- **State:** Log only. Types 0/1/2/3 select skill latency, skill latency overflow, RTT latency, RTT latency overflow. Other types select unknown.
- **Privacy branch:** If the daemon and config exist and config +1628 is false, replace the client-address value with zero for logging. The 348 field name is `_allowClientIPAddressInLogs`. The log also obtains server-side identity and connection metadata. No such values are part of this spec.
- **Output/failure:** No reply or broadcast. Decode failure logs only. Return true. No kick, daemon request or DB procedure.

## 11 — G_HelloReqCmd — partial

- **381:** `0x1403703D0`. **348:** `HandleGame_HelloReq`, `0x14036E100`.
- **Wire:** `hhqsqb`: command, protocol version, property version, publisher string, publisher-property version, opaque handshake bytes. Publisher decode capacity is 40 UTF-16 units. Handshake decode capacity is 4000 bytes.
- **Checks, in order:** Unauthenticated dispatch; decode; protocol equals 1278 and property version equals `0x12D261D120403`; publisher equals `PUBLISHER_CODES[daemon publisherCode]`; publisher-code switch; positive blob length; handshake-parameter operation.
- **Publisher comparison:** The UTF-16 comparison in `0x1403703D0` stops at the first NUL, not at the counted string end. The Rust decoder still consumes the complete counted string before it reads later fields.
- **Publisher switch:** Codes 1, 9, 11–15 do not check the extra version. Codes 2–8 and 10 require `0x40000000000`. Other codes fail. These are version constants, not keys.
- **State/output:** Compose 12, `hb`, with the opaque reply. Use the RSA-family handshake → AES-128 session step from doc 04. Send under the session lock. If session +256 is 3, set it to 5; release the old handshake processor at +272; clear +272; assign current processor +264 from session processor +280. The reply uses the old current processor before this assignment. Instructions in `0x1403703D0` and the 348 name guide agree.
- **Failure:** Return false after decode, version, publisher, empty-blob or operation failure. No explicit failure reply or user-manager kick in this function. ❓ The dispatcher/session consequence of false is not closed here.
- **Dependencies:** `0x1406A65D0` is the handshake-operation boundary. No DB or other-daemon call. Do not inspect key setup at `0x1406E3100`.
- **Rust boundary:** `Services::hello` supplies Unknown, Rejected or an opaque reply. Unknown never approves the operation. `Outcome::session_effects` records send-with-current-processor before session-processor activation. The adapter must apply this vector in order; these sends are not duplicated in `replies`. A known mode 3 becomes 5. The native 4000-byte packet leaves 3994 bytes for the reply after size, command and blob count. An invalid adapter length remains unsupported. No key setup or framing acceptance is implemented.
- **❓ Missing:** Full framing/processor acceptance remains outside this handler spec. No secret material is specified.

## 13 — G_MobileHelloReqCmd — done

- **381:** `0x140370890`. **348:** `HandleGame_MobileHelloReq`, `0x14036E510`.
- **Wire:** Repeat descriptor `hhsChh;`: command, protocol:h, publisher:s, count:C, repeated property-id:h and property-version:h. The repeat decoder starts after the command. Publisher capacity is 40 UTF-16 units.
- **Checks, in order:** Unauthenticated dispatch; decode initial two fields; protocol equals 1278; decode nonnegative repeat count; decode each pair; compare each pair to the switch below. The decoded publisher is not compared in this handler. Zero pairs succeed. No complete-set or uniqueness check appears.

| Property ID | Required version | 348 name guide |
| ---: | ---: | --- |
| 2 | 301 | CreatureProperty |
| 3 | 38 | WeaponProperty |
| 4 | 29 | CostumeProperty |
| 5 | 18 | GroceryProperty |
| 7 | 9 | QuestProperty |
| 8 | 9 | QuestList |
| 9 | 4 | EffectProperty |
| 10 | 3 | RecycleTimeProperty |
| 11 | 20 | GemProperty |
| 12 | 2 | DiscoveryArea |
| 13 | 2 | ActivatedTeleport |
| 14 | 4 | ClientOption |
| 15 | 20 | AccessoryProperty |
| 16, 17 | 4 | SummonedProperty, ExtraCreatureProperty |
| 20 | 3 | ContentQuotaProperty |
| 21–25 | 11 | AcquiredSkill1–5 |
| 26 | 7 | SkillPage |
| 27 | 9 | RandomStore; ID differs from 348 |
| 34, 35 | 1 | SkillSkinProperty, MentoringProperty |

- **Failure:** Send 15, `hh`, reason 2 for decode/count failure. Use reason 63 for protocol mismatch, unknown property ID or property-version mismatch. Return true on every ordinary branch. No direct kick.
- **Output/state:** Success sends command 14, `h`. Literal `0x1408CFBE8` is verified. No persistent state change, DB call, daemon call or broadcast in this function.
- **Repeat boundary:** Shared count reader `0x14069DA00` consumes two bytes. This handler converts it to signed short and rejects negative values. Thus the count range is 0..32767; missing count also takes reason 2. Pair decode failure takes reason 2. No numeric table lookup occurs; the property switch is a code constant list. No unread game-logic continuation remains.

## 18 — G_ServerPingReqCmd — partial

- **381:** `0x140370F10`. **348:** `HandleGame_ServerPingReq`, `0x14036EBC0`.
- **Wire:** `hqq`: command, sequence, timestamp. See doc 06 for timer behavior.
- **Checks/state:** PC dispatch; copy sent pair from PC +20712 for logging; decode. On success, overwrite received pair at +20728. This handler does not compare the echo against the sent pair before storage.
- **Failure/output:** Decode failure logs and returns false. Success returns true. No ordinary reply, broadcast, table lookup, DB request or daemon request.
- **STOP:** The conditional GameGuard call at `0x1403710A5` targets `0x14036DE10`. Do not follow it. ❓ False-return close consequence remains open. Timer policy remains partial in doc 06.

## 23 — G_LobbyTokenAuthReqCmd — partial

- **381:** `0x140371170`. **348:** `HandleGame_LobbyTokenAuthReq`, `0x14036EDE0`.
- **Wire:** `hhqq`: command, world ID, client key, lobby token. Decode return is not checked. Do not invent a native malformed-input result.
- **Checks, in order:** Reject arena config (`+237 != 0 && +248 != 0`) with 25/5. Decode. Require world ID >=1 and membership in daemon world list at +86748, count +86848; else 25/1937. Require nonzero client key; else 25/1939.
- **State/calls:** Build `LobbyTokenAuthReqParam` with accepted connection metadata. Construct `LobbyTokenAuthTran` at `0x14014C7B0`. Set transaction ID +24. Register through `0x1400D4FE0`, then dispatch through `0x1400D4A80`.
- **Failure/output:** Construction/registration failure sends 25, `hh`, reason 2. All visible paths return true. No direct kick or broadcast.
- **Transaction start:** Vtable `0x1409CE8B8` routes work to `0x14014CCB0`. Initialization `0x14014EE00` requires event 8 and a nonnull parameter. It copies world ID, client key, lobby token and accepted connection metadata into the transaction. It creates a pending user indexed by transaction ID. A duplicate pending-user index takes the 25 failure path. No authentication success occurs at registration.
- **Daemon request, verified in 381:** Work state 1/event 0 sends Lobby command 5, `hqqqi`: transaction ID:q, client key:q, lobby token:q, accepted connection-address bits:i. Route by the requested world ID. A send failure sends client 25/2 and removes the pending-user entry. No key values or connection values are recorded here.
- **Consume-token failure:** State 1/event 28 reads the backend reason short. Map 15/16→1939, 17→1936, 18/19/20→2, all others→1. Remove the pending-user entry, send client 25 `hh`, then transit to state 17. Source: `0x14014CCB0`, state-1 switch. This mapping does not apply to later transaction states.
- **Consume-token success subset:** State 1/event 27 calls `0x14014EFE0`. Require nonzero returned character ID and returned mode byte in 1..3. Copy account, character, charge/recovery and grade state into the pending user/transaction. Invalid response uses Lobby command 12 `hqq` to undo the consumed token before the client failure path. Later recovery, charge and DB paths remain incomplete.
- **Command 24, complete body:** Work state 16/event 0 sends `hQiQih`, then transits to state 17. Body order: charge-apply-type byte from transaction +1980; remaining-play-time:i from +1984; charge-base-type byte from +1988; charge-item-ID:i from +1992; reservations:h from +1996. Instructions `0x14014ED65..0x14014EDA7` verify all arguments. The 348 name guide is `LobbyTokenAuthTran::successLobbyTokenAuth`, `0x140156A40`; its command number is not used. The optional time-based-play branch runs before this send. Do not send 24 directly after Lobby command 5.
- **Rust subset:** The wrapper retains typed world/key/token fields only after queue success. `LobbyTokenAuthTransaction::registered` requires completed initialization and pending-user indexing. Its start emits Lobby 5. Its send callback and state-1 consume-failure callback preserve pending-user removal and termination plans. Unknown or out-of-order callbacks remain unsupported. The separate command-24 codec does not bypass the later transaction states and does not set authentication.
- **gw1 transaction evidence:** Build 381 `0x14014CCB0` uses the state/event table below. A successful send is not a successful callback. Cached commands use `hqq` (transaction ID, character ID) unless the table states otherwise. The backend decoder and procedure behind each command remain ❓.

| State | Start request | Success event → next state | Failure event / client reason |
| ---: | --- | --- | --- |
| 2 | Cached 289 | 164 → preload decoder `0x14014F5D0`, then 3 | Other nonzero event: backend 2/3→2, else 1 |
| 3 | Initialize character `0x14014F930` | 166 → 4; transaction mismatch logs but still advances | 167: 215→2, 216→1940 plus Cached 35, else 1 |
| 4 | Cached 358 | 355 → 5 when config +476 is set, else 7 | 356: 3/240→1950, else 1 |
| 5 | Cached 342 | 344 → 6 | 345: 3/232→1948, else 1 |
| 6 | Cached 339 | 333 → 7 | 334: 3/231→1949, else 1 |
| 7 | Cached 409, special route `0x14032AD30` | 335 → 8 | 336: 3/252→1951, else 1 |
| 8 | Cached 376 `hqqh`, final short from rule +548 | 337 → 9 | 338 → 1 |
| 9 | Cached 138 | 219 → 10 | 220 → 2 |
| 10 | Cached 330 | 339 → 11 when config +1354 is set, else 14 | 340: 3/226→1947, else 1 |
| 11–13 | Optional two-lane recovery | Both lane thresholds 1, 2, 3 advance to 12, 13, 14 | Helper-specific recovery remains ❓ |
| 14 | Cached 32 | 35 → load callback `0x140152BC0` | 36: 2/24/26/27→2; 25→1940 and `0x1403EF8E0`; else 1 |
| 15 | Lobby 30 `hqqq`: transaction, character, client key | 39 → charge callback `0x140153CF0`, then 16 | 40 → `0x1401540C0`: 36..41→1941..1946, else 1 |
| 16 | Optional time-based-play work, then client 24 | → 17 | No early command 24 |

- **Rollback boundary:** The ordinary pre-load failure paths send Lobby 12 `hqq` (character ID, client key), send client 25, remove the pending user, and enter state 17. Loaded-PC and charge failures also remove the user and release the World PC. Do not apply the pending-only rollback after PC binding. Transaction mismatch is checked at most later success events, but state 3 has the exception in the table.
- **Preload callback:** `0x14014F5D0` reads a signed record count. Each record contains a kind byte and nine dwords. Kinds 1..16 are valid. It stores kinds 1/3/4/7/10/11/12/14/15. Other valid kinds are ignored. Nonpositive count skips the loop. Decode and invalid-kind failures roll back with client 25/2. Instructions `0x14014F8AA..0x14014F8B4` set the reason to 2 and command to 25 before `0x1406A5D30`.
- **Initialize request:** `0x14014F930` builds seasonal/content definitions and calls `0x1403216A0`. It skips kind 2. Config +1356/+1357 filters groups. Fourteen requirement values come from table slot 2943. Config +1368 enables extra data. ❓ The Cached encoder, table field names and DB procedure remain open.
- **Load callback boundary:** `0x140152BC0` checks character ID, allocates a PC/cache route, decodes through `0x1405E47E0`, adds the PC through `0x1403F7040`, and binds the user through `0x1403EFD10`. Missing route/allocation uses Cached 35 and Lobby 12 rollback. Mode 1 starts charge state 15. Modes 2/3 set default rights 2 only when user rights are zero, copy rights to the PC, and enter state 16 with remaining time -1 and zero base/item/reservations. Their apply types are 1/0. **STOP:** The load callback contains excluded integration call sites `0x140153529→0x140001000`, `0x140153540→0x140001030`, `0x140153902→0x140705F80`, and an unidentified conditional integration `0x14015398D→0x140228D20`. Do not follow or implement these branches.
- **Charge success:** `0x140153CF0` applies the charge to the user before state 16. Copy response remaining time at +132, base at +136, item at +140, reservations at +144. The apply type is 0. A missing user does not authorize success. Charge-application failure performs loaded-PC rollback. ❓ Exact ownership, omitted failure reasons and conditional kicks remain open.
- **❓ Missing:** Backend reply codecs and DB procedures, PC decode/ownership, optional recovery helpers, complete rollback and conditional kick paths. Command 24 is no longer a body gap. No key material is specified.

## 26 — G_ReturnToLobbyReqCmd

End-to-end sequence: `Docs/Server/LobbyDaemon/23-return-to-lobby.md`.

### 26.1 Handler

- **381:** `0x140371500`. **348:** `HandleGame_ReturnToLobbyReq`, `0x14036F0F0`.
- **Wire:** No request-body decoder. Client packet size 16 (`game-commands.tsv` row 26).
- **Order:** (1) If arena config +237 is nonzero and +248 is nonzero: send 28 `hh` reason 5, return true. Do not follow the arena path. (2) Read the user at PC +20696. If the user is null or PC +20752 is nonzero: log only, return true. (3) Log `pcDbId` (user +216) and `cliKey` (user +248). (4) Call `UserManager::LogoutUser` `0x1403F0340` (348 `0x1403EAE90`). False → send 28 `hh` reason 1954, return true. (5) Allocate the transaction (0xA0 bytes, base constructor `0x14002F440(tran, user)`, vtable `0x140C42E80`, type value 205066 at +8, byte +104 = 0). Transaction ID = new ID from GameDaemon +5128, stored at +24. (6) Register in the transaction manager (`0x1400D4FE0`). Success → start with event 9 (`0x1400D4A80(mgr, tran, 9, 0)`). Register failure → send 28 `hh` reason 2, release the transaction. Source: `0x140371500` lines 11–74.
- **Important:** The user leaves the user manager in step 4, before the transaction runs. A later socket close does not start a `LogoutTran` (see 26.5).

### 26.2 UserManager::LogoutUser (`0x1403F0340`, 348 `0x1403EAE90`)

- Find the user in the active list by `PcDbId`. If found and `CliKey` and `AuthWaitKey` are equal to the stored user: remove the entry; send external-integration notices (348: `SecurityRemoveClient`, `StopGamePlayTime`, `ChangeStatus(2, 20)`; do not port, integration path); release one reference; return true.
- Else search the waiting list by `AuthWaitKey` and remove a match. ❓ The return value on this path (348 line 141 returns 0 on one path).
- No save, no zone leave and no notice to other players in this function. Other callers: `0x140037880` ❓, `0x14007B970` (socket close).

### 26.3 ReturnToLobbyTran (subclass of LeaveWorldTran)

348 names: `ReturnToLobbyTran::work` `0x14013BC10`, `init` `0x14013BBE0`, `failProcess` `0x14013BDC0`, `tryLeaveWorld` `0x14013BE20`, `leaveWorldOk` `0x14013BF80`, `leaveWorldFail` `0x14013C030`. Definition: `LeaveWorldTran::_def` (fin state 8). 381 vtable `0x140C42E80`: +0x30 work `0x14013AC70`, +0x38 init `0x14013AC40`, +0x40 failProcess `0x14013B0D0`.

Transaction fields (381): +24 ID, +32 state, +80 user, +88 PC object (optional), +96 showcase object.

| State | Event 0 (try) | Events | Next |
| ---: | --- | --- | --- |
| 0 | init `0x14013AC40`: event 9 → `LeaveWorldTran::init` `0x140138D80`; other event → state 8 | | 1 or 2 |
| init | If config +76: allocate the showcase object at +96. If config +1540 and the account name (user +132) is not empty: post a game-log record (kind 13) to service +5824. User +260 == 1 → state 1; else log "skip" → state 2. | | |
| 1 | `tryStopCharge` `0x140138FA0`: charge key (user +264) empty → state 2. Else Lobby 33 `hqqqs` (tid, pcDbId, cliKey, charge key) to world user +236. Send failure → log, state 2. | 41 ok `0x1401390B0` → 2; 42 fail `0x140139120` → 2 | 2 |
| 2 | `tryLeaveZone` `0x140139190`: PC with a zone → `World::leave(world, pc, tid, &arena)` posts the zone leave; wait. Arena result → arena branch (not followed; only that branch sets an expire token, event 52, when config `_transactionExpiredCheckEnable`). Zone null → PC cleanup (`0x140608770`, `0x14060DD30`), `0x1403EA930(user, 0)`, state 3. | 48 leave ok `0x1401396A0` (remove expire token, restore mobile position, `User::ResetPc`) → 3; 49 expire `0x1401397B0` (destroy zone object, reason "leave zone fail by expired(leave world)", `ResetPc`) → 3; 52 arena ok `0x140139990` → 3 | 3 |
| 3 | `tryUpdateCharacterInfo` `0x140139C20`: `SendUpdateCharacterDataReq` (`0x14014A700`) to Lobby (tid, pcDbId, …, showcase). Command 39 `UpdateCharacterDataReq`, format string `hqqqhb` (`0x14014A700` line 50, writer init with 39; only caller `0x140139C20`). Send failure → state 4. | 43 ok → 4; 44 fail → 4 | 4 |
| 4 | If config +1946 and `User::AutoTakeTimeBasedReward(user, tid, 1, 0, 0)` (`0x1403EC9E0`) is true: wait. Else state 5. | 47 response → 5 | 5 |
| 5 | `tryUpdateAccountInfo` `0x140139DF0`: `User::CalcTimeBasedPlay`, then `SendUpdateAccountDataReq` (Lobby 42; tid, pcDbId, cliKey, bot score, time, time-based-play list, account exp, content quota, recycle times). | 45 ok `0x14013A310` → 6; 46 fail `0x14013A380` → 6 ❓ | 6 |
| 6 | `tryUnloadCharacter` `0x14013A820`: Cached 35 `hqq` (tid, pcDbId) on the hashed cache channel. | 37 ok → 7; 38 fail → 7 | 7 |
| 7 | `tryLeaveWorld`: if +96 set, `clearArenaSession(pcDbId, +96 name)` (`0x14013AAB0`). Lobby 8 `hqqq` (tid, pcDbId user +216, cliKey user +248) to world user +236 (send at `0x14013AEFC`). Send failure → client 28 `hh` 1952, return false (→ fin 8). | 31 → client 27 `hq` (token key from Lobby 9), state 8. 32 → reason short r; r in 21..23 → 28 `hh` 1954, else 28 `hh` 1. State 8. | 8 |

- Each step failure does not stop the leave: the next step runs. Only the send failure at state 7 ends the transaction early.
- **Lobby replies:** 381 handler `0x14013EB10` decodes 9 `hqq` and runs event 31 with a pointer to the token key. `0x14013EBF0` decodes 10 `hqE` and runs event 32 with the reason. Unknown transaction ID → log only, no client reply.
- **failProcess** `0x14013B0D0`: send client 28 `hh` 1952. ❓ Trigger (expiry/manager path) not read.
- **Timeout:** No expire token is set for states 1, 3, 5, 6, 7 on the ordinary path (expire tokens: callers of `GameTranExpireToken::Set` 348 `0x140225AE0`; only the arena branch of `tryLeaveZone`). The global checker lifetime is set at 348 `GameDaemon::initNetwork` → `SetExpireChecker`. ❓ Value.
- **Client 27:** `hq`, lobby token key (Lobby 9 field 2). Size 24. Client 28: `hh` reason; reasons 5, 1954 (handler), 2, 1952, 1954, 1.
- **Socket close:** The transaction does not close the client socket. ❓ The client closes it after 27.

### 26.4 Quirks

| Name | Address | Behaviour | Port |
| --- | --- | --- | --- |
| `quirk_rtl_no_timeout` | `0x14013AC70` state 7 | No expire token: if the Lobby never replies 9/10, the transaction and the client wait without end. | fixed (add a timeout that sends 28/1952; see `Docs/Plan/cards/lp2-3-rust.md`) |
| `quirk_leave_fail_as_enter_fail` | LobbyDaemon `0x140088470` lines 82, 101 | Job init/start failure sends 7 `EnterWorldFail` (reasons 2, 4), not 10. GameDaemon ReturnToLobbyTran has no event for 7, so the transaction waits (see above). | fixed (send 10) |

### 26.5 Other exits that send Lobby 12 LogoutNotify

- **LogoutTran** (348 vtable `0x140904180`, work `0x14013C130`; 381 vtable `0x1408D5BB0`, work `0x14013B180`). Same states 1–6 as 26.3 (state 2 uses the logout reason). Final step (348 `LogoutTran::logoutNotify` `0x14013C2B0`; 381 send `0x14013B40C`): `clearArenaSession`; Lobby 12 `hqq` (pcDbId, cliKey) to world user +236; if reason at +162 is nonzero, client 951 `hU` (348: 917) reason; state 8.
- **Start 1 — socket close** `0x14007B970` (348 `ClientSessionHandler::OnClose` `0x14007D850`): needs a user and session byte +208 zero. Arena config → `UserManager::Kick`. Else `LogoutUser`; true → new LogoutTran, start event 9. False → log only (this is the path after ReturnToLobby).
- **Start 2 — kick** `0x1403EF8E0` (348 `UserManager::Kick` `0x1403EA430`, 26 callers in 381): arena branch not followed. Else find the user (`0x1403F0040`), then LogoutTran with the kick reason, event 9. ❓ Lines 64–98 detail.
- **KickTran** (Lobby 14 KickReq; 381 work `0x14013B4C0`): kick ok → Lobby 15 (lobby tid) `0x14013B700`; failProcess → Lobby 16 `hqE` (lobby tid, 2) `0x14013B6B0`.
- Lobby 12 `hqq` is also sent by the lobby-token-auth rollback (`0x14014CCB0`, many sites).

## 29 — G_CleanUserConnectionInfoInCligateReqCmd — done

- **381:** `0x1403717A0`. **348:** `HandleGame_CleanUserConnectionInfoInCligateReq`, `0x14036F360`.
- **Wire:** No body decoder.
- **Checks/order:** PC dispatch. If arena config +237 and +248 are nonzero, log and return true without sending.
- **Daemon call:** Otherwise call `LobbyDaemonChannelManager::Send` (`0x140149740`) for PC world ID at +96. Send command 17, `hqQQ`: PC DB ID from +20744, zero byte, one byte. Exact format bytes at `0x14099C248` are verified.
- **State/output:** No local game-state mutation. Log send success or failure. No client reply, broadcast, DB procedure or kick. Return true. No daemon-response wait is installed here.

## 31 — G_EnterWorldReqCmd — partial

- **381:** `0x1403718A0`. **348:** `HandleGame_EnterWorldReq`, `0x14036F470`.
- **Wire:** `hic`: command, clientVersion:i, characterId:c. The 348 typed function resolves the misleading log noted in doc 05: the integer is `_clientVersion`; the byte is named `characterId`. The byte is logged but is not used to select a character in the ordinary code read here.
- **Checks/order:** PC dispatch; reject arena config or PC arena state 2, 3, 5 or 6 under PC lock +37480; decode; reject `_receivedEnterWorldReq` at +12576.
- **State:** Set +12576 to true and store clientVersion at +20704. These changes precede all later failures.
- **STOP:** The optional integration virtual call at `0x140371AC8` is named `SecurityAddClient` in 348. Do not follow this path or its integration continuation. The ordinary path resumes at composite send `0x1405F5980`.
- **Ordinary continuation:** Send initial PC composite through `0x1405F5980` before destination resolution. Copy PC position +268/+270/+272 and yaw +274. If game-packet-right +20672 has bit 8, save geo-zone +264 and position at +38728..+38736. Only this branch requires MobileZone at daemon +5888. Set the target runtime zone and geo-zone from MobileZone. Read mobile spawn index 1, if available; use its pose only when record kind is 1. Missing/wrong-kind spawn records retain the copied PC pose; they do not call the translator. The branch without bit 8 calls `Pc::TranslateZoneOnEnterWorld` at `0x1405F1230`. Then call `World::Enter` at `0x1403F4A60`.
- **World entry subset `0x1403F4A60`:** A null PC fails. A zero zone ID uses the World default. Resolve the runtime zone through `0x1403F6A20`; missing zone fails. Return the spawn-queue result from `0x14022EC20`. Optional party/guild work follows that call. Its task-allocation failure does not replace the saved result. A successful queue is not a completed spawn.
- **Output/failure:** Success composite is 32; the complete field layout and allocation groups are below. Failure is 33, `hh`: arena 5; decode/duplicate 2; missing mobile zone 2; destination translation 1; World::Enter failure 1. Translation and World::Enter failure return false. Other handled branches return true. The omitted reason arguments are verified in instructions. No direct user-manager kick is shown.
- **Command 32 fixed prefix:** Builder `0x1405F5980` writes `hqQihhhhh`: world ID; runtime object ID; jumping-character-state byte; chat client address dword; chat client port; chat world ID; manage port; signed time-zone bias; user game-packet rights (zero without a user). Reserve 25 bytes. Chat world ID uses arena ID +10000 or daemon world ID when config +492/+512 are both set; otherwise call `0x14033ED20(1)`. Manage port is zero unless config +160 is set. The complete property table and native descriptor are in the gw1 evidence section below.
- **Command 32 suffix:** After all 422 properties, write extra properties as counted bytes; the opaque broadcast block as counted bytes; recycle count and `(group:c, remaining:i)` records; exactly two acquired-skill pages `(page:Q, skills:b)`; four config bytes at +704/+708/+712/+716; PC-cafe publisher byte; PC DB ID:q; content-ban count and `(index:Q, from:q, to:q)` records; test-server Boolean; craft-union count and `(id:c, level:h)` records; time-based-play count and records; slate count and records. Do not add a second count around packed property bytes.
- **Suffix sources and limits:** `0x140618730` scans recycle groups 1..25 and clamps expired signed remaining time to zero. Acquired-skill property 2 comes from pages 1 and 2. Content bans scan indices 0..8, only with a user and config +721; omit a record only when both times are zero. Craft scans two slots; only IDs 1..14 emit a row. `0x1403E8C00` emits two time records with a user, otherwise zero: index:Q, schedule:i, expire:q, last-reset:q, duration:i, acknowledged-time:h, four reward bytes. Instructions `0x1403E8D2A..0x1403E8DA9` verify all ten arguments. The start-time and activated fields are not sent. `0x1401BB5E0` emits two slate pages: page:c, six `(item:i, level:c)` pairs, nine stone IDs:h, new stone ID:h, new stone position:c. Build-348 names: `TimeBasedPlayInfoList::ComposeToClient` at `0x1403E2F10`, `SlateContainer::ComposePureSync` at `0x140012D20`, `Slate::ComposePureSync` at `0x140010E50`.
- **Fragment allocation:** Build-381 splitter `0x1400DDE50` writes command and a four-byte mark after the size. First mark accumulates total payload; intermediate mark is -1; `0x1400DDF70` sets the final continuation mark to zero. `0x1400DDFE0` clears pointers. Builder allocation groups are: fixed prefix; whole creature-property block; each counted extra/opaque block; whole recycle list; skill count; each skill page; four config bytes; publisher/DB-ID pair; whole ban list; test flag; craft count and each three-byte row; time count and each 31-byte row; slate count and each 52-byte page. None of these groups splits across buffers. Capacity is 4000 bytes including the eight-byte fragment header. The new codec rejects an oversized indivisible group; this is an adapter error, not a new native client-failure rule.
- **Post-32 order:** Before destination selection, send the summon initial packet when a summon exists (`0x1405F6E80`), quest-list snapshot (`0x140571920`), completed quests (`0x1405F9A00`), current UTC time (`0x1405F88C0`, command 423), and builder/mentoring/collecting/fishing/additional sync through `0x1405F52F0`, `0x1405F9BA0`, `0x14007FD60`, `0x1400B98C0`, `0x1405FE760`. Store entry tick at PC +20792 after UTC send. Rule config +1028 enables the raid-unbind list (`0x1401A5B10`). ❓ Several packet bodies and side effects remain unread. Sending command 32 alone does not authorize destination processing.
- **Destination evidence:** `0x1405F1230` reads source geo-zone, saved return-spawn key, table slots 3267/3343/3243, and special destination slot 3027. Source subtype 74 or byte +1227 uses destination +1296, random bounds at +24..+34, retained yaw, and a terrain query. Restriction results 2/8 select rule +1080/+1084. Subtype 87 with signed PC faction <=0 selects rule +1088/+1092. Call `0x1405EFBD0` with mode -1 and entry flags. Reason 1154 can send 33/83 on the slot-3267 +43 fallback branch. Final spawn key combines geo-zone and spawn index; a found record supplies XYZ at +24/+26/+28 and yaw +36. The visible translator returns true on its terminal paths. ❓ Destination allocation, selector helpers, table names and full fallback semantics remain open.
- **Rust command-32 boundary:** `Services::initial_world` supplies a complete typed snapshot for the current PC. Its default is unknown (`None`). The codec validates all 422 property values before construction. The handler checks the supplied runtime and DB identity before encoding. Identity and encoding errors remain adapter errors; they do not create native command-33 reasons. After a valid encoding, `SendFragmentWithCurrentProcessor` records each complete size/command/mark/body fragment in order. The adapter must not add a second header or duplicate these sends in `replies`. The handler retains the entry latch on every later error. It stops with `Unsupported` before post-32 sync and destination work. It does not queue or authorize travel.
- **❓ Missing:** Destination/world-entry callees, post-32 sync packets, DB source, table mapping and failure cleanup. The command-32 wire body is specified and connected to the receive plan through the snapshot boundary. No DB-backed snapshot provider or socket adapter is supplied. Do not reset +12576 on failure without evidence.

## 42 — G_EnterZoneReqCmd — partial

- **381:** `0x140371EE0`. **348:** `HandleGame_EnterZoneReq`, `0x14036FAA0`.
- **Wire:** `hiuuuh`: command, spawnActionKey:i, summoned x/y/z shorts, summoned yaw:h. Decode return is not checked. See doc 05 for the field correspondence.
- **Checks/order:** PC dispatch; snapshot zone +2224; decode; require zone; require user +20696 with +20752 clear. Missing user logs only. Missing zone sends 44, `hE`, reason 2.
- **State/calls:** Allocate `EnterZoneTask` with vtable `0x140BC1628`. Attach zone through `0x14025DB30`. Store caster ID +128. Add task as PC child through `0x14069BF60`. Set child flag +138. Copy correlation to +144 and summon pose to +148..+154. Queue through `0x140254FD0`.
- **STOP:** The conditional anti-bot delay call at `0x1403721D5` targets `0x1402550E0`. Do not follow that restricted branch. Zone-entry continuation also stops at anti-cheat reference `0x1405DF767` in `0x1405DE040`.
- **Task `0x1402912F0`:** A caster kind other than 1 logs only. Resolve the current spawn action through PC action-manager virtual +88 with argument zero. Missing action sends 44/2. Action state other than 2 sends 44/1149. Only a correlation mismatch at action +208 sends 44/1150. On match, call `0x1404BF580` with the summoned pose, then call `0x1405F1FA0`, unlink the task child and release it. Replies use `hE`. Do not combine the three failure cases.
- **Spawn subset `0x1404BF1E0`:** Set properties 99, 100 and 101 from the supplied position in that order. A failed set ends the action and returns false. Raise the local Z to the supplied floor when necessary. A nonzero spawn context writes creature +2328. Call `0x1402304A0` before the later `0x14026A180` update. A failed later update ends the action; it does not retract a prior send. Yaw uses property-102 bounds from metadata. Do not treat command 43 as proof that every later mutation succeeded.
- **Summon subset `0x1404BF580`:** PC spawn runs first. For a live summon and PC job 6, test the client summon position against terrain, environment availability and the squared configured distance at rule config +188. On rejection, use the PC spawn pose. Raise summon Z to the floor. Pass the client summon yaw through the property bounds. Successful summon spawn sets control state +6984 to 1. Other summon branches and cleanup remain partial.
- **Failure/output:** Queue failure sends 44, `hE`, reason 2, verified in instructions. No direct kick. No DB/daemon call in the wrapper.
- **Spawn action start:** `0x14022EC20` requires an open zone and a supported native creature kind. Ordinary PC entry allocates action class 2 and stores pose, yaw, reason, context, flags and a nonzero incremented correlation. `0x1404BEAE0` resolves any follow target, validates terrain through `0x1404C1A70`, calls prepare `0x1404BEED0`, links the pending spawn action, and starts a 1,800,000-tick timeout. Validation tries the requested cell first, then geo-zone spawn 1 of kind 1. Reject unavailable environments. Clamp Z into the selected floor/ceiling interval. A failed validation ends the action.
- **Prepare 41:** `0x1404BEED0` sets PC zone and XYZ; mirrors zone/XYZ to the summon when present; increments PC and summon transition counters. Format `hqhiiccKuuuhhh`: runtime zone ID:q, channel:h, geo-zone ID:i, correlation:i, config byte:c, action flag:c, zone block:K, XYZ:u/u/u, yaw:h, PC transition:h, summon transition:h (zero without summon). The authoritative action pose supplies the packet, not the later client summon pose. Additive codec `packet::zone_entry::PrepareZone` encodes this full packet. `EnterZoneOk` encodes only command 43 and its context qword. These codecs do not run action preparation, appearance delivery or spawn completion. The receive handler does not send them from an unresolved continuation.
- **Appearance/43 order:** `0x1402304A0` inserts into the ordinary or mobile spatial structure, runs the creature spawn virtual, records the spawn, and queues appearance through `0x14024B0C0` before client 43 `hq` with the action context. It then calls `0x140254770`. The mobile path requires its broadcast structure. Ordinary PC entry also updates the zone enter list and population state. The appearance queue fans out through the cell and up to nine neighbors; it does not immediately compose a known network packet. ❓ Appearance builder, recipients, wire-delivery order and the creature spawn virtual remain open. Internal summon event 2560 is not a network command.
- **Summon branches:** A live summon on a job other than 6 takes cleanup. After a successful job-6 spawn, set control state 1, then call `0x14061D450` and `0x1405F3FE0`. A nonlive summon can queue `0x1401BBC30`/`0x1401BBCB0` with reason 5 when attraction virtual +312 permits it. Owner cleanup uses `0x14009D8C0`. Allocation/initialization failure and normal completion detach the spawn action and call its finish virtual +208. ❓ Attraction variants, summon-control updates, cleanup and the finish implementation remain open.
- **Enter-zone condition check:** `0x1405F1FA0` checks party conditions for subtype 38 and reserved-party/zone identity for subtype 77. Mismatches call `0x1405ECC70`. It clears PC +20680/+20688 at the end. ❓ The leave helper and complete party behavior remain open. The task calls this check even when the spawn helper failed; it does not retract an earlier command 43.
- **❓ Missing:** Spawn virtual/finish and timeout paths, appearance packet builder/recipients, summon-control and cleanup helpers, party-condition leave path, downstream DB/daemon effects. Restricted integration branches remain excluded.

## 929 — G_ContentsRuleListReqCmd — partial

- **381:** `0x140370250`. **348:** `HandleGame_ContentsRuleListReq`, `0x14036DF60`.
- **Wire:** No request-body decoder. Do not infer an exact-length check.
- **Checks:** Any-state dispatch. Read last request tick `L` from session +200 extra-data pointer, or use zero. Compute `T = GetTickCount64() - daemon start tick`.
- **Order:** Allow if `L <= 0`, `L <= ConfigLastReloadTime`, `T <= 30000`, or `T-L >= 30000`. These are signed 64-bit comparisons in `0x140370250`. Preserve the raw tick bits. A future positive L does not pass through unsigned subtraction. Otherwise reject.
- **State:** On allow, copy the cached packet at `0x140EE5888`, if present. Store T through the extra-data pointer, if present. Apply the existing session encryption step to the copied buffer. Queue it under the session lock through `0x1406A6420`. Clear the temporary packet. No cryptographic internals are specified.
- **Failure:** Send 931, `hE`, reason 2167 to this session. No direct kick. Return true.
- **Dependencies:** No DB or other-daemon call in the receive handler. Reload-time global: `0x140EE5898`.
- **Cache owner:** Initial load `0x140087330` reads 167 rule definitions and calls builder `0x140087D40`. Reload `0x140087720` reads 18 reloadable definitions, clones the config, increments revision `0x140EE586C`, then calls the builder. Success atomically replaces cache/config and stores daemon-relative reload tick in `0x140EE5898`. Failure decrements the revision and keeps the previous cache/config. Old objects enter deferred-release lists.
- **Success builder `0x140087D40`:** Creates command 930 with fragment handling. It converts an XML header, supported option entries and the closing config element to UTF-8. Each entry has a counted byte block. Types 1/2/3/4/6 select formatting branches. The loop covers 167 definitions. Do not copy the build-348 command ID.
- **Tick prefix:** The wrapper writes T before encryption/send, including the null-cache branch. A missing extra-data pointer means no tick storage. The Rust plan records this verified mutation and still stops before the incomplete send contract.
- **❓ Missing:** Exact build-381 fragment prefix/metadata, config option field mapping and null-cache send behavior. Do not substitute an empty success packet.

## 174 — G_ClearLogoutPartyMemberReqCmd — partial

- **381:** `0x14038DF10`. **348:** `HandleGame_ClearLogoutPartyMemberReq`, `0x14038D6A0`.
- **Wire:** No body decoder.
- **Checks/order:** Require PC zone +2224. Construct task through `0x14036D800`. Initialize through `0x1402B4920(zone, PC)`. Queue through `0x140254FD0`. Each failure sends 175, `hh`, reason 2. Return true.
- **State:** Task gets zone/caster and child relation. No direct party mutation in wrapper.
- **Task:** `ClearLogoutPartyMemberTask`, vtable `0x140BBA228`, start `0x1402B49D0`. Require nonzero party ID at PC +21112, else send 175/2. With a party, call `TeamManager::ClearExpiredPartyMember`, `0x1401DD540`. The 348 name guide is start `0x1402B7F90`.
- **Manager subset:** Acquire manager then team lock. Resolve team by runtime PC ID; failure sends 175/2. If team byte +124 is set, require leader account/object pair +112/+104 to match PC +20744/+40; mismatch sends 263/2. Otherwise require helper `0x1401EE320` to accept account/object and mode 3; failure sends 175/2. Iterate party indexes in team +88, count +52. Resolve each party through `0x1401EE540`; skip parties with nonzero challenge link +176. Gather expired-member results through `0x1401EE620`. Dispatch result-specific removal helpers before notification `0x1401EF4D0`, which runs after manager unlock while team lock remains held. No generic success packet is proved.
- **Output/calls:** No immediate success or broadcast. No direct DB/daemon call or kick.
- **❓ Missing:** Exact leader-state meaning of team +124, membership-mode 3 rule, expiration predicate, result mutation helpers, notification bodies/recipients, DB/daemon work and all late failures.

## 262 — G_ClearLogoutTeamMemberReqCmd — partial

- **381:** `0x140393200`. **348:** `HandleGame_ClearLogoutTeamMemberReq`, `0x1403931A0`.
- **Wire:** No body decoder.
- **Checks/order:** Require config +725 (`_teamSystemEnable` in 348). Require zone +2224. Construct and initialize the same task as command 174. Queue through `0x140254FD0`.
- **Failure/output:** Any failed wrapper check sends 263, `hh`, reason 2. Return true. No direct kick, success or broadcast.
- **State/dependencies:** Same queued task as 174. No direct team mutation or DB/daemon request.
- **Deferred output:** Shared task `0x1402B49D0` still sends 175/2 when party ID is zero, even for request 262. The manager also has both 175/2 and 263/2 branches. Preserve this distinction.
- **❓ Missing:** Same remaining manager, mutation and notification paths as 174.

## 300 — G_ChannelListReqCmd — partial

- **381:** `0x140394150`. **348:** `HandleGame_ChannelListReq`, `0x140394200`.
- **Wire:** `h`. Decode return is ignored.
- **STOP:** Restriction helper call at `0x140394181` targets `0x14060E280`. The 348 name identifies an anti-bot restriction query. Do not follow it.
- **Ordinary checks/state:** After the permitted branch, require zone. Allocate ChannelListTask, vtable `0x140B95958`. Require native PC kind +46 equal to 1. Attach zone/caster and child relation. Store geo-zone ID at task +144. Queue. Wrapper failure sends 302, `hh`, reason 2. Return true.
- **Task `0x140296990`:** Under World ZoneMgrCS at daemon +5896, find current geo-zone in PersistentZoneList at +5864. Copy `maxChannelCount` from channel-list +0 and 51 enable bytes from +3208..+3258. Unlock before send. Missing entry sends 302/1189. Found entry sends 301, `hhb`: maximum channel count, counted 51-byte enable block, to requester only.
- **State/calls:** No list mutation, broadcast, DB or daemon request in the ordinary task. No direct kick.
- **❓ Missing:** Restricted branch remains excluded. Channel-list population and the meaning of each enable byte remain open. Do not synthesize list entries from table count.

## 303 — G_ChangeChannelReqCmd — partial

- **381:** `0x1403943A0`. **348:** `HandleGame_ChangeChannelReq`, `0x140394480`.
- **Wire:** `hh`: command, signed channel short. Decode return is ignored.
- **Wrapper checks:** Require zone; restriction-query boundary at `0x140394427` → `0x14060E280` (STOP); ordinary branch creates ChangeChannelTask, vtable `0x140BA7EA8`; require PC kind 1; store channel at task +144; attach/queue. Missing zone, wrong kind or queue failure sends 304/2. Restriction-result failure sends 304/1196. Replies use `hh`. Return true.
- **Task start:** `0x140296BF0`. All failures below send 304, `hh`, to requester.

| Order | Check | Reason on failure |
| ---: | --- | ---: |
| 1 | No achievement-reward flag +21096 and no jumping-ticket flag +13752, each under its lock | 1194 |
| 2 | PC +888 bit 0 is clear (combat mode in 348) | 2 |
| 3 | Trying-instant-payment flag +21080 is clear under its lock | 1195 |
| 4 | Outside arena daemon, PC arena session is not 2, 3, 5 or 6 | 5 |
| 5 | Requested channel differs from zone +16600 | 1190 |
| 6 | For positive channel, `World::AvailableChannel` accepts current geo-zone/channel | 1192 |
| 7 | If builder_right +532 <=0 and ignore-cooldown +12580 is zero: last change tick +20872 is zero or elapsed >=160000 | 1193 |
| 8 | For non-builder: geo-zone is not either configured destination (+632/+636 in rule config), not zone type 40, and not type 87 while rule config +932 is enabled | 1191 |
| 9 | If zone attraction +16536 exists, its virtual +296 channel check succeeds | Supplied short |
| 10 | Without attraction, requested channel is not zero | 1191 |
| 11 | Action class 3 can be created and initialized through `0x1404C2920` | 2 |

- **State:** On successful initialization only, store daemon-relative tick at PC +20872. Post LeaveAction through `0x1404C3090`. This precedes deferred completion.
- **STOP:** Task's availability check also calls restriction helper `0x14060E280`. Do not follow that branch.
- **❓ Missing:** AvailableChannel implementation, attraction virtual variants, zone type field name, LeaveAction routing/rollback, success packets, broadcasts, daemon commands and DB procedures. Negative channels skip the positive-channel availability check; do not add a blanket positive-only rule.

## 305 — G_SaveClientOptionReqCmd — partial

- **381:** `0x140394600`. **348:** `HandleGame_SaveClientOptionReq`, `0x140394750`.
- **Wire:** `hQb`: command, option type byte, counted option bytes. Decode destination capacity is 4000.
- **Checks/order:** Decode; require signed length <=3300; require option type 1 or 2. Failures send 306, `hh`, reason 2. Return true.
- **Daemon helper:** `CachedChannel::SendClientOptionData`, `0x140317380`. If PC DB ID +20744 is zero, log only. Otherwise compose exact repeat descriptor `hQCqqchb;` at `0x140B69EA8`. The instruction sequence at `0x140317492` starts command 17, then byte zero, then a repeated record. Each record starts with PC DB ID:q, sign-extended option type:q, property ID 14:c and version 2:h, followed by the counted block. The repeat count occupies two bytes. One record is added before the helper sends the packet. The helper embeds version short 2, original length short and raw option bytes in a counted block. It sends through `0x14032AB10`.
- **State/output:** No local option assignment in wrapper. No immediate success or broadcast. No direct kick.
- **❓ Missing:** Cached response handler, DB procedure, persistence failures and success reply. Do not treat queueing as a completed save.

## 411 — G_AutoLogoutCheckReqCmd — done

- **381:** `0x14039CB30`. **348:** `HandleGame_AutoLogoutCheckReq`, `0x14039C730`.
- **Wire:** No body decoder.
- **Checks/order:** Require zone; create AutoLogoutCheckTask, vtable `0x140BB3598`; require PC kind 1; attach zone/caster and child relation; queue. Failed wrapper checks send 413, `hh`, reason 2. Return true.
- **Task `0x1402A0800`:** If config +537 (`_gameLogEnable` in 348) is clear, or caster is absent/not kind 1, do nothing. Otherwise derive state detail 1/2/3 for PC state +578 equal to 0/1/2, else zero. Build a report with current PC/zone metadata. Call `0x140200860`.
- **State/output:** No logout, timer change or client success appears in the read task. The name alone does not prove logout. No direct kick or broadcast.
- **Report sink:** Instructions in `0x140200860` call `GetLocalTime` and `0x1406A2FC0` at `0x140200BBA`. Report ID 1009 is a log identifier, not a client command. The sink uses optional LogArchive global `0x140F77BB8`; it composes a type-5 archive record through `0x1406C0E50` and queues it through `0x1406AE790` on archive +48. An absent archive does nothing. No game reply, logout, timer mutation or DB procedure occurs in this report path. Queueing does not prove archive delivery.
- **Rust plan:** Keep only the report ID and state detail. Omit identity, address and formatted PC metadata. This is a redacted log plan, not the native archive wire codec.

## 422 — G_CurrentUTCTimeReqCmd — done

- **381:** `0x1403A1AB0`; sender `0x1405F88C0`. **348:** `HandleGame_CurrentUTCTimeReq`, `0x14039FC00`; `Pc::SendCurrentUTCTimeOk`, `0x1405EE6F0`.
- **Wire:** No body decoder.
- **Checks/state:** PC dispatch only. Read `_time64(nullptr)`. No game-state mutation.
- **Output:** Send 423, `hq`, to requester. The q is the native signed 64-bit UTC Unix time in seconds. The command immediate `0x1A7` and saved `_time64` result are verified in sender instructions.
- **Failure/dependencies:** No error reply, broadcast, table, kick, daemon request or DB call. Handler returns true.

## 809 — G_ExtendJumpingCharacterCreationQuotaWithWarehouseItemReqCmd — partial

- **381:** `0x1403C1580`. **348:** `HandleGame_ExtendJumpingCharacterCreationQuotaWithWarehouseItemReq`, `0x1403BC2E0`.
- **Wire:** `hqqi`: command, labelId:q, itemInstanceId:q, itemDataId:i. Decode return is ignored.
- **Checks/state:** PC dispatch only. Log that the operation is unavailable. No ownership, quota, inventory or table check. No mutation.
- **Output:** Always send 811, `hEqq`, reason 1967, decoded labelId and itemInstanceId. Return true. No broadcast, kick, daemon call or DB request.
- **❓ Missing:** Native output bits on malformed input. Locals for the echoed q values are not initialized before unchecked decode. Valid-input behavior is fully read. Do not implement the operation named by the request.

## 812 — G_UpgradeJumpingCharacterStateWithCreationQuotaReqCmd — done

- **381:** `0x1403C1650`. **348:** `HandleGame_UpgradeJumpingCharacterStateWithCreationQuotaReq`, `0x1403BC3B0`.
- **Wire:** No body decoder.
- **Checks/state:** PC dispatch only. Log unavailable operation. No quota, character-state or table check. No mutation.
- **Output:** Send 814, `hE`, reason 1974 to requester. Return true. No broadcast, kick, daemon call or DB request.

## 887 — G_QQLogoutReqCmd — partial

- **381 dispatch:** `0x1400DE550`. **348 name:** `-`.
- **Thunk proof:** Bytes at dispatch start encode a relative jump to `0x140371430`. No function creation or database change was needed.
- **Wire:** No decoder in the thunk target.
- **Checks/order:** If daemon +2720 is null, return false. Otherwise get PC user +20696 only when nonzero and PC +20752 is clear. If no user, return true without work.
- **Calls/state:** With a user, call interface virtual +8 on daemon +2720 with user metadata pointer +40 and length +104. Release user and return true. No direct PC state mutation, reply, broadcast or kick.
- **❓ Missing:** Interface identity, external call effects, daemon/DB ownership, and dispatcher handling of false. Do not mark the operation unused or complete.

## 981 — G_EnterZoneDirectReqCmd — partial

- **381:** `0x1403CD3C0`. **348 name:** `-`.
- **Wire:** Exact descriptor `hQqcChh;`: command, entry-type byte, opaque q, flag byte, count:C, repeated two shorts. ❓ Semantic names for q, flag and pairs need types from a matching version.
- **Checks/order:** Reject arena config or arena session 2/3/5/6 with 983/5. Decode first three fields; failure is 983/2. Require repeat count 0..16; failure is 983/2399. Decode pairs; failure is 983/2. Require zone; failure is 983/2. Create task vtable `0x140B9D6A0`; require PC kind 1 and entry-type 1..3; attach and queue.
- **State:** Task entry type +144, q +152, flag +160, count +162, pair array +164 (4-byte stride, capacity 16). Common caster/zone/child state also changes.
- **Failure/output:** Init/queue failure sends 983/2. Replies use `hE` and go to requester. Return true. No immediate success, broadcast or kick.
- **Task `0x1402BDCA0`, ordered prefix:** Recheck arena (983/5). Require zone +16224 (983/2), then its record and record category byte +40 equal to 1 (983/2388). Require PC life-state byte +578 zero (983/2390), combat bit at +888 clear (983/2391), no achievement/jumping-ticket flags (983/2392), and no instant-payment flag (983/2393). Entry type 1 calls `0x1402BE630`; type 3 calls `0x1402BF0B0`; type 2 sends 983/3 at this stage. Nonzero helper reasons pass through.
- **Task continuation subset:** A positive cost exceeding current currency sends 983/2401. Nonempty dispose orders must pass `Inventory::CheckDispose` with reason 18; failure sends 983/2399. Resolve destination through table slot 3243. Create LeaveAction class 3, mode 3, movement reason 20. Store type-1 flag at action +54414. Resolve named return-to-game/move-to-arena records through table slot 2703. Successful initialization attaches cost/dispose orders, posts the action, then sends 982 `h`. This reply does not prove completed travel. Create/init failure sends 983/2.
- **❓ Missing:** Entry helper authorization, content/zone table fields, LeaveAction cost/disposal execution, spawn path, daemon calls and DB procedures.

## 984 — G_LeaveZoneDirectReqCmd — partial

- **381:** `0x1403CD930`. **348 name:** `-`.
- **Wire:** `h`.
- **Checks/order:** Decode; require zone; construct through `0x14036D9A0`; initialize through `0x1402BF490(zone, PC)`; queue. Initialization requires nonnull zone/caster and native kind 1. Each failure logs only. Return true.
- **State/output:** Wrapper only creates task/child state. No direct client failure or success, broadcast, DB request, daemon call or kick.
- **Task `0x1402BF540`:** If PC +13740 is nonzero and zone helper `0x140254F60` returns nonzero for the PC job/state, log only. Next, achievement/jumping-ticket flags log only; instant-payment flag logs only. Otherwise zone +17568 selects `0x140617440`. Without that pointer, a present attraction at zone +16536 receives virtual +248 with the PC. No attraction means no work. No direct client reply occurs in this task.
- **❓ Missing:** Zone helper predicate, `0x140617440`, attraction virtual variants, destination/ownership rules, persistent state and backend work.

## 1149 — G_UseJumpingCharacterTicketReqCmd — partial

- **381:** `0x1403D89E0`. **348:** `HandleGame_UseJumpingCharacterTicketReq`, `0x1403D3600`.
- **Wire:** `hqqqc`: command, itemInstanceId:q, labelId:q, third q, factionId:c. The third q writes into `itemDataId` plus the adjacent 4 bytes in the native parameter object. Only its low 32 bits are used for the wrapper table check. Preserve all eight wire bytes. ❓ Full parameter layout requires a type check.
- **Checks/order:** Decode (2483); account-inventory config +404 enabled (2); not arena config +237/+248 (5); user exists and PC +20752 clear (2); zone exists (2); low-32 itemDataId belongs to sorted jumping-ticket list at DataManager +82432 (2499); additional-service item lookup at DataManager table pointer +11640 with key `itemDataId | 0x100000000` succeeds (2499); transaction creation and registration (2).
- **Failure:** Send 1151, `hEq`, with the parenthesized reason and q zero to requester. Return true. No direct kick or broadcast.
- **State/calls:** Construct `AccountInventoryJumpingCharacter2TicketTran` at `0x1401C30B0`; assign transaction ID +24; register/start through `0x1400D4FE0`/`0x1400D4A80`. No direct item consumption or character mutation in wrapper.
- **❓ Missing:** Exact additional-service table name/fields, transaction request/response commands, item ownership/consumption, quota/character changes, DB procedure, rollback, success and conditional kick paths.


### gw1 — Command 32 field evidence (381.8867)

Sources: builder `0x1405F5980`; descriptor `0x140DAA240`; 422 public field definitions at `0x1407CA330`, stride 48; metadata initialization `0x1406985C0`; packed getter `0x140697E50` and copy `0x1406981E0`. Build 348 names agree where fields still exist. Build 381 definitions supply changed IDs and fields. No opaque cryptographic bytes were read.

The builder writes a nine-field fixed prefix, then all property IDs 1..422. It advances 373 descriptor slots after the property block. It does not filter these properties by sync flags. The table below lists every property, not only the encounter-sync subset.

Native descriptor (the two `?` slots are eight-byte ability values copied directly, not composer calls):

```text
hihqQihhhhhhcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccsihhhhcicqqqqqccqcihhhhhhcqcccqcqiqqqqqqqqqqqqcqcccccbhhhicciiiiccccccqcciqqqqqqqqqqqqqqqqqhqqqqhqcqqhhciiqqhqiqcicccci?i?ihhhhiiiihhhhhhhhhiihhhhhhhhhhhhhhhhhiihhiihhhhhihihhhhhhhhhhhhhhhhhhhhhhhiihhihhhhhhhhhhihihhhhhccciciciiqcccccccciiccciccciccciiiiiihhhhiiiihhhhhhhhhhhhhbKCci;CQb;cccccqCQqq;QCch;CQiqqihcccc;Ccicicicicicichhhhhhhhhhc;
```

Field types: 1=byte; 2=short; 3=dword; 4=qword; 7=ability short; 8=ability dword; 9=ability qword; 10=packed bit; 11=flag byte packed by the getter; 12=counted UTF-16; 14=counted bytes. Types 7/8/9 copy only their 2/4/8-byte value, not their native ability object. Name capacity is 32 UTF-16 units. Survey capacity is 10 bytes.

Packed runs: IDs 166..173 emit one byte. IDs 218..241 emit three bytes. IDs 344..367 emit three bytes. Other members of each eight-field group emit no bytes. Type-10 groups retain each stored bit. **Native type-11 quirk:** instructions `0x140698080..0x1406980BC` read the first member offset for every bit in each group. Thus each group emits 0x00 or 0xFF from IDs 344, 352, or 360. Do not silently replace this with independent flag packing. The resulting 373 slot kinds match the descriptor exactly.

<!-- gw1-creature-fields-start -->
| ID | Type | Name |
| ---: | ---: | --- |
| 1 | 2 | world-id |
| 2 | 1 | race |
| 3 | 1 | sex |
| 4 | 1 | job |
| 5 | 1 | appearance1 |
| 6 | 1 | appearance2 |
| 7 | 1 | appearance3 |
| 8 | 1 | appearance4 |
| 9 | 1 | appearance5 |
| 10 | 1 | appearance6 |
| 11 | 1 | appearance7 |
| 12 | 1 | appearance8 |
| 13 | 1 | appearance9 |
| 14 | 1 | appearance10 |
| 15 | 1 | appearance11 |
| 16 | 1 | appearance12 |
| 17 | 1 | appearance13 |
| 18 | 1 | appearance14 |
| 19 | 1 | appearance15 |
| 20 | 1 | appearance16 |
| 21 | 1 | appearance17 |
| 22 | 1 | appearance18 |
| 23 | 1 | appearance19 |
| 24 | 1 | appearance20 |
| 25 | 1 | appearance21 |
| 26 | 1 | appearance22 |
| 27 | 1 | appearance23 |
| 28 | 1 | appearance24 |
| 29 | 1 | appearance25 |
| 30 | 1 | appearance26 |
| 31 | 1 | appearance27 |
| 32 | 1 | appearance28 |
| 33 | 1 | appearance29 |
| 34 | 1 | appearance30 |
| 35 | 1 | appearance31 |
| 36 | 1 | appearance32 |
| 37 | 1 | appearance33 |
| 38 | 1 | appearance34 |
| 39 | 1 | appearance35 |
| 40 | 1 | appearance36 |
| 41 | 1 | appearance37 |
| 42 | 1 | appearance38 |
| 43 | 1 | appearance39 |
| 44 | 1 | appearance40 |
| 45 | 1 | appearance41 |
| 46 | 1 | appearance42 |
| 47 | 1 | appearance43 |
| 48 | 1 | appearance44 |
| 49 | 1 | appearance45 |
| 50 | 1 | appearance46 |
| 51 | 1 | appearance47 |
| 52 | 1 | appearance48 |
| 53 | 1 | appearance49 |
| 54 | 1 | appearance50 |
| 55 | 1 | appearance51 |
| 56 | 1 | appearance52 |
| 57 | 1 | appearance53 |
| 58 | 1 | appearance54 |
| 59 | 1 | appearance55 |
| 60 | 1 | appearance56 |
| 61 | 1 | appearance57 |
| 62 | 1 | appearance58 |
| 63 | 1 | appearance59 |
| 64 | 1 | appearance60 |
| 65 | 1 | appearance61 |
| 66 | 1 | appearance62 |
| 67 | 1 | appearance63 |
| 68 | 1 | appearance64 |
| 69 | 1 | appearance65 |
| 70 | 1 | appearance66 |
| 71 | 1 | appearance67 |
| 72 | 1 | appearance68 |
| 73 | 1 | appearance69 |
| 74 | 1 | appearance70 |
| 75 | 1 | appearance71 |
| 76 | 1 | appearance72 |
| 77 | 1 | appearance73 |
| 78 | 1 | appearance74 |
| 79 | 1 | appearance75 |
| 80 | 1 | appearance76 |
| 81 | 1 | appearance77 |
| 82 | 1 | appearance78 |
| 83 | 1 | appearance79 |
| 84 | 1 | appearance80 |
| 85 | 1 | appearance81 |
| 86 | 1 | appearance82 |
| 87 | 1 | appearance83 |
| 88 | 1 | appearance84 |
| 89 | 1 | appearance85 |
| 90 | 1 | appearance86 |
| 91 | 1 | appearance87 |
| 92 | 1 | appearance88 |
| 93 | 1 | appearance89 |
| 94 | 1 | appearance90 |
| 95 | 1 | appearance91 |
| 96 | 1 | appearance92 |
| 97 | 12 | name |
| 98 | 3 | geo-zone |
| 99 | 2 | x |
| 100 | 2 | y |
| 101 | 2 | z |
| 102 | 2 | yaw |
| 103 | 1 | level |
| 104 | 3 | exp |
| 105 | 1 | mastery-level |
| 106 | 4 | mastery-exp |
| 107 | 4 | hp |
| 108 | 4 | guard-gauge |
| 109 | 4 | money |
| 110 | 4 | money-diff |
| 111 | 1 | faction |
| 112 | 1 | faction2 |
| 113 | 4 | guild-id |
| 114 | 1 | activated-faction |
| 115 | 3 | faction-reputation |
| 116 | 2 | achievement-id |
| 117 | 2 | achievement-step |
| 118 | 2 | ability-achievement-id |
| 119 | 2 | ability-achievement-step |
| 120 | 2 | sp |
| 121 | 2 | sp2 |
| 122 | 1 | stance |
| 123 | 4 | link-target |
| 124 | 1 | link-type |
| 125 | 1 | link-state |
| 126 | 1 | link-stage |
| 127 | 4 | target |
| 128 | 1 | action-type |
| 129 | 4 | action-target |
| 130 | 3 | additional-hp |
| 131 | 4 | effect-attribute |
| 132 | 4 | effect-attribute-2 |
| 133 | 4 | effect-attribute-3 |
| 134 | 4 | immune-effect-attribute |
| 135 | 4 | immune-effect-attribute-2 |
| 136 | 4 | immune-effect-attribute-3 |
| 137 | 4 | effect-flag |
| 138 | 4 | effect-flag-2 |
| 139 | 4 | effect-flag-3 |
| 140 | 4 | effect-flag-4 |
| 141 | 4 | field-item |
| 142 | 4 | summoned |
| 143 | 1 | convoy-id |
| 144 | 4 | top-contribution-group-id |
| 145 | 1 | inventory-size |
| 146 | 1 | depository-size |
| 147 | 1 | wardrobe-size |
| 148 | 1 | premium-depository-size |
| 149 | 1 | builder-right |
| 150 | 14 | survey |
| 151 | 2 | style-score |
| 152 | 2 | immune-breaker-attribute |
| 153 | 2 | immune-breaker-count |
| 154 | 3 | faction-score |
| 155 | 1 | faction-killed-count |
| 156 | 1 | combat-side |
| 157 | 3 | duel-point |
| 158 | 3 | party-battle-point |
| 159 | 3 | field-play-point |
| 160 | 3 | life-contents-point |
| 161 | 1 | duel-grade |
| 162 | 1 | duel-grade-type |
| 163 | 1 | dead |
| 164 | 1 | gm-mode |
| 165 | 1 | grade-level |
| 166 | 10 | grade-broadcast-benefit-1 |
| 167 | 10 | grade-broadcast-benefit-2 |
| 168 | 10 | grade-broadcast-benefit-3 |
| 169 | 10 | grade-broadcast-benefit-4 |
| 170 | 10 | grade-broadcast-benefit-5 |
| 171 | 10 | grade-broadcast-benefit-6 |
| 172 | 10 | grade-broadcast-benefit-7 |
| 173 | 10 | grade-broadcast-benefit-8 |
| 174 | 4 | logout-time |
| 175 | 1 | instant-heart-count |
| 176 | 1 | heart-count |
| 177 | 3 | skill-score |
| 178 | 4 | bound-raid-dungeon-id-1 |
| 179 | 4 | bound-raid-dungeon-id-2 |
| 180 | 4 | bound-raid-dungeon-id-3 |
| 181 | 4 | bound-raid-dungeon-id-4 |
| 182 | 4 | bound-raid-dungeon-id-5 |
| 183 | 4 | bound-raid-dungeon-id-6 |
| 184 | 4 | bound-raid-dungeon-id-7 |
| 185 | 4 | bound-raid-dungeon-id-8 |
| 186 | 4 | bound-raid-dungeon-id-9 |
| 187 | 4 | bound-raid-dungeon-id-10 |
| 188 | 4 | bound-raid-dungeon-id-11 |
| 189 | 4 | bound-raid-dungeon-id-12 |
| 190 | 4 | bound-raid-dungeon-id-13 |
| 191 | 4 | bound-raid-dungeon-id-14 |
| 192 | 4 | bound-raid-dungeon-id-15 |
| 193 | 4 | bound-raid-dungeon-id-16 |
| 194 | 4 | bound-raid-dungeon-id-17 |
| 195 | 2 | boss-second-gauge |
| 196 | 4 | accumulated-damage-slot-tao |
| 197 | 4 | accumulated-damage-slot-1 |
| 198 | 4 | accumulated-damage-slot-2 |
| 199 | 4 | accumulated-damage-slot-3 |
| 200 | 2 | skill-skin-id |
| 201 | 4 | pvp-mode-cooltime |
| 202 | 1 | ghost-type |
| 203 | 4 | challenge-party-id |
| 204 | 4 | challenge-party-out-time |
| 205 | 2 | acquired-skill-build-up-point |
| 206 | 2 | usable-skill-build-up-point |
| 207 | 1 | job-style |
| 208 | 3 | account-exp-to-add |
| 209 | 3 | account-exp-added |
| 210 | 4 | account-exp-added-time |
| 211 | 4 | account-exp-by-pc |
| 212 | 2 | account-level |
| 213 | 4 | account-exp |
| 214 | 3 | account-exp-quota-per-day |
| 215 | 4 | account-exp-last-update-time |
| 216 | 1 | activated-badge-page |
| 217 | 3 | equip-badge-gear-score |
| 218 | 10 | combat-mode |
| 219 | 10 | falling |
| 220 | 10 | convoy |
| 221 | 10 | attackable |
| 222 | 10 | casting |
| 223 | 10 | immune-breaker-disable |
| 224 | 10 | npc-interactive |
| 225 | 10 | npc-talkable |
| 226 | 10 | special-skill-gauge-full |
| 227 | 10 | pvp-mode |
| 228 | 10 | pvp-safe-area |
| 229 | 10 | zone-move-state |
| 230 | 10 | public-reserve-5 |
| 231 | 10 | public-reserve-6 |
| 232 | 10 | public-reserve-7 |
| 233 | 10 | public-reserve-8 |
| 234 | 10 | detect-hiding |
| 235 | 10 | guild-invitation-refusal |
| 236 | 10 | private-reserve-3 |
| 237 | 10 | private-reserve-4 |
| 238 | 10 | private-reserve-5 |
| 239 | 10 | private-reserve-6 |
| 240 | 10 | private-reserve-7 |
| 241 | 10 | private-reserve-8 |
| 242 | 1 | slate-page |
| 243 | 3 | guild-point |
| 244 | 9 | max-hp |
| 245 | 8 | max-hp-equip |
| 246 | 9 | max-guard-gauge |
| 247 | 8 | max-guard-gauge-equip |
| 248 | 7 | max-sp |
| 249 | 7 | max-sp2 |
| 250 | 7 | speed |
| 251 | 7 | modify-cast-speed-percent |
| 252 | 8 | hp-regen |
| 253 | 8 | hp-regen-equip |
| 254 | 8 | hp-regen-combat |
| 255 | 8 | hp-regen-combat-equip |
| 256 | 7 | attack-hit-base-percent |
| 257 | 7 | attack-hit-value |
| 258 | 7 | attack-hit-value-equip |
| 259 | 7 | attack-pierce-base-percent |
| 260 | 7 | attack-parry-pierce-percent |
| 261 | 7 | attack-pierce-value |
| 262 | 7 | attack-pierce-value-equip |
| 263 | 7 | attack-critical-base-percent |
| 264 | 7 | attack-critical-damage-percent |
| 265 | 8 | attack-critical-value |
| 266 | 8 | attack-critical-value-equip |
| 267 | 7 | attack-critical-damage-value |
| 268 | 7 | attack-critical-damage-value-equip |
| 269 | 7 | defend-critical-base-percent |
| 270 | 7 | defend-critical-damage-percent |
| 271 | 7 | defend-critical-value |
| 272 | 7 | defend-critical-value-equip |
| 273 | 7 | defend-bounce-percent |
| 274 | 7 | defend-dodge-base-percent |
| 275 | 7 | defend-dodge-value |
| 276 | 7 | defend-dodge-value-equip |
| 277 | 7 | defend-parry-base-percent |
| 278 | 7 | defend-parry-value |
| 279 | 7 | defend-parry-value-equip |
| 280 | 7 | defend-parry-reduce-percent |
| 281 | 7 | defend-parry-reduce-diff |
| 282 | 7 | defend-perfect-parry-base-percent |
| 283 | 7 | defend-immune-base-percent |
| 284 | 8 | attack-power-creature-min |
| 285 | 8 | attack-power-creature-max |
| 286 | 7 | attack-power-equip-min |
| 287 | 7 | attack-power-equip-max |
| 288 | 8 | defend-power-creature-value |
| 289 | 8 | defend-power-equip-value |
| 290 | 7 | defend-resist-power-creature-value |
| 291 | 7 | defend-resist-power-equip-value |
| 292 | 7 | defend-physical-damage-reduce-percent |
| 293 | 7 | defend-force-damage-reduce-percent |
| 294 | 7 | attack-damage-modify-percent |
| 295 | 8 | attack-damage-modify-diff |
| 296 | 7 | defend-damage-modify-percent |
| 297 | 8 | defend-damage-modify-diff |
| 298 | 7 | defend-miss-base-percent |
| 299 | 7 | attack-stiff-duration-base-percent |
| 300 | 7 | attack-stiff-duration-value |
| 301 | 7 | defend-stiff-duration-base-percent |
| 302 | 7 | defend-stiff-duration-value |
| 303 | 7 | cast-duration-base-percent |
| 304 | 7 | cast-duration-value |
| 305 | 7 | attack-concentrate-value |
| 306 | 7 | attack-concentrate-value-equip |
| 307 | 7 | defend-perfect-parry-reduce-percent |
| 308 | 7 | defend-counter-reduce-percent |
| 309 | 7 | pve-boss-level-npc-attack-power-creature-min |
| 310 | 7 | pve-boss-level-npc-attack-power-creature-max |
| 311 | 7 | pve-boss-level-npc-attack-power-equip-min |
| 312 | 7 | pve-boss-level-npc-attack-power-equip-max |
| 313 | 7 | pve-boss-level-npc-defend-power-creature-value |
| 314 | 7 | pve-boss-level-npc-defend-power-equip-value |
| 315 | 7 | pvp-attack-power-creature-min |
| 316 | 7 | pvp-attack-power-creature-max |
| 317 | 7 | pvp-attack-power-equip-min |
| 318 | 7 | pvp-attack-power-equip-max |
| 319 | 7 | pvp-defend-power-creature-value |
| 320 | 7 | pvp-defend-power-equip-value |
| 321 | 8 | job-ability-1 |
| 322 | 8 | job-ability-2 |
| 323 | 7 | heal-power-base-percent |
| 324 | 7 | heal-power-value |
| 325 | 8 | heal-power-diff |
| 326 | 7 | aoe-defend-base-percent |
| 327 | 7 | aoe-defend-power-value |
| 328 | 7 | abnormal-attack-base-percent |
| 329 | 7 | abnormal-attack-power-value |
| 330 | 7 | abnormal-attack-power-value-equip |
| 331 | 7 | abnormal-defend-base-percent |
| 332 | 7 | abnormal-defend-power-value |
| 333 | 7 | hate-base-percent |
| 334 | 7 | hate-power-creature-value |
| 335 | 7 | hate-power-equip-value |
| 336 | 8 | additional-exp-diff-by-kill |
| 337 | 7 | additional-exp-percent-by-kill |
| 338 | 8 | additional-mastery-exp-diff-by-kill |
| 339 | 7 | additional-mastery-exp-percent-by-kill |
| 340 | 7 | additional-faction-score-max-percent |
| 341 | 7 | attack-attribute-value |
| 342 | 7 | attack-attribute-value-equip |
| 343 | 7 | attack-attribute-base-percent |
| 344 | 11 | invisible |
| 345 | 11 | block-move |
| 346 | 11 | block-skill |
| 347 | 11 | block-physical-skill |
| 348 | 11 | block-force-skill |
| 349 | 11 | immune-damage |
| 350 | 11 | immune-death |
| 351 | 11 | immune-debuff |
| 352 | 11 | exception-target |
| 353 | 11 | exception-detect |
| 354 | 11 | exception-hostile-target |
| 355 | 11 | immune-casting-delay |
| 356 | 11 | block-dodge |
| 357 | 11 | block-bounce |
| 358 | 11 | block-parry |
| 359 | 11 | block-perfect-parry |
| 360 | 11 | front-perfect-parry |
| 361 | 11 | back-perfect-parry |
| 362 | 11 | front-bounce |
| 363 | 11 | back-bounce |
| 364 | 11 | debug-invincible |
| 365 | 11 | debug-invisible |
| 366 | 11 | abnormality-reserve-7 |
| 367 | 11 | abnormality-reserve-8 |
| 368 | 3 | equip-hand |
| 369 | 1 | equip-hand-level |
| 370 | 3 | equip-hand-appearance-item |
| 371 | 1 | equip-hand-appearance-item-level |
| 372 | 3 | equip-hand-alt |
| 373 | 3 | equip-body |
| 374 | 4 | equip-body-guild-id |
| 375 | 1 | equip-body-custom1 |
| 376 | 1 | equip-body-custom2 |
| 377 | 1 | equip-body-custom3 |
| 378 | 1 | equip-body-custom4 |
| 379 | 1 | equip-body-custom5 |
| 380 | 1 | equip-body-custom6 |
| 381 | 1 | equip-body-custom7 |
| 382 | 1 | equip-body-custom8 |
| 383 | 3 | equip-ear |
| 384 | 3 | equip-eye |
| 385 | 1 | equip-eye-custom1 |
| 386 | 1 | equip-eye-custom2 |
| 387 | 1 | equip-eye-custom3 |
| 388 | 3 | equip-head |
| 389 | 1 | equip-head-custom1 |
| 390 | 1 | equip-head-custom2 |
| 391 | 1 | equip-head-custom3 |
| 392 | 3 | equip-body-attach |
| 393 | 1 | equip-body-attach-custom1 |
| 394 | 1 | equip-body-attach-custom2 |
| 395 | 1 | equip-body-attach-custom3 |
| 396 | 3 | equip-weapon-gem |
| 397 | 3 | equip-pet-1 |
| 398 | 3 | equip-pet-1-appearance-item |
| 399 | 3 | equip-pet-2 |
| 400 | 3 | equip-pet-2-appearance-item |
| 401 | 3 | equip-badge-appearance-item |
| 402 | 7 | attack-stiff-duration-equip-value |
| 403 | 7 | defend-stiff-duration-equip-value |
| 404 | 7 | aoe-defend-power-value-equip |
| 405 | 7 | heal-power-equip-value |
| 406 | 8 | defend-strength-creature-value |
| 407 | 8 | defend-strength-equip-value |
| 408 | 8 | attack-precise-creature-value |
| 409 | 8 | attack-precise-equip-value |
| 410 | 7 | attack-aoe-pierce-value |
| 411 | 7 | attack-aoe-pierce-value-equip |
| 412 | 7 | attack-abnormal-hit-base-percent |
| 413 | 7 | attack-abnormal-hit-value |
| 414 | 7 | attack-abnormal-hit-equip-value |
| 415 | 7 | defend-abnormal-dodge-base-percent |
| 416 | 7 | defend-abnormal-dodge-value |
| 417 | 7 | defend-abnormal-dodge-equip-value |
| 418 | 7 | support-power-base-percent |
| 419 | 7 | support-power-value |
| 420 | 7 | support-power-equip-value |
| 421 | 7 | hypermove-power-value |
| 422 | 7 | hypermove-power-equip-value |
<!-- gw1-creature-fields-end -->
