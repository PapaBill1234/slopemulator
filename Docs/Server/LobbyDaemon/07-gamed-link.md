# 07 — GameDaemon link

## Report

| Part | State | Evidence |
| --- | --- | --- |
| Eight handler formats | Partial: 1 and 83 are now full; six remain partial | 307 `0x1400722B0`, `0x1400712C0`; G4 field tables |
| Transport | Partial: connection, Hello, framing, transaction expiry and close path checked | 307 `0x14009A0B0`, `0x14007A250`, `0x14007A4E0`, `0x14007F730` |
| First real link | Checklist added; not tested; full success payload remains a blocker | 307 `0x140087D40`, `0x140083BE0`, `0x140083D60` |

**New totals: 45 full / 6 partial**, up from 43 / 8. All 51 receive literals remain checked.
New full commands: **1, 83**. Partial commands: **5, 42, 80, 92, 100, 104**.
❓ Gaps: entry-field names, time-record expiry meaning, NP status-field names,
grade-field names, four mileage request meanings, fragment metadata bits,
keep-alive, socket deadlines, reconnect delay, and pending-entry cancellation.
Research stopped at the turn limit for new work. The eight-handler task is not complete.
Only the two permitted spec files changed. `STATUS.md` stayed unchanged.
No Rust code, network use, binary patch, or IDB save was done.

## G4 — Current specification

G4 replaces all conflicting G2 and G3 claims below.
The old reports, coverage lists and remaining-work lists are historical.
Full means checked receive and reply layouts, including field meanings.
It does not mean complete job, DB, NP or timer behavior.
✅ means target build 0.0.307 evidence. ❓ means guide-only or unchecked.
All unqualified addresses in G4 refer to build 0.0.307.
The guide is build 0.0.348. Its names do not prove target semantics.

### Field notation

Tables give fields in wire order. An offset starts after the command.
`h`, `E` and repeat count `C` use 2 bytes. `i` uses 4 bytes.
`q` uses 8 bytes. `Q` and `c` use 1 byte. `o` uses 16 bytes.
Scalars use little-endian order. Preserve raw bits unless a row gives signed use.
`s` uses a 2-byte UTF-16 code-unit count, then 2 bytes per unit. It sends no NUL.
`b` uses a 2-byte byte count, then that many bytes.
Each `C[...]` has its own 2-byte count. `;` adds no bytes.
Sources: `0x1400E6610`, `0x1400E69A0`, `0x1400DA270`, `0x1400DA540`.

### 1 — HelloReq and replies (full)

| Order / body offset | Field | Code / bytes | Meaning and evidence |
| --- | --- | --- | --- |
| 1 / 0 | cluster_id | h / 2 | ✅ Configured world cluster; `0x140086D50` |
| 2 / 2 | world_id | h / 2 | ✅ Preloaded world; `0x140086D50` |
| 3 / 4 | packet_version | h / 2 | ✅ Must equal 80 in 307; `0x140086D50` |
| 4 / 6 | internal_port | h / 2 | ✅ Peer identity input, not the LobbyDaemon listener port; `0x140086D50 -> 0x14003F920` |

Request literal: `hhhhh`. Request packet size: 12 bytes.
Success command 2 uses `hCQiiiqqChii;;qqqi`.
Its body starts with a count of 2 schedules.

| Schedule field order | Code / bytes | Meaning | Target source |
| --- | --- | --- | --- |
| 1 | Q / 1 | ✅ target_type: 0 daily, 1 whole-schedule | schedule byte +12; `0x1400722B0`, `0x140031CC0`, `0x140096C00` |
| 2 | i / 4 | ✅ daily reset hour | +4; `0x1400722B0`, `0x140001D50` |
| 3 | i / 4 | ✅ reset time-zone bias in seconds | +8; NP minutes multiplied by 60 at `0x1400722B0` |
| 4 | i / 4 | ✅ schedule number / promotion ID | +0; `0x1400722B0` |
| 5 | q / 8 | ✅ schedule start UTC, epoch seconds | +16; `0x1400722B0`, `0x140031CC0` |
| 6 | q / 8 | ✅ schedule end UTC, epoch seconds | +24; same sources |
| 7 | C / 2 | ✅ rule count, at most 6 | rules +32, stride 28; `0x140096C00` |
| 7.1 | h / 2 | ✅ required play minutes | rule +0; `0x1400722B0` log `minutes` |
| 7.2 | i / 4 | ✅ normal reward policy ID | rule +4; `0x1400722B0` logs `normal`, `n-reward` |
| 7.3 | i / 4 | ✅ premium reward policy ID | rule +16; `0x1400722B0` logs `premium`, `p-reward` |

The policy builder maps NP board 6 to target 0 and board 5 to target 1.
It groups rewards by required minutes. It sorts the rules.
The unsent sentinel is `h=0xFFFF,i=0,i=0`.
The old label `period mode` describes reset behavior. The field identity is `target_type`.
The target sends two reward policy IDs per rule. The guide sends four.
Do not use the guide literal for 307.

| Tail order after schedules | Code / bytes | Meaning and target source |
| --- | --- | --- |
| 1 | q / 8 | ✅ random-store schedule number; daemon +2112 |
| 2 | q / 8 | ✅ slot-count initialization schedule number; daemon +2120 |
| 3 | q / 8 | ✅ premium-draw-count initialization schedule number; daemon +2128 |
| 4 | i / 4 | ✅ game-mileage effective duration |

Tail sources: `0x140097EB0 -> 0x1400A4E00`, `0x140086D50`.
❓ The effective-duration unit is not established here.
This unit gap does not change the field's wire type or application identity.
The command-2 packet has `96 + 10 * total_rule_count` bytes.
Failure command 3 contains only `error:h` at body offset 0.
Errors: 1 for shared preallocation checks, 2 for setup/compose failure, 12 for registration failure.
Sources: `0x140086D50`, `0x1400E0800`.

### Shared account-time record (commands 5, 42 and 104)

The record is `Qiqqihcc`, 29 wire bytes. Its memory stride is 32 bytes.
A repeat has a 2-byte count. Outgoing lists contain target 0, then target 1.
The receive decoder rejects a target index above 1.
Do not assume that the receive count must equal 2. The checked decoder uses the sent count.

| Order / record offset | Field | Code / bytes | Meaning and source |
| --- | --- | --- | --- |
| 1 / 0 | target_type | Q / 1 | ✅ 0 daily / NP board 6; 1 whole-schedule / NP board 5; `0x1400320E0`, `0x140077A00` |
| 2 / 1 | schedule_number | i / 4 | ✅ record +0; matched with schedule IDs at `0x1400370D0` |
| 3 / 5 | expire_time_utc | q / 8 | ✅ record +8 is copied and persisted as a timestamp; ❓ expiry name from 348 `0x140032ED0`; target `0x140036EF0`, `0x14009D350` |
| 4 / 13 | reward_request_time | q / 8 | ✅ incoming record +16; reward update stores request time here; outgoing composers send zero, not +16; `0x1400370D0`, `0x140031E80`, `0x140031FD0` |
| 5 / 21 | duration_seconds | i / 4 | ✅ record +4; update compares with 60 * acknowledged minutes; `0x1400370D0` |
| 6 / 25 | acknowledged_play_minutes | h / 2 | ✅ record +26; request play time reaches this field; `0x1400370D0` |
| 7 / 27 | normal_reward_state | c / 1 | ✅ record +24; reward-state bitmask, OR-updated for normal rule slots; `0x1400370D0`, policy `0x1400722B0` |
| 8 / 28 | premium_reward_state | c / 1 | ✅ record +25; reward-state bitmask, OR-updated for premium rule slots; same sources |

Command 42 copies incoming +16 if the schedule is zero or incoming +16 is nonzero.
Otherwise it keeps the old +16. Source: `0x140036EF0`.
❓ Complete reset rules, duplicate target-index policy and all timestamp constraints remain open.

### 5 — EnterWorldReq and replies (partial)

Request literal: `hqqqi`. Body size: 28 bytes. Packet size: 32 bytes.

| Order / offset | Field | Code / bytes | Meaning and target source |
| --- | --- | --- | --- |
| 1 / 0 | game_transaction_id | q / 8 | ✅ Reply correlation; `0x140087D40` |
| 2 / 8 | client_key | q / 8 | ✅ Lobby session key; same source |
| 3 / 16 | token_key | q / 8 | ✅ Type-1 world-entry token; `0x140087D40 -> 0x140093EB0` |
| 4 / 24 | address | i / 4 | ✅ Client address bits used by token validation; same sources; no address value recorded |

Success command 6 uses `hiqqocbbbbssQhhiQCQiqqihcc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiq`.
The initial `i` is fragment metadata, not an application field.
The table below gives every application group in order.
Fields inside each group follow the listed order. Shared records use the tables in G4.

| Order | Fields in order | Code / bytes | Sources and meaning state |
| --- | --- | --- | --- |
| 1 | game_transaction_id, selected_character_id | q / 8 each | ✅ job +96/+104; `0x140083D60` |
| 2 | account_id | o / 16 | ✅ session +48 source; ❓ account-ID name from 348 `0x140085240` |
| 3 | is_mobile | c / 1 | ✅ boolean(session +1970); ❓ name from 348 `0x140085240` |
| 4 | auth_provider_code, external_account_key, external_account_name, external_account_data | b / 2+N each | ✅ length/data +468/+470, +488/+490, +556/+558, +816/+818; ❓ names from 348 `0x140085240` |
| 5 | account_dbfield_string, user_name | s / 2+2N each | ✅ session +64, +232; ❓ names from 348 `0x140085240` |
| 6 | role_bit_2, maximum_character_level, maximum_showcase_value, bot_score, bot_score_ignore | Q / 1; h / 2; h / 2; i / 4; Q / 1 | ✅ source bit/session +362, maxima, +196, +200 at `0x140083D60`; ❓ role meaning, showcase value meaning and ignore name |
| 7 | account_time_records | C / 2 + 29 per record | ✅ `0x140031E80`; shared record above; ❓ expiry-field meaning |
| 8 | eligible characters: character_id, world_id, character_name | C / 2; each q / 8, h / 2, s / 2+2N | ✅ character +0/+24/+30; state 2, unblocked; `0x140083D60` |
| 9 | slot_attributes, character_rights, jumping_character_state, charge_apply_type | i / 4; Q / 1 each | ✅ selected character +148/+144/+112 and mapped charge values 1/2/3; ❓ first three names from 348 `0x140085240` |
| 10 | pc_cafe_code | s / 2+2N | ✅ session +2096 or empty; ❓ name from 348 `0x140085240` |
| 11 | grade summary | C / 2 + 20 per record | ✅ zero or one record; `0x1400528E0`; grade table below |
| 12 | current_grade_revision | i / 4 | ✅ session +2032; `0x140083D60`, comparison at `0x140034F90` |
| 13 | grade records | C / 2 + 29 per record | ✅ `0x1400529B0 -> 0x140051850`; grade table below |
| 14 | content bans: ban_type, start_time, end_time | C / 2; each i / 4, q / 8, q / 8 | ✅ record +0/+8/+16, stride 24, `0x140035B70`; ❓ individual semantic names |
| 15 | closest_grade_end_time | q / 8 | ✅ session +1984; ❓ name from 348 `0x140085240` |
| 16 | account_exp, daily_exp_quota, exp_last_update_time | q / 8, i / 4, q / 8 | ✅ session +432/+440/+448; `0x140083D60`, `0x140037370` |

Failure command 7 uses `hqE`: transaction `q` at 0, error `E` at 8.
Source: `0x1400805A0`.
Token errors: expired 16, other invalid token 15, wrong world 17.
Job init failure uses 4. Registration failure uses 2. Wrong session state uses 6.
Sources: `0x140087D40`, `0x140083BE0`.
The job requires session state `0x30003`. Success installs world presence and state `0x40000`.
Sources: `0x140083BE0`, `0x140083D60`.

### 42 — UpdateAccountDataReq and replies (partial)

Full literal: `hqqqiCQiqqihcc;qiq`. The body is `qqqi C[Qiqqihcc] qiq`.
Source: `0x14008B130`, list decoder `0x1400320E0`.

| Order / offset | Field | Code / bytes | Meaning and evidence |
| --- | --- | --- | --- |
| 1 / 0 | transaction_id | q / 8 | ✅ Reply correlation; `0x14008B130` |
| 2 / 8 | character_id | q / 8 | ✅ World session lookup; same source |
| 3 / 16 | client_key | q / 8 | ✅ Session-key check; same source |
| 4 / 24 | bot_score | i / 4 | ✅ -1 preserves cached score; `0x140036EF0` |
| 5 / 28 | account_time_records | C / 2 + 29N | ✅ Shared record table; ❓ expiry meaning |
| 6 / 30+29N | account_exp | q / 8 | ✅ -1 skips EXP update; `0x140037370` |
| 7 / 38+29N | daily_exp_quota | i / 4 | ✅ EXP quota; `0x140037370` |
| 8 / 42+29N | exp_last_update_time | q / 8 | ✅ EXP update timestamp; `0x140037370` |

Request packet size: `54 + 29N` bytes.
Time-info persistence is feature-gated. It calls `dbo.usp_UpdateGameAccountTimeBasedPlayInfo`.
Source: `0x140036EF0 -> 0x14009D350`.
Reply 43: `transaction_id:q` at 0, 8 bytes. Sender: `0x1400809A0`.
Reply 44: `transaction_id:q` at 0, `error:E` at 8, 10 bytes. Sender: `0x140080A60`.
The time-record expiry name keeps command 42 partial.

### 80 — SecondPasswordGetStatusReq and replies (partial)

