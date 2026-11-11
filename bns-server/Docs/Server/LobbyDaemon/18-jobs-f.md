# 18 — Jobs, batch C: start session, enter world, kick

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`. Framework and session states: `13-jobs-a.md`.

Terms used in this file:

| Term | Meaning |
| --- | --- |
| NP OK | Status 200, or status 400 with the reason text `Success` (`14-jobs-b.md`) |
| FINAL | Set the session state to 6.0 (393216) and call `CliSession::finalize` |
| F(n) | Send the Fail message of the job to LobbyGate with code `n` |
| Task `a/b` | `a` = response task, `b` = expiry task of the NP or gamed request |

## 1. StartCliSessionWnpTokenJob (start task 8, fin 16)

Address of `work`: see `12-job-map.md`. Fail message: `StartSessionWithNPTokenFail`.

### 1.1 State map

```text
0 init ─► 1 consume token ─► 3 last presences ─► 4 list slots ─► 5 create slots ─► 7 presence login
              │ (no activity status)      │ (normal)        └─► 6 set slot data ─┘      │
              └──────────────────────────►┴────────────────────────────────────────────►│
7 ─► 8 user profile ─► 9 game time ─► 10 grade level ─► 11 all grades ─► 12 user bans ─► 13 RMS inquiry ─► 14 promotion status ─► 15 login account ─► 16 fin
```

Each of the states 8 to 14 runs only if its configuration flag is on. The job skips to the next state whose flag is on.

| State | Request (NP game service) | Tasks | Entered if |
| --- | --- | --- | --- |
| 1 | `ConsumeGameToken` (portal account id, NP token, client IPv4) | 34 / 35 | always ❓ (init also has a path to state 15, condition not read) |
| 3 | `ListUserLastPresences` (user id) | 72 / 73 | `_enableActivityStatus` |
| 4 | Slot list (generic message, task 40 is the expiry) | 39 / 40 | activity status is not "normal" |
| 5 | `SlotCreateActivitySlots` | 41 ❓ / 42 | state 4 found no activity slot |
| 6 | `SlotSetActivitySlotData` | 43 / 44 | state 4 found an activity slot |
| 7 | `PresenceLogin` (game account name, user name, game account id) | 45 / 46 | always |
| 8 | `ProfileGetUserProfile` | 93 / 94 | `_teenAgeBoundary > 0` |
| 9 | `GameTimeGetGameTime` | 95 / 96 | `_enableGameTimeSchedule` |
| 10 | Grade level (generic message; group id `_gradeLevelGroupId`, or key `_gameGradeKeyForGetLevel` if the group id is ≤ 0) | 78 / 79 | `_useGradeLevel` |
| 11 | `GradeListAllGrades` | 76 / 77 | `_useGrade` |
| 12 | `ListUserBans` | 87 / 88 | `_enableContentBan` |
| 13 | `RMSInquiry` (portal account id, IPv4) | 89 / 90 | `_enableRMSInquiry` |
| 14 | `PromotionStampGetUserPromotionStatus` | 101 / 102 | ❓ time-based promotion ids exist (the flag was not read) |
| 15 | none (local login and DB load) | — | always |

### 1.2 Common rules

| Event | Result |
| --- | --- |
| The NP game service channel is absent, or the send fails | F(2), FINAL |
| Any expiry task | F(2), FINAL, state 16 |
| A response arrives and the session state is not 1.2 (65538) | F(5). No FINAL. |
| A response is not NP OK in states 1, 3, 4, 5, 6 | F(15), FINAL |
| A response is not NP OK in states 8 to 14 | F(2), FINAL |

### 1.3 State facts

| State | Fact |
| --- | --- |
| 0 init | The session state must be 1.1 (65537); if not, F(5). The job sets a presence (`CliSession::setPresence`); a failure gives F(2), FINAL. Then the session state becomes 1.2. |
| 1 response | Copies the game account name and the user name (64 characters each). Calls `CliSession::setRole` with the role list of the response. Next: 3 if `_enableActivityStatus`, else 7. One not-OK path sends F(2) ❓ in place of F(15). |
| 3 response | Computes the time since the last presence. Compares it with `_activityRestingStatusPeriod` and `_activityPreRestingStatusPeriod`. "Normal" status → 7. Other status → 4. |
| 4 response | Slot count field ≠ 0 → 6 ("activity slot exists"). Else → 5. |
| 5 response | Created slot id ≠ 0 → 7. Else F(15) and state 16. |
| 7 response | Not NP OK: reason 3352 → F(18); reason 3716 → F(16); other → F(2); then FINAL. OK: `CliSession::npLogin`, `setPcCafeCode`, `setPcCafe`; the external account key size is set to 4. If the session state is not 1.2, the job sends `PresenceLogout` and F(2). |
| 8 response | Age field < `_teenAgeBoundary` → the session flag `_teen` is true. |
| 9 response | Not NP OK → F(2) or F(17) ❓ (17 = game time refused; condition not read). One path sends `StartSessionWithLobbyTokenFail` code 2 in place of the NP-token Fail. Probable original bug ❓. |
| 10 response | `GradeService::SetGradeLevel`. Next is always 11 ❓ (no flag test seen). |
| 11 response | `GradeService::SetGradeList`. |
| 12 response | `CliSession::setContentBans`. |
| 13 response | `CliSession::setRMSInquiryResult`. |
| 14 | The job asks one time for each promotion id. Board type is 5 or 6 by target type. `CliSession::setUserPromotionStatus` stores each answer. When no id is left → 15. |
| 15 login | `CliSession::login` fails → F(16), FINAL. The job postpones one more job (start task 1 ❓ which class). `CliSession::loadDB` fails → F(16); DB result 26 → F(2) ❓; 27 → F(18) ❓; then FINAL. |
| 15 OK | If `_enableInitialUserName` is on and the user name starts with a prefix of `_initialUserNamePrefixList`: session state 1.3 (65539), Ok message with first field 1. Else: session state 2.0, Ok message with first field 0. The Ok message carries the info-gate token, and the info-gate address and port if `_useInfoGate` is on, `_useGrade`, the grade service data and `_closestEndTime`. |

## 2. StartCliSessionWlobbyTokenJob (start task 15, fin 4)

Return from the world to the lobby. Fail message: `StartSessionWithLobbyTokenFail`.

| State | Action | Tasks |
| --- | --- | --- |
| 0 init | The session state must be 5.2 (327682) and the gate must exist; if not, F(5). Next: 1 if `_useGradeLevel`; else 2 if `_useGrade` and `_enableReloadGradeWhileReturnToLobby`; else 3. | 15 |
| 1 | Grade level request (same form as §1 state 10). OK → `SetGradeLevel`; next 2 if `_enableReloadGradeWhileReturnToLobby`, else 3. | 78 / 79 |
| 2 | `GradeListAllGrades`. OK → `SetGradeList`; next 3. | 76 / 77 |
| 3 start | Makes a `LobbyGatePresence` with the gate channel id. `CliSession::setPresence` fails → FINAL, F(2). OK → session state 2.0, `StartSessionWithLobbyTokenOk` (`_useGrade`, grade data, remaining jumping-character quota). If `_enableFriendMessageNotification` is on, the job sends an NP presence update with app data `location` = `lobby` (no transaction). Next 4. | — |

All failures in states 1 and 2 (no channel, send fails, not NP OK, expiry): F(2), FINAL. An expiry also moves to state 4.

## 3. TryEnterGameWorldJob (start task 13, fin 8)

The client asks for a world token for one character slot. Fail message: `GetEnterWorldTokenFail`. Ok message: `GetEnterWorldTokenOk` (token key, world id).

### 3.1 State map

```text
0 init ─► 1 restore pc (gamed) ─► 2 second password status ─► 3 second password verified ─► 4 get slot ─► 5 charge info ─► 6 try enter ─► 7 issue token ─► 8 fin
   │            (only state 5 "stuck" characters)   (only if _enableSecondPassword)                    (only if _applyCharge)
   └─ mobile session: 0 or 1 ─► 6
