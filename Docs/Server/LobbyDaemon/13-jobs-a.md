# 13 — Jobs, batch A: framework and small session jobs

Source: build 348 (PDB). Addresses are 348 addresses. All facts are ❓ for build 307 until the bodies are compared.
Index: `12-job-map.md`.

## 1. Job framework

| Fact | Source |
| --- | --- |
| A job has `_id`, `_state` (starts 0), `_lastState`, a lock, and a definition with `type` and `fin` (the final state). | `LobbyJob`, `0x1400C1710` |
| `transit(state)` fails if `state <= 0`, `state > fin`, or `state < _state`. A move back is refused. | `0x1400C1710` |
| `execute(task, param)`: if `_state == fin`, return. Call `work`. If `work` returns false and the job is not final, move to `fin`. A final job is removed from the manager. | `0x1400C18D0`, `0x1400C2020` |
| `resume(state, task)`: runs `work(task, null)` only if `_state == state` and the job is not final. Used by transaction expiry. | `0x1400C1990`, `0x14007B7A0` |
| `CliSessionJob::dispatch_work` holds the owner session lock during `work`. | `0x14003C010` |
| `LobbyJobManager::Register` gives the job a new id and puts it in the map `id % hashCount`. | `0x1400C1C90` |
| `Start` = `Register` + `Execute` in the same thread. `Postpone` = `Register` + a queued event that calls `Execute` later. | `0x1400C1ED0`, `0x1400C1F30` |
| `ExecuteW(jobid, task, param)` finds the job by id. A missing job gives the log "no such job" and no reply. | `0x1400C20A0` |
| A transaction stores (job id, state at send time, expiry task). On expiry it calls `ResumeW`. | `0x14007B7E0` |
| The usual `work` form: state 0 + start task → `init`; then task 0 falls into the next state function in the same call. | all jobs |

## 2. Session states

`CliSession._state` is `major << 16 | minor`.

| Value | Hex | Name (from log text) |
| --- | --- | --- |
| 65537 | 0x10001 | 1.1 anonymous (NP token init done) |
| 65538, 65539 | 0x10002, 0x10003 | 1.2, 1.3 start-session steps ❓ |
| 131072 | 0x20000 | 2.0 lobby (character select) |
| 196609 | 0x30001 | 3.1 lobby-to-world, try ❓ |
| 196610 | 0x30002 | 3.2 L2W wait (queue) |
| 196611 | 0x30003 | 3.3 L2W ok (enter token issued) |
| 262144 | 0x40000 | 4.0 in world |
| 327681 | 0x50001 | 5.1 world-to-lobby, leave token issued |
| 327682 | 0x50002 | 5.2 W2L, anonymous presence set |
| 327683 | 0x50003 | 5.3 ❓ |
| 393216 | 0x60000 | 6.0 final |

## 3. Jobs

Reply codes are the last argument of the named `Send…Fail` function.

### 3.1 InitCliSessionWnpTokenJob (task 7, fin 1) — `0x1400981C0`

| Step | Rule |
| --- | --- |
| 1 | Parameter: gate channel, gate key, NP token (id128), portal account id (id128), IPv4. A null channel, zero gate key or zero token ends the job with no reply. |
| 2 | Copy the mobile flag from the gate channel (+33) to the session. Set state 1.1. Store IPv4. |
| 3 | `CliSessionTable::Insert` fails → `InitSessionWithNPTokenFail(gatekey, 6)`. |
| 4 | Set an anonymous lobby-gate presence (channel, gate key). Failure → state 6.0, `finalize`, Fail 6. |
| 5 | `InitSessionWithNPTokenOk(gatekey, clikey, token, portal account id)`. Transit 1. |

### 3.2 InitCliSessionWlobbyTokenJob (task 14, fin 2) — `0x140054AE0`, `0x140054C40`

