# 22 — Comparison of build 348 with build 307 (batch G)

The specs `13` to `21` use build 348 (PDB). The target is build 307. This file gives the differences. A fact that is not in this file is the same in the two builds by the test of section 1.

## 1. Method

Tools: `tools/ld_match/` (run in IDA and offline). Function map: `fn-map-307.json`.

| Step | Tool | Result |
| --- | --- | --- |
| 1. Fingerprint of each function: strings, callees, size | `fp.py` | 3087 functions (348), 3047 functions (307) |
| 2. Match: command tables, unique strings, callee order, call context | `match.py`, `match2.py` | 2007 pairs |
| 3. Give the 307 functions the 348 names and argument sizes | — | 1978 prototypes |
| 4. Compare the call order and the strings of each pair | `d1.py` | 1720 of 1864 named pairs are equal |
| 5. Compare the constants of each pair (call arguments, compare values, switch cases, return values) | `ct.py`, `d2.py` | 1813 of 1864 named pairs are equal |

| Group | Matched |
| --- | --- |
| Job methods | 467 of 472 (the 5 others are base class helpers and destructors) |
| Packet handlers | all but 2 (section 3) |
| `LobbyDB::*` wrappers | 90 of 94 (section 2) |

Limits of the test:

- The test ignores the values 0 and 1, struct offsets, and virtual calls. ❓ A difference only in these is not found.
- The test does not compare the order of the checks in a function body. ❓
- A pair "equal" means: same strings, same callees in the same order, same constants above 1.

## 2. Database wrappers (correction of `21-lobbydb-wrappers.md`)

| Fact | Value |
| --- | --- |
| Wrappers in build 307 | 90 |
| Wrappers only in build 348 | 4: `LoadGameAccountContentQuotaList`, `UpdateGameAccountContentQuota`, `LoadGameAccountRecycleTimeList`, `UpdateGameAccountRecycleTime` |
| Wrappers in build 307 that `db-binding.json` does not have | 3 (table below) |

| Wrapper | Address in 307 | SQL |
| --- | --- | --- |
| `LobbyDB::LoadGameWorldCluster` | `0x14009edd0` | `{?=CALL dbo.usp_LoadGameWorldCluster}` |
| `LobbyDB::LoadGameWorld` | `0x14009f6e0` | `{?=CALL dbo.usp_LoadGameWorld}` |
| `LobbyDB::LoadMileage` | `0x1400b22e0` | `{?=CALL dbo.usp_LoadMileage}` |

The 3 procedures have no parameter list. ❓ This is the probable cause of the gap in `db-binding.json`. Build 307 loads the world list with the same 2 procedures as build 348. The column binds of the 3 wrappers are not in `db-binding.json` (open).

All other wrappers have the same SQL text and the same bind constants in the two builds.

## 3. Functions only in build 348 (do not port)

| Area | Functions |
| --- | --- |
| Account content quota | `HandleLobbyGame_UpdateAccountContentQuotaReq`, `CliSession::UpdateAccountContentQuota`, `CliSession::fetchAccountContentQuotaList`, `GamedChannel::SendUpdateAccountContentQuotaOk` / `Fail`, 2 wrappers |
| Account recycle time | `HandleLobbyGame_UpdateAccountRecycleTimeReq`, `CliSession::UpdateAccountRecycleTime`, `CliSession::fetchAccountRecycleTimeList`, `GamedChannel::SendUpdateAccountRecycleTimeOk` / `Fail`, 2 wrappers |
| Common config | `BnsCommonConfig::*` (load, reload, dump). `LobbyDaemon::reload` of 307 does not reload it. |
| Reload timer | `LobbydReloadInvalidRestoreTimer` (60,000 ms). Not in 307. Remove it from the timer list of `20-handlers.md`. |
| Showcase | `DecomposeShowcaseFsd` in `EnterGameWorldJob::enterGameWorld` |
| Play info | `TimeBasedPlayInfoList::SetPlayInfoFromGame` in `CliSession::UpdateAccountData` |

