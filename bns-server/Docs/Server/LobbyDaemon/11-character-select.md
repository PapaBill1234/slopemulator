# Character screen — LobbyDaemon 0.0.307

## 1. Scope and evidence

This is a partial specification. It is not an implementation-complete contract.
The target is the 2020 `LobbyDaemon.exe`, build 0.0.307.
Target evidence comes from `ida/delegate/LobbyDaemon_2020_copy.i64`.
The guide is build 0.0.348, from `ida/delegate/LobbyDaemon_348_copy.i64`.
Guide names and addresses below come from `command-tables.json` and existing link specs.
The guide database was not opened in this pass. ❓ Cross-build equivalence is not proved.
Target addresses take precedence.

| Command | 307 handler | 348 guide address and name (`HandleLobbyLobbyGate_` prefix) |
| --- | --- | --- |
| D38 | `0x1400C0DD0` | `0x1400C5630` GetCharacterInfoReq |
| D41 | `0x1400C1270` | `0x1400C5AE0` CheckCharacterNameReq |
| D44 | `0x1400C12A0` | `0x1400C5B10` CreateCharacterReq |
| D47 | `0x1400C1920` | `0x1400C6190` DeleteCharacterReq |
| D51 | `0x1400C1F60` | `0x1400C67E0` CancelWaitCharacterDeletionReq |
| D54 | `0x1400C25A0` | `0x1400C6E30` RecoverCharacterReq |
| D57 | `0x1400C2CA0` | `0x1400C7540` BindCharacterSlotReq |
| D79 | `0x1400C4200` | `0x1400C8AA0` GetServiceEntryListReq |
| D103 | `0x1400C6790` | `0x1400CAFD0` CreateInitialUserNameReq |

All these table entries require a channel. Their receive fragment flag is false.
The common job wrapper at `0x14003ABA0` locks session offset `+2152`.
The dispatcher is vtable slot `+48`, not slot `+40`.

## 2. Wire contract

Packets use `size:u16le, command:i16le, body`.
Formats include the command `h`.
`h` and `E` use two bytes. `i` uses four bytes. `q` uses eight bytes.
`Q` uses one byte. `o` uses 16 GUID bytes.
`s` uses a two-byte UTF-16-unit count, then UTF-16LE units. It has no wire NUL.
`b` uses a two-byte byte count, then bytes.
`C[...]` adds a two-byte repeat count.

| Request | Receive format | Fields after command |
| --- | --- | --- |
| D38 | `hqo` | cliKey, slot GUID |
| D41 | No decode | The handler is a no-op. Do not infer a request body. |
| D44 | `hqohsbQ` | cliKey, slot GUID, world, name, appearance, jumping flag |
| D47 / D51 / D54 | `hqo` | cliKey, slot GUID |
| D57 | `hqoq` | cliKey, slot GUID, PCID |
| D79 | `hq` | cliKey |
| D103 | `hqs` | cliKey, requested user name |

D44 sets appearance decode capacity to 256 bytes.
Its name destination has 33 UTF-16 units, including the local NUL.
The DB binding accepts at most 32 name units.

| Reply | Format | Fields after command | 307 sender |
| --- | --- | --- | --- |
| D39 | `hqqohsbqQQi` | cliKey, character row | D38 handler and row composer |
| D40 | `hqoE` | cliKey, slot, reason | `0x1400B8380` |
| D45 | `hqqohsbqQQii` | cliKey, character row, extra i | `0x1400B8470` |
| D46 | `hqoE` | cliKey, slot, reason | `0x1400B85E0` |
| D48 | `hqoQi` | cliKey, slot, state byte, wait seconds | `0x1400B86D0` |
| D49 | `hqoQE` | cliKey, slot, state byte, reason | `0x1400B87D0` |
| D52 | `hqo` | cliKey, slot | `0x1400B88E0` |
| D53 | `hqoQE` | cliKey, slot, state byte, reason | `0x1400B89B0` |
| D55 | `hqo` | cliKey, slot | `0x1400B97E0` |
| D56 | `hqoQE` | cliKey, slot, state byte, reason | `0x1400B98B0` |
| D59 | ❓ `hqoE` | cliKey, slot, reason; literal needs a final check | `0x1400B99C0` |
| D80 | `hiqCqQqohq;` | fragment marker, cliKey, service rows | D79 handler |
| D81 | `hqE` | cliKey, reason | `0x1400BA910` |
| D104 | `hqs` | cliKey, accepted user name | `0x1400BB7E0` |
| D105 | `hqE` | cliKey, reason | `0x1400BB900` |

