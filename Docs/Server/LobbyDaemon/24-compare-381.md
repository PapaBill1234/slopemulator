# 24 — Comparison of build 381 with build 348 (2019 + PDB) and build 307

Status: complete (stage 3). Section 3 lists only the pairs that were read. Open items: section 9.

## 1. Source rule, method, counts

Source rule (user decision, 2026-11-12):

| What | Source |
| --- | --- |
| Logic: jobs, checks, order of steps, database calls, timers | 2019 + PDB build (348) |
| Packets: command ids, handler table flags, formats, field order and sizes, reply codes | 381 build |
| 381 logic that differs from 348 and that the client can see | 381 build |

Method (scratch scripts in `.scratch/`, run in IDA on 381, 348 and 307):

| Step | Result |
| --- | --- |
| Dump the 5 handler tables (stride 16: handler +0, channel byte +8, fragment byte +9). Formats = format strings that the handler and its direct callees reference. Same method on all 3 builds. | `command-tables-381.json` |
| String census: every string that a function references (direct or through 1 pointer), in all 3 builds | 3714 (381), 3692 (348), 3602 (307) strings |
| Decompile each named function; collect call targets and the constants in compares, switch cases and returns | 1663 (381), 3060 (348) functions |
| Pair by name; accept a pair only with table, NP dispatch or string evidence | `fn-map-381.json`: 610 pairs (119 handler table, 42 NP dispatch, 385 same strings and constants, 64 same strings) |

Limits: VS2019 inlines more than VS2010. A constant that is only in one build is often a compiler effect (inlined packet builders, range compares such as `x - 1 <= 1` for `x == 1 || x == 2`). 284 pairs have different compare/return constants. Section 3 lists only the pairs that were read. ❓ The other pairs are not read.

## 2. Tables and formats

### 2.1 Handler tables (381)

| Table | 381 address | Dispatcher (381) | Max command | Rows |
| --- | --- | --- | --- | --- |
| LobbyManage | `0x140200180` | `ManageSessionHandler::PacketHandler` `0x140059430` | 16 | 6 |
| LobbyLobbyGate | `0x1401ffa90` | `LobbyGateChannelManager::PacketHandler` `0x1400c0610` | 110 | 35 |
| LobbyInfoGate | `0x1401ff9e0` | `InfoGateChannelManager::PacketHandler` `0x140030100` | 10 | 3 |
| LobbyGame (gamed) | `0x1401ff2a0` | `GamedChannelManager::PacketHandler` `0x14007da00` | 115 | 53 |
| AccountInventoryLobby | `0x1401ff050` | `AccountInventorySessionHandler::PacketHandler` `0x140009710` | 36 | 23 |

381 against 348: the 5 tables have the same command ids, the same handlers (by command), the same channel and fragment flags, and the same handler formats. One tool difference: AccountInventory command 34 (`HandleAccountInventoryLobby_GoodsInventoryChangedNotify`) finds `hqQq` at depth 1 only in 381. The string is in all 3 builds (inlined callee). Not a change.

381 against 307:

| Table | Command | Name | Change |
| --- | --- | --- | --- |
| LobbyGame | — | table size | max command 109 (307) → 115 (381) |
| LobbyGame | 110 | `HandleLobbyGame_UpdateAccountContentQuotaReq` | new in 381 (in 348) |
| LobbyGame | 113 | `HandleLobbyGame_UpdateAccountRecycleTimeReq` | new in 381 (in 348) |
| LobbyGame | 1 | `HandleLobbyGame_HelloReq` | reply `HelloOk` format `hCQiiiqqChii;;qqqi` (307) → `hCQiiiqqChiiii;;qqqi` (381) |
| LobbyGame | 42 | `HandleLobbyGame_UpdateAccountDataReq` | `hqqqiCQiqqihcc;qiq` (307) → `hqqqiCQiqqihcccc;qiqCihqq;Chq;` (381) |

All other 114 rows: same command, flags and formats as 307. The LobbyLobbyGate, LobbyInfoGate and LobbyManage tables (the client side through LobbyGate) have no change against 307.

### 2.2 All format strings

The string census finds no format string, no SQL text and no config key that is in 381 and not in 348, or in 348 and not in 381. Thus every packet format of 381 is the 348 format. The 4 format rows of `22-compare-307.md` §4 (column "Build 348") are the 381 formats: `EnterWorldOk`, `HelloOk`, `UpdateAccountDataReq`, `TakeAccountTimeBasedPlayRewardOk`.

### 2.3 NP (Sts) dispatch