| Command | Body fields in order | Bytes per field | Source |
| --- | --- | --- | --- |
| 80 | transaction:q, character_id:q, second_password_type:c | 8, 8, 1 | ✅ `0x14008FC90`, `0x1400706E0` |
| 81 | transaction:q, character_id:q, blocked:Q, auth_failure_limit:i, auth_failure_count:i, blocked_until:q, delete_at:q, action_mask:q | 8, 8, 1, 4, 4, 8, 8, 8 | ✅ order and copies; ❓ six status meanings; `0x140070960 -> 0x140082430` |
| 82 | transaction:q, character_id:q, error:E | 8, 8, 2 | ✅ `0x140082570` |

Body offsets for 81: 0, 8, 16, 17, 21, 25, 33, 41.
Response sources for its last six fields: +96 compared with 1, +100, +104, +112, +120, +128.
The six names come from guide 348 `0x140071CE0`.
❓ The target NP response decoder remains unchecked.
The request sends NP command 1711. Callback task: 63.
NP status 200, or 400 with case-insensitive `Success`, sends 81.
NP unavailable/send failure sends 82 error 7. Null callback sends error 2.
NP error 3333 maps to error 67. Other failures map to 1.
Sources: `0x1400706E0`, `0x140070960`.

### 83 — SecondPasswordIsVerifiedReq and replies (full)

| Order / offset | Field | Code / bytes | Meaning and target source |
| --- | --- | --- | --- |
| 1 / 0 | transaction | q / 8 | ✅ Correlation; `0x140090060` |
| 2 / 8 | character_id | q / 8 | ✅ World session selector; same source |
| 3 / 16 | second_password_type | c / 1 | ✅ job +112, sign-extended into first NP request integer; `0x1400712C0` |
| 4 / 17 | action_selector | Q / 1 | ✅ job +113, sign-extended into second NP request integer; not normalized to boolean; `0x1400712C0` |

Request literal: `hqqcQ`. Body size: 18 bytes. Packet size: 22 bytes.
`action_selector` is an opaque operation selector sent to NP command 2711.
❓ The enum values are not mapped. The guide name is `_actionIndex` at 348 `0x140072640`.
Unknown enum labels do not change the checked operational meaning or byte encoding.
Success 84: `transaction:q,character_id:q`, offsets 0/8, body size 16.
Failure 85: the same fields, then `error:E` at 16, body size 18.
Senders: `0x140082670`, `0x140082760`. Callback: `0x140071550`.
Success has no extra verified flag. Receipt of command 84 is the success indication.
NP success uses the same 200/400-Success test as command 80.
NP errors 3333/3334/3345/3346 map to 68/69/70/71. Default error: 1.
NP unavailable/send failure: 7. Null callback: 2. Callback task: 65.

### 92 — GetGradeListReq and replies (partial)

| Command | Body fields in order | Bytes per field | Source |
| --- | --- | --- | --- |
| 92 | transaction:q, location_hint:Q, character_id:q, requested_revision:i | 8, 1, 8, 4 | ✅ `0x140090E80` |
| 93 | transaction:q, location_hint:Q, character_id:q, current_revision:i, summary:iiiii, grades:C[iiQiqq] | 8, 1, 8, 4, 20, 2+29N | ✅ `0x140034F90`, `0x1400528E0`, `0x140051850` |
| 94 | transaction:q, location_hint:Q, character_id:q, error:E | 8, 1, 8, 2 | ✅ `0x1400829F0` |

Command 93 uses fragment mode 2 and full literal `hiqQqiiiiiiCiiQiqq;`.
Its initial `i` is fragment metadata. Logical prefix offsets: 0, 8, 9, 17.
The summary starts at logical offset 21. The repeat count starts at 41.
Unlike command 6, command 93 sends one summary directly, without a summary count.

| Shared grade group | Field order and widths | Target source | Meaning state |
| --- | --- | --- | --- |
| Summary | level:i/4, score:i/4, minimum_level_score:i/4, maximum_level_score:i/4, maximum_score:i/4 | grade-state +4/+8/+12/+16/+20; `0x1400528E0` | ❓ Names from 348 `0x140054400`; target order checked |
| Record | game_grade_key:i/4, grade_id:i/4, activated:Q/1, grade_count:i/4, effective_from:q/8, effective_to:q/8 | record +0/+4/+8/+12/+16/+24; `0x140051850` | ❓ Complete names; 348 `0x1400544D0` checks list filters; target order checked |

Record offsets: 0, 4, 8, 9, 13, 21. Record size: 29 bytes.
The target sends positive key/ID/count records. Activated records must not have expired.
Source: `0x1400529B0`.
Current revision == requested revision: reply 94 error 89.
Current revision < requested revision: reply 94 error 90.
Composition failure: reply 94 error 2. Source: `0x140034F90`.
❓ Target semantic checks for the guide grade names remain open.

### 100 — DepositGameMileageReq and replies (partial)

Request full literal: `hqQQqqoqqqqqsQccc`.
`L` below is the character-name UTF-16 code-unit count.

| Order / offset | Field | Code / bytes | Meaning and target evidence |
| --- | --- | --- | --- |
| 1 / 0 | transaction | q / 8 | ✅ `0x140091720`, decode destination v99 |
| 2 / 8 | location_hint | Q / 1 | ✅ v107 byte +1440; target log |
| 3 / 9 | deposit_type | Q / 1 | ✅ v107 byte +0; 0 GameMileage, 1 GameCash; other types fail |
| 4 / 10 | request_id | q / 8 | ✅ v107[1]; target log |
| 5 / 18 | character_id | q / 8 | ✅ v97; target log and callback +24 |
| 6 / 26 | user_id | o / 16 | ✅ v107[2..3], copied into job callback data; ❓ exact account-namespace label |
| 7 / 42 | base_points | q / 8 | ✅ v96; reason field `base` |
| 8 / 50 | bonus_points | q / 8 | ✅ v98; reason field `bonus`; deposited amount = base + bonus |
| 9 / 58 | job_parameter_q | q / 8 | ✅ destination v107[6]; ❓ application meaning |
| 10 / 66 | registration_time | q / 8 | ✅ v100, passed to ISO-8601 formatter `0x14002E9B0`; ❓ exact clock contract |
| 11 / 74 | external_key / request_sub_id | q / 8 | ✅ v107[179]; target log `request-sub-id`; reply echoes it |
| 12 / 82 | character_name | s / 2+2L | ✅ v111, destination capacity 33 code units including destination NUL |
| 13 / 84+2L | notification_flag | Q / 1 | ✅ v95; job flag is input == 1 |
| 14 / 85+2L | trailing_c_0 | c / 1 | ✅ v107 byte +1441; ❓ meaning |
| 15 / 86+2L | trailing_c_1 | c / 1 | ✅ v107 byte +1442; ❓ meaning |
| 16 / 87+2L | trailing_c_2 | c / 1 | ✅ v107 byte +1443; ❓ meaning |

All decode destinations were checked in disassembly at `0x140091720`.
Request packet size: `92 + 2L` bytes.

| Reply | Ordered fields | Code / bytes per field | Source |
| --- | --- | --- | --- |
| Common prefix | transaction, location_hint, deposit_type, request_id, external_key, character_id, user_id | q/8, Q/1, Q/1, q/8, q/8, q/8, o/16 | ✅ `0x140083050`, `0x1400831C0`, callback disassembly |
| 101 tail | deposit_id, currency_id, deposited_amount, callback_balance_or_result | q/8, h/2, q/8, i/4 | ✅ sources; ❓ special-result semantics; `0x1400915A0 -> 0x140083050`, NP `0x140048DE0` |
| 102 tail | error, error_info | E/2, i/4 | ✅ `0x1400831C0` |

Common prefix offsets: 0, 8, 9, 10, 18, 26, 34.
101 tail offsets: 50, 58, 60, 68. Body size: 72. Full literal: `hqQQqqqoqhqi`.
102 tail offsets: 50, 52. Body size: 56. Full literal: `hqQQqqqoEi`.
Callback parameter sources: request +0, external key +8, user ID +16,
deposited amount +32, currency ID +40, deposit type +42, location hint +43.
Callback object +16 holds transaction; +24 holds character ID.
Sources: vtable `0x14018A9F0`, `0x140091540`, `0x1400915A0`, `0x140091610`, `0x140091670`.
The NP response has deposit ID at +96 and logged balance at +104.
NP error 4901 also takes the success callback. ❓ Its final integer contract needs a separate check.
Direct errors: disabled 7, invalid type/reason build 4, registration failure 2.
Callback errors: null result 2, NP failure 92 with NP error_info, timeout 91.
Decode failure logs and returns without a failure reply.
Sources: `0x140091720`, `0x140048DE0`, callback disassembly.

### 104 — TakeAccountTimeBasedPlayRewardReq and replies (partial)

| Order / offset | Field | Code / bytes | Meaning and target source |
| --- | --- | --- | --- |
| 1 / 0 | transaction | q / 8 | ✅ `0x140092AF0`, job +96 |
| 2 / 8 | character_id | q / 8 | ✅ World session selector; job +104 |
| 3 / 16 | schedule_number | i / 4 | ✅ job +112; update matches schedule ID |
| 4 / 20 | target_type | Q / 1 | ✅ job +120; 0 -> NP board 6, 1 -> NP board 5 |
| 5 / 21 | acknowledged_play_minutes | h / 2 | ✅ sign-extended into job +116; update uses minutes * 60 |
| 6 / 23 | request_time | q / 8 | ✅ job +128; nonzero value updates time record +16 |

Sources: `0x140092AF0`, `0x140077A00`, `0x1400370D0`.
Request full literal: `hqqiQhq`. Body size: 31. Packet size: 35.
Success command 105 uses `hqqiQhqhCQiqqihcc;Cqi;`. It is not fragmented.

| Reply body order | Fields | Code / bytes | Target source |
| --- | --- | --- | --- |
| 105.1 | Same six request fields | qqiQhq / 31 | ✅ `0x140078B00`, job offsets above |
| 105.2 / offset 31 | delivery_failure_count | h / 2 | ✅ Count of non-delivered reward records; `0x1400781D0` |
| 105.3 / offset 33 | account_time_records | C / 2 + 29N | ✅ `0x140031FD0`; ❓ expiry-field meaning |
| 105.4 / offset 35+29N | rewards: user_promotion_reward_id, reward_policy_id | C / 2; each q / 8, i / 4 | ✅ array +0, policy array +96, count +144; names in `0x1400781D0` log and `0x140078B00` |
| 106.1 | Same six request fields | qqiQhq / 31 | ✅ `0x140083620` |
| 106.2 / offset 31 | error | E / 2 | ✅ `0x140083620` |
| 106.3 / offset 33 | error_info | i / 4 | ✅ Original NP error code, or zero for local failures; `0x1400781D0`, `0x140092AF0` |

105 packet size: `41 + 29N + 12R` bytes. Normal outgoing N is 2.
The target caps local reward arrays at 12 records. Delivery status 3 means delivered.
The callback updates reward state only from delivered records.
Sources: `0x1400781D0 -> 0x1400370D0 -> 0x140078B00`.
No rewards: error 96. Missing session: 5. Job registration failure: 2.
Wrong playing state: 6. Disabled feature: 7.
NP errors 10/42/6723/6725/6726/6727 map to 4/93/95/97/98/99.
Other NP failures map to 94. Null callback maps to 2.
Decode failure logs and returns without reply 106.
❓ The shared expiry-field meaning keeps this handler partial. Its success command and literal are checked.

## G4 — Transport contract for Rust

| Topic | Contract / implementation limit | Evidence |
| --- | --- | --- |
| Connection owner | ✅ GameDaemon initiates. LobbyDaemon listens. LobbyDaemon rejects outbound connect events for this manager. | `0x14009A0B0`, `0x14007F620`, `0x14007F680` |
| Listener identity | ✅ Internal port ID 5; daemon manager +2024; TCP port from config +192. ID 5 is not the TCP port. | `0x14009A0B0 -> 0x1400E16F0` |
| Peer discovery | ✅ LobbyDaemon matches accepted peer identity and Hello internal port to configured GameDaemon data. ❓ The GameDaemon-side LobbyDaemon address option and DNS policy were not checked. | `0x140086D50 -> 0x14003F920`; `gamed-list.config.xml` names below |
| Registry | ✅ Load clusters before worlds. The world must refer to an existing cluster. Hello does not create a world row. | `0x14004D0F0` |
| Hello | ✅ Send command 1, require command 2 before service open. Failure is command 3. Match cluster, world, peer identity and version. | `0x140086D50` |
| Version | ✅ 307 uses 80. The 348 guide uses 83. No version negotiation or fallback is established. | `0x140086D50`; guide Hello `0x140088260` |
| Admission | ✅ Hello registers a world-indexed channel. Command 4 sets world +120 live. Duplicate live/service-open closes the channel. | `0x14007F380`, `0x1400877B0` |
| Normal frame | ✅ `size:u16le, command:u16le, body`. Size includes all these bytes. Buffer TCP input across partial reads. Split combined frames. The buffering steps are Rust requirements, not a live test. | codec `0x1400E6610`, dispatcher `0x14007F920` |
| Fragmented replies | ✅ Commands 6 and 93 use mode 2 and a 4-byte `i` metadata item after the command. Application data follows it. ❓ Metadata bit layout, continuation rules and exact split limits remain open. | `0x140083D60`, `0x140034F90`, `0x1400D9810` |
| Receive commands | ✅ Commands 1..109 only; 51 populated slots. Only 1 and 4 can run without a channel. All target receive fragment flags are clear. | table `0x1401F9DE0`, `0x14007F920` |
| Keep-alive | ❓ No GameDaemon heartbeat command, interval or socket keep-alive policy is established. The NP keep-alive is a different link. Do not send NP command 1002 here. | G4 checked GameDaemon dispatcher and callbacks; no GameDaemon timer proof |
| Transaction timeout | ✅ Channel lifetime is config +248. Send sets deadline = monotonic tick delta + lifetime. Tick expiry removes due transactions and calls virtual slot +40 outside the lock. Units are milliseconds from GetTickCount64. | `0x140086D50`, `0x14007A110`, `0x14007A250`, `0x14007A4E0` |
| Other deadlines | ❓ Hello, socket idle, connect and world-entry job deadline values are not established. Port-setup argument 6 is not proved to be a timeout. | `0x14009A0B0`, `0x1400E16F0` |
| Reconnect | ✅ LobbyDaemon does not dial this peer. A new GameDaemon connection must pass Hello and service-open again. ❓ GameDaemon retry delay/backoff and duplicate-channel race policy remain unchecked. | `0x14007F680`, `0x140086D50`, `0x14007F730` |
| Link close | ✅ Remove channel, clear world live, collect world-indexed sessions, start logout task 118 for each. | `0x14007F730 -> 0x14003B960 -> 0x14003D810` |
| Playing-session cleanup | ✅ Logout init requires state 0x40000 and the matching playing world. It sets state 0x60000, stops play, closes presence and finalizes. | `0x140059780`, `0x1400599F0` |
| Pending world entry | ✅ Enter job requires 0x30003. The checked close-triggered logout init accepts only 0x40000. ❓ This does not prove cancellation of queued entry jobs, pending tokens or not-yet-indexed sessions. | `0x140083BE0`, `0x140059780`, `0x14003D810` |