❓ D45's extra i meaning is not proved.
❓ D50 and D58 payloads remain open.
❓ D49/D53/D56 state arguments need assembly checks. The decompiler omits arguments at some calls.
❓ D80 row field meanings need the service-specific virtual serializers.

### Character row and cache conflict

The row format is `qohsbqQQi`.
It contains PCID, slot, world, name, showcase, an eight-byte value, wire state,
jumping state, and deletion seconds remaining.
Source: 307 composer `0x1400304B0`.
The eight-byte value comes from character offset `+104`.
❓ Its source, epoch and exact meaning remain open.
The final i is zero unless wire state is 2.
For state 2, the composer calculates `max(0, timestamp(+96) - time64())`.
This is a seconds countdown, not an absolute last-play timestamp.

| Internal state | Wire state | Source |
| --- | --- | --- |
| 2 | 1 | `0x1400301C0` |
| 3 or 5 | 3 | Same |
| 4 | 2 | Same |
| Other | 0 | Same |

The current Rust `Character.state` does not separate internal state from wire state.
`MemoryStore::finish_character_restoration` writes 2.
The current D36 serializer sends that value unchanged.
The Rust fields `deletion_time:q` and `last_play_end:i` do not prove target meanings.
D36's prefix also uses representative PCID/world in Rust.
Gate spec 05 identifies those prefix fields as now/time-zone bias.
These are existing cache-model gaps. Do not use their names as target evidence.
The SQL adapter rejects non-NULL character timestamps. Keep that refusal until conversion is specified.

## 3. D38 GetCharacterInfo

Source: 307 `0x1400C0DD0`; cache helper `0x1400364B0`.
Allowed target states:
- `(state & 0xFFFF0000) == 0x20000`.
- Or state `0x10003`.

Resolve the full nonzero slot GUID in the session cache.
Compose D39 from that character.
No DB call, other-link call or timer occurs.

| Condition | Result |
| --- | --- |
| Malformed decode | Log and drop; no failure reply in the checked handler |
| No session | D40/4 |
| Wrong session state | D40/5 |
| Zero or missing slot | D40/6 |
| Other cache/compose failure | D40/2 |
| Success | D39 |

Local Rust session states cover only a subset of target states.
Any D38 implementation must identify that limit.

## 4. D41 CheckCharacterName

Source: 307 `0x1400C1270`.
The function logs and returns true.
It does not decode, look up a session, check a name, send a reply, call DB,
call another link, or start a timer.
There is no success/failure wire map.
The LobbyGate command table also blocks C41 before the relay.
A generated name-check success reply would change target behavior.
Do not add one.

## 5. D44 CreateCharacter

Constructor: 307 `0x14003FC40`; dispatcher `0x14003FDD0`.
Vtable: `0x14014EE48`. Entry event: 10.
The job init at `0x14003FEB0` requires session state `0x20000`.
Its wrong-state branch terminates without a checked failure send.
It requires a nonzero slot, world, nonempty name and nonempty appearance.
It checks jumping policy, world registration, world eligibility and a GameDaemon channel.
The world eligibility helper is `0x14004B4C0`.
It then applies the name rules in section 10.
Config offset `+648` can add an external account/name check at `0x140079E20`.
❓ The option name and complete semantics of that extra check remain open.

