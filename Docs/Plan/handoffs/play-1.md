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
