# Handoff play-1: client login through the Rust StServers

Date: 11 Nov 2026. Model: Fable 5.2. Stages 1–6 are done and pushed. Stage 7 (enter world) is **not done**. The work stopped there, as the card tells.

## 1. State

```text
client ─► CliGate ─► RouteSrv ─► AuthSrv ─► GameAccountSrv / SlotSrv / PlayStubs     DONE (loopback test)
client ─► LobbyGate ─► LobbyDaemon ─► GameGate ─► AuthSrv (token)                    DONE (loopback test)
client ─► character select                                                           DONE (loopback test), live test open
client ─► GameDaemon ─► world                                                        NOT DONE (stage 7)
```

| Stage | Result | Commit title | Tests |
| --- | --- | --- | --- |
| 1 Gap map | Done | `StServers: play path gap map…` | `Docs/Platform/StServers/16-play-path.md` |
| 2 AuthSrv | Done | `AuthSrv: phase 1 crate…` | `authsrv`: `login_finish_and_tokens`, `wrong_password_deletes_the_session`, `request_errors`, 2 GUID tests |
| 3 Services | Done | `StServers: GameAccountSrv, SlotSrv, PlayStubs and CliGate forward path` | `gameaccountsrv` 2, `slotsrv` 1, `play-stubs` 1 |
| 4 GameGate | Done | `GameGate: token path and daemon-side client for lobbyd` | `gamegate`: `game_token_through_the_gate` |
| 5 Run script | Done | `tools: run_play.sh and play_account.sh…` | Real start of 9 services on this host (alternate ports) |
| 6 Loopback test | Done to the character list | `LobbyDaemon: play path loopback test…` | `lobbydaemon --test play_path`: `client_logs_in_through_stservers_to_the_character_list` |
| 7 Enter world | **Not done** | — | — |
| 8 Live instructions | Part: §3 below (login to character select) | — | — |

## 2. Why stage 7 stopped

| # | Blocker | Evidence |
| --- | --- | --- |
| 1 | GameDaemon has no listener and no binary. The crate has handlers and checks only. | `Program/Server/GameDaemon/src` (no socket) |
| 2 | Command 24 (token reply): the success body is not recovered ❓. | `Docs/Server/GameDaemon/05-enter-world.md`, ordered path row 4 |
| 3 | Command 32 (initial player composite): only a prefix is known. 422 property ids with custom `?` fields and 11 more groups have no layout ❓. A header-only packet is not valid. | `05-enter-world.md` "Command 32" |
| 4 | Spawn and appear packets before command 43: ids and bodies are not recovered ❓. | `05-enter-world.md` row "Unresolved order"; `06-minimum-session.md` "Implementation gate" |
| 5 | Command 929 after HelloOk: purpose and server reply are open ❓. | `06-minimum-session.md` |
| 6 | LobbyDaemon has no TCP link to a GameDaemon. Character create and world entry return reason 2 in `lobbyd`. | `Program/Server/LobbyDaemon/src/gamed.rs` (local interface only) |

Items 2–5 are reverse engineering work on `GameDaemon.exe` and the client (IDA). Each one is larger than one session step. A guess for command 32 puts the client in a crash or an endless load screen, so no partial code was written.

Proposed next cards, in this order:

| Card | Work | Source |
| --- | --- | --- |
| world-1 | GameDaemon listener: Hello 11/12 with the RSA key of `server-keys/`, AES-128 frames, command 929, token 23/24 (body from IDA) | `04-world-connect.md`; server `0x1403703D0`, `0x140371170`; client `0xF3AF90` |
| world-2 | Command 32 full layout: property writer `0x140697E50` and the 11 groups; a DB-free default character | sender `0x1405F5980`, literal `0x140DAA240` |
| world-3 | Commands 41, 42, 43 and the appearance queue `0x14024B0C0`; ping 17/18 | `05-enter-world.md`, `06-minimum-session.md` |
| world-4 | LobbyDaemon ↔ GameDaemon TCP link (D25 and character create) | `Docs/Server/LobbyDaemon/07-gamed-link.md` |

## 3. Live test now: login to character select

The client files on the laptop already point to this host (`patches.xml`: `np-address`, `lobby-gate-address`; ports 6600 and 10900).