Ordered main path:
1. Call `usp_CheckValidityOfCharacterName`, wrapper `0x1400A00F0`.
2. Complete intermediate slot/quota work. ❓ Its full NP/DB order remains open.
3. Call `usp_TryCharacterCreation`, helper `0x140030680`, wrapper `0x1400A02C0`.
4. Add the pending character to the cache.
5. Send GameDaemon create command 20, sender `0x14007FDC0`.
6. On Game success, call `usp_FinishCharacterCreation`, helper `0x140030AE0`, wrapper `0x1400A06B0`.
7. Send D45 after successful completion.

The Game request format is `hqqobisbQh`.
Replies are command 21 `hqhhb` at `0x140089290`, or command 22 `hqE` at `0x1400895A0`.
❓ Request field meanings and D45's extra i remain open.

Game rejection path: remove the transaction, call `usp_CancelCharacterCreation`
for pending internal state 1, remove the cache entry, then send D46.
Source: `0x140042CA0`; cancellation helper `0x140030D30`; wrapper `0x1400A0A60`.
The timeout path at `0x1400430E0` removes the cache entry and sends D46.
❓ A timeout DB rollback was not proved. Do not infer it from the rejection path.

| Job state | Events | Functions |
| --- | --- | --- |
| 0 | 10 | Init `0x14003FEB0` |
| 1 | 0 / 37 / 38 | `0x140041600` / `0x140041790` / `0x140041E20` |
| 2 | 0 / 105 / 106 / 107 | `0x140041F30` / `0x140042610` / `0x140042CA0` / `0x1400430E0` |
| 3 | Terminal | — |

❓ Full D46 reasons, quota compensation and intermediate slot checks remain open.
Event 107 is a timeout event ID. It is not a duration.
Do not substitute a guessed timeout or a SQL return value for a wire reason.

## 6. D47 DeleteCharacter

Constructor: 307 `0x1400434C0`; dispatcher `0x1400435E0`.
Vtable: `0x1401513E8`. Entry event: 11.
Init `0x140043740` requires state `0x20000`.
It checks the slot, conflicting services (`0x140036030`), registered world,
world eligibility, and a GameDaemon channel.
NP/account-inventory checks precede the DB/Game phase.
❓ Their complete wire errors and request order remain open.

Ordered checked DB/Game phase:
1. Recheck the service conflict at `0x140046650`.
2. Call `usp_TryCharacterDeletion`, helper `0x140030E20`, wrapper `0x1400A0C30`.
3. Send GameDaemon delete command 23, format `hqqbi`, sender `0x14007FFD0`.
4. On send failure or Game rejection, call `usp_CancelCharacterDeletion`, helper `0x1400310E0`, wrapper `0x1400A10D0`.
5. On success, call `usp_FinishCharacterDeletion`, wrapper `0x1400A0E00`.
6. Send D48. A rejected path sends D49.

Game success is command 24 `hqh`, handler `0x140089800`.
Game failure is command 25 `hqE`, handler `0x140089A60`.
A completion notification uses command 29 `hq`, sender `0x1400802A0`.
❓ Its exact ordering against every DB outcome remains open.

Immediate completion: `0x140047740` calls the finish helper `0x140030F90`
with delay zero, removes the cached character, and sends D48.
The character's internal state becomes 6.
Wait completion: `0x140047B10` chooses a delay, calls the same finish helper,
keeps internal state 4, and sends D48.
It uses config offset `+288` when jumping byte `+112` is 1,
or when returned level is below config offset `+296`.
Otherwise it uses config offset `+292`.

| Original option name | Type / use |
| --- | --- |
| `wait-character-deletion` | Boolean policy |
| `short-term-character-deletion-time` | Integer duration |
| `long-term-character-deletion-time` | Integer duration |
| `long-term-character-deletion-criteria-level` | Integer level |
| `character-deletion-scheduler-period` | Integer period |
| `character-deletion-scheduler-concurrent-processing-count` | Integer count |
| `check-character-goods-for-character-deletion` | Boolean policy |