The gamed handler table of 307 ends at command 108. The table of 348 ends at command 114.

## 4. Packet formats that differ

| Message | Build 307 | Build 348 |
| --- | --- | --- |
| gamed `EnterWorldOk` (`EnterGameWorldJob::enterGameWorld`) | `hiqqocbbbbssQhhiQCQiqqihcc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiq` | the same with `cccc` and the tail `Cihqq;Chq;` |
| gamed `HelloOk` | `hCQiiiqqChii;;qqqi` | `hCQiiiqqChiiii;;qqqi` |
| gamed `UpdateAccountDataReq` | `hqqqiCQiqqihcc;qiq` | `hqqqiCQiqqihcccc;qiqCihqq;Chq;` |
| gamed `TakeAccountTimeBasedPlayRewardOk` | `hqqiQhqhCQiqqihcc;Cqi;` | `hqqiQhqhCQiqqihcccc;Cqi;` |

Rule: the time-based play entry has 2 `c` fields in 307 and 4 in 348. The lists for content quota (`Cihqq;`) and recycle time (`Chq;`) are not in 307.

## 5. Behaviour that differs

| Function | Build 307 | Build 348 |
| --- | --- | --- |
| `HandleLobbyGame_HelloReq` | compares a value with 80 ❓ (protocol version) | 83 |
| `BLDaemonInfoTable::load` | version string `14` | `15` |
| Message library version check | 116 | 119 |
| Time-based play: target types | 2 | 4 |
| Time-based play: reward records in the NP response (`19-jobs-g.md` §7) | 12 | 24 |
| `TimeBasedPlaySeasonSchedule` entry size | 28 bytes | 52 bytes |
| `SetupNpScheduleInfoJob::getPromotionPolicyAck` | has a "duplicate reward" failure log. Slot count 12 ❓. | no such log. Counts 24 and 4. |
| `StartCliSessionWnpTokenJob::listUserLastPresencesResponse` | has one more check: "presence type must be server-logout type" (log line 556). ❓ Reply code of this path. | no such check. Has one more path with code 15. |
| `RecoverPcJob::restoreGameWorldPcExpired` | builds the Fail packet in the function: command 56, format `hqoQE`, code 2 ❓ | calls `LobbyGateChannel::SendRecoverCharacterFail(2)`. Same result ❓. |
| `HandleLobbyLobbyGate_HelloReq` | one failure path sends `"hh"` (3, 2) in the handler | same reply through another path ❓ |
| `LobbyConfig::Check` | checks `np-app-id`, `np-client-app-id`, `np-program-id`, `np-game-code` in the function | calls `GetId128FromGuidString` |
| `LobbyDaemon::reload` | no common config reload, no restore timer | both |
| `LobbyGateChannel::SendDeleteCharacterFail`, `SendCancelWaitCharacterDeletionFail`, `SendRecoverCharacterFail` | no compare with 2 or 3 | compares with 2 and 3 ❓ (not read) |

## 6. Result for the port

| Spec | Use for build 307 |
| --- | --- |
| `13` to `19` (jobs) | All state machines and reply codes are valid. Exceptions: section 5 (2 job methods) and the time-based play counts. |
| `20-handlers.md` | Valid. Remove the 2 handlers and the timer of section 3. Use the formats of section 4. |
| `21-lobbydb-wrappers.md` | Valid with the correction of section 2. |

## 7. Open ❓

| Item | Where |
| --- | --- |
| Reply code of the extra presence check in 307 | §5 |
| Column binds of the 3 wrappers that `db-binding.json` does not have | §2 |
| The 3 Fail senders with the 348-only compare | §5 |
| Differences only in the values 0 and 1 or in the order of checks | §1 |
| Caller maps from `dbrtn` to wire codes (`21-lobbydb-wrappers.md`) | not in this batch |