`DispatchNPLobbyMessageHandler` `0x1400cb0e0` (348: `0x1400cb710`): 44 message ids, the same ids and handlers as 348. The 381 IDB has a wrong port name: `0x1400cd700` is named `Handle_SrvCharge_RequestStop` (❓ comment), but it handles message 2907 and uses the string "[np], handle srv-charge notify apply new charge it…" and calls `CliSession::ApplyNewChargeItem` `0x140039640`. Its 348 name is `Handle_SrvCharge_NotifyApplyNewChargeItem`. The real `Handle_SrvCharge_RequestStop` is `0x1400cd0f0`.

## 3. Handler and job changes (read so far)

| 381 address | 348 name | Finding | Effect |
| --- | --- | --- | --- |
| `0x140083ee0` | `HandleLobbyGame_HelloReq` | compares the gamed protocol value with 83 (line 69), as 348. 307: 80. | link |
| `0x1400495f0` | `DepositMileageJob::depositReq` | NP deposit request id changed. 381 converts the `BnsCommonConfig` key `np-game-code` (global `Block` = config instance, field +0x218; key table `0x140200b48`, row +0xC0: offset 0x218, end 0x430, string, default `bnsprog`) with `WideCharToMultiByte` (output size 7, return value not checked, output is an 8-byte zeroed local) and puts it first. Formats: location 0 `%s_%I64u` (348 `%I64u`); location ≠ 0 `%s_%d%.2d+_%I64u` (348 `%d%.2d+%I64u`); deposit type 1 `%s_%d%.2d+%I64u+%d+%I64u_%I64u` (348 `%d%.2d+%I64u+%d+%d+%d+%I64u_%I64u` with location, type, pc db id, `_pcSex`, `_pcJob`, `_pcRace`, request id, sub id; 348 `depositReq` `0x14004a3e0` lines 62–70). The 1-byte field is `this+1523` = `_pcJob` (348 `DepositMileageJob::init` `0x140049da0` line 38; 381 `init` `0x140048f00` line 105 copies `param+1443`). 381 drops race and sex. Buffer 0x33 bytes. See quirk `quirk_np_game_code_prefix` (§7). | link (NP) |
| `0x1400400b0` | `CreateCharacterJob::init` | world denied: error 9 → same path as 348, 13 → 130, 14 → 131, other → 2. 381 uses `v - 9`, `v - 4`, `== 1` (compiler form). | none |
| `0x140043db0` | `DeleteCharacterJob::work` (IDB name `deleteOk` has ❓; 348 `0x140045240`) | `deleteOk` (event 108) is inlined in `work`. Transit to `5 + (waitCharacterDeletion != 0)`, as 348 `deleteOk` `0x140048890` (6 if wait, else 5). Events 62/63/64/65/168/169/170/109/110 are the `work` switch. | none |
| `0x140079fc0` | `TryEnterGameWorldJob::restorePcFail` | error 35 path; 33..34 → code 110 to `SendGetEnterWorldTokenFail`, as 348 `0x14007cba0` line 115 | none |
| `0x1400806d0` | `EnterGameWorldJob::init` | 381 test `event == 117` is the inlined `work` entry; state 196611 test as 348 `0x1400850c0` | none |
| `0x14005b6e0` | `Mileage::Initialize` | 381 inlines `MileageConfig::Check` (fever start < 86400, type 1..3). 348 calls it. | none |
| `0x140076fe0` | `TakeAccountTimeBasedPlayRewardJob::sendOk` | inlined packet builder; format is the 348 format `hqqiQhqhCQiqqihcccc;Cqi;` (§2.2) | none (format: §2) |
| `0x1400c4c00`, `0x1400c8980` | `HandleLobbyLobbyGate_GetCharacterInfoListReq`, `…GetServiceEntryListReq` | 65539/131072 are inlined variant type tests; 348 constant 28 is the error text table size. Same `Send…Fail` calls. | none |
| `0x1400d3250` | `StartCliSessionWnpTokenJob::listUserLastPresencesResponse` | fail codes 5 (line 152) and 15 (line 215), then state 393216; as 348 | none |
| `0x1400bd670` | `LobbyGateChannel::SendDeleteCharacterFail` | maps client state 1→1, 2→2, 3→3, other→0, command 49, `hqoQE`; same as 348 (compiler form `v-1`). The 307 note "no compare with 2 or 3" (`22` §5) is a compiler effect too ❓ (307 not read). | none |
| `0x140062570` | `RecoverPcJob::restoreGameWorldPcExpired` | calls `LobbyGateChannel::SendRecoverCharacterFail` as 348 | none |
| `0x1400c2040` | `HandleLobbyLobbyGate_InitSessionWithNPTokenReq` | bound 7 is the size of the log string table `CSM_*` at `0x140119880`. Log only. | internal |
| `0x140055e30` | `LobbyLocale::IsAllowedChar` | same ranges as 348 (compiler form) | none |
| `0x140093d20` | `LobbyConfig::Check` | 36 = GUID length (inlined `GetId128FromGuidString`); 32766/32767 is a compiler form | none |
| `0x14009d910` and other `LobbyDB::*` | wrappers | each has `!= 10` on a variant type (line 188). ❓ compiler or ODBC helper inline | internal ❓ |