Only option names were read. No operator values were copied.
❓ Name-to-offset binding and config duration units still need proof.
The wire countdown uses seconds, but that does not prove every config unit.
Do not add guessed deletion-wait defaults.

| Job phase | Reply / timeout event IDs | Checked functions |
| --- | --- | --- |
| 1 | 62 / 63 | `0x1400447E0`, `0x140044B00`, `0x1400450C0` |
| 2 | 64 / 65 | `0x140045270`, `0x1400455A0`, `0x140045900` |
| 3 | 168 / 169 / 170 | `0x140045AB0`, `0x140045E60`, `0x140046120`, `0x140046340` |
| 4 | 108 / 109 / 110 | `0x140046650`, `0x140046C30`, `0x140046DB0`, `0x140047430` |
| 5 / 6 | Immediate / waiting completion | `0x140047740` / `0x140047B10` |
| 7 | Terminal | — |

❓ Full D49 reasons, state byte, D50 notification and timeout compensation remain open.

## 7. D51 CancelWaitCharacterDeletion

Constructor: 307 `0x140017C30`; dispatcher `0x140017D40`.
Vtable: `0x14013B730`. Entry event: 12.
Init `0x140017E00` requires session `0x20000` and a nonzero full slot GUID.
It checks registered world and world eligibility.
The wrong-session-state branch terminates without a checked reply.

Ordered path:
1. At `0x1400189B0`, call character helper `0x1400311D0`.
2. That helper accepts internal state 4 only.
3. Call `usp_TryCharacterRestoration`, wrapper `0x1400A12A0`.
4. Clear character timestamp `+96`. Set internal state 5.
5. At `0x140018D90`, acquire GameDaemon and send restore command 26 through `0x140080150`.
6. On success at `0x1400191F0`, remove the transaction and call `usp_FinishCharacterRestoration` through helper `0x140031310`, wrapper `0x1400A1470`.
7. Set internal state 2 after DB success. Send D52.

The first helper returns 10 for internal state 3 or 5, 4 for other non-4 states,
and 6 for a DB failure. These are helper results, not proved D53 wire reasons.
Game rejection at `0x140019670` sends D53.
Timeout at `0x140019B20` sends D53.
❓ Full wire reasons and rejection/timeout compensation remain open.
Events 111, 112 and 113 select success, failure and timeout.
❓ The shared transaction lifetime and config binding remain open.
This operation is not `usp_CancelCharacterDeletion`.

## 8. D54 RecoverCharacter

Constructor: 307 `0x140063B00`; dispatcher `0x140063E30`.
Vtable: `0x14016B6D8`. Entry event: 18.
Parameter helper `0x140063C10` requires a gate and a nonzero slot.
Init `0x140063EC0` requires session `0x20000`.
It resolves the cached character, registered world, eligibility and GameDaemon channel.
❓ Full init rejection reasons remain open.

Ordered path:
1. Send GameDaemon restore command 26 at `0x140064710`.
2. Remove the transaction on success at `0x140064900`.
3. Also accept Game failure reason 35 at `0x140064C40` as a completed restoration result.
4. Run completion helper `0x140065270`.
5. For internal state 3, call `usp_CancelCharacterDeletion` and set state 2.
6. For internal state 5, call `usp_FinishCharacterRestoration` and set state 2.
7. Send D55 after successful completion. Otherwise send D56.

The completion helper sets reason 2 for an unsupported state or DB failure.
❓ Full D56 state/reason arguments still need assembly checks.
Timeout function: `0x1400650C0`.
Events 111/112/113 select success/failure/timeout.
❓ Timeout duration remains open.
D54 does not run the D51 state-4 try-restoration phase.

## 9. D103 CreateInitialUserName

