# 14 — Jobs, batch A2: one-request jobs (NP and account inventory)

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`.

## 1. Common form

```text
state 0 --start task--> init --ok--> state 1 --task 0--> request (send + transaction with expiry task)
state 1 --response task--> response --> fin (2)
state 1 --expiry task-->  expire   --> fin (2)
```

An NP response is OK when `status == 200`, or `status == 400` and the result text equals `Success` (no case). Else the NP error code (param +48) selects the reply code. The default reply code is 1.

## 2. NP jobs started by GameDaemon

Reply target: `GamedChannel::Send…Fail(gametid, [pcid], code)`. Required session state: 4.0 (in world).

| Job | Start | Resp | Expire | NP request | Wrong state | Send fails | Null resp | Expire code | NP error → code |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SecondPasswordGetStatusJob | 129 | 62 | 63 | second password status ❓ (vtable folded with IsVerified) | 6 | 7 | 2 | 7 | 3333 → 67 |
| SecondPasswordIsVerifiedJob | 130 | 64 | 65 | RequestSecondPasswordIsVerified | 6 (sent with `SendSecondPasswordGetStatusFail`: original bug, copy it) | 7 | 2 | 7 | 3333 → 68, 3334 → 69, 3345 → 70, 3346 → 71 |
| StartChargeJob | 120 | 54 | 55 | `SendRequestChargeStart(userid, ipv4, wid, ❓)` | 6 | 2 | 2 | 2 | 4100 → 36, 4101 → 37, 4102 → 38, 4103 → 39, 4107 → 40, 4109 → 41 |
| StopChargeJob | 121 | 56 | 57 | `SendRequestChargeStop(userid, ipv4, chargeKey)` | 42 (session has no charge key) | 2 | 2 | 2 | 4100 → 43, 4104 → 44 |
| AbandonChargeJob | 122 | 58 | 59 | `SendRequestChargeAbandon(userid, chargeKey)` | no reply | no reply | no reply | — | — |
| AuctionBidJob | 135 | 85 | 86 | auction bid ❓ (vtable folded) | 6; service off (`enableAuctionEscrowService`) → 7 | 2 | 2 | ❓ | 10 → 4, 4903 → 84, 4904 → 85, 5500 → 76, 5502..5507 → 77..82, 5513 → 83, 5589 → 86 |

Notes:

- SecondPasswordGetStatus OK reply: `Ok(gametid, pcid, param+96 == 1, param+100, …)` ❓ rest of the field list.
- SecondPassword inputs: gamed, gametid, pcid, type (byte), and for IsVerified an action index (byte).
- StartCharge OK: if the session is still 4.0, store the charge key (param +120, text) and send `StartChargeOk(gametid, chargeKey, param+252, (param+256 != 1) + 1, param+260, …)` ❓. If the session left the world: send `RequestChargeStop` with no transaction, then Fail 6.
- StopCharge `init`: a changed charge key gives the log "charge-key changed" and clears the session key ❓ exact rule.
- AbandonCharge: on a failed or expired abandon the job postpones a KickJob (tag 9 object) ❓ parameters.
- AbandonCharge request sets expiry task 57 in the pseudocode, not 59 ❓ check the assembly (possible original bug).

## 3. Account inventory jobs started by LobbyGate

Reply target: `LobbyGateChannel::Send…Fail(clikey, code)`. Required session state: 2.0 (lobby). The job owns one transaction object.

| Job | Start | Ok | Fail | Timeout | Request | Wrong state | Channel absent | Unsent | Bad ack | Timeout code | Fail ack → code |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GetGoodsCountJob | 19 | 144 | 145 | 146 | `SendGetGoodsCountReq(account id)` | 5 | 7 | 7 | 2 | 2 | ❓ one value → 7, else 2 |
| ListGoodsJob | 20 | 147 | 148 | 149 | `SendListGoodsReq(account id, language code)` | 5 | 7 | 7 | 2 | 2 | ❓ one value → 7, else 2 |
| TakeGoodsJob | 21 | 150 | 151 | 152 | `SendTakeGoodsReq(account id, goods id)` | 5; bad parameter → 6 | 7 | 7 | 2 | 2 | 3, 5 → 7; 15..19 → 166..170; else 2 |

OK replies: `GetGoodsCountOk(clikey, count)`, `ListGoodsOk(clikey, list, count)`, `TakeGoodsOk(clikey, goods id, list, item count)`.
