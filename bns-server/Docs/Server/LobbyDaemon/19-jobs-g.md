# 19 — Jobs, batch D: slot, user name, jumping character, play reward, NP schedule

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`. Terms (NP OK, F(n), tasks `a/b`): `18-jobs-f.md`. Account-inventory skeleton: `17-jobs-e.md`.

Account-inventory (AccInv) task numbers are the same in all jobs:

| Request | Tasks ok / fail / timeout |
| --- | --- |
| `RetrieveSingleWarehouseItem` (check item) | 156 / 157 / 158 |
| `TryWarehouseItemConsumption` | 159 / 160 / 161 |
| `FinishWarehouseItemConsumption` | 162 / 163 / 164 |
| `ConsumeWarehouseItem` (one step) | 165 / 166 / 167 |

## 1. AddSlotJob (fin 8)

Adds one character slot with an account-inventory item. Messages: `AddSlotWithAccountInventoryItemOk` / `Fail`.

```text
0 init ─► 1 check item ─► 2 check slot capacity (NP) ─► 3 register service entry ─► 4 try consume ─► 5 create slot (NP) ─► 6 finish consume ─► 7 finish service entry ─► 8 fin
                              (only if _checkSlotCapacity)
```

| State | Fact | Codes |
| --- | --- | --- |
| 0 init | Bad parameter → 1 ❓. Session state is not 2.0 → 5. A lobby service entry exists → 9 ❓. AccInv channel absent → 7 ❓. Slot count ≥ `_maxSlotCount` → 206 ❓. | 1, 9, 5, 206, 7 |
| 1 check item | Send fails or timeout → 7. Fail ack: reason 20 ❓ → 204, other → 2. Ok: item not usable → 205; item data id wrong → 206 ❓; other → 2. Next 2 if `_checkSlotCapacity`, else 3. | 7, 204, 205, 206, 2 |
| 2 capacity | NP request, tasks 41 / 42. Send fails or expiry → 7. Not NP OK → 2. No free capacity → 207. One path → 1 ❓. | 7, 2, 207, 1 |
| 3 register | `CliSession::addLobbyService`. Failure → 208. Thread errors 496 and 504 are in the map ❓. | 208, 2 |
| 4 try consume | All failures → 208. | 208 |
| 5 create slot | NP request (tasks ❓). All failures → 208. | 208 |
| 6 finish consume | All failures → 208. | 208 |
| 7 finish | `CliSession::removeLobbyService`, then Ok. | — |

Failures in the states 4 to 6 keep the service entry ❓ (the cleanup path was not read).

## 2. ChangeUserNameJob (start task 29, fin 5)

Changes the NP user name with an account-inventory item. Messages: `ChangeUserNameWithAccountInventoryItemOk` / `Fail`.

```text
0 init ─► 1 check item ─► 2 change user name (NP) ─► 3 consume item ─► 4 finish lobby service ─► 5 fin
```

| State | Fact | Codes |
| --- | --- | --- |
| 0 init | Parameter absent or empty → 1 ❓. `_enableChangeUserNameService` off → 7. Session state is not 2.0 → 5. A service exists → 9. Name check (table in §4, base 212). The check is skipped or changed if `_tencentUICEnable` is on ❓. | 1, 7, 5, 9, 6, 211–215 |
| 1 check item | Send fails or timeout → 7. Fail ack: reason 20 → 209, other → 2. Ok: item absent → 211; item type field ≠ 1 → 210. | 7, 209, 210, 211, 2 |
| 2 change name | `CliSession::addRenameUserService`, then NP `AuthChangeMyUserNameIncomplete`, tasks 70 / 71. Send fails or expiry → 2. Session state is not 2.0 → 5 ❓. Not NP OK: name in use → 216; status 407 → 218; other NP refusal → 219; reason 3003 path → 217 ❓; else 2. | 2, 216, 217, 218, 219 |
| 3 consume | `ConsumeWarehouseItem`. Ok → 4. Fail or timeout: the job sends **Ok** to the client and goes to 5. The name is changed already. | — |
| 4 finish | Releases the rename service. Sends Ok. | — |

## 3. CreateInitialUserNameJob (start task 27, fin 2)

Sets the first user name for an account whose name has a configured prefix (`18-jobs-f.md` §1.3, session state 1.3). Messages: `CreateInitialUserNameOk(newUserName)` / `Fail`.

| Step | Fact | Codes |
| --- | --- | --- |
| init | `_enableInitialUserName` off → 6 ❓. Empty name → 1 ❓. Session state is not 1.3 (65539) → 5. | 6, 1, 5 |
| Name check | Uses `_allowedUserNameLenMin`, `_allowedUserNameLenMax`, `_enableMultilingualUserName`, `_forceSentenceCaseForUserName`. Table in §4, base 224. With `_tencentUICEnable` one path gives 228 ❓. | 6, 224–227 |
| Request | NP change-user-name request, tasks 70 / 71. Channel absent → 2. Send fails → 7. Expiry → 7. | 2, 7 |
| Response | Not NP OK: name in use → 228; status 407 → 230; other → 231; one path → 229 ❓; else 2. OK → Ok message, the session continues to 2.0 ❓ (the state write was not read). | 228–231, 2 |

## 4. Name check result map

| `CHECKNAME_*` | Change user name | Initial user name |
| --- | --- | --- |
| INVALID_PARAMTER, SENTENCE_CASE, MULTILINGUAL | 6 | 6 |
| TOO_SHORT | 212 | 224 |
| TOO_LONG | 213 | 225 |
| SIDE_SPACE, DOUBLE_SPACE, SPACE_OVERFLOW | 214 | 226 |
| NOT_ALLOWED | 215 | 227 |
| FORBIDDEN | 215 ❓ | 227 ❓ |

## 5. ExtendJumpingCharacterCreationQuotaJob (fin 7)

Same 7-state skeleton as `17-jobs-e.md` (check item, register entry, try consume, finish consume, finish entry), with no gamed request step. The requester is the lobby client or gamed (in world). The job sends the reply to the requester.

| Fact | Value |
| --- | --- |
| Flag | `_enableJumpingCharacterCreation` off → 7 |
| Init codes | 2, 1, 9, 5, 222 |
| Check item | fail ack reason 20 ❓ → 220; other → 2; 221 and 223 = item unusable / wrong data id ❓ |
| Thread errors in the entry step | 415, 423 ❓ |
| Lobby requester | `LobbyGateChannel::SendExtendJumpingCharacterCreationQuotaWithAccountInventoryItemOk` / `Fail(code)` |
| Gamed requester | `GamedChannel::Send…Ok` / `Fail(mapped code)` |

Code map for the gamed requester:

| Lobby code | 2 | 5 | 7 | 9 | 220 | 221 | 222 | 223 | other |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Gamed code | 2 | 6 | 7 | 59 | 60 | 61 | 62 | 63 | 1 |

## 6. UpgradeJumpingCharacterStateJob (fin 2)

gamed asks to move a jumping character to its next state. Two reply pairs exist. `_consumeCreationQuota` selects the pair.

| `_consumeCreationQuota` | Ok / Fail message to gamed |
| --- | --- |
| true | `UpgradeJumpingCharacterStateWithCreationQuotaOk` / `Fail` |
| false | `UpdateJumpingCharacterStateOk` / `Fail` |

| Check (in order) | Code |
| --- | --- |
| A parameter (gamed tid, world id, pcid) is zero | job ends, no reply ❓ |
| Session state is not 4.0 | 5 |
| `_useJumpingCharacterCreationQuota` or `_useJumpingCharacterCreationQuotaInWorld` off (only tested with quota ❓) | 7 |
| Character not found by pcid | 2 |
| `jumpingCharacterState` of the character is not 1 | 65 |
| With quota: `reservedCreationCount + creationCount >= creationQuota` | 64 |
| `Character::UpgradeJumpingCharacterState(character, account or null, flag)` ≠ 0 | 2 |

Ok carries the gamed tid and the new jumping state. With quota the call gets the game account, so the DB call also uses one quota unit.

## 7. TakeAccountTimeBasedPlayRewardJob (start task 143, fin 2)

gamed asks for the time-based play reward of an account. Fail message to gamed: `TakeAccountTimeBasedPlayRewardFail(gameTid, pcdbid, scheduleNumber, targetType, playTime, requestTime, …, failReason, npError)`.

| Step | Fact |
| --- | --- |
| init | Board type: 5 or 6 by target type (0 → none ❓). Session state is not 4.0 → Fail. `ContentsRuleConfig._useTimeBasedPlaySchedule` off → Fail 7 with NP error 0. |
| Request | NP `PromotionStampAttendPromotionSecured` (user id, schedule number, board type, …), tasks 103 / 104. Send fails → Fail. |
| Response not NP OK | NP errors 10, 42, 6723, 6725, 6727 each have one fail reason; 6726 → reason 98; default reason 94. The Fail carries the NP error code as its last field. ❓ exact reason for each of the 5 errors. |
| Response OK | Reads up to 24 reward records (32 bytes each). A record with status 3 is a delivered reward ❓. `CliSession::UpdateTimeBasedPlayReward` stores the result. No reward delivered and count 0 → Fail. Else the Ok packet (command 8 ❓) with a repeat list of reward ids and the count of failed rewards. |
| Expiry | Fail (NP error 0) ❓. |

## 8. SetupNpScheduleInfoJob (fin 4)

Runs at start-up or by timer, with no session. It loads the promotion schedule from NP into the daemon.

| State | Fact | Tasks |
| --- | --- | --- |
| 0 | Moves to 1. | — |
| 1 | NP `PromotionStampListCurrentPromotions`. The response sorts the promotions into the time-based schedule slots (indexes 0 to 3 ❓). | 97 / 98 |
| 2 | NP `PromotionStampGetPromotionPolicy` for each promotion found. The response fills the schedule data (up to 5 ❓ entries, buffer 2048). Target type map: 1 → 0 ❓, 2 → 2, 3 → 3. | 99 / 100 |
| 3 | Stores the result. Next 4. | — |

`StartCliSessionWnpTokenJob::init` reads this schedule (`scheduleNumber != 0`) to build its promotion id list (`18-jobs-f.md` §1.1 state 14).

## 9. Small classes

| Class | Fact |
| --- | --- |
| `NPLobbyJobTransaction` | Holds (job id, job state, expiry task). The NP reply or the expiry calls `LobbyJobManager::ExecuteW` / `ResumeW` (`13-jobs-a.md` §1). |
| `DepositPcCafeMileageJobManageCallback` | Callback of the manage port for the mileage deposit job (`15-jobs-c.md`). It sends the job result to the manage client ❓. |

## 10. Open ❓

| Item | Where |
| --- | --- |
| Init code order and conditions of AddSlot, ExtendQuota | §1, §5 |
| Cleanup of the service entry after a late AddSlot failure | §1 |
| NP reason values behind 216–219 and 228–231 | §2, §3 |
| Fail reasons of the 5 NP errors in the play reward job | §7 |
| Slot indexes and counts in the schedule job | §8 |