Dispatcher: 307 `0x14002EED0`; vtable `0x140141E70`.
Entry event: 27. Init: `0x14002EF50`.
Allowed session state is `0x10003`, not the ordinary Lobby state `0x20000`.
Wrong state sends D105/5. An absent/empty name sends D105/6.
Config offset `+360` is the enable flag.
❓ The disabled branch needs an assembly check before its behavior can be specified.

The validator uses config `+364` minimum and `+368` maximum.
It permits zero spaces and enables numeric characters.
It uses sentence-case flag `+372` and multilingual flag `+373`.
It passes no forbidden-character string.

| RuleSet result | D105 reason |
| --- | --- |
| 3 / 10 / 11 | 6 |
| 4 | 224 |
| 5 | 225 |
| 6 | 227 |
| 7 / 8 / 9 | 226 |
| Other failure | 1 |

The config `+648` check can also apply here.
Missing required account data sends reason 2. A failed extra check sends 228.
At `0x14002F790`, send NP command 2108.
There is no LobbyDB call.
Unavailable NP sends reason 7. Send failure sends 2.
Reply function `0x14002FAB0` accepts status 200,
or status 400 with case-insensitive `Success`.
It copies the requested user name to session `+232`, sets state `0x20000`, and sends D104.
A null callback sends D105/2.

| NP error | ❓ Arithmetic-derived D105 reason; final assembly check pending |
| --- | --- |
| 4 / 48 / 3000 / 3002 | 2 |
| 10 / 3035 | 6 |
| 3003 / 3042 | 229 |
| 3041 | 228 |
| 3336 | 231 |
| 3743 | 230 |
| Other | 1 |

Timeout `0x14002FEC0` sends D105/7.
Events 70/71 are reply/timeout IDs, not durations.
❓ NP lifetime config and exact command-2108 payload remain open.

Original option names: `initial-user-name-enable`, `initial-user-name-prefix-list`,
`allowed-min-user-name-length`, `allowed-max-user-name-length`,
`force-sentence-case-for-user-name`, `enable-multilingual-for-user-name`.
No config values were copied.

## 10. Name rules

Source: 307 `0x14006AC30`, RuleSet validator.
This is not an ASCII-only regular expression.
It measures NUL-terminated UTF-16 units.
RuleSet flags at `+21/+22` can select counting helper `0x140056FB0` instead.
That helper has Thai/other language cluster paths. ❓ Full cluster rules remain open.

Checks run in this order:
1. Minimum length: result 4.
2. Maximum length: result 5.
3. Optional sentence case through `0x140056AD0`: result 10 on failure.
4. Optional single-language check through `0x1400569C0`: result 11 on failure.
5. Per-unit configured language checks through `0x1400565C0`.
6. Space and optional punctuation rules.
7. A filter call through `0x14009BB60` on the constructed non-space buffer.

The language helper accepts ASCII A-Z/a-z before the forbidden-list check.
It accepts ASCII 0-9 at that point only when numeric input is enabled.
Thus the forbidden string does not override those early accepted ASCII ranges.
Other units must pass the configured language rule and forbidden-list check.
Checked ranges include Hangul, Hiragana/Katakana, CJK, Cyrillic and Thai.
Some languages require a byte lookup table at `0x14015B590`.
❓ The full table, language-ID bindings, case behavior and cluster rules remain open.

| Space rule (U+0020) | RuleSet result |
| --- | --- |
| Total exceeds configured amount | 9 |
| First or last unit | 7 |
| Consecutive spaces | 8 |

Ordinary invalid units return 6.
Optional punctuation table `0x1401F8818` has three 12-byte records:
U+30FB, U+3005 and U+30FC; each has count limit 2.
Their edge/repeat flag bytes are zero.
The validator supports count/edge/repeat errors 15/13/14.
The filter call maps return 2 to RuleSet 3, return 3 to RuleSet 12,
and other nonzero returns to RuleSet 2.
❓ Filter data and filter implementation remain unverified.
No licence/helper internals were followed.