Strings in 381 only (behaviour not read): "[game-world-registry], fetch world info list for infogate fail; invalid…" (`FetchWorldInfoListForInfoGate`), "[lobby-gate], broadcast fail; invalid parameter" (`Broadcast`), "[acb], setup np info fail; invalid parameter" (`presenceLoginResponse` `0x1400d5960`), "LOBERR_INVALID_PARAMETER" (`HandleLobbyGame_UpdateBotScoreNotify` `0x140085d60`).
Strings in 348 only: `BnsCommonConfig` and `ContentsRuleConfig` reload logs, "[lobby-gate], handle hello fail; fetch pc-cafe-mileage schedule list fail", "[gamed], handle start charge request fail; init job fail", "[pc], finish create fail; set level fail", several "== CRASH ==" logs. ❓ Not read; probably log changes.

## 4. Config

No config key string differs between 381 and 348. Build strings: message library version `122` (`PrintDependency`; 348 `119`, 307 `116`); daemon version `0.0.0.484` (348 `0.0.346.440`); XML reader `XmlReader2019_sv64.dll`, console `bsConsole2019_sv64.dll`; config log text uses `{:s}`. ❓ Defaults in `LobbyConfig::Load` not compared (no constant difference found by name).

## 5. Gamed link

Same message ids and formats as 348 (section 2). Against 307 the rows of `22` §4 and the 2 new commands apply. Protocol value 83.

## 6. Database

No SQL text differs between 381 and 348. 381 has the 4 wrappers that 307 does not have (`LoadGameAccountContentQuotaList`, `UpdateGameAccountContentQuota`, `LoadGameAccountRecycleTimeList`, `UpdateGameAccountRecycleTime`).

## 7. New in 381

Only the mileage deposit id (section 3, `DepositMileageJob::depositReq` `0x1400495f0`). Effect: link (NP). The other read pairs are compiler effects (inlining, range forms). ❓ The pairs in `.scratch/d2.json` that section 3 does not list are not read; the string and format census (§2.2) found no other change.

Quirk:

| Name | Address | What the code does | Value it gets | Reached by | Port |
| --- | --- | --- | --- | --- | --- |
| `quirk_np_game_code_prefix` | `0x1400495f0` (call at `0x14004965b`+) | Converts `np-game-code` with an output size of 7 bytes and does not check the result. | Code of 6 characters or fewer: the full code. Code of 7 or more characters (the default `bnsprog` has 7): the call fails; ❓ the buffer holds the first 7 bytes (Windows writes the part that fits) and the 8th byte stays 0. | config `bns.common.config.xml` key `np-game-code` | `keep value`: prefix = first 7 bytes of the ASCII code (no memory error) |

## 8. 307 rules in the Rust code

381 gamed format strings (verified in the 381 IDB, with the one code reference):

| Message | 381 string address | Used at | Format |
| --- | --- | --- | --- |
| gamed `HelloReq` | `0x14017acd8` | `0x140083f59` | `hhhhh` |
| gamed `HelloOk` | `0x14017ba60` | `0x140084475` | `hCQiiiqqChiiii;;qqqi` |
| gamed `EnterWorldOk` | `0x140179cd0` | `0x1400811f2` | `hiqqocbbbbssQhhiQCQiqqihcccc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiqCihqq;Chq;` |
| gamed `UpdateAccountDataReq` | `0x140180318` | `0x140088596` | `hqqqiCQiqqihcccc;qiqCihqq;Chq;` |

