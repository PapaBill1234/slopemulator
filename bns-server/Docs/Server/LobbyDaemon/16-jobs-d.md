# 16 — Jobs, batch B1: create, delete, cancel deletion, recover

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`. Framework and session states: `13-jobs-a.md`. NP OK rule: `14-jobs-b.md` §1.

Terms: "2.0" = session state 0x20000 (lobby). "6.0" = 0x60000 (final). "fin" = the final job state. `SendXFail(clikey, slot id, …, code)` goes to the LobbyGate channel from the input.

## 1. CreateCharacterJob (task 10, fin 3)

### 1.1 Input

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | LobbyGate channel |
| 8 | 16 | slot id (id128, not 0) |
| 24 | 2 | world id (not 0) |
| 26 | 2 × n | character name (UTF-16, zero end, not empty) |
| 184 | — | make data (size at 348, i16, not 0) |
| 350 | 1 | jumping character flag |

### 1.2 States

| State | Task | Method | Result |
| --- | --- | --- | --- |
| 0 | 10 | `init` | checks of §1.3 → state 1 |
| 1 | 0 | `reqGetSlot` | NP `SendRequestSlotGetSlot(account id, slot id)`; unsent → 2 |
| 1 | 37 | `responseGetSlot` | §1.4 → state 2 |
| 1 | 38 | `expireGetSlot` | 2 |
| 2 | 0 | `tryCreateCharacter` | §1.5; sends `CreateCharacterReq` to gamed with transaction task 107 |
| 2 | 105 | `createCharacterOk` | §1.6 |
| 2 | 106 | `createCharacterFail` | cancel, 2 |
| 2 | 107 | `createCharacterExpire` | cancel, 2 |

Each failure sends `SendCreateCharacterFail` and moves to state 3.

### 1.3 `init` checks (in order)

| # | Rule | Code |
| --- | --- | --- |
| 1 | Session not 2.0 | no reply |
| 2 | Bad input (§1.1) | 6 |
| 3 | Jumping flag = 1 and `_enableJumpingCharacterCreation` off | 2 |
| 4 | Jumping flag ≠ 0, `_enableContentBan` on, `_contentBanIdForJumpingCharacter` ≠ 0, and the session has that ban with `from ≤ now ≤ to` | 132 |
| 5 | Jumping flag ≠ 0, `_useJumpingCharacterCreationQuota` on, `…QuotaInWorld` off, and `reserved + creationCount ≥ creationQuota` | 133 |
| 6 | `GameWorldRegistry::Get(world)` = none | 128 |
| 7 | `GameWorld::PermitCharacterManagement(world, session, true)` false | thread error 14 → 131, 13 → 130 ❓, other → 2 (9 ❓) |
| 8 | `GamedChannelManager::Acquire(world)` = none | 129 |
| 9 | `_tencentUICEnable` on: external account key size ≤ 0 → 130; name filter refuses → 123 | 130 / 123 |
| 10 | `RuleSet::CheckName(name, len min, len max, space amount, numeric, sentence case, multilingual, forbidden characters)` ≠ OK | invalid parameter → 6 ❓; values up to `CHECKNAME_MULTILINGUAL` → 122; `CHECKNAME_FORBIDDEN` → 123 |
| 11 | `LobbyDB::CheckValidityOfCharacterName(name, sentence case)` fails | DB return 3 → 125; 2 → 124 ❓; other → 2 |

Config keys for rule 10: `_allowedCharacterNameLenMin`, `…LenMax`, `_allowedCharacterNameSpaceAmount`, `_enableNumericForCharacterName`, `_forceSentenceCaseForCharacterName`, `_enableMultilingualCharacterName`, `_forbiddenCharactersForCharacterName`.

Then the job copies the input and makes a `LobbyJobTransaction` (tag 10).

### 1.4 `responseGetSlot`

| Rule | Code |
| --- | --- |
| Session 6.0 | 5 |
| No parameter | 2 |
| NP result not OK | 126 |
| Reply slot id ≠ job slot id | 2 |
| Slot data check fails ❓ (log line empty) | 126 |
| Reply flag at +120 = 0 and `getLobbyServiceByType(session, reply +128, 1)` = none ❓ | ❓ (log shows `service-id`) |
| Else | state 2 |

### 1.5 `tryCreateCharacter`

| # | Rule | Code |
| --- | --- | --- |
| 1 | A session character has the same slot id | 127 |
| 2 | Jumping + quota (as rule 5): quota full | 133 |
| 3 | Jumping + quota: `++reservedCreationCount` | — |
| 4 | `Character::TryCreation(account, slot id, world, name, len)` fails | 125, 124 or 2 by the return value ❓ |
| 5 | Add to `session._characters`; `_pcid` = new pcid | add fail → 2 |
| 6 | `GamedChannel::SendCreateCharacterReq(tran(107), pcid, account id, external key, key size, ipv4, name, make data, size, …, top level, jumping)` | — |

"Top level" is the highest `level` of the session characters. On a failure after rule 3 the job gives back the reserved quota ❓.

### 1.6 Results from gamed

| Case | Action |
| --- | --- |
| Ok, session 6.0 | 5 ❓ (no cancel) |
| Ok, no parameter / no character | 2 |
| Ok | `Character::FinishCreation(param[0], param[1], mode, use quota, data at +4, param[258])`. Mode: not jumping 0; jumping and quota off 2; jumping, quota on and not in world 1 (use quota = true). Fail → 2. Ok → `SendCreateCharacterOk(clikey, pcid, slot id, belonging world, name, …)`; `param[1]` = remaining jumping quota. |
| Fail or expire | `Character::CancelCreation`, remove and delete the character, give back the reserved quota, code 2. Session 6.0: log only ❓. |

## 2. DeleteCharacterJob (task 11, fin 7)

Input: gate channel (+0), slot id (+8, not 0). `SendDeleteCharacterFail(clikey, slot id, client state, code)`.

### 2.1 States

| State | Task | Method | Next |
| --- | --- | --- | --- |
| 0 | 11 | `init` | 1, 3 or 4 |
| 1 | 0 / 62 / 63 | second password status: request / response / expire | 2, 3 or 4 |
| 2 | 0 / 64 / 65 | second password verified: request / response / expire | 3 or 4 |
| 3 | 0 / 168 / 169 / 170 | account inventory goods count: request / ok / fail / expire | 4 |
| 4 | 0 / 108 / 109 / 110 | gamed delete: request / ok / fail / expire | 5 or 6 |
| 5 | 0 | `directDelete` | 7 |
| 6 | 0 | `scheduleDelete` | 7 |

Rule "goods or delete": `_useAccountInventory` and `_checkCharacterGoodsForCharacterDeletion` both on → state 3, else state 4.

### 2.2 `init`

| Rule | Code |
| --- | --- |
| Session not 2.0 | no reply |
| Slot id = 0 | ❓ (log "invalid parameter") |
| No session character with that slot id | 134 |
| `queryLobbyService(session, character)` ≠ none (character in a service entry) | 9 |
| World absent or `PermitCharacterManagement(world, session, false)` false | thread error 9 → 138 ❓, else 143 or 2 ❓ |
| `GamedChannelManager::Acquire(belonging world)` = none | 138 |
| Goods check on and account-inventory channel absent | 7 |
| Else | `_enableSecondPassword` on → state 1; else "goods or delete" |

### 2.3 Second password (states 1 and 2)

Both requests go to the NP game-service channel (message identity ❓, folded vtable). Channel absent or unsent → 7. Expire → 7 (session not 2.0 → 5). Session not 2.0 at the response → 5.

| Response | Rule | Result |
| --- | --- | --- |
| Status | NP OK, flag `+128 & 1` = 0 (no password set) | "goods or delete" |
| Status | NP OK, password set, value at +96 ≠ 1 | state 2 |
| Status | NP OK, password set, value at +96 = 1 (blocked) | 145 |
| Status | NP error code 3333 | "goods or delete" ❓ |
| Status | other NP error | 2 |
| Verified | no parameter | 2 |
| Verified | NP OK | "goods or delete" |
| Verified | NP error 0xD05 (3333) | 144 |
| Verified | 0xD06 | 145 |
| Verified | 0xD11 | 146 |
| Verified | 0xD12 | 147 |
| Verified | other | 1 |

### 2.4 Goods count (state 3)

| Step | Rule | Code |
| --- | --- | --- |
| request | No such character | 134 |
| request | `AccountInventoryChannel::SendRetrieveCharacterGoodsCountReq(tran, account id, pcid)` unsent | 7 |
| ok | No parameter | 2 |
| ok | `param[0] > 0` or `param[1] > 0` (i32) | 142 |
| ok | Else | state 4 |
| fail | One ack value → 7 ❓, else 2 | 7 / 2 |
| expire | — | 2 |

### 2.5 Delete (states 4–6)

| Step | Rule | Result |
| --- | --- | --- |
| `deleteReq` | No such character | 134 |
| `deleteReq` | Character in a service entry | 9 |
| `deleteReq` | `Character::TryDeletion` fails | return 1 → 137 ❓; other values → 136 or 2 ❓ |
| `deleteReq` | `GamedChannel::SendDeleteCharacterReq(tran(110), pcid, external key, key size, ipv4)` unsent | `Character::CancelDeletion`; ❓ reply |
| `deleteOk` | `_level` = level from the Ok parameter | `_waitCharacterDeletion` off → state 5; on → state 6 |
| `deleteFail` | `Character::CancelDeletion`; map of the gamed reason | 28 → 139, 29 → 140, 30 → 141, 31 → 148, 32 → 149, else 2 |
| `deleteExpire` | — | 2 |
| `directDelete` | `Character::FinishDeletion(ch, 0)`; fail → 3 ❓ | `SendDeleteCharacterOk(clikey, slot id, 0, 0)`; the character object is deleted |
| `scheduleDelete` | Wait time = `_shortTermWaitCharacterDeletionTime` if ❓ flag or `_level < _longTermWaitCharacterDeletionCriteriaLevel`, else `_longTermWaitCharacterDeletionTime`. `Character::FinishDeletion(ch, wait)` | `SendDeleteCharacterOk(clikey, slot id, 1, wait)` |

Client state in a Fail reply: the job reads the character state. State 2 → `CLIENTSTATE_IN_USUAL`; 3 or 5 → `CLIENTSTATE_IN_WAIT_DELETION`; no character → `CLIENTSTATE_UNKNOWN`.

## 3. CancelWaitCharacterDeletionJob (task 12, fin 3)

Input: `CancelWaitCharacterDeletionReqParam { lgate, slotid }`. `SendCancelWaitCharacterDeletionFail(clikey, slot id, client state, code)`.

| State | Task | Method | Rule | Result |
| --- | --- | --- | --- | --- |
| 0 | 12 | `init` | Session not 2.0 | no reply |
| 0 | 12 | `init` | No parameter or bad slot id | ❓ code (log "invalid parameter"); reply only if `lgate` is set |
| 0 | 12 | `init` | No session character with that slot id | 150 |
| 0 | 12 | `init` | World not registered | 154 |
| 0 | 12 | `init` | `PermitCharacterManagement(world, session, false)` false | 154 |
| 0 | 12 | `init` | Else | state 1 |
| 1 | 0 | try restore | No such character | 150 |
| 1 | 0 | try restore | `Character::TryRestore` fails | 153, 152 or 2 by the return value ❓ |
| 1 | 0 | try restore | Ok | state 2 |
| 2 | 0 | request | No such character | 150 |
| 2 | 0 | request | `GamedChannelManager::Acquire(belonging world)` = none | 154 with the client state of the character |
| 2 | 0 | request | `GamedChannel::SendRestoreCharacterReq(tran(113), pcid)` | unsent ❓ |
| 2 | 111 | `restoreCharacterOk` | Character gone | 150 |
| 2 | 111 | `restoreCharacterOk` | `Character::FinishRestore` fails | 2 with the client state |
| 2 | 111 | `restoreCharacterOk` | Ok | `SendCancelWaitCharacterDeletionOk(clikey, slot id)` |
| 2 | 112 | `restoreCharacterFail` | Character gone → 150; else | 2 with the client state |
| 2 | 113 | `restoreCharacterExpire` | Character gone → 150; else | 2 with the client state |

Client state from the character state: 2 → `IN_USUAL`; 4 → `IN_WAIT_DELETION`; 3 and 5 → a third value ❓ (the busy states). Compare `16` §2.5, where the delete job reads the same field with other case values ❓.

## 4. RecoverPcJob (task 18, fin 2)

Use: the client asks to recover a character that is stuck in state 3 (try deletion) or state 5 (try restore). `Initialize(lgate, slot id)` stores the input; a bad input logs "invalid parameter". `SendRecoverCharacterFail(clikey, slot id, client state, code)`.

| State | Task | Method | Rule | Result |
| --- | --- | --- | --- | --- |
| 0 | 18 | `init` | Session not 2.0 | 5 |
| 0 | 18 | `init` | No such character | 158 |
| 0 | 18 | `init` | Character state not 3 and not 5 ("not stuck") | 159 with the client state (2 → 1, 4 → 2, 3 and 5 → 3) |
| 0 | 18 | `init` | World absent ❓ | 160 |
| 0 | 18 | `init` | `PermitCharacterManagement(world, session, false)` false | 160, 161 or 2 by the thread error ❓ |
| 0 | 18 | `init` | `GamedChannelManager::Acquire(belonging world)` = none | 160 |
| 0 | 18 | `init` | Else | state 1 |
| 1 | 0 | `restoreGameWorldPcReq` | `SendRestoreCharacterReq(tran(113), pcid)` unsent | 160 |
| 1 | 111 | `restoreGameWorldPcOk` | Session 6.0 → 5. Character gone → 158. Else `__restoreStuckPc`: ok → `SendRecoverCharacterOk(clikey, slot id)`; fail → Fail with its reason and the client state | — |
| 1 | 112 | `restoreGameWorldPcFail` | No parameter → 2. One gamed reason means "pc already restored in game-world" ❓ value: then the same path as Ok (6.0 → 5, gone → 158, `__restoreStuckPc`). Other reason → 2 | — |
| 1 | 113 | `restoreGameWorldPcExpired` | — | 2 |

`__restoreStuckPc(pc, &failReason)`:

| Character state | Call | Fail reason |
| --- | --- | --- |
| 3 | `Character::CancelDeletion` | ❓ |
| 5 | `Character::FinishRestore` | ❓ |
| other | none (log "invalid character state") | ❓ |

Note: the gamed tasks 111/112/113 are the same for this job and for `CancelWaitCharacterDeletionJob`.