### Config names only

No config values are recorded.

| File / option name | Type or role | Evidence state |
| --- | --- | --- |
| `bin/lobbyd.config.xml`: `world-port` | TCP port, integer | ✅ Name at line 66; ❓ direct option-name-to-+192 assignment not traced |
| `bin/gamed-list.config.xml`: `gamed-list/gamed` | Peer list | ✅ Tag names only |
| `gamed` attributes `wid`, `ip-address`, `internal-port` | World ID, peer-address text, peer internal port | ✅ Names only; target identity check `0x14003F920`; no values recorded |
| `lobby-job-transaction-life-time` | Transaction lifetime | ✅ Name at line 94; ❓ name-to-+248 assignment not traced |
| `lobby-token-life-time` | Token lifetime | ✅ Name at line 96; ❓ complete field binding and clock contract |
| `stop-play-game-world-timeout` | Stop-play job timeout | ✅ Name at line 100; ❓ not a proved socket or entry timeout |
| `timer-interval`, `timer-max-time` | Tick scheduler settings | ✅ Names; ❓ no GameDaemon heartbeat mapping |
| `np-ping-interval` | NP link setting | ✅ Name; not evidence of GameDaemon keep-alive |

### Rust safety requirements where target behavior is open

These are design requirements, not recovered target behavior.
- Give each TCP connection a generation ID.
- Reject a late entry completion for a closed generation.
- Remove that connection from world admission before asynchronous cleanup.
- Do not replay an old entry request after reconnect.
- Obtain a fresh valid token through the normal lobby path.
- Keep socket deadlines separate from token expiry and transaction expiry.
- Do not invent a heartbeat command or success payload.

❓ A target-compatible pending-entry cancellation rule still needs proof.
The checked playing-session logout path is not that proof.

## G4 — First real link checklist

This is a Rust implementation checklist. No network test ran.
Use one preloaded world and one existing eligible character.
Do not claim real entry until the command-6 payload and fragment metadata gaps are closed.

| Order | Action / minimum commands | Expected result | Error path / evidence |
| --- | --- | --- | --- |
| 1 | Load cluster/world registry and configured peer identity. Start the listener. | World exists but is not live. | Missing cluster/world blocks Hello; `0x14004D0F0`, `0x140086D50` |
| 2 | Accept GameDaemon TCP. Receive 1 `cluster:h,world:h,version:h,internal_port:h`. | Send 2 with two schedules and the checked tail. Register channel. | Send 3 on Hello failure. Version 80 only for 307; `0x140086D50` |
| 3 | Receive 4 `cluster:h,world:h`. | Set world live. No success reply. | Duplicate live closes channel; `0x1400877B0` |
| 4 | Complete normal LobbyGate login, character selection and world-transfer token issue. | Session has selected character, service location, type-1 token and state 0x30003. | Do not fabricate account, token or presence success. See existing gate specs 04/05 and `0x140093EB0`, `0x140083BE0`. ❓ Complete cross-link prerequisites are not re-specified here. |
| 5 | Receive 5 `transaction:q,client_key:q,token_key:q,address:i`. | Validate token and world. Register entry job task 117. | Reply 7: 16 expired, 15 other invalid token, 17 wrong world, 4 init failure, 2 start failure; `0x140087D40` |
| 6 | Find selected character and game-world location. Start play. Install world presence. | Set 0x40000. Send 6 with the full ordered composite. | Reply 7 error 2 on checked play/presence/compose failures; 6 on wrong initial state; `0x140083D60`, `0x140083BE0`. ❓ Remaining payload meanings block certified compatibility. |
| 7 | Match reply transaction and character. Parse every group in 6. | One character is admitted to the world. | An empty invented success is not sufficient. No second GameDaemon acknowledgement is shown in this checked path. |
| 8 | Add operational cleanup: inbound 8/12/15/16; outbound 9/10/13/14 and target-specific 7. | Leave, logout and kick can finish. | 8 init/start can send 7, not 10; kick completion policy remains partial. Sources in the historical world-entry table. |
| 9 | Add persistence updates when GameDaemon sends them: 39 -> 40/41; 42 -> 43/44; 107 -> 108/109. | Cached state changes follow checked update results. | These are not required before the first 6. Do not acknowledge unsupported data updates as persisted. |
| 10 | Test close during Hello, before 5, while 5 is queued, and after 6. Then reconnect. | New Hello and service-open; no stale live admission. | ✅ Playing cleanup path checked. ❓ Pending-entry race and exact retry timing unverified. |

Minimum one-entry command set: receive **1, 4, 5**; send **2, 3, 6, 7**.
This is a bounded fixture minimum, not the full set a real GameDaemon may send.
A practical link must also support the cleanup and conditional update commands above.
Charging, second-password, inventory and grade traffic depend on the peer's enabled features.
❓ Their exact first-entry emission order is not established from LobbyDaemon alone.

## G4 — Boundaries and remaining gaps

| Boundary | Source / state |
| --- | --- |
| Tencent TSS `tss_get_sdk` | ✅ 307 string `0x140175FC8`; out of scope; no internals analysed |
| Tencent TSS `tss_sdk` | ✅ 307 string `0x140175FD8`; out of scope; no internals analysed |
| GameGuard `ggauth64_27.dll`, `csauth3.cfg` | ✅ File-name evidence in the 2020 set; ❓ no target EXE reference address established; no internals analysed |
| BloodCert licence checks | Task exclusion. No reimplementation or licence analysis. ❓ No reference address mapped. |
| `BNSHelper.dll` | Task exclusion. Not mapped or called. ❓ No reference address mapped. |

Remaining format gaps: 5 entry names and shared records; 42 time expiry name;
80 target NP status names; 92 target grade names; 100 q/three-byte request meanings
and special callback result; 104 shared time expiry name.
Transport gaps: fragment metadata, GameDaemon endpoint option, heartbeat, socket deadlines,
retry timing and complete pending-entry cancellation.
Complete DB/NP persistence and kick final-state policy remain outside full-format coverage.
Only the two permitted spec files are outputs. No config values or identity values are included.

## Historical G2/G3 evidence ledger

The remainder preserves prior research. Its old coverage totals and gap labels are not current.
Use the G4 sections and root JSON totals for implementation status.

### Earlier G2 report (historical)

| Part | State | Evidence (307) |
| --- | --- | --- |
| 1. Handler formats | ✅ 36 full; ❓ 15 partial | Table `0x1401F9DE0`; receive/reply checks include `0x140092FD0`, `0x1400773E0` |
| 2. HelloOk / EnterWorldOk | ✅ Ordered groups and source offsets; ❓ complete meanings | `0x140096C00`, `0x140031CC0`, `0x140001D50`, `0x140083D60` |
| 3. Job DB calls | ✅ Listed paths and SQL names; ❓ complete job maps | `0x140059740`, `0x140029800`, `0x14002C810`; `target-db-calls.json` |

**New totals: 36 full / 15 partial**, up from 15 / 36. All 51 receive literals remain checked.
Full covers receive fields and immediate or checked link replies. It does not certify all job or DB behavior.
❓ Partial commands: 1, 5, 30, 42, 50, 69, 72, 75, 80, 83, 89, 92, 97, 100, 104.
❓ Main gaps: schedule rule meanings, composite field names, reply tuples, and command-105 success format.
❓ DB maps remain open for jobs from 30, 33, 38, 80, 83, 89, 97, 100, 104.
❓ Rename-type use, auction field meanings, kick completion policy, and unmapped boundary addresses remain open.
✅ Coverage counts were checked. ❓ JSON parser validation did not run; shell approval is required.
Research stopped before turn 20. Only the two permitted spec files changed.
`STATUS.md` stayed unchanged. IDA closed without a save.
No Rust code, network use, binary patch, or IDB save was done.


## Evidence and scope

✅ means seen in build 0.0.307 code or data.
❓ means guide-only evidence or an unchecked claim.
The 348 name is an identity aid. It does not prove target behavior.
All addresses below refer to 307 unless a column says 348.
`gamed-link.json` contains the same inventory and separate evidence states.
Its G2 maps and current coverage replace the earlier field labels.
Keys with the `baseline_` prefix preserve the earlier pass. They are not current coverage claims.

Full literals include command `h`. Body literals remove only this first `h`.
The body does not include the two-byte packet size. The size includes itself.
Source: dispatcher `0x14007F920`; decoder wrapper `0x1400D9D00`.
✅ Basic target codec: `h/C/E=2`, `i=4`, `q=8`, `Q/c=1`, `o=16` bytes.
Scalar stores use little-endian order on x86-64. The codec copies bits; field signedness needs handler evidence.
✅ `s`: two-byte UTF-16 code-unit count, then UTF-16LE units, without a wire NUL.
The decoder appends a NUL in its destination. It requires capacity of at least count + 1.
✅ `b`: two-byte byte length, then bytes. The decoder rejects a negative signed-16 length.
❓ Full blob destination-capacity behavior and all iterator receive paths remain unchecked.
✅ `C` stores a two-byte repeat count. A matching `;` ends its format. The separator adds no bytes.
Sources: `0x1400E6610`, `0x1400E69A0`, `0x1400DA270`, `0x1400DA540`.
❓ Complete message field meanings and repeated payloads remain partial.
Sources for further codec checks: `0x1400D98D0`, `0x1400E6610`, `0x1400D9D00`, `0x1400E69A0`.


## 1. Link setup

| Fact | State | Source / helper addresses |
| --- | --- | --- |
| Listener uses internal port ID 5, daemon manager +2024, config port field +192. Port ID 5 is not the TCP port value. | ✅ | `0x14009a0b0` /  |
| Registry loads clusters, then worlds. Worlds must fit configured ID range and refer to an existing cluster. Hello attaches to a preloaded world. | ✅ | `0x14004d0f0` / `0x14004b730`, `0x14004b8b0`, `0x14004d080`, `0x14004cfe0`, `0x14004abc0`, `0x14004ac20`, `0x14004c890` |
| Hello checks packet version 80 and configured peer identity. It creates a 48-byte world-indexed TransactionChannel. It does not set the live flag. | ✅ | `0x140086d50` / `0x14003f920`, `0x14004cca0`, `0x14007a110`, `0x14007f380` |
| ServiceOpenNotify checks cluster/world and sets world +120 to 1. Duplicate live closes the world channel. | ✅ | `0x1400877b0` /  |
| Channel add/remove/acquire use a critical section and world-ID-indexed slots. Add/acquire retain references; remove releases the table reference. | ✅ | `0x14007f380` / `0x14007f440`, `0x14007f550` |
| OnAccept returns true. OnConnect rejects outbound connections. | ✅ | `0x14007f620` / `0x14007f680` |
| Close removes channel, clears world live flag and starts logout cleanup for sessions in that world. | ✅ | `0x14007f730` / `0x14003b960`, `0x14003d810`, `0x140059650`, `0x1400bd250`, `0x1400bd5e0` |
| Session lookup selects the per-world character-ID tree under a lock and retains the returned session. | ✅ | `0x14003d710` /  |
| Dispatcher accepts commands 1..109. Table entries are 16 bytes. Flags +8/+9 select channel-required/fragment behavior. Only 1 and 4 do not require a channel. All 51 fragment flags are false. | ✅ | `0x14007f920` /  |
| For channel handlers, dispatcher does not propagate the handler return value. Hello and ServiceOpen return values use the direct session path. | ✅ | `0x14007f920` /  |

### Registry and table layout

| Object | Target layout or action | Evidence |
| --- | --- | --- |
| Registry | Cluster load `0x14009EDD0`, then world load `0x14009F6E0` | ✅ `0x14004D0F0` |
| Registry | World pointer array +72; minimum world ID +80; size +84 | ✅ `0x14004D0F0` |
| World | Cluster ID +18; live flag +120 | ✅ `0x140086D50`, `0x1400877B0` |
| Channel manager | Table +8; minimum ID +16; size +24; count +28; lock +32 | ✅ `0x14007F440`, `0x14007F550` |
| Session table | Per-world character-ID tree with reference retention | ✅ `0x14003D710` |
| Disconnect | Select world sessions; start `LogoutInGameWorldJob`, task 118 | ✅ `0x14003B960`, `0x14003D810`, `0x140059650`, `0x1400BD5E0` |

### Hello and service open

| Message | Full literal | Body | Target source |
| --- | --- | --- | --- |
| 1 HelloReq | `hhhhh` | `hhhh`: cluster, world, version, internal port | ✅ `0x140086D50` |
| Hello success | `hCQiiiqqChii;;qqqi` | `CQiiiqqChii;;qqqi` | ✅ command/literal `0x14017C898`, handler `0x140086D50` |
| 3 HelloFail | `hh` | `h`: error | ✅ `0x1400E0800`, caller `0x140086D50` |
| 4 ServiceOpenNotify | `hhh` | `hh`: cluster, world | ✅ `0x1400877B0` |