```

| State | Request | Tasks |
| --- | --- | --- |
| 1 | gamed `RestoreCharacterReq` (pcid) | 111 ok / 112 fail / 113 expiry |
| 2 | NP second password status | 62 / 63 |
| 3 | NP second password "is verified" | 64 / 65 |
| 4 | NP `SlotGetSlot` (user id, slot id) | 37 / 38 |
| 5 | NP `ChargeGetInfo` (user id, IPv4) | 52 / 53 |
| 6, 7 | local | — |

### 3.2 init checks (in order)

| Check | Code |
| --- | --- |
| Session state is not 2.0 | 5 |
| No character has the slot id | 101 |
| Character job is 11 and `ContentsRuleConfig._enablePlayWarrior` is off | 117 |
| Character is blocked | 9 |
| Character state 3 | 10 |
| Character state is not 2 and not 5 | 4 |
| Character status `in_playing` or `in_service` | 5 |
| A lobby service entry exists for the character, unless its type is 2 and `_permitEnterWorldInChangeCharacterAppearance` is on | 9 ❓ or 104 ❓ |
| World-related refusals (3 values, selector not read) | 110, 102, 109 ❓ |

After the checks: the session state becomes 3.1 (196609). A mobile session (`_isMobile`) gets the roles 2 and 3. A character in state 5 goes to state 1. Else a mobile session goes to state 6. Else the job goes to state 2 if `_enableSecondPassword` is on, else to state 4.

### 3.3 State facts

All failures after init set the session state back to 2.0 (131072), except where the table says "F(5) only".

| State | Fact |
| --- | --- |
| 1 send fails or expiry | F(104) |
| 1 ok, and 1 fail with a "restorable" reason | Find the character by pcid; absent → F(101). `Character::FinishRestore` ≠ 0 → F(2). Then the same branch as after init (6, 2 or 4). |
| 1 fail | Reason 35 → continue as ok ❓. Reason 33 or 34 → F(110). Other → F(1) ❓. One early path sends F(2). |
| 2 response | Session state is not 3.1 → F(5) only. Not NP OK → F(2). Reason 3333 (no second password set ❓) → state 4. Flag bit 0 of the status field clear → state 4 ❓. Status value 1 → state 3. Else F(114). |
| 3 response | Session state is not 3.1 → F(5) only. NP OK → state 4. Reason 3333 (0xD05) → 113; 3334 (0xD06) → 114; 3345 (0xD11) → 115; 3346 (0xD12) → 116; other → F(1) or F(2) ❓. |
| 2, 3 send fails or expiry | F(7). Expiry with a wrong session state → F(5) only. |
| 4 response | Not NP OK → F(2). The slot id of the response must be equal to the requested slot id; if not, F(5) ❓. Next: 5 if `_applyCharge`, else 6. |
| 5 response | Not NP OK → F(2). Uses the charge fields, `freePlayType`, `_limitNoChargeInPcCafe` and the PC cafe data of the session. Refusal → F(112) (PC cafe limit ❓) or F(111) (no play right ❓). OK → state 6. |
| 4, 5 send fails or expiry | F(2); with a wrong session state F(5) only. |
| 6 try enter | `CliSession::enter(owner, waitInfo)`. OK with no queue position (`wid == -1`) → state 7. OK with a queue position → session state 3.2 (196610), F(107), then `StartWaitNotify` with the wait data, state 8. Failure by thread error: 7 or 9 → 103; 8 → 104; 10 → 105; 11 → 106; 12 → 108; other → 2. |
| 7 issue token | `LobbyTokenDesk::Issue` OK → session state 3.3 (196611), `GetEnterWorldTokenOk`. Failure → `CliSession::leave`, session state 2.0, F(2). |

Code 107 is not an error for the client. It says "you are in the queue".

## 4. EnterGameWorldJob (start task 117, fin 2)

gamed starts this job when the client reaches the world with its token (`gamed`, `gametid`, `pcid` all not zero). Fail message to gamed: `EnterWorldFail`.

| Step | Fact |
| --- | --- |
| init | The session state must be 3.3 (196611); if not, Fail 6. |
| Find character | By pcid in the session character list. Absent → Fail 2, close presence, FINAL. |
| Service location | The session `_srvloc` must be of kind game world. Absent → Fail 2 ❓, FINAL. |
| Start play | `Character::StartGameWorldPlay(character, ipv4)`. Failure → close presence, Fail 2 ❓. |
| Presence | Makes a `GameWorldPresence` (belonging world, pcid ❓, gamed transaction id). `setPresence` fails → FINAL, Fail 2. |
| State | Session state 4.0 (262144). Calls `srvloc->OnEnter(session)`. |
| Account maxima | Loops over all characters. Decomposes each showcase. Keeps the highest level and the highest value of one showcase byte field ❓. A decompose failure → FINAL, Fail 2. |
| Time-based play | `TimeBasedPlayInfoList::ValidateDurations`. |
| Reply | One fragment packet to gamed, command 6 ❓, format `hiqqocbbbbssQhhiQCQiqqihcccc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiqCihqq;Chq;`. Any compose failure → FINAL, Fail 2. |

Reply content in order ❓ (field names from the source, not the full list):

| Part | Content |
| --- | --- |
| Head (10 fields) | gamed transaction id, pcid, account id, auth provider code, external account key, external account name, … |
| 5 fields | role bit 2 (`roles & 4`), highest level, highest showcase value, … |
| Time-based play list | `TimeBasedPlayInfoList::Compose` |
| Repeat: characters | For each character in state 2 and not blocked: pcid, belonging world, name |
| 1 field | slot attribute of the entering character |
| 2 fields | character right, jumping-character field |
| 1 field | service location type: 1, 2, 3 or 0 |
| 1 string | PC cafe code (empty if none) |
| Repeat: grade level | Only if `_useGrade` (`GradeService::FetchGradeLevel`) |
| 1 field | grade revision |
| Repeat: grade list | Only if `_useGrade` (`FetchGameGradeList`) |
| Repeat: content bans | `CliSession::fetchContentBans` |
| 1 field | `_closestEndTime` |
| 3 fields | account exp, exp quota per day, last update time |

## 5. KickJob (start task 1, fin 5)

Parameters: kick reason and an optional `Demander` object. The demander gets the result at the end.

| State | Action |
| --- | --- |
| 0 init | Session already 6.0 → result 3, state 4. Else the job stores the old state in `_last`, sets the session to 6.0, state 1. |
| 1 kick | Acts by `_last` and by the presence kind (table below). Next 2 or 3. |
| 2 world kick | Needs a `GameWorld` presence. Acquires the gamed channel of the world; absent → result 1, state 3. Calls `srvloc->OnLeave`. Sends `KickReq` with `GetGameWorldKickReason(kickReason)`. Tasks 114 ok / 115 fail / 116 expiry. |
| 2 ok | Result 1, state 3. |
| 2 fail | Reason 24 → result 1. Other → result 2. State 3. |
| 2 expiry | Result 2, state 3. |
| 3 | `CliSession::finalize`, state 4. |
| 4 | `demander->Response(result)` if a demander exists. State 5. |

Result values: 1 = kicked, 2 = world kick failed, 3 = session was final already.

State 1 by old session state ❓ (the branch order was read through a filter):

| Old state | Presence | Action |
| --- | --- | --- |
| 1.1, 1.2, 1.3, 2.0, 3.1 | LobbyGate | `kickPresence(reason)`, result 1, state 3 |
| 3.2 (queue) | LobbyGate | `CliSession::leave`, `kickPresence`, result 1, state 3 |
| 3.3 | LobbyGate | `kickPresence`; ends the world play of the playing character ❓ |
| 4.0 | GameWorld | state 2 (world kick) |
| 5.1 | LobbyGate ❓ | `kickPresence`, result 1 ❓ |
| any | wrong kind or AnonymousLobbyGate | `closePresence`, `Character::EndGameWorldPlay` for the playing character, result 2 ❓ |

## 6. Open ❓

| Item | Where |
| --- | --- |
| Start-session init path to state 15 | §1.1 |
| Flag for state 14, and the class of the postponed job in state 15 | §1.3 |
| DB results 26 and 27 in `loadDB` | §1.3 |
| Try-enter init selectors for 110, 102, 109 and 9 / 104 | §3.2 |
| Restore fail reasons 33, 34, 35 | §3.3 |
| Charge rule for 111 and 112 | §3.3 |
| Enter-world reply: full field list and command number | §4 |
| Kick branch table | §5 |