| Step | Command (in `bns-server/`) | Result |
| --- | --- | --- |
| 1 | `kill 1038314 1100761` | Frees ports 6600 and 10900. The two processes are left from the test of 8 Nov: `forward.py 6600 16600` (its target port is closed) and an old `lobbygate`. `run_play.sh` does not kill them. |
| 2 | `tools/play_account.sh <LoginName> <UserName>` | Asks for the password (`keyhere`). Writes `secrets/play-accounts.tsv` and `secrets/play-game-accounts.tsv`. Use the login name that the client sends: the typed name plus the client postfix if the name has no `@`. |
| 3 | `tools/run_play.sh start` | 9 services in tmux session `play`. `tools/run_play.sh status` must show `up` on each line. |
| 4 | Start the client on the laptop (`start.bat`, with `/LoginMode`). Type the login name and password. | See the log table. |
| 5 | `tools/run_play.sh stop` | Stops the stack. |

What each log shows (`logs/play/`):

| Log | Good line | Bad line and meaning |
| --- | --- | --- |
| `CliGate.log` | `LOGIN, <UserId>, <address>, <login name>` | `LOGIN-FAILED`: wrong password or SRP mismatch. No line at all: the client did not reach port 6600. |
| `AuthSrv.log` | `Auth/LoginStart -> 0`, `Auth/KeyData -> 0`, `Auth/LoginFinish -> 0`, `Auth/RequestToken -> 0`, `Auth/RequestGameToken -> 0` | `LoginStart -> 3101`: the login name is not in the account file (check the postfix). `KeyData -> 11`: wrong password. |
| `GameAccountSrv.log` | `GameAccount/ListMyAccounts -> 0`, `GameAccount/ListAccounts -> 0` | Code 10: no `GameCode` in the request ❓. |
| `PlayStubs.log`, `SlotSrv.log` | `Access/GetAppUserRights -> 0`, `VirtualCurrency/GetBalance -> 0`, `Slot/ListSlots -> 0` | Code 52: the client sent a command that is not ported. Send the line. |
| `GameGate.log` | `Auth/ConsumeGameToken -> 0` | `-> 3010`: token not found (wrong user, wrong game code, or older than 300 s). |
| `LobbyGate.log`, `LobbyDaemon.log` | client login and the character list request | — |

Expected end of the live test: the client shows the server and character screen with no character. Character create fails (blocker 6). The client cannot enter the world.

## 4. ❓ items that the first live login will test

| Item | Where |
| --- | --- |
| RC4 key = full 64-byte SRP key K, both directions, on after the KeyData reply | `cligate::crypt`, spec 11 §4, §6 |
| `Sts/Connect` reply shape from CliGate | spec 11 §7.1 |
| `Auth/GetMyUserInfo` after login: forwarded (the binary handler needs a login session) | spec 11 §3 note |
| Reply shapes of `ListMyAccounts` (`GameAccountId` in the item) and `GetMyUserInfo` | `GameAccountSrv/README.md`, `AuthSrv/README.md` |
| Login name case and postfix in the SRP identity | spec 15 §4.1; `authsrv::accounts::identity` |
| `AccessMask` value 0 | `PlayStubs/README.md` |

If the live login fails at the first step, the old path still works for a comparison: `sts-local` (`Program/Platform/Sts`) had live logins on 8 Nov.

## 5. Not done, with reason

| Item | Reason |
| --- | --- |
| AuthSrv DB calls through DbSrv | The account DB has 0 rows and no NameSrv exists to resolve a login name. A TSV file has the same columns. |
| SessionGateSrv crate | The game client login path sends no `Session` command (`PlayStubs/README.md`). |
| AccessSrv, PresenceSrv, VirtualCurrencySrv | Not analysed. Fixed replies in `play-stubs`; `Presence/Login` stays local in CliGate. |
| GameGate whitelist, `TokenInfo`, `Presence/Login` chain | Not on the token path. Listed in `GameGate/README.md`. |
| `Docs/Plan/abuse-notes/play-1.md` | No abuse case was found in this work. |
| Enter-world part of the loopback test | Stage 7. |

## 6. Decisions that differ from the card

| Card text | Decision | Reason |
| --- | --- | --- |
| Move SRP, KeyData, RC4 out of `sts` | `authsrv` uses `sts::srp6` and `sts::key_data` as a dependency | `cligate` and `routesrv` depend on `sts` already; a move adds risk before the live test |
| One crate each for SessionGateSrv, GameAccountSrv, SlotSrv | GameAccountSrv, SlotSrv, and `play-stubs`; no SessionGateSrv | See §5 |
| Trailer `Claude Fable 5.1` | `Claude Fable 5.2` | The session model is Fable 5.2 |

## 7. Next step