Hello checks the configured daemon identity and accepted peer address.
No address value is copied here.
The world and cluster must exist before Hello.
Hello creates the channel. Service open sets the live flag.
The transaction-channel constructor is `0x14007A110`.
The lifetime comes from config field +248. Source: ✅ `0x140086D50`.

The target schedule composer is `0x140096C00`.
Three random-store schedule values follow as `qqq`.
The final `i` is the game-mileage effective duration.
Source: ✅ `0x140086D50`; ❓ complete schedule field map.
The guide literal is `hCQiiiqqChiiii;;qqqi`, not the target literal.

HelloOk command 2 is confirmed in target disassembly.
The first `0x1400D9990` call uses `r8d=1` for one field and `r9d=2` for the command.
The schedule layout is `C[Qiiiqq C[hii]]`, followed by `qqqi`.
The outer count is 2. Each schedule has at most 6 rules.
A rule slot with `h=0xFFFF` and both `i` values zero ends the rules.
Source: ✅ `0x140096C00`; ❓ complete semantic names for schedule fields.

| HelloFail error | Target branch | Evidence |
| --- | --- | --- |
| 1 | Decode or another shared preallocation check fails | ✅ `0x140086D50`, disassembly at `sub_1400E0800` calls |
| 2 | Channel init/accept or response composition fails | ✅ `0x140086D50`, disassembly |
| 12 | Channel manager registration fails | ✅ `0x140086D50`, disassembly |

The dispatcher accepts commands 1..109.
It rejects commands 110 and 113 from the guide.
It rejects empty table slots.
Only commands 1 and 4 can run without a shared channel.
All 51 target handlers have the fragment flag clear.
Source: ✅ `0x14007F920`, table `0x1401F9DE0`.

## 2. All 51 GameDaemon handlers

`RX full / body` gives the checked receive literal, then its body literal.
The name state applies only to the 348 identity.
`S<number>` selects a sender in the next table.
✅ beside an S reference means a direct call or a checked job/helper path.
❓ means that the sender literal is checked, but the completion path is not.
`DB —` means no direct DB call was found. It does not exclude downstream DB work.
`T` means the handler resumes an existing transaction. It does not start a new job.
Job class names use the guide identity. Target allocation and task routes are checked.
Full format rows: **4, 8, 11, 12, 15, 16, 17, 21, 22, 24, 25, 27, 28, 33, 38, 39, 45, 48, 49, 53, 54, 55, 58, 59, 61, 62, 64, 65, 67, 68, 78, 79, 87, 88, 103, 107**.
These rows have complete receive and immediate/checked reply layouts.
All other rows remain partial. Full format coverage does not mean complete handler behavior.
The G2 sections replace older partial field labels and reply-path notes in the inventory.

| Cmd | 307 handler | 348 name / source | RX full / body | Reply sender | Direct DB / job |
| --- | --- | --- | --- | --- | --- |
| 1 | `0x140086d50` | ✅ `HandleLobbyGame_HelloReq`<br>348 `0x140088260` | ✅ `hhhhh` / `hhhh` | ✅ S2; ✅ S3 | DB — |
| 4 | `0x1400877b0` | ❓ `HandleLobbyGame_ServiceOpenNotify`<br>348 `0x140088cd0` | ✅ `hhh` / `hh` | ✅ no direct GameDaemon reply seen | DB — |
| 5 | `0x140087d40` | ❓ `HandleLobbyGame_EnterWorldReq`<br>348 `0x140089270` | ✅ `hqqqi` / `qqqi` | ✅ S6; ✅ S7 | DB —; ✅ start / ❓ EnterGameWorldJob `0x140083a50` task 117 |
| 8 | `0x140088470` | ❓ `HandleLobbyGame_LeaveWorldReq`<br>348 `0x1400899a0` | ✅ `hqqq` / `qqq` | ✅ S9; ✅ S10; ✅ S7 | DB —; ✅ start / ❓ LeaveGameWorldJob `0x1400863d0` task 119 |
| 11 | `0x140088860` | ❓ `HandleLobbyGame_UpdateBotScoreNotify`<br>348 `0x140089d90` | ✅ `hqqi` / `qqi` | ✅ no direct GameDaemon reply seen | DB — |
| 12 | `0x140088b30` | ❓ `HandleLobbyGame_LogoutNotify`<br>348 `0x14008a060` | ✅ `hqq` / `qq` | ✅ no direct GameDaemon reply seen | DB —; ✅ start / ❓ LogoutInGameWorldJob `0x140059650` task 118 |
| 15 | `0x140088ed0` | ❓ `HandleLobbyGame_KickOk`<br>348 `0x14008a400` | ✅ `hq` / `q` | ✅ T; no direct reply | DB —; ✅ T |
| 16 | `0x140089030` | ❓ `HandleLobbyGame_KickFail`<br>348 `0x14008a560` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 17 | `0x140087a10` | ❓ `HandleLobbyGame_NPPresenceKickReq`<br>348 `0x140088f30` | ✅ `hqQQ` / `qQQ` | ❓ S18 | DB — |
| 21 | `0x140089290` | ✅ `HandleLobbyGame_CreateCharacterOk`<br>348 `0x14008a7c0` | ✅ `hqhhb` / `qhhb` | ✅ T; no direct reply | DB —; ✅ T |
| 22 | `0x1400895a0` | ❓ `HandleLobbyGame_CreateCharacterFail`<br>348 `0x14008aad0` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 24 | `0x140089800` | ❓ `HandleLobbyGame_DeleteCharacterOk`<br>348 `0x14008ad30` | ✅ `hqh` / `qh` | ✅ T; no direct reply | DB —; ✅ T |
| 25 | `0x140089a60` | ❓ `HandleLobbyGame_DeleteCharacterFail`<br>348 `0x14008af90` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 27 | `0x140089cc0` | ❓ `HandleLobbyGame_RestoreCharacterOk`<br>348 `0x14008b1f0` | ✅ `hq` / `q` | ✅ T; no direct reply | DB —; ✅ T |
| 28 | `0x140089e20` | ❓ `HandleLobbyGame_RestoreCharacterFail`<br>348 `0x14008b350` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 30 | `0x14008a080` | ❓ `HandleLobbyGame_StartChargeReq`<br>348 `0x14008b5b0` | ✅ `hqqq` / `qqq` | ❓ S31; ✅ S32 | DB —; ✅ start / ❓ StartChargeJob `0x140074650` task 120 |
| 33 | `0x14008a480` | ✅ `HandleLobbyGame_StopChargeReq`<br>348 `0x14008b9b0` | ✅ `hqqqs` / `qqqs` | ❓ S34; ✅ S35 | DB —; ✅ start / ❓ StopChargeJob `0x140076d70` task 121 |
| 38 | `0x14008a930` | ❓ `HandleLobbyGame_AbandonChargeNotify`<br>348 `0x14008be60` | ✅ `hqqs` / `qqs` | ✅ no direct GameDaemon reply seen | DB —; ✅ start / ❓ AbandonChargeJob `0x140004450` task 122 |
| 39 | `0x14008ad20` | ✅ `HandleLobbyGame_UpdateCharacterDataReq`<br>348 `0x14008c250` | ✅ `hqqqhb` / `qqqhb` | ✅ S40; ✅ S41 | DB — |
| 42 | `0x14008b130` | ❓ `HandleLobbyGame_UpdateAccountDataReq`<br>348 `0x14008c660` | ✅ `hqqqiCQiqqihcc;qiq` / `qqqiCQiqqihcc;qiq` | ✅ S43; ✅ S44 | DB — |
| 45 | `0x14008b770` | ❓ `HandleLobbyGame_TryRegisterGuildReq`<br>348 `0x14008d590` | ✅ `hqcs` / `qcs` | ✅ S46; ✅ S47 | ✅ `0x1400a2890` (LobbyDB::TryGuildRegistration; ❓ name) |
| 48 | `0x14008bb80` | ❓ `HandleLobbyGame_FinishRegisterGuildNotify`<br>348 `0x14008d9a0` | ✅ `hqq` / `qq` | ✅ no direct GameDaemon reply seen | ✅ `0x1400a2c20` (LobbyDB::FinishGuildRegistration; ❓ name) |
| 49 | `0x14008bcf0` | ❓ `HandleLobbyGame_CancelRegisterGuildNotify`<br>348 `0x14008db10` | ✅ `hqq` / `qq` | ✅ no direct GameDaemon reply seen | ✅ `0x1400a2df0` (LobbyDB::CancelGuildRegistration; ❓ name) |
| 50 | `0x14008be60` | ✅ `HandleLobbyGame_TryRenameGuildReq`<br>348 `0x14008dc80` | ✅ `hqqcs` / `qqcs` | ✅ S51; ✅ S52 | ✅ `0x1400a3200` (LobbyDB::TryRenameGuild; ❓ name) |
| 53 | `0x14008c1f0` | ❓ `HandleLobbyGame_FinishRenameGuildNotify`<br>348 `0x14008e010` | ✅ `hqqq` / `qqq` | ✅ no direct GameDaemon reply seen | ✅ `0x1400a3570` (LobbyDB::FinishRenameGuild; ❓ name) |
| 54 | `0x14008c400` | ❓ `HandleLobbyGame_CancelRenameGuildNotify`<br>348 `0x14008e220` | ✅ `hq` / `q` | ✅ no direct GameDaemon reply seen | ✅ `0x1400a37d0` (LobbyDB::CancelRenameGuild; ❓ name) |
| 55 | `0x14008c570` | ❓ `HandleLobbyGame_DeleteGuildNotify`<br>348 `0x14008e390` | ✅ `hqq` / `qq` | ✅ no direct GameDaemon reply seen | ✅ `0x1400a2fc0` (LobbyDB::DeleteGuild; ❓ name) |
| 58 | `0x14008c700` | ❓ `HandleLobbyGame_ChangeCharacterAppearanceOk`<br>348 `0x14008e520` | ✅ `hqb` / `qb` | ✅ T; no direct reply | DB —; ✅ T |
| 59 | `0x14008ca00` | ❓ `HandleLobbyGame_ChangeCharacterAppearanceFail`<br>348 `0x14008e820` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 61 | `0x14008cc90` | ❓ `HandleLobbyGame_ChangeCharacterSexOk`<br>348 `0x14008eab0` | ✅ `hqb` / `qb` | ✅ T; no direct reply | DB —; ✅ T |
| 62 | `0x14008cf90` | ❓ `HandleLobbyGame_ChangeCharacterSexFail`<br>348 `0x14008edb0` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 64 | `0x14008d220` | ❓ `HandleLobbyGame_ChangeCharacterRaceOk`<br>348 `0x14008f040` | ✅ `hqb` / `qb` | ✅ T; no direct reply | DB —; ✅ T |
| 65 | `0x14008d520` | ❓ `HandleLobbyGame_ChangeCharacterRaceFail`<br>348 `0x14008f340` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 67 | `0x14008d7b0` | ❓ `HandleLobbyGame_RenameCharacterOk`<br>348 `0x14008f5d0` | ✅ `hqq` / `qq` | ✅ T; no direct reply | DB —; ✅ T |
| 68 | `0x14008da50` | ❓ `HandleLobbyGame_RenameCharacterFail`<br>348 `0x14008f870` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 69 | `0x14008dce0` | ✅ `HandleLobbyGame_ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemReq`<br>348 `0x14008fb00` | ✅ `hqqqqqi` / `qqqqqi` | ❓ S70; ✅ S71 | DB —; ✅ start / ❓ ExtendJumpingCharacterCreationQuotaJob `0x140029700` (register/transit) |
| 72 | `0x14008e1a0` | ❓ `HandleLobbyGame_UpgradeJumpingCharacterStateWithCreationQuotaReq`<br>348 `0x14008ffc0` | ✅ `hqqq` / `qqq` | ❓ S73; ✅ S74 | DB —; ✅ start / ❓ UpgradeJumpingCharacterStateJob `0x14002c780` (register/transit) |
| 75 | `0x14008e5b0` | ❓ `HandleLobbyGame_UpdateJumpingCharacterStateReq`<br>348 `0x1400903d0` | ✅ `hqqq` / `qqq` | ❓ S76; ✅ S77 | DB —; ✅ start / ❓ UpgradeJumpingCharacterStateJob `0x14002c780` (register/transit) |
| 78 | `0x14008e9c0` | ✅ `HandleLobbyGame_SetAppDataNotify`<br>348 `0x1400907e0` | ✅ `hqCss;` / `qCss;` | ✅ no direct GameDaemon reply seen | DB — |
| 79 | `0x14008f150` | ✅ `HandleLobbyGame_FriendMessageNotify`<br>348 `0x140090f70` | ✅ `hqsCss;` / `qsCss;` | ✅ no direct GameDaemon reply seen | DB — |
| 80 | `0x14008fc90` | ❓ `HandleLobbyGame_SecondPasswordGetStatusReq`<br>348 `0x140091ab0` | ✅ `hqqc` / `qqc` | ❓ S81; ✅ S82 | DB —; ✅ start / ❓ SecondPasswordGetStatusJob (register/transit) |
| 83 | `0x140090060` | ✅ `HandleLobbyGame_SecondPasswordIsVerifiedReq`<br>348 `0x140091e80` | ✅ `hqqcQ` / `qqcQ` | ❓ S84; ✅ S85 | DB —; ✅ start / ❓ SecondPasswordIsVerifiedJob (register/transit) |
| 87 | `0x140090460` | ❓ `HandleLobbyGame_AuctionEscrowOk`<br>348 `0x140092280` | ✅ `hq` / `q` | ✅ T; no direct reply | DB —; ✅ T |
| 88 | `0x1400905e0` | ❓ `HandleLobbyGame_AuctionEscrowFail`<br>348 `0x140092400` | ✅ `hqE` / `qE` | ✅ T; no direct reply | DB —; ✅ T |
| 89 | `0x140090880` | ✅ `HandleLobbyGame_AuctionBidReq`<br>348 `0x1400926a0` | ✅ `hqohq` / `qohq` | ❓ S90; ✅ S91 | DB —; ✅ start / ❓ AuctionBidJob (register/transit) |
| 92 | `0x140090e80` | ✅ `HandleLobbyGame_GetGradeListReq`<br>348 `0x140092ca0` | ✅ `hqQqi` / `qQqi` | ✅ S93; ✅ S94 | DB — |
| 97 | `0x1400910e0` | ✅ `HandleLobbyGame_ActivateGradeReq`<br>348 `0x140092f00` | ✅ `hqqQi` / `qqQi` | ❓ S98; ✅ S99 | DB —; ✅ start / ❓ ActivateGradeJob (register/transit) |
| 100 | `0x140091720` | ✅ `HandleLobbyGame_DepositGameMileageReq`<br>348 `0x140093540` | ✅ `hqQQqqoqqqqqsQccc` / `qQQqqoqqqqqsQccc` | ❓ S101; ✅ S102 | DB —; ✅ start / ❓ DepositMileageJob `0x140047f40` (register/transit) |
| 103 | `0x140092960` | ❓ `HandleLobbyGame_BoastEventInWorldNotify`<br>348 `0x140094780` | ✅ `hhsh` / `hsh` | ✅ no direct GameDaemon reply seen | DB — |
| 104 | `0x140092af0` | ✅ `HandleLobbyGame_TakeAccountTimeBasedPlayRewardReq`<br>348 `0x140094910` | ✅ `hqqiQhq` / `qqiQhq` | ❓ S105; ✅ S106 | DB —; ✅ start / ❓ TakeAccountTimeBasedPlayRewardJob (register/transit) |
| 107 | `0x140092fd0` | ✅ `HandleLobbyGame_UpdateAccountExpReq`<br>348 `0x140094df0` | ✅ `hqqqqiq` / `qqqqiq` | ✅ S108; ✅ S109 | DB — |