Character-name option bindings in `0x14003FEB0`:

| Option | Config offset |
| --- | --- |
| `allowed-min-character-name-length` | +268 |
| `allowed-max-character-name-length` | +272 |
| `allowed-character-name-space-amount` | +276 |
| `enable-numeric-for-character-name` | +316 |
| `force-sentence-case-for-character-name` | +317 |
| `enable-multilingual-for-character-name` | +318 |
| `forbidden-characters-for-character-name` | +320 |

❓ Option-to-offset identity uses the existing name/field evidence; descriptor binding needs a final check.
Other scoped names: `character-name-reuse-limit-duration`,
`enable-jumping-character-creation`, `use-jumping-character-creation-quota`,
`use-jumping-character-creation-quota-in-world`.
Do not invent values or use an incomplete validator to approve production names.

## 11. D57 and D79

D57 receives `hqoq` and starts event 33 with vtable `0x14013A220`.
Missing session sends D59/4. Job-registration failure sends D59/2.
`usp_BindCharacterSlot` uses wrapper `0x1400A3E80`.
It takes PCID bigint IN, then slot GUID IN.
The recreated schema has no such procedure.
❓ Allowed job states, full NP slot sequence, D58, errors and timers remain open.
Do not invent a SQL procedure definition from its name.

D79 uses cache helper `0x140036630`.
It accepts exactly `0x20000` or `0x10003`.
Missing session sends D81/4. Wrong state sends D81/5.
Other cache/serialization failure sends D81/2.
It sends fragmented D80 on success.
No direct DB call, other-link call or timer occurs in this checked path.
❓ Row meanings require the service serializers.

## 12. DB contract and schema conflicts

Procedure names and types use `db-binding.json`.
The binary is authoritative when the recreated schema disagrees.
All wrappers require both successful execution and procedure return zero.
A driver error and a nonzero procedure return are separate results.
Neither becomes a wire reason without a caller map.

| Procedure | 307 wrapper | Parameters |
| --- | --- | --- |
| `usp_CheckValidityOfCharacterName` | `0x1400A00F0` | name nvarchar IN; 32 units, 66-byte buffer |
| `usp_TryCharacterCreation` | `0x1400A02C0` | account GUID, slot GUID, world smallint, name nvarchar(32), right tinyint, PCID bigint INOUT |
| `usp_FinishCharacterCreation` | `0x1400A06B0` | PCID, job smallint, level smallint, jumping tinyint, consume-quota bit, showcase varbinary |
| `usp_CancelCharacterCreation` | `0x1400A0A60` | PCID |
| `usp_TryCharacterDeletion` | `0x1400A0C30` | PCID |
| `usp_FinishCharacterDeletion` | `0x1400A0E00` | PCID, duration unsigned i32/SQL int, deletion datetime OUT |
| `usp_CancelCharacterDeletion` | `0x1400A10D0` | PCID |
| `usp_TryCharacterRestoration` | `0x1400A12A0` | PCID |
| `usp_FinishCharacterRestoration` | `0x1400A1470` | PCID |
| `usp_BindCharacterSlot` | `0x1400A3E80` | PCID, slot GUID |

The recreated `usp_TryCharacterCreation` inserts internal state 1.
Its declared returns include 0 success, 1 generic, 2 duplicate and 3 limited name.
The character helper `0x140030680` maps procedure 1/2/3 to helper 6/7/8.
❓ The next helper-to-wire map remains open.
The recreated `usp_CancelCharacterDeletion` updates state 3 to 2 only.
This agrees with the target rollback helper. It does not cancel state-4 deletion wait.
The four earlier 512-byte binding limits still conflict with recreated varbinary(4096).
See `08-db-binding.md` and `09-store-check.md`.
No SQL connection was made in this pass.

## 13. Unchanged gate check

Evidence: `spec/04-dispatch.md`, `spec/05-handlers.md`,
`Program/Server/Packet/src/dispatch.rs`, `Program/Server/LobbyGate/src/server.rs`.