1. Run the live test of §3. Send the `logs/play/*.log` lines of the first failure, or the screen that the client reaches.
2. Start card world-1 (§2).

## 8. Live test result (11 Nov 2026)

The real client (build 381) logged in through the Rust StServers and reached character select.

| # | Found in the live test | Fix |
| --- | --- | --- |
| 1 | The client stays on "connecting" when `Slot/ListSlots` has zero slots: `NPSession::ReceiveCharacterListSuccess` (`Client.exe` 0xC1DF90) returns before `LobbyGateSession::ReceiveNPCharacterListSuccessNotify` (0xCA1950), so cmd 27 is not sent. | `slotsrv` returns 4 slots of type `char` (client string 0x15D9888). Parser: `StsMsgCli32.dll` sub_10005B40, all `Slot` children are optional. |
| 2 | A second login of the same account got start failure D11 ("game access has been limited"). The reason code was not logged ❓. Likely cause: the presence of the old session. | `StubNp.presence_replace` (set in `lobbyd`); `lobbyd` logs the reason of a start failure. |
| 3 | Client cmd 71 `L_GetServiceEntryListReq` → D79 had no handler in `lobbyd`; the gate link closed and the client waited. | D79 → D80 with an empty list ❓ (the 307 job is not ported). `lobbyd` logs each unsupported command id. |

Verified live: SRP login, RC4 switch, `Sts/Connect` reply, the post-login requests, `GetMyUserInfo` forward, game token, LobbyGate login, world list, character list. The ❓ items of §4 passed this test.
| 4 | Character create failed: `lobbyd` had no GameDaemon. | `LocalGameDaemon` in `lobbyd` (used with `--gamegate`). It builds the showcase record in process. **Verified live: create works.** Not parity: the original sends the request to GameDaemon. |

## 9. FSD record facts (GameDaemon 348, PDB names; build 381 has the same format string)

| Fact | Source |
| --- | --- |
| `DecomposeMakePcFsd` reads the make-PC record. Symbols: 0 race, 1 sex, 2 job, 3 appearance (92 bytes), 4 name. | 0x1400BCC10 |
| `ComposeShowcaseFsd` writes the lobby showcase record. | 0x1400BD3E0 |
| `FsdComposer::ComposeV` layout: u16 total size, u16 format size, format characters, then per field (u16 symbol id, value). | 0x1406E9900 |
| Format characters: `c` 1 byte, `w` 2, `d` 4, `q` 8, `s` UTF-16 with an end zero, `N` u16 count then values without ids. | 0x1406E9900 |
| Showcase format (59 characters): `cccNcssdwwwcddwqdcddccccccccddcccdcccdcccqdcddddccccwcwccdd`. Order: race, sex, job, appearance, name, title, geo zone, x, y, z, level, exp, hp, fp, money, equipment. | `FsdShowcaseRecord` |
| Request handler and transaction. | `HandleLobbyGame_CreateCharacterReq` 0x140140730, `CreateCharacterTran` init 0x140072800 |

Rust: `shared::fsd` (`decode_make_pc`, `compose_new_showcase`). ❓ The local record sets level 1 and all other fields to zero. The original fills them from data tables.

Limit: `lobbyd` keeps characters in memory. A restart removes them.

## 10. Enter world, first try (11 Nov 2026)

`gamedaemon` now answers commands 31 and 42. No live result yet.

| Client command | Server reply | Source |
| --- | --- | --- |
| 23 | 24; reads the character record | `shared::local_entry` |
| 31 | 32 (fragments), then 41 | `Packet::world_entry`, `Packet::zone_entry` |
| 42 (key = key of 41) | 43 | `Packet::zone_entry` |

New facts (build 348 GameDaemon, PDB names):

| Fact | Source |
| --- | --- |
| PC object id = `(1 << 48) + index`. Index is 1..19999 (`MaxIndex` 20000, mask `0xFFFF`, shift 0, type 1). | `World::initPcManager` 0x1403EDDC0, `GameObjectIdManager::Make` 0x14001D090 |
| Command 41 fields: zone id `q`, channel `h`, GeoZone record id `i`, spawn action key `i`, world clock byte `c`, arena wait cancel `c`, zone broadcast block `K`, x, y, z, yaw, PC transition count, summon transition count. In build 348 the command number is 39. | `PcSpawnAction::prepareSpawn` 0x1404B4E50 |
| Each level-1 `PcInitialSetting` record gives zone 1101. Spawn `1101_SP1`: position -3199, 9241, 599; yaw 45. ❓ Units. | `pcinitialsettingdata.xml`, `zonepcspawndata.1101.xml` (build 381 data) |
| Level-1 record 1 of `pcdata.xml`: max-hp 303, speed 62. | build 381 data |