### Sender formats

Each row gives a target command and literal. The role name comes from the guide or the checked caller.
❓ Role identity alone does not prove a completion path.
All rows below have checked command/literal evidence at the listed target address.
Complete semantic layouts remain partial except S7, S9, S10, S18, S34, S35, S40, S41, S46, S47, S108, and S109.
The G2 section gives the newly checked sender fields. S51/S52 remain partial because rename-type use is not checked in disassembly.

| Sender | Role | 307 source | Full format | Body format |
| --- | --- | --- | --- | --- |
| S3 ✅ | HelloFail | `0x1400e0800` | `hh` | `h` |
| S6 ✅ | EnterWorldOk | `0x140083d60` | `hiqqocbbbbssQhhiQCQiqqihcc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiq` | `iqqocbbbbssQhhiQCQiqqihcc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiq` |
| S7 ✅ | EnterWorldFail | `0x1400805a0` | `hqE` | `qE` |
| S9 ✅ | LeaveWorldOk | `0x140080670` | `hqq` | `qq` |
| S10 ✅ | LeaveWorldFail | `0x140080740` | `hqE` | `qE` |
| S13 ✅ | CloseSessionNotify | `0x140080360` | `hq` | `q` |
| S14 ✅ | KickReq | `0x140080420` | `hqqqqQ` | `qqqqQ` |
| S18 ✅ | NPPresenceKickReply | `0x140087a10` | `hqQQ` | `qQQ` |
| S19 ✅ | StopPlayNotify | `0x14007fcf0` | `hqQ` | `qQ` |
| S20 ✅ | CreateCharacterReq | `0x14007fdc0` | `hqqobisbQh` | `qqobisbQh` |
| S23 ✅ | DeleteCharacterReq | `0x14007ffd0` | `hqqbi` | `qqbi` |
| S26 ✅ | RestoreCharacterReq | `0x140080150` | `hqq` | `qq` |
| S29 ✅ | DeleteCharacterCompleteNotify | `0x1400802a0` | `hq` | `q` |
| S31 ✅ | StartChargeOk | `0x140081150` | `hqsiQih` | `qsiQih` |
| S32 ✅ | StartChargeFail | `0x140081250` | `hqE` | `qE` |
| S34 ✅ | StopChargeOk | `0x140081320` | `hq` | `q` |
| S35 ✅ | StopChargeFail | `0x1400813e0` | `hqE` | `qE` |
| S36 ✅ | UpdateChargeRemainTimeNotify | `0x1400814b0` | `hqihi` | `qihi` |
| S37 ✅ | ApplyNewChargeItemNotify | `0x140081590` | `hqsiih` | `qsiih` |
| S40 ✅ | UpdateCharacterDataOk | `0x140080810` | `hq` | `q` |
| S41 ✅ | UpdateCharacterDataFail | `0x1400808d0` | `hqE` | `qE` |
| S43 ✅ | UpdateAccountDataOk | `0x1400809a0` | `hq` | `q` |
| S44 ✅ | UpdateAccountDataFail | `0x140080a60` | `hqE` | `qE` |
| S46 ✅ | TryRegisterGuildOk | `0x140080b30` | `hqq` | `qq` |
| S47 ✅ | TryRegisterGuildFail | `0x140080c00` | `hqE` | `qE` |
| S51 ✅ | TryRenameGuildOk | `0x140080cd0` | `hqqcq` | `qqcq` |
| S52 ✅ | TryRenameGuildFail | `0x140080dc0` | `hqqcE` | `qqcE` |
| S56 ✅ | AccountInventoryNotify | `0x140080ea0` | `hiqQCqqi;Qq` | `iqQCqqi;Qq` |
| S57 ✅ | ChangeCharacterAppearanceReq | `0x140081680` | `hqqqbb` | `qqqbb` |
| S60 ✅ | ChangeCharacterSexReq | `0x140081870` | `hqqqcbb` | `qqqcbb` |
| S63 ✅ | ChangeCharacterRaceReq | `0x140081a90` | `hqqqccbb` | `qqqccbb` |
| S66 ✅ | RenameCharacterReq | `0x140081cc0` | `hqqqss` | `qqqss` |
| S70 ✅ | ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemOk | `0x140081e80` | `hqi` | `qi` |
| S71 ✅ | ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemFail | `0x140081f70` | `hqE` | `qE` |
| S73 ✅ | UpgradeJumpingCharacterStateWithCreationQuotaOk | `0x140082060` | `hqQi` | `qQi` |
| S74 ✅ | UpgradeJumpingCharacterStateWithCreationQuotaFail | `0x140082160` | `hqE` | `qE` |
| S76 ✅ | UpdateJumpingCharacterStateOk | `0x140082250` | `hqQ` | `qQ` |
| S77 ✅ | UpdateJumpingCharacterStateFail | `0x140082340` | `hqE` | `qE` |
| S81 ✅ | SecondPasswordGetStatusOk | `0x140082430` | `hqqQiiqqq` | `qqQiiqqq` |
| S82 ✅ | SecondPasswordGetStatusFail | `0x140082570` | `hqqE` | `qqE` |
| S84 ✅ | SecondPasswordIsVerifiedOk | `0x140082670` | `hqq` | `qq` |
| S85 ✅ | SecondPasswordIsVerifiedFail | `0x140082760` | `hqqE` | `qqE` |
| S86 ✅ | AuctionEscrowReq | `0x140082860` | `hqqqoq` | `qqqoq` |
| S90 ✅ | AuctionBidOk | `0x140083310` | `hqo` | `qo` |
| S91 ✅ | AuctionBidFail | `0x140083490` | `hqoE` | `qoE` |
| S93 ✅ | GetGradeListOk | `0x140034f90` | `hiqQqiiiiiiCiiQiqq;` | `iqQqiiiiiiCiiQiqq;` |
| S94 ✅ | GetGradeListFail | `0x1400829f0` | `hqQqE` | `qQqE` |
| S95 ✅ | ChangedGradeNotify | `0x140082d60` | `hqiiiQiqqQ` | `qiiiQiqqQ` |
| S96 ✅ | ChangedGradeLevelNotify | `0x140082f40` | `hqiiiiii` | `qiiiiii` |
| S98 ✅ | ActivateGradeOk | `0x140082b00` | `hqqiiqq` | `qqiiqq` |
| S99 ✅ | ActivateGradeFail | `0x140082c30` | `hqqQiE` | `qqQiE` |
| S101 ✅ | DepositGameMileageOk | `0x140083050` | `hqQQqqqoqhqi` | `qQQqqqoqhqi` |
| S102 ✅ | DepositGameMileageFail | `0x1400831c0` | `hqQQqqqoEi` | `qQQqqqoEi` |
| S106 ✅ | TakeAccountTimeBasedPlayRewardFail | `0x140083620` | `hqqiQhqEi` | `qqiQhqEi` |
| S108 ✅ | UpdateAccountExpOk | `0x140083770` | `hqq` | `qq` |
| S109 ✅ | UpdateAccountExpFail | `0x140083840` | `hqqE` | `qqE` |
| S2 ✅ | HelloOk | `0x140086D50` | `hCQiiiqqChii;;qqqi` | `CQiiiqqChii;;qqqi` |
| S105 ❓ | TakeAccountTimeBasedPlayRewardOk | Handler `0x140092AF0` | ❓ not recovered | ❓ not recovered |

### Receive field order

The next table records semantic field names.
✅ rows have focused target checks.
❓ rows retain guide meanings or unchecked result names.
Do not treat repeat separators as ordinary scalar fields.

| Cmd | Body field order | State and source |
| --- | --- | --- |
| 1 | `cluster_id:h, world_id:h, packet_version:h, internal_port:h` | ✅; `0x140086d50` |
| 4 | `cluster_id:h, world_id:h` | ✅ target decoder and field use; `0x1400877b0` |
| 5 | `game_transaction_id:q, client_key:q, token_key:q, address:i` | ✅; `0x140087d40` |
| 8 | `game_transaction_id:q, character_id:q, client_key:q` | ✅ target decoder and field use; `0x140088470` |
| 11 | `character_id:q, client_key:q, bot_score:i` | ❓ semantic names need separate target checks; `0x140088860` |
| 12 | `character_id:q, client_key:q` | ❓ semantic names need separate target checks; `0x140088b30` |
| 15 | `transaction_id:q` | ✅ target decoder and field use; `0x140088ed0` |
| 16 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x140089030` |
| 17 | `character_id:q, flag_1:Q, flag_2:Q` | ❓ semantic names need separate target checks; `0x140087a10` |
| 21 | `transaction_id:q, result_1:h, result_2:h, showcase:b` | ❓ semantic names need separate target checks; `0x140089290` |
| 22 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x1400895a0` |
| 24 | `transaction_id:q, result:h` | ❓ semantic names need separate target checks; `0x140089800` |
| 25 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x140089a60` |
| 27 | `transaction_id:q` | ✅ target decoder and field use; `0x140089cc0` |
| 28 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x140089e20` |
| 30 | `game_transaction_id:q, character_id:q, client_key:q` | ❓ semantic names need separate target checks; `0x14008a080` |
| 33 | `game_transaction_id:q, character_id:q, client_key:q, charge_key:s` | ❓ semantic names need separate target checks; `0x14008a480` |
| 38 | `character_id:q, client_key:q, charge_key:s` | ❓ semantic names need separate target checks; `0x14008a930` |
| 39 | `game_transaction_id:q, character_id:q, client_key:q, level:h, showcase:b` | ✅ target decoder and field use; `0x14008ad20` |
| 45 | `transaction_id:q, guild_type:c, guild_name:s` | ❓ semantic names need separate target checks; `0x14008b770` |
| 48 | `transaction_id:q, guild_id:q` | ❓ semantic names need separate target checks; `0x14008bb80` |
| 49 | `transaction_id:q, guild_id:q` | ❓ semantic names need separate target checks; `0x14008bcf0` |
| 50 | `transaction_id:q, guild_id:q, rename_type:c, new_name:s` | ❓ semantic names need separate target checks; `0x14008be60` |
| 53 | `transaction_id:q, guild_id:q, rename_key:q` | ❓ semantic names need separate target checks; `0x14008c1f0` |
| 54 | `rename_key:q` | ❓ semantic names need separate target checks; `0x14008c400` |
| 55 | `transaction_id:q, guild_id:q` | ❓ semantic names need separate target checks; `0x14008c570` |
| 58 | `transaction_id:q, showcase:b` | ❓ semantic names need separate target checks; `0x14008c700` |
| 59 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x14008ca00` |
| 61 | `transaction_id:q, showcase:b` | ❓ semantic names need separate target checks; `0x14008cc90` |
| 62 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x14008cf90` |
| 64 | `transaction_id:q, showcase:b` | ❓ semantic names need separate target checks; `0x14008d220` |
| 65 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x14008d520` |
| 67 | `transaction_id:q, result:q` | ❓ semantic names need separate target checks; `0x14008d7b0` |
| 68 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x14008da50` |
| 80 | `game_transaction_id:q, character_id:q, type:c` | ❓ semantic names need separate target checks; `0x14008fc90` |
| 83 | `game_transaction_id:q, character_id:q, type:c, flag:Q` | ❓ semantic names need separate target checks; `0x140090060` |
| 87 | `transaction_id:q` | ✅ target decoder and field use; `0x140090460` |
| 88 | `transaction_id:q, error:E` | ✅ target decoder and field use; `0x1400905e0` |
| 92 | `game_transaction_id:q, location_hint:Q, character_id:q, requested_revision:i` | ❓ semantic names need separate target checks; `0x140090e80` |
| 97 | `game_transaction_id:q, character_id:q, key_type:Q, key:i` | ❓ semantic names need separate target checks; `0x1400910e0` |
| 103 | `cluster_id:h, character_name:s, event_data:h` | ❓ semantic names need separate target checks; `0x140092960` |


