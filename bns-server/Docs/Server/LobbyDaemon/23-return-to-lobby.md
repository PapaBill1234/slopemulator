# 23 — Return to lobby (GameDaemon 381 and LobbyDaemon 307)

GameDaemon detail: `Docs/Server/GameDaemon/10-session-handlers.md` §26. Addresses: GameDaemon 381 unless "348"; LobbyDaemon 307.

## 1. Sequence

```
Client            GameDaemon                         LobbyDaemon                  LobbyGate
  |-- G 26 ------->| 0x140371500 LogoutUser, tran start
  |                | 1 stop charge -- L33 hqqqs ------->| (41/42)
  |                | 2 World::leave (wait event 48)
  |                | 3 L UpdateCharacterData ---------->| (43/44)
  |                | 4 time-based reward (opt, event 47)
  |                | 5 L42 UpdateAccountData --------->| (45/46)
  |                | 6 Cached 35 hqq unload (37/38)
  |                | 7 L8 hqqq (tid,pcDbId,cliKey) --->| 0x140088470 → job 119
  |                |<-- L9 hqq (tid, token key) -------| state 5.1, token type 2
  |<-- G 27 hq ----| (token key)       (or L10 hqE → G 28 hh)
  |-- L6 LoginWithLobbyTokenReq (cli_key, lobby_token) ------------------------------->|
  |                                                    |<-- 12 InitSessionWithLobbyTokenReq hqqqi
  |                                                    | Verify(type 2) → job 14 → state 5.2
```

## 2. Messages

| Msg | Dir | Format | Fields | Source |
| --- | --- | --- | --- | --- |
| G 26 ReturnToLobbyReq | C→G | `h` | none | `0x140371500` |
| G 27 ReturnToLobbyOk | G→C | `hq` | lobby token key | `0x14013AC70` state 7 event 31; client `packet-layouts.tsv` row 27 (`lobby_token` +16) |
| G 28 ReturnToLobbyFail | G→C | `hh` | reason: 5, 1954, 2, 1952, 1 | §26 |
| L 33 StopCharge | G→L | `hqqqs` | tid, pcDbId, cliKey, charge key | `0x140138FA0`; 348 `0x14013A8D0` |
| L 8 LeaveWorldReq | G→L | `hqqq` | game tid, pcDbId, cliKey | send `0x14013AEFC`; decode `0x140088470` |
| L 9 LeaveWorldOk | L→G | `hqq` | game tid, token key | `0x140086980` line 131; G decode `0x14013EB10` |
| L 10 LeaveWorldFail | L→G | `hqE` | game tid, reason | G decode `0x14013EBF0` |
| L 12 LogoutNotify | G→L | `hqq` | pcDbId, cliKey | LogoutTran 381 `0x14013B40C` |
| LG 6 LoginWithLobbyTokenReq | C→LG | | cli_key i64 +16, lobby_token i64 +24 | `Docs/Client/RE/lobby-commands.tsv` row 6 |
| LG→L 12 InitSessionWithLobbyTokenReq | LG→L | `hqqqi` | gate key, cli key, token key, IPv4 | `0x1400BEFC0` line 48 |

## 3. LobbyDaemon 307 checks

- **LeaveWorldReq handler** `0x140088470`: decode `hqqq`. Select the session by pcid (`CliSessionTable::Select_index_pcid` `0x14003D710`). No session → `LeaveWorldFail` 5 (line 142). Client key differs from session +32 → Fail 5 (line 124). Job init failure → **7 EnterWorldFail** reason 2 (line 82); job start failure → **7** reason 4 (line 101). Quirk `quirk_leave_fail_as_enter_fail`. Start task 119. ✅ confirms `13-jobs-a.md` §3.6 entry.
- **Job** `LeaveGameWorldJob::leaveGameWorld` `0x140086980`: `Character::EndGameWorldPlay` (`0x140031930`); construct `LobbyToken(session)` `0x1400936A0` with vtable `0x14018BE28` (type function `0x140104860` returns 2); `LobbyTokenDesk::Issue` `0x140093D20`. Ok → store token key (token +48) at job +112, `SendLeaveWorldOk(channel, tid, key)`, transit 2. Fail → `SendLeaveWorldFail(…, 2)`, `closePresence`, `finalize`.
- **Issue** `0x140093D20`: key from generator at desk +64 (`GenerateLobbyTokenkey` `0x140096100`); deadline token +64 = tick − start tick + desk +56 (lifetime, msec). Add to list +104 and map.
- **Lifetime config:** key `lobby-token-life-time`, description "msec. life time of lobby token", config table row at `0x1401FD200` (config offset 0x100). ❓ Default value and the copy from config +0x100 into desk +56.
- **Expiry:** `LobbyTokenDesk::Tick` `0x140094140` removes expired tokens; `LobbyToken::Expire` `0x140093A50` postpones a session job from pool `0x1401FC340` (task 1, field +96 = 8). The same pool is used by `HandleLobbyManage_KickReq`. ❓ Job class (probably kick/logout of the session).
- **Verify** `LobbyTokenDesk::Verify` `0x140093EB0(type, key, cliKey, ip, &expired)`: zero argument → null. Find token by key. `LobbyToken::Verify` `0x140093780`: session not null; type equal (InitSession passes 2); key equal token +48; cliKey equal session +32; IPv4 equal session +40 — mismatch rejects when config (+1864)+264 is set; ❓ behaviour when not set. On success remove the token from the desk (single use).
- **InitSessionWithLobbyTokenReq** `0x1400BEFC0`: Verify null and expired → Fail 19; null otherwise → Fail 20; other errors → Fail 2. Ok → `InitCliSessionWlobbyTokenJob` task 14 (`13-jobs-a.md` §3.2).

## 4. Client

- 27 handler `0xF1ECE0` (`HandleReturnToLobbyOkPacket`); field `lobby_token` i64 at +16 (`10-quests-commands.tsv` row 27).
- The client reconnects to LobbyGate and sends LG 6 with `cli_key` and `lobby_token` (`04-lobbygate.md` lines 8, 20, 147). ❓ Which address it connects to (stored LobbyGate address, `04-lobbygate.md` line 306 is open).

## 5. Open ❓

- GameDaemon `failProcess` trigger and global expire lifetime.
- Lobby token lifetime default. The config row `0x1401FD200` holds name `lobby-token-life-time`, flags, and a pointer `0x140128D3C` that is not a string or code; the default is not in the row. Use the value from `lobbyd.config.xml` (not in the worktree).
- `LogoutUser` waiting-list return value.
- Checked: UpdateCharacterData from 381 `0x14014A700` is Lobby 39, format `hqqqhb` (line 50).
- Who closes the GameDaemon socket after 27.