Local choices (not parity):

| Choice | ❓ |
| --- | --- |
| `lobbyd` writes the make-PC record to `logs/play/entry/<owner>.pc` at world admission. `gamedaemon` reads it at command 23 by client key, or takes the newest file. | ❓ owner = client key has no live proof |
| One fixed PC object id (index 1). Runtime zone id = 1101. Channel 1. Spawn key 1. | ❓ |
| Broadcast block: 16 zero bytes in commands 32 and 41. | ❓ size and use |
| All properties except world, race, sex, job, appearance, name, zone, position, yaw, level, hp, max-hp, speed are zero. Skill pages are empty. | ❓ |
| No sync packets after 32. No ping 17. | ❓ |

## 11. Command 929 → 930 (11 Nov 2026, live)

Live result: the game Hello 11/12 works with the LobbyGate handshake layout. The client then sends 929 and waits.

| Fact | Source |
| --- | --- |
| 930 descriptor `hihhiCb;`: major version `h`, minor `h`, revision `i`, count, counted UTF-8 elements. | 381 builder 0x140087D40; 348 `ContentsRuleConfig::MakeListPacket` 0x140089C50 |
| Build 381: major 74, minor 0. Revision = `dword_140EE586C`. Build 348: major 65, command 896. | same |
| Elements: XML header (`module="contents-rule-config"`, `release-side="client"`), one `<option name='N' value='V'/>` for each of 167 rules, `</config>`. | same |
| Rule types (table 0x140ECAD88, 56 bytes each): 1 `%d` (98), 2 `%I64d` (9), 4 bool `yes`/`no` (56), 6 string (4). | same |
| Failure reply in 348 is command 897, reason 2086. | `HandleGame_ContentsRuleListReq` 0x14036DF60 |

Local: `gamedaemon --contents-rule <file>` reads `ContentsRule.gamed.config.xml` at start. ❓ File order, revision 0, no defaults for absent options, no 30 s rate limit.

## 12. Move to the database (11 Nov 2026)

Goal: remove the local short cuts of §10. Characters go through `LobbyDB` and `BlGame`, as in the original.

| Stage | Work | State |
| --- | --- | --- |
| 1 | `lobbyd` uses `LobbyDB` through `MssqlStore` in the play stack | Done |
| 2 | Real `lobbyd` ↔ `gamedaemon` link (Hello 1/2, world live 4, create 20 → 21/22, entry 5 → 6/7) | Done |
| 3 | `gamedaemon` creates and loads the PC in `BlGame` | Done |
| 4 | Remove `LocalGameDaemon` and `shared::local_entry` | Done |

Stage 1 facts:

| Fact | Source |
| --- | --- |
| The recreated `LobbyDB` had 39 indexes. The image database has 75. `usp_CheckValidityOfCharacterName` needs `IX_Character_UniqueCharacterName` (index hint) and failed with SQL error 308. | `LobbyDB.mdf` of the BNSX image, attached as `LobbyDB_bnsx` |
| `LobbyDB` is now a schema-only clone of the image database (`DBCC CLONEDATABASE`, then read-write). 37 tables, 178 procedures, 75 indexes, 0 rows. The old copy is `LobbyDB_recreated`. | container `bns-mssql` |
| The image database has rows in `GameWorld` (1), `GameWorldCluster` (1), `RandomStoreInitializationSchedule` (1) and account tables. The clone has none. ❓ Seed rows that `lobbyd` needs. | `LobbyDB_bnsx` |
| The 5 tests of `tests/mssql.rs` pass on the clone. | `cargo test -p lobbydaemon --features mssql --test mssql` |

`tools/run_play.sh` builds `lobbyd` with feature `mssql`. It reads the connection string from `secrets/lobbyd.mssql.url` inside the service shell. The string is not on a command line. Without the file, `lobbyd` keeps characters in memory.

Stages 2 to 4, code:

| Part | File |
| --- | --- |
| Link messages 1, 2, 3, 4, 5, 6, 7, 20, 21, 22 | `Program/Server/Packet/src/gamed_link.rs` |
| `lobbyd` listener and `LinkGameDaemon` (option `--world-port`, 21800 as in the original `lobbyd.config.xml`) | `Program/Server/LobbyDaemon/src/gamed_link.rs` |
| `gamedaemon` link client (option `--lobby`, `--world <cluster>,<world>`) | `Program/Server/GameDaemon/src/link.rs` |
| `BlGame` create and load | `Program/Server/GameDaemon/src/db.rs` |
| Test of both sides on loopback (with `GAMED_MSSQL_URL`: real create, load, delete of PC 900002) | `Program/Server/GameDaemon/tests/link.rs` |