### Full-format byte layouts

All offsets below start at the body, after the command.
All scalar fields use little-endian order.
The packet has a two-byte size, then a two-byte command.
The packet size includes these four bytes.
Sources: ✅ `0x14007F920`, `0x1400E6610`, `0x1400E69A0`.

| Commands | Body layout | Packet bytes | Handler sources |
| --- | --- | --- | --- |
| 4 | cluster `h` at 0; world `h` at 2 | 8 | `0x1400877B0` |
| 8 | transaction `q` at 0; character `q` at 8; client key `q` at 16 | 28 | `0x140088470` |
| 15, 27, 87 | transaction `q` at 0 | 12 | `0x140088ED0`, `0x140089CC0`, `0x140090460` |
| 16, 22, 25, 28, 59, 62, 65, 68, 88 | transaction `q` at 0; error `E` at 8 | 14 | Respective 307 handler rows above |
| 39 | transaction `q` at 0; character `q` at 8; client key `q` at 16; level `h` at 24; blob length `h` at 26; showcase bytes at 28 | 32 + blob length | `0x14008AD20` |

Command 4 has no immediate reply.
The transaction-result handlers resume an existing job. They have no immediate link reply.
Source: ✅ the respective handler rows and `0x1400BD660` / `0x1400BD990`.

| Reply | Body layout | Packet bytes | Source |
| --- | --- | --- | --- |
| 9 | transaction `q` at 0; new lobby token key `q` at 8 | 20 | ✅ `0x140080670`, `0x140086980` |
| 7 / 10 | transaction `q` at 0; error `E` at 8 | 14 | ✅ `0x1400805A0`, `0x140080740` |
| 40 | transaction `q` at 0 | 12 | ✅ `0x140080810` |
| 41 | transaction `q` at 0; error `E` at 8 | 14 | ✅ `0x1400808D0` |

The 512-byte showcase bound is a target decode-buffer bound.
It is not a general protocol maximum for all blobs.
Source: ✅ `0x14008AD20`.

### Handler details and indirect work

| Cmd | Checked action or remaining gap | Source |
| --- | --- | --- |
| 1 | ✅ Checks packet version 80, configured peer identity, preloaded world, matching cluster and live flag. Creates channel; does not set live. | `0x140086d50` |
| 4 | ✅ Sets world +120 to 1. No reply. Duplicate live closes the channel and fails. | `0x1400877b0` |
| 5 | ✅ Verifies type-1 entry token, client key, address, expiry and world. Starts task 117. Errors: expired 16; other token failure 15; world mismatch 17; init failure 4; start failure 2. | `0x140087d40` |
| 8 | ✅ Target disassembly compares decoded client key with session +32. Init/start failure uses command 7, not command 10. Job init errors: wrong world 22, wrong playing character 23, missing character 21, wrong state 6. Job source: `0x140086530`. | `0x140088470` |
| 11 | ✅ Calls session helper 0x1400367d0. | `0x140088860` |
| 12 | ✅ Starts task 118 after session lookup. | `0x140088b30` |
| 17 | ✅ Calls NP helper 0x1400cc190 and sends inline command 18 with hqQQ. | `0x140087a10` |
| 39 | ✅ Showcase decode capacity 512. Validates session/client key. Calls 0x140036bb0. Missing session/key mismatch: 5. Update failure: 45 when lobby error is 6, else 2. | `0x14008ad20` |
| 42 | ✅ Calls list decoder 0x1400320e0 and session updaters 0x140036ef0, 0x140037370. | `0x14008b130` |
| 45 | ✅ Validates name with 0x14006ac30 before direct DB operation. | `0x14008b770` |
| 50 | ✅ Validates name with 0x14006ac30 before direct DB operation. | `0x14008be60` |
| 78 | ✅ Forwards through NP helper 0x140002c30; G2 checks repeated key/value strings. | `0x14008e9c0` |
| 79 | ✅ Calls 0x140036d70 and NP friend helper 0x1400cc1e0; G2 checks message name and repeated key/value strings. | `0x14008f150` |
| 92 | ✅ Success compose helper is 0x140034f90, command 93. | `0x140090e80` |
| 100 | ✅ Complex mileage tuple remains ❓ partial. Helpers include 0x1400600e0, 0x1400128a0 and 0x14002e9b0. | `0x140091720` |
| 103 | ✅ Calls helper 0x14002e090. | `0x140092960` |
| 104 | ✅ Success sender/format remains ❓ unchecked. | `0x140092af0` |
| 107 | ✅ Calls session updater 0x140037370. | `0x140092fd0` |

The character-data DB path is checked:
`39 / 0x14008AD20 -> 0x140036BB0 -> 0x1400A21C0`.
`0x1400A21C0` is `LobbyDB::UpdateCharacterData` in the existing exact name map.
The updater changes cached level/showcase only after DB success.
The handler then sends command 40. It sends command 41 on failure.
Sources: ✅ `0x140036BB0`, `0x14008AD20`.

| Direct DB operation | Handler source | DB source |
| --- | --- | --- |
| ❓ TryRegisterGuild identity | ✅ 45 `0x14008B770` | ✅ call `0x1400A2890` |
| ❓ FinishGuildRegistration identity | ✅ 48 `0x14008BB80` | ✅ call `0x1400A2C20` |
| ❓ CancelGuildRegistration identity | ✅ 49 `0x14008BCF0` | ✅ call `0x1400A2DF0` |
| ❓ TryRenameGuild identity | ✅ 50 `0x14008BE60` | ✅ call `0x1400A3200` |
| ❓ FinishRenameGuild identity | ✅ 53 `0x14008C1F0` | ✅ call `0x1400A3570` |
| ❓ CancelRenameGuild identity | ✅ 54 `0x14008C400` | ✅ call `0x1400A37D0` |
| ❓ DeleteGuild identity | ✅ 55 `0x14008C570` | ✅ call `0x1400A2FC0` |

✅ More indirect DB paths are checked below. The names retain guide identity status.

| Handler | Checked call chain | Guide operation identity |
| --- | --- | --- |
| 5 | `0x140087D40 -> 0x140083D60 -> 0x1400317F0 -> 0x1400A1D00` | ❓ `LobbyDB::UpdateLastPlayStart` |
| 8 | `0x140088470 -> 0x140086980 -> 0x140031930 -> 0x1400A1FA0` | ❓ `LobbyDB::UpdateLastPlayEnd` |
| 42 | `0x14008B130 -> 0x140036EF0 -> 0x14009D350` | ❓ `LobbyDB::UpdateGameAccountTimeBasedPlayInfo` |
| 42 / 107 | `0x140036EF0` is separate from `0x140037370 -> 0x1400A4AF0` | ❓ Last callee lies in the DB region; SQL operation not checked |

❓ Other job and session-helper DB paths are not complete.
The JSON separates direct DB calls from downstream DB calls.
It does not claim that an empty list means no persistence.
Transaction completion helpers include `0x1400BD660`, `0x1400BD990`, `0x1400BCC80`, and `0x1400BD890`.
Sources: ✅ target reply handlers in the 51-row table.

## 3. World-entry messages and order

This is a path order, not one mandatory sequence for all sessions.
Character updates can occur during play.
Kick, logout, and link close are alternate exit paths.

| Step | Direction | Message / action | State and source |
| --- | --- | --- | --- |
| 1 | GameDaemon -> LobbyDaemon | 1 HelloReq. Use preconfigured cluster/world and packet version 80. | ✅; `0x140086d50` |
| 2 | LobbyDaemon -> GameDaemon | 2 HelloOk. Compose schedule via 0x140096c00, then three random-store schedule q values and mileage effective-duration i. | ✅ command/literal/schedule structure; ❓ schedule meanings; `0x140086d50` |
| 3 | GameDaemon -> LobbyDaemon | 4 ServiceOpenNotify. Marks the preloaded world live. No reply. | ✅; `0x1400877b0` |
| 4 | GameDaemon -> LobbyDaemon | 5 EnterWorldReq. Verify entry token through 0x140093eb0. Initialize 0x140083a50; start task 117. | ✅; `0x140087d40` |
| 5 | LobbyDaemon internal | EnterGameWorldJob. Find character and charge/presence state. Call play-start helper 0x1400317f0, clear old presence via 0x140034560, then install game-world presence via 0x140034480. Compose success only after these checks. | ✅; `0x140083d60` |
| 6 | LobbyDaemon -> GameDaemon | 6 EnterWorldOk or 7 EnterWorldFail. Success has an `i` fragment header after command `h`, then game transaction and character IDs `qq`. The G2 section expands the groups. Failure uses hqE at 0x1400805a0. | ✅ command/literal and group sources; ❓ full semantic map; `0x140083d60` |
| 7 | GameDaemon -> LobbyDaemon | 39 UpdateCharacterDataReq. Optional during play. DB 0x1400a21c0 runs through updater 0x140036bb0 before 40 hq; failure is 41 hqE. | ✅; `0x14008ad20` |
| 8 | GameDaemon -> LobbyDaemon | 8 LeaveWorldReq. Check world/character/client key, initialize 0x1400863d0; start task 119. | ✅; `0x140088470` |
| 9 | LobbyDaemon -> GameDaemon | 9 LeaveWorldOk or failure. Call play-stop helper 0x140031930. Construct token via 0x1400936a0 and issue via 0x140093d20. Store new token key from token +48, set session state 0x50001, then send transaction ID and token key. Token issue failure sends 10 hqE and cleans up session. Handler init/start failure instead sends 7 hqE. | ✅; `0x140086980` |
| 10 | LobbyDaemon -> GameDaemon -> LobbyDaemon | 14 KickReq -> 15 KickOk or 16 KickFail. Send through 0x140080420 and resume the existing transaction via inbound handlers. Completion paths 0x140085dd0 / 0x140085f10; timeout 0x140086070. Exact post-kick policy remains unchecked. | ✅; `0x140085b00` |
| 11 | GameDaemon -> LobbyDaemon | 12 LogoutNotify. Starts task 118. Link close also starts logout jobs at 0x14003b960. This is an alternate exit path, not a required step after LeaveWorldOk. | ✅; `0x140088b30` |

### First GameDaemon stub

This list is a design recommendation. No live test was done.

| Need | Stub action | Evidence |
| --- | --- | --- |
| Required | Send 1, then 4, with configured cluster/world and version 80 | ✅ `0x140086D50`, `0x1400877B0` |
| Required | Send 5 with a real type-1 entry token, client key and address | ✅ `0x140087D40`, `0x140093EB0` |
| Required | Read 6/7 and match the game transaction | ✅ `0x140083D60`, `0x1400805A0`; ❓ full success payload |
| Required | Send 8; read 9/10 and the target-specific command-7 failure | ✅ `0x140088470`, `0x140086980` |
| Required | Read 14; send 15 or 16 with the outstanding transaction ID | ✅ `0x140080420`, `0x140088ED0`, `0x140089030` |
| Required | Send 12 for the logout path; accept command 13 | ✅ `0x140088B30`, `0x140080360`; ❓ complete close-session policy |
| Required for persistence path | Send 39; read 40/41 | ✅ `0x14008AD20`, `0x140036BB0` |
| Optional | Send 11 for bot-score updates | ✅ `0x140088860` |
| Fixture-dependent | Implement character create/delete/restore request replies, or use an existing eligible character | ❓ design choice; target senders `0x14007FDC0`, `0x14007FFD0`, `0x140080150` |
| Blocker | Expand HelloOk schedule semantic fields | ❓ schedule meanings; ✅ command 2 at `0x140086D50`, schedule structure at `0x140096C00` |
| Blocker | Expand EnterWorldOk repeated payload and codec | ❓ `0x140083D60` |

The target compares the decoded LeaveWorld client key with session +32.
The zero shown in one pseudocode log is not a wire rule.
Source: ✅ `0x140088470`, disassembly near the `0x14003D710` call and `cmp [r12+20h], rax`.

## G2 — Receive layout checks

✅ Build 0.0.307. These checks replace the older ❓ receive-field rows for the commands below.
The order is the wire order. Each `s` has a two-byte code-unit count and UTF-16LE data.
Each `b` has a two-byte byte count and raw data. A `C[...]` starts with a two-byte count.
These are field labels, not captured field values.

