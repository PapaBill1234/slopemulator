# 20 — Batch E: handlers without a job, timers, manage port

Build 307: read `22-compare-307.md` first. It removes 2 handlers and 1 timer and gives the 307 packet formats.

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`. Command numbers: `command-tables.json`. Terms (NP OK, tasks `a/b`): `18-jobs-f.md`.

Common codes in this file: 2 = bad packet or internal error, 4 = no such session or index, 5 = session not found or client key mismatch (gamed side).

## 1. Handler classes

| Class | Count | Rule |
| --- | --- | --- |
| Hello | 4 | Checks the peer, makes the channel, sends the first data (§2). |
| Job start | 38 | Finds the session, makes the job, registers it. Failure → the Fail of the request with 2 (job start) or 4 / 5 (no session). The job specs are in `13` to `19`. |
| Job resume (peer reply) | 60 | `…Ok` / `…Fail` from gamed, account inventory, post office and NP. The handler passes the reply to the job (§3). |
| Direct | 31 | The handler does all work (§4 to §8). |

## 2. Hello handlers

All four handlers answer a failure with the packet `"hh"` = (command 3, reason) and close. A decompose failure and a packet version mismatch are failures.

| Peer | Request format | Checks, in order | Ok data |
| --- | --- | --- | --- |
| Lobby gate | `hhhh` cmd, gate wid, packet version, internal port | Version. `BLDaemonInfoTable::Find(gate wid)` (lobby gate or mobile gate). Peer IP = `internal_ipv4` of the table row. World info list. If `_enableMileage`: mileage season expire time and fever time list. Accept. `LobbyGateChannelManager::Add` (duplicate → close both channels). | Fragment packet: world info list, then the fever time list. |
| Info gate | `hhhh` cmd ❓ order, port, version | Version (value 3 ❓). Table row. Peer IP. `FetchWorldInfoListForInfoGate`. Accept. `InfoGateChannelManager::Add` (duplicate → close both). | World info list. |
| gamed | `hhhhh` cmd, world cluster id, world id, packet version, internal port | Version. Table row. Peer IP. `GameWorldRegistry::Get(world id)` exists. Cluster id matches. No channel exists for the world (duplicate → close both). `InitializeTransactionChannel(_lobbyJobTransactionLifeTime)`. Accept. `GamedChannelManager::Add`. | Format `hCQiiiqqChiiii;;qqqi` ❓ fields, then the time-based play season schedule, then the random store initialization schedule, then `_gameMileageEffectiveDurationInDays`. |
| Manage | `hhh` cmd, manage wid, packet version | Version = 7. No IP check ❓. | Packet `"h"` = (2). |

`Game_ServiceOpenNotify` (`hhh` cmd, cluster id, world id): same world checks as the gamed Hello. Then the world state becomes live. No reply.

## 3. Job resume handlers

Two forms exist.

| Form | Used by | Rule |
| --- | --- | --- |
| A | Most `…Ok` / `…Fail` of gamed, account inventory, post office | `LobbyJobManager::find(job id)`, `LobbyJob::transit(task, packet)`, and `remove` if the job reached fin. |
| B | All NP replies; gamed `KickOk`, `RestoreCharacterOk`, `AuctionEscrowOk`; AccInv `TryWarehouseItemConsumptionOk`, `ConsumeWarehouseItemOk` | `LobbyJobManager::ExecuteW(job id, task, message)`. |

NP form B guard: the message type is a response, a transaction exists, and `GetKind() == 1`. Else the handler drops the message. The task numbers are in the job specs (examples: profile 93, game time 95, get slot 37).

| No job, log only | Rule |
| --- | --- |
| AccInv `HelloOk` / `HelloFail`, `CancelWarehouseItemConsumptionOk` | Log only. |
| Post office `HelloOk` / `HelloFail` | Log only. |
| NP `SrvGate_RegisterService`, `SrvPres_Message`, `SrvPres_Kick` responses | NP OK test, then log only. |
| NP `CommonPres_UserInfo` notify | Log only. |
| `LobbyGate_CheckCharacterNameReq` | Not available. Log only, no reply. |

## 4. Lobby gate, direct handlers

| Request | Format | Fact | Codes |
| --- | --- | --- | --- |
| `InitSessionWithNPTokenReq` | `hqooi` cmd, gate key, NP token, portal account id, IP | `CliSessionManager::InitSessionWnpToken`. Its error is the Fail code. | 2, error |
| `StartSessionWithNPTokenReq` | `hqoo` cmd, client key, NP token, portal account id | `CliSessionManager::StartSessionWnpToken` (starts the job of `18-jobs-f.md` §1). | error |
| `GetWaitInfoReq` | `hq` | `CliSession::GetGameWorldWaitInfo` → Ok (position, turn, average wait time in ms). | 4, error |
| `GetCharacterInfoListReq` | `hq` | `CliSession::FetchCharacterInfoList` into a fragment packet. | 4, error |
| `GetCharacterInfoReq` | `hqo` cmd, client key, slot id | `CliSession::FetchCharacterInfo`. Reply format `hqqohsbqQQi` ❓ fields. | 4, error |
| `GetServiceEntryListReq` | `hq` | `CliSession::FetchServiceEntryList`. | 4, error |
| `LostAnonymousConnectionNotify` | — | Session by gate key → `LogoutJob`. | — |
| `LostConnectionNotify`, `LogoutNotify` | — | Session by client key → `LogoutJob`. | — |

Job start facts not in the job specs:

| Request | Fact |
| --- | --- |
| `InitSessionWithLobbyTokenReq` | `LobbyTokenDesk::Verify` runs in the handler. Failure → Fail with the verify error. |
| `GetEnterWorldTokenReq` | Codes 6 ❓ and 4 in the handler. |
| `GetChargeInfoReq`, `ActivateGradeReq` | Code 7 in the handler (feature off ❓). |
| `DeleteCharacterReq`, `CancelWaitCharacterDeletionReq`, `RecoverCharacterReq` | The handler composes the Fail packet itself (`PacketBufferPool::allocate`). |

### 4.1 Handler checks and reply formats (build 307, read 2026-11-12)

Source: build 307 handlers (Hex-Rays, `ida/LobbyDaemon_2020/LobbyDaemon.exe.i64`) and the `LobbyGateChannel::Send*` senders (format string in the sender). The sender addresses come from `fn-map-307.json`.

The Ok / Fail command numbers come from `02-links.md` (command numbering ❓). All handlers below: a decode failure gives no reply. "Job start fail" is `LobbyJobManager::Register` false.

| Req | Handler | Order of checks → Fail code | Ok / Fail cmd | Ok format | Fail format |
| --- | --- | --- | --- | --- | --- |
| 28 GetWaitInfo | `0x1400c0520` | no session → 4; `GetGameWorldWaitInfo` error → that error | 29 / 30 | `hqiiCch;` (`0x1400b8ff0`) | `hqE` |
| 31 GiveupWait | `0x1400c0770` | no session → 4; job start fail → 2; one more path → 6 ❓ | 32 / 33 | `hq` | `hqE` |
| 57 BindCharacterSlot | `0x1400c2ca0` | no session → 4; job start fail → 2 | 58 / 59 | `hqoq` (`20-handlers` §4 table above) | `hqoqE` (clikey, slot id, pcid, code) |
| 60 GetChargeInfo | `0x1400c3250` | no session → 4; config byte +400 = 0 → 7; job start fail → 2 | 61 / 62 | `hqQQihQiQQQ` | `hqE` |
| 106 ActivateGrade | `0x1400c6af0` | config byte +413 (`useGrade`) = 0 → 7, before the session lookup; no session → 4; job start fail → 2 | 107 / 108 | `hqiiqq` | `hqQiE` (clikey, game grade key, grade id, code) |
| 82 appearance | `0x1400c44c0` | no session → 4; job start fail → 2; then the job (`17-jobs-e.md` §3) | 83 / 84 | `hqobqq` | `hqoqqE` (clikey, slot id, label id, item instance id, code) |
| 85 sex | `0x1400c4ac0` | as 82 | 86 / 87 | `hqobqq` | `hqoqqE` |
| 88 race | `0x1400c5100` | as 82 | 89 / 90 | `hqobqq` | `hqoqqE` |
| 91 rename | `0x1400c5780` | as 82 | 92 / 93 | `hqosqq` | `hqoqqE` |
| 94 add slot | `0x1400c5d20` | as 82 | 95 / 96 | `hqqqo` | `hqqqE` (clikey, label id, item instance id, code) |
| 97 user name | `0x1400c6070` | as 82 | 98 / 99 | `hqsqqi` | `hqqqE` |
| 100 jumping quota | `0x1400c6430` | as 82 | 101 / 102 | `hqqqi` | `hqqqE` |

Other senders: `CloseSessionNotify` `hq`; `StartWaitNotify` `hqiiCch;`; `EndWaitNotify` `hqq`.

Bug, not ported: the 91 handler sends the job-start failure (code 2) with `SendChangeCharacterAppearanceWithAccountInventoryItemFail` (command 84). The port sends 93.

## 5. gamed, direct handlers

Session lookup is by pcid (`CliSessionTable::Select_index_pcid`). "Key check" = the client key in the request must be the client key of the session; a mismatch → 5.

| Request | Format | Fact | Codes |
| --- | --- | --- | --- |
| `NPPresenceKickReq` | `hqQQ` ❓ | Session by pcdbid. NP channel must exist. Target app must be known. `NPGameServiceChannel::SendRequestPresenceKick`. The handler sends a kick notify to gamed ❓. | log only |
| `UpdateBotScoreNotify` | — | `CliSession::UpdateBotScore`. | — |
| `UpdateCharacterDataReq` | `hqqqhb` cmd, game tid, pcid, client key, level, showcase blob | Key check. `CliSession::UpdateCharacterData(pcid, level, showcase)`. | 5, error |
| `UpdateAccountDataReq` | Repeat packet: game tid, pcid, client key, bot score; time-based play info list; account exp info (3 fields); content quota list (4 fields each); recycle time list (2 fields each) | Key check. Calls, in order: `UpdateAccountData`, `UpdateAccountExp`, `UpdateAccountContentQuota`, `UpdateAccountRecycleTime`. The first failure gives the Fail code ❓ (later calls may still run). | 5, error |
| `UpdateAccountExpReq` | game tid, pcid, client key; exp info (3 fields) | Key check. `CliSession::UpdateAccountExp`. Ok carries pcid. | 5, error |
| `UpdateAccountContentQuotaReq` | same head; quota list | `CliSession::UpdateAccountContentQuota`. | 5, error |
| `UpdateAccountRecycleTimeReq` | same head; recycle list | `CliSession::UpdateAccountRecycleTime`. | 5, error |
| `GetGradeListReq` | game tid, location hint, pcid, request revision | `CliSession::HandleGameGetGradeList` sends the reply. | 5 |
| `SetAppDataNotify` | pcid; repeat (key, value) | Needs `_enableFriendMessageNotification` ❓ (same flag as friend message). Key and value length checks. Sends the NP presence `SetAppData` message. No reply to gamed. | log only |
| `FriendMessageNotify` | pcid; message name; repeat (key, value) | Needs `_enableFriendMessageNotification`. Length checks. `SendRequestFriendSendMessage`. One message type also calls `CliSession::UpdateCharacterLevel` ❓. | log only |
| `BoastEventInWorldNotify` | — | `InfoGateChannelManager::BroadcastBoastEventInWorldNotify`. | — |

Job start facts:

| Request | Fact |
| --- | --- |
| `EnterWorldReq` | `LobbyTokenDesk::Verify` in the handler. Codes 2, 4, 17, verify error. |
| `LeaveWorldReq` | Codes 2, 4, 5. One failure path sends `EnterWorldFail`, not `LeaveWorldFail` ❓ (probable original bug; copy it). |
| `DepositGameMileageReq`, `TakeAccountTimeBasedPlayRewardReq` | Fail with reason 0 fields when the job does not start ❓. |

### 5.1 Guild name handlers (LobbyDB, no job)

| Request | Format | Fact | Codes |
| --- | --- | --- | --- |
| `TryRegisterGuildReq` | `hqcs` cmd, game tid, guild type, name (max 32) | World must be registered → else 2. `RuleSet::CheckName` → its mapped code. `LobbyDB::TryGuildRegistration(cluster id, world id, type, name)` → Ok(game tid, guild id). DB failure → mapped code ❓. | 2, name code, DB code |
| `FinishRegisterGuildNotify` | `hqq` cmd, game tid, guild id | `LobbyDB::FinishGuildRegistration`. No reply. | — |
| `CancelRegisterGuildNotify` | `hqq` | `LobbyDB::CancelGuildRegistration`. No reply. | — |
| `TryRenameGuildReq` | `hqqcs` cmd, tid, guild id, rename type, new name | `RuleSet::CheckName` → 4, 50 or 51 ❓ map. `LobbyDB::TryRenameGuild(guild id, type, name, _guildNameReuseLimitDuration)` → Ok(tid, guild id, type, rename key). DB failure → mapped code. | 2, 4, 50, 51, DB code |
| `FinishRenameGuildNotify` | `hqqq` cmd, tid, guild id, rename key | `LobbyDB::FinishRenameGuild`. No reply. | — |
| `CancelRenameGuildNotify` | `hq` cmd, rename key | `LobbyDB::CancelRenameGuild`. No reply. | — |
| `DeleteGuildNotify` | `hqq` cmd, game tid, guild id | `LobbyDB::DeleteGuild(guild id, _guildNameReuseLimitDuration)`. No reply. | — |

## 6. Info gate

| Request | Format | Fact | Codes |
| --- | --- | --- | --- |
| `AuthTokenReq` | `hqqo` cmd, tid, client key, info gate token | Session by client key → else 4. Token = `_infoGateToken` of the session (16 bytes) → else 15. Ok carries the user id. | 2, 4, 15 |
| `GetWorldInfoListReq` | `h` | `FetchWorldInfoListForInfoGate`, row format `hiChhQ;` ❓ fields. | 2 ❓ |

## 7. Manage port

| Request | Format | Fact | Codes |
| --- | --- | --- | --- |
| `HelloReq` | §2 | — | — |
| `AnnounceNotify` | `hs` cmd, message (max 1024) | `LobbyGateChannelManager::BroadcastAnnounceNotify`. No reply. | — |
| `KickReq` | `hqo` cmd, tid, account id | Session by account → else 4. Starts `KickJob` with the manage session as demander (`18-jobs-f.md` §5). | 2, 4 |
| `CleanupGameAccountIdIndexReq` | `hqo` | Finds the account index entry and deletes it from the session table. | 2, 4 |
| `CleanupPcidIndexReq` | `hqqh` cmd, tid, pcid, world id | Finds the pcid index entry and deletes it. | 2, 4 |
| `RetryMileageDepositReq` | `hqq` ❓ order: cmd, tid, deposit request id | `LobbyDB::RetrieveMileageDeposit` → else 3. Starts `DepositMileageJob` (task 5) with `_mileageDepositRequestCode` and `_mileageDepositReason`. Start failure → 2. One path → 9 ❓. | 2, 3, 9 |

## 8. NP requests and notifies (no job)

| Message | Fact |
| --- | --- |
| Charge stop request | Session by user id. `CliSession::OnChargeStopRequested` → refusal sends `ChargeRequestStopFail("ErrBadData", 4)`. Else `CliSession::ReserveStopPlay(false, …, 9)`. |
| Charge new item notify | `CliSession::ApplyNewChargeItem`. |
| Charge remain time notify | `CliSession::NotifyChargeRemainTime`. |

## 9. Timers

All timers use `GlobalTimerQueue`. Each timer adds itself again in `HandleEvent`.

| Timer | Period | Work |
| --- | --- | --- |
| `LobbyTick` | `_period` ❓ value | Makes one `LobbyTickEvent(elapsed)` for each listener. Elapsed = `GetTickCount64() - StartSystemTick64`. Listeners: `LobbyTokenDesk::Tick`, `MileageInterimDepositTimer::Tick`, `MileageBatchRetryDeposit::Tick` ❓ full list. |
| `LobbyCharacterDeletionScheduler` | `period` from config ❓ name | `LobbyDB::RetrieveExpiredDeletionWaitingList(concurrentProcessingCount)`, then `LobbyDB::ExpireCharacterDeletionWaiting(pcid)` for each row. Start fails if it runs already or the period is invalid. |
| `GameTimeScheduler` | 3600 s | One queue for each hour of the week (168 queues). Each event moves to the next queue and runs its nodes. |
| `MileageInterimDepositTimer` | node expiry = register time + `_period` | Expired nodes run `Node::Expire` (interim mileage deposit). |
| `MileageBatchRetryDeposit` | each tick | `LobbyDB::LoadRetryMileageDeposit(_retryCountPerTick, _retryIdFrom, …)`. Starts one `DepositMileageJob` for each row. |
| `LobbydReloadInvalidRestoreTimer` | ❓ | `restoreInvalid` ❓ (not read). |
| `NPPing` | `NPPing::_interval` | Ping to NP. |
| `ReconnectTimer` | ❓ | Reconnect of an outbound channel (NP, account inventory, post office) ❓. |
| `ObjPoolTimer` | ❓ | Pool statistics ❓. |
| Stop play (`CliSession::TimeoutStopPlay`) | set by `ReserveStopPlay` | Ends play when the charge time ends. The session must be in a service location. |

`GameWorldRegistry::Update` (caller ❓: timer or manage command): `LobbyDB::LoadGameWorldCluster`, then `LobbyDB::LoadGameWorld`. It updates or adds clusters and worlds. A new larger max player count ends the wait of queued sessions with `EndWaitGameWorldJob` (postponed).

## 10. Open ❓

| Item | Where |
| --- | --- |
| Field lists of `hCQiiiqqChiiii;;qqqi`, `hqqohsbqQQi`, `hiChhQ;` | §2, §4, §6 |
| Info gate Hello field order and version value | §2 |
| Name check and DB code maps of the guild handlers | §5.1 |
| Fail code after a partial `UpdateAccountDataReq` | §5 |
| Periods and callers of the timers | §9 |
| `LobbydReloadInvalidRestoreTimer`, `ReconnectTimer`, `ObjPoolTimer` bodies | §9 |