```text
client ─ L_CreateCharacterReq ─► lobbyd ─ LobbyDB (name, slot, PC id)
                                   │ 20 (PC id, account, name, make-PC record)
                                   ▼
                               gamedaemon ─ BlGame.dbo.usp_CreateCharacter
                                   │ 21 (job, level, showcase) or 22 (reason)
                                   ▼
client ─ command 23 (key, token) ─► gamedaemon ─ 5 ─► lobbyd (consume world token) ─ 6 (PC id) or 7
client ─ command 31 ─► gamedaemon ─ BlGame.dbo.usp_LoadCreatureProperty ─► 32, 41
```

Facts of stages 2 and 3 (GameDaemon build 381 unless a different build is given):

| Fact | Source |
| --- | --- |
| `BlGame` is a schema-only clone of the image database: 107 tables, 476 procedures, 153 indexes. `Configuration` has the 2 image rows (`WorldClusterID 1`, `WorldID 1`); trigger `utr_InstanceIdReseed` was off for that insert. The old copy is `BlGame_recreated`. | `BlGame.mdf` of the BNSX image, attached as `BlGame_bnsx` |
| Create handler `HandleLobbyGame_CreateCharacterReq`. Command 20 body `qqobisbQh`: lobby transaction, PC id, game account id, blob (max 64 bytes ❓ external account data), `i` (❓ client IPv4), name (33 units), make-PC record (max 256 bytes), is-jumping, top level. The world id comes from the channel. | `0x14013F0F0`, format `0x14099D280` |
| Failure reasons of the create: 25 (race > 11, job > 19, sex > 3, job not creatable, no race-sex-job record, no jumping data); 2 (no init zone, no pc data, send to Cached failed). Cached reasons map 2/13 → 2, 3 → 3, 10 → 4, 11 → 26, 12 → 25, other → 1. | `CreateCharacterTran` dispatcher `0x14006F1B0`, init `0x14006F3D0` |
| The original sends the create to CacheDaemon through CacheGate. It does not call SQL. CacheDaemon calls `usp_CreateCharacter`. | `0x140070580` → `0x140323AE0`, log "tryCreateCharacter send request to Cached" |
| Reply 21 `hqhhb`: transaction, job, level, showcase record. | `0x140149920`, format `0x1409A8D88`, showcase builder `0x1400BB110` |
| `usp_CreateCharacter` has 453 parameters. The groups `Quest1`, `Quest2`, `Costume`, `Weapon` are optional (all NULL = no row). A create with defaults makes rows in 11 tables: CreatureProperty, ActivatedTeleport, ClientOption (2), DiscoveryArea, QuestList, RandomStore (2), SkillPage, SkillSkinProperty, SlateProperty (2), EnergyProperty, MentoringProperty. | `BlGame`, live probe (rows removed) |
| `usp_CreateCharacter` and `usp_LoadCreatureProperty` fail inside an open transaction ("invalid initial transaction"). | procedure text |
| Option-type keys of the 2 `ClientOption` rows and store numbers of the 2 `RandomStore` rows must differ (primary keys). The port uses 1 and 2. ❓ Original values. | `PK_ClientOption`, live probe |

Not parity (❓ local):

| Item | Reason |
| --- | --- |
| `gamedaemon` calls `BlGame` directly | CacheDaemon is not ported |
| Start values: only zone 1101, position, yaw, level 1, hp 303 are set. All other values are 0 or empty | `PcInitialSetting` (table 2019) and pc data (table 2007) are not read |
| No appearance range checks (tables 687, 675), no creatable-job check, no jumping character | Not ported |
| Link packet version 83 (build 348) | The build-381 value is not verified |
| Reply 6 uses the build-307 field list with empty lists | The build-381 list is not verified; `hqqqiCQiqqihcccc;qiqCihqq;Chq;` at `0x1409A90A8` is a related 381 string |
| Reply 24 with a reason in the last field on failure | The failure form is not verified |
| Delete, restore, eligibility, admission wait: no link message | Not ported |

`tools/run_play.sh` gives `gamedaemon` the connection string from `secrets/gamed.mssql.url` in the same way as `lobbyd`. Characters made before this change are not in `BlGame`; make a new character.