| Client request | Daemon request | Daemon replies → client replies |
| --- | --- | --- |
| C30 | D38 | D39→C31, D40→C32 |
| C36 | D44 | D45→C37, D46→C38 |
| C39 | D47 | D48→C40, D49→C41, D50→C42 |
| C43 | D51 | D52→C44, D53→C45 |
| C46 | D54 | D55→C47, D56→C48 |
| C49 | D57 | D58→C50, D59→C51 |
| C71 | D79 | D80→C72, D81→C73 |
| C95 | D103 | D104→C96, D105→C97 |

Ordinary request relays add cliKey and change the command. They copy the remaining bytes.
Ordinary reply relays remove cliKey and change the command. They copy the remaining bytes.
C41→D41 exists in the map, but the authenticated command table rejects C41.
D42/D43 are bypass entries with no mapping, so their client command is C0.
D36 and D80 are not raw relays. The gate joins, decodes and rebuilds fragments.
No gate change is proposed. Do not enable C41 to make a test pass.
The requested name-check/create/delete/recovery gate flow cannot include a successful C41 check unchanged.
Test the no-op and the blocked route as separate target rules instead.

## 14. Retained policies and boundaries

Keep existing five-second local NP/world deadlines until a verified config replaces them.
INIT expiry stays opt-in. Positive token lifetimes and same-link retention stay required.
Keep the SQL ten-second local timeout and no-retry policy for uncertain writes.
Keep disabled optional login/NP refresh paths disabled.
Do not add dependencies or change proto/server behavior or `Program/Tools/Emu/`.
Use generated fixtures and the local deterministic NP stub only.
SQL integration execution must skip when `LOBBYD_MSSQL_URL` is unset.
Live SQL validation and generated-row cleanup require a separate permitted operator run.

| Excluded boundary | Source |
| --- | --- |
| Tencent TSS `tss_get_sdk` | 307 string `0x140175FC8` |
| Tencent TSS `tss_sdk` | 307 string `0x140175FD8` |
| GameGuard `ggauth64_27.dll` | Supplied server-set directory entry; ❓ no target EXE address established |
| GameGuard `csauth3.cfg` | Supplied server-set directory entry; ❓ no target EXE address established |
| BloodCert / `BNSHelper.dll` | Explicit task exclusion; no mapping, calls or implementation |

No excluded client executable was read.
No binary or IDA database was patched or saved.
No secret, connection value, sample account name or sample IP address belongs in this spec.

## 15. Step 11 implementation contract (local subset)

Build 0.0.307. Checked sources: create completion `0x140042610`, rejection
`0x140042CA0`; delete completion `0x140046C30`, rejection `0x140046DB0`;
restore try `0x1400189B0`, completion `0x1400191F0`, rejection `0x140019670`,
timeout `0x140019B20`; recovery completion `0x140065270`;
initial-name init `0x14002EF50`, callback `0x14002FAB0`.
Game create returns job, level and showcase. D45's extra integer is remaining
jumping creation quota. Delete success returns character level.
D54 completes internal state 3 with CancelCharacterDeletion, or 5 with
FinishCharacterRestoration. Unsupported state and DB failure set reason 2.
D103 has no LobbyDB procedure. Use the NP boundary, not a guessed SQL call.
Its NP error arithmetic agrees with section 9. ❓ Assembly verification remains.
The disabled D103 init branch sets terminal job state, but continues parameter
checks in the decompiled function. ❓ Do not claim a disabled success reply.

### Local policy R15