| Cmd | Checked body fields | Immediate link result | 307 source |
| --- | --- | --- | --- |
| 11 | character_id:q, client_key:q, bot_score:i | None | ✅ `0x140088860` |
| 12 | character_id:q, client_key:q | None; starts logout job | ✅ `0x140088B30` |
| 17 | character_id:q, target_app:Q, force:Q | 18 echoes all three fields after NP send success | ✅ `0x140087A10` |
| 21 | transaction_id:q, character_job:h, level:h, showcase:b | Resumes transaction; no immediate reply | ✅ `0x140089290` |
| 24 | transaction_id:q, level:h | Resumes transaction; no immediate reply | ✅ `0x140089800` |
| 30 | game_transaction_id:q, character_id:q, client_key:q | 31 / 32; completion check continues | ✅ `0x14008A080` |
| 33 | game_transaction_id:q, character_id:q, client_key:q, charge_key:s | 34: transaction:q; 35: transaction:q,error:E. Success path checked at `0x1400773E0`. | ✅ `0x14008A480`, `0x140081320` |
| 38 | character_id:q, client_key:q, charge_key:s | None | ✅ `0x14008A930` |
| 45 | game_transaction_id:q, guild_type:c, guild_name:s | 46: transaction:q, allocated_guild_id:q; 47: transaction:q, error:E | ✅ `0x14008B770`, `0x140080B30`, `0x140080C00` |
| 48 | game_transaction_id:q, guild_id:q | None | ✅ `0x14008BB80` |
| 49 | game_transaction_id:q, guild_id:q | None | ✅ `0x14008BCF0` |
| 50 | game_transaction_id:q, guild_id:q, rename_type:c, new_name:s | 51: transaction:q, guild_id:q, type:c, rename_key:q; 52 replaces final q with error:E. ❓ Pseudocode shows type 0; disassembly must confirm its use. | ✅ structure `0x14008BE60`, `0x140080CD0`, `0x140080DC0` |
| 53 | game_transaction_id:q, guild_id:q, rename_key:q | None | ✅ `0x14008C1F0` |
| 54 | rename_key:q | None | ✅ `0x14008C400` |
| 55 | game_transaction_id:q, guild_id:q | None | ✅ `0x14008C570` |
| 58 / 61 / 64 | transaction_id:q, showcase:b | Resumes transaction; no immediate reply | ✅ `0x14008C700`, `0x14008CC90`, `0x14008D220` |
| 67 | transaction_id:q, character_id:q | Resumes transaction; no immediate reply | ✅ `0x14008D7B0` |
| 72 / 75 | game_transaction_id:q, character_id:q, client_key:q | 73/74 or 76/77; completion check continues | ✅ `0x14008E1A0`, `0x14008E5B0` |
| 78 | character_id:q, app_data:C[key:s,value:s] | None | ✅ `0x14008E9C0` |
| 79 | character_id:q, message_name:s, arguments:C[key:s,value:s] | None | ✅ `0x14008F150` |
| 80 | game_transaction_id:q, character_id:q, second_password_type:c | 81 / 82; completion check continues | ✅ `0x14008FC90` |
| 83 | game_transaction_id:q, character_id:q, second_password_type:c, flag:Q | 84 / 85; flag meaning remains ❓ | ✅ `0x140090060` |
| 89 | character_id:q, auction_id:o, world_id:h, bid_amount:q | 90 / 91; no transaction ID field. ❓ Final two semantic labels need a variadic-argument check. | ✅ structure `0x140090880` |
| 92 | game_transaction_id:q, location_hint:Q, character_id:q, requested_revision:i | 93 / 94 | ✅ `0x140090E80` |
| 97 | game_transaction_id:q, character_id:q, key_type:Q, key:i | 98 / 99 | ✅ `0x1400910E0` |
| 103 | world_cluster_id:h, character_name:s, boast_event_data_id:h | None | ✅ `0x140092960` |
| 107 | game_transaction_id:q, character_id:q, client_key:q, account_exp:q, quota:i, last_update_time:q | 108: transaction:q,character:q; 109: transaction:q,character:q,error:E | ✅ `0x140092FD0`, `0x140083770`, `0x140083840`, `0x140037370` |

✅ Command 107 uses two three-field iterator reads. Disassembly confirms character lookup and reply arguments.
The zero character ID in incomplete pseudocode is not a wire rule. Source: `0x140092FD0`.
❓ Command 50 stays partial. Its decoded `c` and apparent constant-zero DB/reply argument need a disassembly check.

✅ Command 21 uses a 512-byte showcase destination. Source: `0x140089290`.
✅ Commands 58/61/64 use the blob-view iterator. It rejects negative lengths and truncated data.
It returns the length-prefix address and payload address. It adds no wire bytes.
Source: `0x1400DA030`.
✅ Commands 78/79 require a nonempty key. Values can be empty. Command 79 also requires a nonempty message name.
Sources: `0x14008E9C0`, `0x14008F150`.
❓ The target enum names for target_app and the command-83 flag remain unchecked.

## G2 — HelloOk and EnterWorldOk composites

✅ All sources in this section are build 0.0.307.
The tables use object offsets when a semantic name is not checked.
An offset identifies the value source. It is not a wire offset.
❓ Do not use an offset label as a recovered application meaning.

### HelloOk schedules

✅ The outer count is exactly 2. Each in-memory schedule occupies 200 bytes.
The wire sequence for each schedule is `Qiiiqq C[hii]`.
Source: `0x140096C00`, including its variadic compose arguments.

| Wire field | Source relative to schedule start | State |
| --- | --- | --- |
| `Q` | byte +12 | ✅ period mode: 0 daily, 1 whole schedule; ❓ enum name |
| first `i` | dword +4 | ✅ daily reset hour |
| second `i` | dword +8 | ✅ UTC-offset seconds |
| third `i` | dword +0 | ✅ value source; ❓ semantic name |
| first `q` | qword +16 | ✅ schedule start time |
| second `q` | qword +24 | ✅ schedule end time |
| `C[hii]` | rule slots at +32, stride 28, maximum 6 | ✅ value source; ❓ semantic names |

✅ Each rule uses the word at +0, dword at +4, and dword at +16 of its slot.
The sentinel is `h=0xFFFF, i=0, i=0`. The composer does not send this slot.
The nested repeat count excludes this slot. Source: `0x140096C00`.
✅ The three following `q` values come from daemon +2112, +2120, +2128.
Their load log names are schedule number, slot-count initialization schedule number,
and premium-draw-count initialization schedule number.
Sources: load `0x140097EB0 -> 0x1400A4E00`; send `0x140086D50`.
✅ `0x140031CC0` uses the schedule start/end times and mode.
Mode 0 uses a daily 86400-second window. Mode 1 uses the whole schedule.
✅ `0x140001D50` adds the UTC offset, sets the hour, clears minutes/seconds,
and converts back to UTC. It subtracts the same offset.
❓ The schedule dword +0 and the three rule meanings remain open.

### EnterWorldOk logical field groups

✅ The full literal starts with `hi`. The `i` belongs to the fragment header.
Do not treat it as a transaction ID. The logical payload starts at the first `q`.
The packet composer can start another fragment before a field group.
Source: `0x140083D60 -> 0x1400D9810`, packet mode 2.

| Order | Format | Checked value source or meaning | 307 evidence |
| --- | --- | --- | --- |
| 1 | `qqo` | game transaction; selected character ID; 16 bytes from session +48 | ✅ `0x140083D60`; ❓ semantic name of `o` |
| 2 | `c` | normalized boolean from session +1970 | ✅ `0x140083D60`; ❓ flag name |
| 3 | `bbbb` | length/payload at +468/+470, +488/+490, +556/+558, +816/+818 | ✅ session offsets in `0x140083D60`; ❓ blob meanings |
| 4 | `ss` | strings at session +64 and +232 | ✅ `0x140083D60`; ❓ string meanings |
| 5 | `QhhiQ` | session +362 bit 2; maximum character level; maximum showcase byte; session +196; byte +200 | ✅ `0x140083D60`; ❓ other semantic names |
| 6 | `CQiqqihcc;` | two account-time records; field sources below | ✅ `0x140031E80` |
| 7 | `Cqhs;` | character ID, world ID, character name from character +0/+24/+30 | ✅ `0x140083D60` |
| 8 | `iQQQ` | selected character +148/+144/+112; charge-apply type | ✅ `0x140083D60`; ❓ selected-character field names |
| 9 | `s` | string at session +2096; empty fallback | ✅ `0x140083D60`; ❓ semantic name |
| 10 | `Ciiiii;` | zero or one grade-summary record | ✅ `0x1400528E0`; ❓ individual meanings |
| 11 | `i` | grade-state revision at session +2032 | ✅ value at `0x140083D60`; ❓ revision semantics |
| 12 | `CiiQiqq;` | active grade records | ✅ `0x1400529B0 -> 0x140051850`; ❓ individual meanings |
| 13 | `Ciqq;` | content-ban records at session +1920..+1928 | ✅ `0x140035B70`; ❓ individual meanings |
| 14 | `q` | qword at session +1984 | ✅ disassembly `0x140083D60`; ❓ semantic name |
| 15 | `qiq` | account exp, quota, last-update time at session +432/+440/+448 | ✅ `0x140083D60`, `0x140037370` |

✅ Time records use a 32-byte stride starting at session +368.
The eight wire fields are target index `Q` (0 then 1), `i` at +0, `q` at +8,
zero `q`, `i` at +4, `h` at +26, `c` at +24, `c` at +25.
The outgoing second `q` is zero. It is not copied from record +16.
Source: variadic call in `0x140031E80`.
❓ The semantic names of the seven fields after the index remain open.
✅ Command 42 receives the same list shape through `0x1400320E0`.
It then receives account exp `qiq` through `0x14008B130`.
Its complete body shape is `qqqi C[Qiqqihcc] qiq`.
The prefix is transaction, character, client key, bot score.
❓ Command 42 stays partial until the time-field meanings are checked.

✅ The character-name repeat selects state 2 with byte +136 equal to zero.
It excludes states 3 and 5. Source: `0x140083D60`.
✅ Charge-apply values 1, 2 and 3 map to wire values 1, 2 and 3.
Other values reach the invalid-type path. Source: `0x140083D60`.
✅ The grade-summary fields use grade-state offsets +4/+8/+12/+16/+20.
Source: `0x1400528E0`.
✅ Each grade record uses offsets +0/+4/+8/+12/+16/+24.
The `Q` normalizes byte +8 to a boolean.
The list excludes records with nonpositive first, second or fourth fields.
A nonzero byte +8 also requires qword +24 not earlier than current time.
Sources: `0x1400529B0`, `0x140051850`.
✅ Content-ban records have stride 24 and emit `i` at +0, `q` at +8, `q` at +16.
Source: `0x140035B70`.
❓ Commands 1 and 5 remain partial. Their composite semantic maps are not complete.

## G2 — Job and DB call paths

✅ Procedure names below come from `target-db-calls.json` for build 0.0.307.
The target call graph supplies each edge. A path does not mean that every request uses it.
Cleanup and success paths can call different procedures.
❓ These paths do not certify all transaction states, external callbacks, or SQL effects.

| Handler / job | Checked 307 call path | Procedure |
| --- | --- | --- |
| 5 / enter success | `0x140083D60 -> 0x1400317F0 -> 0x1400A1D00` | ✅ `dbo.usp_UpdateLastPlayStart` |
| 5 / cleanup branch | `0x140083D60 -> 0x140032A10 -> 0x1400334A0 -> 0x14009DE90` | ✅ `dbo.usp_UpdateLastLogout` |
| 8 / leave success | `0x140086980 -> 0x140031930 -> 0x1400A1FA0` | ✅ `dbo.usp_UpdateLastPlayEnd` |
| 8 / cleanup branch | `0x140086980 -> 0x140032A10 -> 0x1400334A0 -> 0x14009DE90` | ✅ `dbo.usp_UpdateLastLogout` |
| 12 / logout | `0x140059740 -> 0x1400599F0 -> 0x140031930 -> 0x1400A1FA0` | ✅ `dbo.usp_UpdateLastPlayEnd` |
| 12 / logout cleanup | `0x140059740 -> 0x1400599F0 -> 0x140032A10 -> 0x1400334A0 -> 0x14009DE90` | ✅ `dbo.usp_UpdateLastLogout` |
| 69 / quota extension | `0x140029800 -> 0x14002AC50 -> 0x14006DE30 -> 0x1400AE650` | ✅ `dbo.usp_TryExtendJumpingCharacterCreationQuota` |
| 69 / item-consumption try | `0x140029800 -> 0x14002B240 -> 0x14006DFE0 -> 0x1400AEBF0` | ✅ `dbo.usp_UpdateLobbyServiceStateToItemConsumptionTryState_ExtendJumpingCharacterCreationQuota` |
| 69 / task done | `0x140029800 -> 0x14002B810 -> 0x14006E1E0 -> 0x1400AF120` | ✅ `dbo.usp_UpdateLobbyServiceStateToTaskDoneState_ExtendJumpingCharacterCreationQuota` |
| 69 / item-consumption finish | `0x140029800 -> 0x14002BB80 -> 0x14006E3E0 -> 0x1400AF650` | ✅ `dbo.usp_UpdateLobbyServiceStateToItemConsumptionFinishState_ExtendJumpingCharacterCreationQuota` |
| 69 / finish | `0x140029800 -> 0x14002C150 -> 0x14006E5E0 -> 0x1400AFB80` | ✅ `dbo.usp_FinishExtendJumpingCharacterCreationQuota` |
| 72 / 75 jumping-state upgrade | `0x14002C810 -> 0x14002CC80 -> 0x140031480 -> 0x1400A40F0` | ✅ `dbo.usp_UpdateJumpingCharacterStateComplete` or `dbo.usp_UpdateJumpingCharacterStateCompleteWithoutQuota` |
| 39 / character-data update | `0x140036BB0 -> 0x1400A21C0` | ✅ `dbo.usp_UpdateCharacterData` |
| 42 / account-time update | `0x140036EF0 -> 0x14009D350` | ✅ `dbo.usp_UpdateGameAccountTimeBasedPlayInfo` |
| 42 / 107 account-exp update | `0x140037370 -> 0x1400A4AF0` | ✅ `dbo.usp_UpdateGameAccountExp` |
| 79 / levelup message | `0x14008F150 -> 0x140036D70 -> 0x1400A25B0` | ✅ `dbo.usp_UpdateCharacterLevel` |

✅ The quota-extension dispatcher is the seventh method in vtable `0x1401404C0`.
The next vtable starts at `0x1401404F8`. It belongs to the upgrade job.
Do not assign the upgrade job's DB path to command 69.
Sources: constructors `0x140029700`, `0x14002C780`; handlers `0x14008DCE0`, `0x14008E1A0`, `0x14008E5B0`.

| Direct guild handler | Checked DB address | SQL procedure identity |
| --- | --- | --- |
| 45 `0x14008B770` | `0x1400A2890` | ✅ `dbo.usp_TryGuildRegistration` |
| 48 `0x14008BB80` | `0x1400A2C20` | ✅ `dbo.usp_FinishGuildRegistration` |
| 49 `0x14008BCF0` | `0x1400A2DF0` | ✅ `dbo.usp_CancelGuildRegistration` |
| 50 `0x14008BE60` | `0x1400A3200` | ✅ `dbo.usp_TryRenameGuild` |
| 53 `0x14008C1F0` | `0x1400A3570` | ✅ `dbo.usp_FinishRenameGuild` |
| 54 `0x14008C400` | `0x1400A37D0` | ✅ `dbo.usp_CancelRenameGuild` |
| 55 `0x14008C570` | `0x1400A2FC0` | ✅ `dbo.usp_DeleteGuild` |