| 307 rule in the Rust code | Where | 381 rule | Effect |
| --- | --- | --- | --- |
| `HELLO_OK = "hCQiiiqqChii;;qqqi"` | `Program/Server/Packet/src/gamed_link.rs:13` | `hCQiiiqqChiiii;;qqqi` (2 more `i` in the repeat entry) | link |
| `ENTER_OK` 307 string (`…ihcc;…qqiq`) | `Program/Server/Packet/src/gamed_link.rs:17` | 381 string above (`cccc`, tail `Cihqq;Chq;`) | link |
| `PACKET_VERSION = 83` with ❓ "381 not verified" | `Program/Server/Packet/src/gamed_link.rs:9-10` | 83 is verified in 381 (`HandleLobbyGame_HelloReq` `0x140083ee0` line 69). Remove the ❓. | link |
| gamed commands 110, 113 missing | gamed link (`packet` crate) | `UpdateAccountContentQuotaReq` (110), `UpdateAccountRecycleTimeReq` (113) exist in 381 (§2.1) | link, db |
| D41 `CheckCharacterNameReq` gives no reply | `src/jobs.rs:456` | same: 381 handler `0x1400c5730` is 38 bytes, no reply | none |
| Goods with account inventory off: reasons 4/5/7 | `src/jobs.rs:885-915` | same: 381 `GetGoodsCountJob::init` `0x14004fc50` sets 5 (line 108, state not 0x20000/0x10003) and 7 (line 147); handler rows 64–76 same formats as 307 | none |
| D60, D106, D79 formats | `src/jobs/services.rs:103,125`, `src/jobs.rs:474` | 381 rows 60 (`hq`/`hqE`), 106 (`hqQi`/`hqQiE`), 79 (`hq`/`hqE`/`hiqCqQqohq;`) same as 307 | none |
| Fail IDs (46, 49, 53, 56; 66–78) | `src/jobs/characters.rs:75`, `src/jobs.rs:899` | LobbyLobbyGate table equal to 307 (§2.1) | none |
| `11-character-select.md` §15 (307 addresses) | spec | ❓ Not mapped to 381 addresses. Logic is 348 = 381 by §2/§3 method. | ❓ |

Rust tests with fixed bytes: only in LobbyGate: `src/wire.rs:100` (frame test), `tests/wire.rs:25` (size/truncation), `tests/handshake.rs:312` (handshake frame). They are framing tests, not LobbyDaemon formats. No test has bytes from a 307 format that changed.

Rust tests that use the 307 gamed formats (no fixed bytes, but a fixed size):

| Test | Where | 307 dependency | 381 effect |
| --- | --- | --- | --- |
| `round_trip` | `Program/Server/Packet/src/gamed_link.rs:386` | `HelloOk`/`EnterOk` round trip; size assert `2 + 2 + 2 + 2 * 31 + 28` (line 443) and comment "10 for each rule" | The 2 new `i` are in the inner rule list (`Chiiii;`). `Message::HelloOk` sends this list empty, thus ❓ the size stays the same; a rule entry is 18 bytes, not 10. `EnterOk` gets 2 more (empty) lists at the end, and the `ihcc;` entry becomes `ihcccc;`. |
| `leave_messages_round_trip` | `gamed_link.rs:450` | formats 8/9/10/12 | none (same in 381) |
| `tests/leave.rs:269`, `tests/link_identity.rs:75,161,212` | `Program/Server/LobbyDaemon/tests/` | decode `Message::HelloOk` through the `packet` crate | none if both ends use the same constant |
| `Program/Server/GameDaemon/src/link.rs:115,126` | GameDaemon | decodes `HelloOk`/`EnterOk` with the same `packet` constants | changes with the `packet` crate; no own format |

`Program/Server/Packet/tests/` has no test that uses `gamed_link`.

Not in Rust yet (for the audit `ld381-2`): gamed commands 110 `HandleLobbyGame_UpdateAccountContentQuotaReq` (request `hqqqCihqq;`, reply formats `hqqE` / `hqq`) and 113 `HandleLobbyGame_UpdateAccountRecycleTimeReq` (request `hqqqChq;`, replies `hqqE` / `hqq`), the 4 database wrappers of §6, and the NP deposit id (§3, §7). ❓ The reply command ids of 110 and 113 are not read (table max command 115). The new tail lists of `EnterWorldOk` (`Cihqq;Chq;`) and `UpdateAccountDataReq` have the same entry layout as the 110 (`ihqq`) and 113 (`hq`) lists.

## 9. Open ❓

| Item | Where |
| --- | --- |
| WideCharToMultiByte output when `np-game-code` has 7 or more characters | §7 `quirk_np_game_code_prefix` |
| Other ≈260 pairs with constant differences, not read | `.scratch/d2.json` |
| IDB name `deleteOk` at 381 `0x140043db0` is really `DeleteCharacterJob::work` | §3 |
| Deposit id buffer is 0x33 bytes; the deposit type 1 id can be longer. Truncation behaviour of the format call not read (381 and 348) | §3 `0x1400495f0` |
| Reply command ids of gamed 110 and 113 | §8 |
| `11-character-select.md` §15 307 addresses not mapped to 381 | §8 |
| `HelloOk` size with empty rule lists stays the same (by format reading, not by test) | §8 |