Character mutation is disabled unless an explicit `CharacterPolicy` is supplied.
It contains generated/authorized slot GUIDs, registered world IDs, name length
bounds and deletion wait seconds. No original config default is inferred.
Only non-jumping creation is supported. Unknown slots/worlds reject.
NP quota, goods and external-name checks are not bypassed as production success:
this policy is a local test fixture approval only. It is not production parity.
The local name subset accepts ASCII letters only, with optional single spaces
inside character names. User names accept ASCII letters/digits and no spaces.
Reject NUL, malformed UTF-16 and all unimplemented language/filter rules.
❓ Original character name wire maps remain open. Local validation uses reason 6.
Local generic Store/Game failures use reason 2. Never send an SQL code as a wire reason.
Local duplicate/limited-name returns remain typed Store errors; wire reason 2 is
used until the original caller map is checked. Missing session uses 4; missing
slot uses 6; wrong ordinary session state drops; D103 wrong state uses 5.
Local service conflict uses reason 2. Unsupported recovery uses reason 2.
❓ Failure state byte uses the current local wire state, not a proved 307 argument.
The lifecycle is stored with the character row (`Character.state`,
`Character.deletion_time`), not kept per LobbyGate link. The store is the truth.
`CharacterState` in LobbyDB: 1 creating, 2 normal, 3 deleting, 4 waiting,
5 restoring, 6 deleted. `usp_LoadCharacterList` returns only states 2 to 5; the
columns are 8 state, 9 DeletionDelayTime, 10 LastPlayEndTime, 13 Blocked.
A row with another state code is refused at load (never a panic).
Serialize normal as 1, waiting as 2, and deleting/restoring as 3 (this is the
wire value, not the DB code). Keep the opaque row qword zero.
A character that one link loaded can be deleted, restored or recovered through
another link. Each transition procedure checks the stored state. A stale link
that holds an old cache gets a failure (reason 2) and changes nothing.
D36/D38 compute the remaining seconds from the stored wall-clock deletion time
(`deletion_time - now`, 0 when it has passed). Cancel after the time has passed fails.
❓ SQL `datetime` is read as UTC. LobbyDB writes `GETDATE()` (server local time),
so the SQL Server host must run in UTC. `usp_FinishCharacterDeletion` returns
`Option<SqlTimestamp>`; the cache after a delete uses `now + seconds`.
Deletion scheduler (`20-handlers.md` §9): one timer task in lobbyd. Each tick calls
`usp_RetrieveExpiredDeletionWaitingList(count)` (state 4, time passed), then
`usp_ExpireCharacterDeletionWaiting(pcid)` for each row (state 4 to 6; it refuses a
row whose time has not passed). ❓ Local defaults: period 60 s, batch 16. The first
tick runs at once. The task uses only the store. An open session keeps an expired
row with countdown 0 until the player logs in again. The GameDaemon is not told.
Game rejection cancels pending create/delete. Restore failure leaves state 5 for
D54 recovery. Timeout leaves uncertain Store state; no automatic write retry.
D103 requires an explicit local initial-name control because normal local login
already requires a user name. NP acceptance changes it back to Lobby.

## 16. Earlier implementation gate (step 10 history)

D41's no-op is specified.
D38's cache operation and errors are specified, but the cache representation has the gaps above.
A local D38 implementation can use only the current wire-ready fixture cache.
It must not claim SQL internal-state or nonzero timestamp parity.
D38 now uses that local cached-row subset. It checks link ownership and Lobby state.
Its truncated requests produce no reply. It sends D40/4, /5, /6 or /2 for the checked failures.
The shared Rust decoder accepts a decoded prefix with trailing bytes. D38 keeps that local policy.
❓ Target D38 trailing-byte behavior was not checked separately.
D41 now accepts a valid trusted frame after Hello and produces no output, regardless of body/session.
Five generated rule tests cover these paths and serializer bounds.
The encrypted gate test now checks C30→D38→D39→C31 and D40→C32.
No Store method or SQL binding is needed by these two cache/no-op handlers.
D44/D47/D51/D54/D103 require the remaining maps and state-model work before implementation.
They remain unimplemented. The full create/delete/cancel/recover gate flow remains untested.
D57/D79 remain supporting research, not complete handlers.