| Step | Rule |
| --- | --- |
| 1 | Session state must be 5.1. If not: `InitSessionWithLobbyTokenFail(gatekey, 5)`. |
| 2 | Close the old presence. Set a new anonymous presence (channel, gate key). |
| 3 | Presence failure → state 6.0, `finalize`, Fail 2. |
| 4 | `CliSession::leave`. State 5.2. `InitSessionWithLobbyTokenOk(gatekey, clikey)`. |

### 3.3 LogoutJob (task 9, fin 2) — `0x140097BF0`, `0x140097D30`

Inputs: gate channel (can be null), flag `close`. `init` stores the session state as `last`. If `last` is 6.0 the job ends.

| `last` | Action |
| --- | --- |
| 1.1 | If `close`: close presence. State 6.0. `finalize`. No notify. |
| 1.2, 1.3, 2.0, 3.1, 5.3 | State 6.0. `finalize`. If gate channel and `close`: `SendCloseSessionNotify(clikey)`. |
| 3.2 | State 6.0. `leave`. `finalize`. Notify as above. |
| 3.3 | No state change (log "absorb logout by L2W_ok state"). Notify as above. |
| 5.2 | If `close`: close presence. State 6.0. `finalize`. No notify. |
| 4.0, 5.1 | Log "invalid state". No change. ❓ 5.1 read from pseudocode layout; check the assembly. |
| other | Crash log "unknown state". No change. |

### 3.4 GiveupWaitGameWorldJob (task 16, fin 2) — `0x140052F70`, `0x1400530D0`

| Step | Rule |
| --- | --- |
| 1 | State is not 3.2 → `GiveupWaitFail(clikey, 5)`. |
| 2 | Service location is null or not a game world → Fail 2. |
| 3 | `leave`. State 2.0. `GiveupWaitOk(clikey)`. |

### 3.5 EndWaitGameWorldJob (task 3, fin 2) — `0x14004B150`, `0x14004B460`

Started by the world queue, not by a packet.

| Step | Rule |
| --- | --- |
| 1 | State is not 3.2 → end, no change. |
| 2 | Presence is null, or its kind is not 2 (lobby gate), or the gate channel does not exist → state 6.0, `finalize`. |
| 3 | Service location is null or not a game world → state 6.0, `finalize`. |
| 4 | Set the wait position of the location to −1. |
| 5 | Issue an `EnterGameWorldToken` (world id, location data, session). Failure → state 6.0, `finalize`. |
| 6 | State 3.3. `SendEndWaitNotify(clikey, token key)`. |

### 3.6 LeaveGameWorldJob (task 119, fin 2) — `0x140087A40`, `0x140087E90`

Inputs: gamed channel, game transaction id, pcid. Any zero input ends the job.

| Step | Rule |
| --- | --- |
| 1 | State is not 4.0 → `LeaveWorldFail(gametid, 6)`. |
| 2 | No playing character → Fail 21. Playing pcid differs → Fail 23. Character world differs from the channel id → Fail 22. |
| 3 | `Character::EndGameWorldPlay`. Call `OnLeave` of the service location. |
| 4 | Issue a `LeaveGameWorldToken`. Failure → Fail 2, state 6.0, close presence, `finalize`. |
| 5 | State 5.1. `LeaveWorldOk(gametid, token key)`. |

Checked (lp2-3): handler `HandleLobbyGame_LeaveWorldReq` `0x140088470` — no session by pcid → Fail 5 (line 142); client key differs from session +32 → Fail 5 (line 124); job init failure → **7** `EnterWorldFail` reason 2 (line 82); job start failure → **7** reason 4 (line 101). Job body `0x140086980` confirms steps 3–5 (token type 2, key from token +48). End-to-end: `23-return-to-lobby.md`.

### 3.7 LogoutInGameWorldJob (task 118, fin 2) — `0x14005ACC0`, `0x14005AF30`

Input: world id (not 0).

| Step | Rule |
| --- | --- |
| 1 | State is not 4.0, or no playing character, or the character world differs → end, no change. |
| 2 | State 6.0. `EndGameWorldPlay`. `OnLeave` of the location. `finalize`. |