✅ `0x1400A40F0` selects `CompleteWithoutQuota` when its third argument is zero.
It selects `Complete` otherwise. The target helper `0x140031480` forwards this flag.
✅ Mileage job method `0x140048780` sends through NP helper `0x140002C30` with callback task 92.
❓ This pass found no demonstrated path from command 100 to the mileage-deposit SQL helpers.
Do not assign those SQL procedures from the job name alone.
❓ No complete procedure map was recovered for jobs started by commands
30, 33, 38, 80, 83, 89, 97, 100, and 104.
The checked direct call paths alone do not prove that these jobs have no persistence.
Transaction-result handlers resume other jobs. This pass does not certify those jobs' DB paths.

## G3 — Part 1 checks

✅ Command 50 receives `transaction:q,guild_id:q,rename_type:c,new_name:s`.
It passes the decoded type to DB argument 3 and reply argument 4.
The success body is `transaction:q,guild_id:q,rename_type:c,rename_key:q`.
The failure body replaces `rename_key:q` with `error:E`.
Source: disassembly `0x14008BE60`; senders `0x140080CD0`, `0x140080DC0`.
The zero shown in G2 pseudocode is not a target wire rule.

| Command | Checked request body | Checked success body and path | Evidence (307) |
| --- | --- | --- | --- |
| 72 | transaction:q,character_id:q,client_key:q | 73: transaction:q,changed_state:Q,remaining_quota:i | ✅ `0x14008E1A0`, `0x14002CC80 -> 0x140082060` |
| 75 | transaction:q,character_id:q,client_key:q | 76: transaction:q,changed_state:Q | ✅ `0x14008E5B0`, `0x14002CC80 -> 0x140082250` |
| 89 | character_id:q,auction_id:o,bid_currency_group_id:h,bid_price:q | 90: character_id:q,auction_id_from_np:o | ✅ `0x140090880`, `0x140010D60 -> 0x140083310` |
| 97 | transaction:q,character_id:q,key_type:Q,key:i | 98: transaction:q,character_id:q,game_grade_key:i,grade_id:i,effective_from:q,effective_to:q | ✅ `0x1400910E0`, `0x14000BCD0 -> 0x140082B00` |

✅ Failures 74/77 use `transaction:q,error:E` at `0x140082160` / `0x140082340`.
✅ Failure 91 uses `character_id:q,auction_id:o,error:E` at `0x140083490`.
✅ Failure 99 uses `transaction:q,character_id:q,key_type:Q,key:i,error:E` at `0x140082C30`.
✅ Command 69 receives `transaction:q,character_id:q,client_key:q,label_id:q,item_instance_id:q,item_data_id:i`.
Source: `0x14008DCE0`. Its success reaches `0x140081E80` from `0x14002C150`.
❓ Its returned quota meaning still needs a focused value-source check.
✅ Command 104 reaches NP send `0x1400CD8B0` through `0x140077E70`.
Its success callback reaches composer `0x140078B00` through `0x1400781D0`.
The success literal is `hqqiQhqhCQiqqihcc;Cqi;` at `0x140078B00`.
❓ Its command number and complete field names still need a disassembly check.
❓ Commands 1, 5, 30, 42, 69, 80, 83, 92, 100, 104 remain partial.

## G3 — Part 2 job persistence map

✅ The table gives checked static call edges in build 0.0.307.
No edge in these listed paths reaches a SQL function in `target-db-calls.json`.
❓ The check is bounded. It does not cover all virtual calls or external NP persistence.
Do not use an empty local procedure list as a claim of no persistence.

| Origin | Checked request or callback path | Local SQL assignment |
| --- | --- | --- |
| 30 | `0x140074BC0 -> 0x1400CC8F0`; callback also sends 31 | ❓ No procedure established for this branch |
| 33 | `0x140076F00 -> 0x140077240 -> 0x1400CC8F0`; callback `0x1400773E0 -> 0x140034C00 -> 0x140002C30 / 0x1400CCAF0` | ❓ No procedure established |
| 38 | `0x1400045A0 -> 0x140004820 -> 0x1400CCA10` | ❓ No procedure established |
| 80 | `0x140070300 -> 0x1400706E0 -> 0x140002C30` | ❓ No procedure established |
| 83 | `0x140070EC0 -> 0x1400712C0 -> 0x140002C30` | ❓ No procedure established |
| 89 | `0x1400104D0 -> 0x140010A10 -> 0x140002C30` | ❓ No procedure established; do not substitute auction-escrow SQL |
| 97 | `0x14000AF30 -> 0x14000B930 -> 0x1400CCCF0` | ❓ No procedure established |
| 100 | `0x1400480C0 -> 0x140048780 -> 0x140002C30`; response `0x140048DE0 -> 0x140002C30` | ❓ No procedure established; do not substitute mileage-deposit SQL |
| 104 | `0x140077980 -> 0x140077E70 -> 0x1400CD8B0`; response `0x1400781D0 -> 0x1400370D0` | ❓ No procedure established |

✅ The binary has `dbo.usp_TryMileageDeposit` at `0x1400B62D0` and
`dbo.usp_FinishMileageDeposit` at `0x1400B6BC0`.
The recreated schema lists both procedures.
❓ No checked edge connects these procedures to command 100's job.
The binary path takes priority over a schema-name match.
✅ Command 69 success returns quota minus creation count, from session +456 minus +460.
Source: `0x14002C150 -> 0x140081E80`. Reserved count is not subtracted here.
❓ This does not prove the complete quota reservation policy.

## G3 — Final format checks and composite candidates

### Additional full formats

| Command | Request body | Reply bodies | Evidence (307) |
| --- | --- | --- | --- |
| 30 | transaction:q,character_id:q,client_key:q | 31: transaction:q,charge_key:s,remaining_play_time:i,charge_apply_type:Q,item_id:i,reservations:h; 32: transaction:q,error:E | ✅ `0x14008A080`, `0x140074BC0`, `0x140081150`, `0x140081250` |
| 69 | transaction:q,character_id:q,client_key:q,label_id:q,item_instance_id:q,item_data_id:i | 70: transaction:q,remaining_quota:i; 71: transaction:q,error:E | ✅ `0x14008DCE0`, `0x14002C150`, `0x140081E80`, `0x140081F70` |

✅ Command 31 uses the NP response string at +120, remaining time at +252,
item ID at +260, and reservation word at +264.
The target maps NP base type 1 to wire apply type 1. It maps other base types to 2.
Source: disassembly at `0x140074BC0`, through call `0x140074FF5`.
The sender uses `hqsiQih` at `0x140081150`.
✅ Command 70 returns session quota +456 minus creation count +460.
Source: `0x14002C150`. This completes its reply layout, not its reservation policy.

### Command 105 success

✅ The target success literal is `hqqiQhqhCQiqqihcc;Cqi;`.
Source: `0x140078B00`. No fragment-header `i` follows the command.
✅ The first compose call emits eight fields, including the command.
The command argument uses zero-base plus `0x69` in `r9d`.
The other prefix sources are job +96, +104, +112, +120, +116, +128,
and the sender's delivery-failure-count argument.
Source: disassembly `0x140078B00`.
✅ The first repeat uses the two account-time records at session +368 through `0x140031FD0`.
✅ The final repeat is `C[q,i]`. It reads reward IDs at array +0 and policy IDs at +96.
It takes the count from array +144. Each iteration advances these arrays by 8 and 4 bytes.
Source: `0x140078B00`.

| Success body group | Semantic labels | State and 307 source |
| --- | --- | --- |
| `qqiQhqh` | transaction, character, schedule number, target type, play time, request UTC time, delivery failure count | ✅ memory order; ❓ names use the 348 guide; `0x140078B00`, init `0x140077A00` |
| `C[Qiqqihcc]` | account-time records | ✅ layout; ❓ complete meanings; `0x140031FD0`, `0x140031E80` |
| `C[qi]` | user promotion reward ID, reward policy ID | ✅ array sources; ❓ names use the 348 guide; `0x140078B00` |

❓ Command 104 request candidate:
`transaction:q,character_id:q,schedule_number:i,target_type:Q,play_time:h,request_time_utc:q`.
Target layout: `0x140092AF0`. Guide names: 348 `0x140094910` and `0x140079B30`.
❓ Failure 106 candidate: the same prefix, then `error:E,error_info:i`.
Target literal and argument order: `0x140083620`; guide names: 348 `0x140084790`.
Do not copy the guide success literal. It has four trailing reward bytes per time record.
The target has two. Source: ✅ `0x140078B00`; guide 348 `0x140079B30`.

### Schedule and EnterWorldOk semantic candidates

The next table separates target sources from guide names.
❓ These names do not raise the handler coverage count.
The target policy-construction check did not run after the final query error.

| Target field or group | Guide candidate | State and target source |
| --- | --- | --- |
| Schedule dword +0 | schedule number | ❓ `0x140096C00`; guide 348 policy builder `0x140073630` |
| Rule word +0 | required play minutes | ❓ `0x140096C00`; guide 348 `0x140073630` uses `Rule.minutes` |
| Rule dwords +4 / +16 | reward policy ID for each of two reward types | ❓ `0x140096C00`; guide 348 has four reward types, not two |
| Time record `i:+0,q:+8,zero:q,i:+4,h:+26,c:+24,c:+25` | schedule number, expire UTC, reserved zero, duration, acknowledged play time, two reward-state bytes | ❓ `0x140031E80`; guide 348 `0x140032ED0` has four reward bytes |
| Entry `c` from session +1970 | is-mobile flag | ❓ `0x140083D60`; guide 348 `0x140085240` |
| Entry four blobs | auth-provider code, external account key, external account name, fourth blob unresolved | ❓ `0x140083D60`; guide 348 `0x140085240` |
| Entry `QhhiQ` | account role bit, maximum character level, maximum showcase value, bot score, final flag unresolved | ❓ additional names; ✅ source order in `0x140083D60`; guide 348 `0x140085240` |
| Selected-character `iQQ` | slot attributes, character rights, jumping-character state | ❓ `0x140083D60`; guide 348 `0x140085240` |
| Entry string from session +2096 | PC-cafe code | ❓ `0x140083D60`; guide 348 `0x140085240` |
| Entry qword from session +1984 | closest grade end time | ❓ `0x140083D60`; guide 348 `0x140085240` |

❓ Command 81 candidate fields after transaction and character:
`blocked:Q,auth_failure_limit:i,auth_failure_count:i,blocked_until:q,delete_at:q,action_mask:q`.
✅ Target completion copies response +96 as a boolean comparison with 1,
then +100, +104, +112, +120 and +128.
Sources: `0x140070960 -> 0x140082430`.
❓ The semantic names come from the matched 348 response at `0x140071CE0`
and sender at `0x140083520`. This pass did not check the target NP response decoder.
❓ The command-83 final byte is a candidate NP action index, not a proven boolean flag.
Target source: `0x1400712C0`, job byte +113. Guide 348: `0x140072640`.

## Remaining work

| Gap | Source for the next check |
| --- | --- |
| ❓ Complete codec iterator paths and field semantic types | `0x1400E6610`, `0x1400E69A0`, `0x1400DA270`, `0x1400DA540` |
| ❓ HelloOk schedule dword +0 and rule meanings | `0x140096C00`; G2 checks the other fixed fields through `0x140031CC0`, `0x140001D50` |
| ❓ EnterWorldOk full field map | `0x140083D60` |
| ❓ Time-based reward success sender and format | `0x140092AF0` |
| ❓ Remaining job DB paths | Jobs from commands 30, 33, 38, 80, 83, 89, 97, 100, 104; G2 lists checked target task sources |
| ❓ Kick success/fail/timeout final state policy | `0x140085DD0`, `0x140085F10`, `0x140086070` |
| ❓ Live protocol interoperation | No network use was permitted |

## Rust port and GameDaemon 381 facts (11 Nov 2026)

| Item | Fact | Source |
| --- | --- | --- |
| Codec | `Program/Server/Packet/src/gamed_link.rs` (1, 2, 3, 4, 5, 6, 7, 20, 21, 22) | — |
| LobbyDaemon side | `Program/Server/LobbyDaemon/src/gamed_link.rs` | — |
| GameDaemon side | `Program/Server/GameDaemon/src/link.rs` | — |
| 1 format `hhhhh` | Same in build 381 | GameDaemon 381 `0x14099BD50`, users `0x14013D340`, `0x140173360` |
| 5 format `hqqqi` | Same in build 381 | GameDaemon 381 `0x1409449E4` |
| 20 format `hqqobisbQh` | transaction, PC id, account id, blob ≤ 64 ❓, `i` ❓ client IPv4, name ≤ 33 units, make-PC record ≤ 256, is-jumping, top level | GameDaemon 381 `0x14099D280`, handler `0x14013F0F0` |
| 21 format `hqhhb` | transaction, job, level, showcase record | GameDaemon 381 `0x1409A8D88`, sender `0x140149920` |
| 22 format `hqE` | transaction, reason (25, 2, 3, 4, 26, 1) | GameDaemon 381 `CreateCharacterTran` `0x14006F1B0` |
| ❓ packet_version in 381 | Not verified. The port uses 83 on both sides | — |
| ❓ 6 in 381 | Not verified. The port uses the 307 field list | — |

## Out-of-scope boundaries

| Scope | Address |
| --- | --- |
| boundary | `0x140175FC8` |
| boundary | `0x140175FD8` |

No boundary internals were analysed.
No config value, credential, certificate, key, account name, or IP address is included.

