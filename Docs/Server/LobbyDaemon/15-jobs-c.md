# 15 — Jobs, batch A3: charge info, service items, auction, mileage, grade, slot bind

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`. Common form: `14-jobs-b.md` §1.

## 1. LobbyGate jobs (session state 2.0 required)

| Job | Start | Ok / resp | Fail | Expire | Request | Wrong state | Channel absent | Unsent | Bad ack | Expire code | Error map |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GetChargeInfoJob | 17 | 52 | — | 53 | NP `SendRequestChargeGetInfo(userid, ipv4)` | no reply | 2 | 2 | 2 | 2 | NP failure → 157 |
| ListLobbyServiceItemJob | 22 | 153 | 154 | 155 | AccInv `SendRetrieveLobbyServiceItemListReq(account id, searchType, searchKey)` | 5 | 7 | 7 | 2 | 2 | ❓ one value → 7, else 2 |
| GetLobbyServiceItemJob | 23 | 156 | 157 | 158 | AccInv `SendRetrieveSingleWarehouseItemReq(item instance id, account id, 0)` | 5 | 7 | 7 | 2 | 2 | ❓ → 7, ❓ → 171, else 2 |

- The service item Fail replies carry the search type (list) or the item instance id (get) before the code.
- `GetChargeInfoOk(clikey, baseType, a, b, c, d, e, f, g, h)`: the job maps NP enum values. Base type: 2 values (else crash log "unknown charge base type"). One field maps 1..4 → 1..4. One maps 1..3 → 1..3. One maps 1 → 1, 2 and 3 → 2, 4 → 3, 5 → 4, 6 → 5. ❓ Field names and order: read `0x…responseChargeGetInfo` in full before the Rust work.

## 2. BindCharacterSlotJob (task 33, NP resp 37, expire 38, fin 3)

Input: gate channel, slot id (id128, not 0), pcid (not 0). A bad input ends the job with no reply.

| Step | Rule | Fail code |
| --- | --- | --- |
| init | Session not 2.0 | 5 |
| init | Character not in the session list | 164 |
| init | Character already has a slot | 165 |
| state 1 | NP `SendRequestSlotGetSlot(account id, slot id)`. Unsent or no channel | 2 |
| resp | Session not 2.0 | 5 |
| resp | Null response | 2 |
| resp | NP failure | 163 |
| resp | Slot id in the response (+96, +104) differs | 2 |
| resp | Slot type is not valid ❓ rule | 162 |
| expire | — | 2 |
| state 2 | `bindCharacterSlot`: session not 2.0 | 5 |
| state 2 | DB bind ❓ (procedure not in the grep view). Results map to 6, 164, 1, 3 ❓ | — |
| ok | Send lobby-gate command 58, format `hqoq`: clikey, slot id, pcid | — |

## 3. ActivateGradeJob (tasks 28 from LobbyGate, ❓ from GameDaemon; resp 80, expire 81)

| Fact | Value |
| --- | --- |
| Lobby start | Inputs gate channel, game grade key, grade id. Session must be 2.0, else code 5. |
| Game start | Inputs gamed channel, game tid, pcid, grade values. Session must be 4.0, else code 6. |
| Config `useGrade` off | Code 2 on both sides. |
| Request | NP `SendRequestGradeActivateGrade(userid, gradeId, gameGradeKey)`. Unsent, no channel, null response, expire → code 2. |
| NP error → internal code | 10 → 3, 5603 → 4, 5604 → 5, else 1 ❓ (0, 1 and 2 sources not seen). |
| `sendFail(internal)` to LobbyGate | 1 → 2, 2 → 5, 3 → 6, 4 → 232, 5 → 233, else 1. |
| `sendFail(internal)` to GameDaemon | 1 → 2, 2 → 6, 3 → 4, 4 → 87, 5 → 88, else 1. |
| OK | `ActivateGradeOk(key, _wtoi(text at +134), dword +112, qword +240, qword +248)`. Lobby form: clikey first. Game form: game tid, pcid first. |

## 4. Auction jobs (started by NP requests)

Replies go to NP: `SendResponseGameAuction…Fail(txnId, text, code, id)`.

### 4.1 AuctionEscrowJob (task 83; gamed ok 131, fail 132, expire 133)

| Step | Rule | NP code |
| --- | --- | --- |
| 1 | Config `enableAuctionEscrowService` off | 5580 |
| 2 | Session not 4.0 | 5582 |
| 3 | Item key differs from config `auctionEscrowMoneyItemKey` | 5583 |
| 4 | Quantity (qword +176) not valid | 5584 |
| 5 | `LobbyDB::TryAuctionEscrow(requestId, accountId, wid, pcid, amount) → escrowId` fails | 48 or ❓ |
| 6 | Transit 1. `GamedChannel::SendAuctionEscrowReq(pcid, amount, requestId, escrowId)` with expire task 133 | — |
| ok | `LobbyDB::FinishAuctionEscrow`. OK → `ResponseGameAuctionEscrowOk(txnId, escrowId)`. Failure → `ErrAuctionGameUnknownResult` 5587 | — |
| fail | `LobbyDB::CancelAuctionEscrow`. Then the gamed reason maps to 5582, 5584, 5585, 5588 or 48 ❓ keys. Cancel failure → log only ❓ | — |
| expire | `ErrAuctionGameUnknownResult` 5587 | — |

### 4.2 AuctionRollbackEscrowJob (task 84; post ok 171, fail 172, expire 173)

| Step | Rule |
| --- | --- |
| 1 | Service off or unknown world → `ErrAuctionGameServiceUnavailable` 5580. |
| 2 | `LobbyDB::TryAuctionEscrowRollback(requestId) → escrowId, deliveryId`. DB error → 48 or 10 ❓. |
| 3 | Build the post message. Failure → 10 or 48 ❓. |
| 4 | Transit 1. `PostOfficeChannel::SendRegisterPostReq(deliveryId, 5, sender (config world id), recipient, message)`. |
| ok | `LobbyDB::FinishAuctionEscrowRollback`. OK → `ResponseGameAuctionRollbackEscrowOk(txnId)`. Failure → `ErrInternal` 48. |
| fail | Post reason is not 12 → `ErrInternal` 48. Reason 12 (duplicate) → finish as in "ok". |
| expire | An NP message with no transaction ❓ (vtable folded). |

### 4.3 AuctionDeliverJob (task 82; post ok 171, fail 172, timeout 173)

| Step | Rule |
| --- | --- |
| 1 | Service off → 5580. Item key is not the money key → 5583. Negative quantity → `ErrAuctionGameInvalidQuantity` 5584. |
| 2 | Event type → post type: SUCCESS → 2 or 4 ❓, EXPIRED → 1, CANCELED → 3. Other → `ErrBadParam` 10. |
| 3 | Post message failure → 10. |
| 4 | `LobbyDB::TryAuctionResultDelivery(...)`. Failure → 5582, `ErrAuctionGameDuplicateRequest` 5586, or 48 ❓ keys. |
| 5 | Transit 1. `SendRegisterPostReq`. Unsent → 5580. |
| ok | `LobbyDB::FinishAuctionResultDelivery`. OK → `ResponseGameAuctionDeliveryOk(txnId, postId)`. Failure → `ErrInternal` 48. |
| fail | Reason is not 12 → `ErrInternal` 48. Reason 12 → finish, then OK. |
| timeout | `ErrAuctionGameServiceUnavailable` 5580. |

## 5. DepositMileageJob (task 5; ack 91, expire 92)

| Fact | Value |
| --- | --- |
| Input (1,444 bytes ❓) | deposit type, request location, request type (bytes 0..2); request id (+8); user id (+16, id128); currency id (+32, u16); amount (+40); +48; +56, +60; reason text (+64, 201 chars); refundable (+265); notification (+266); notification name (+268, 65 wide chars); content (+398, 513 wide chars); pc DB id (+1424); request sub id (+1432); location hint (+1440); race, sex, job (+1441..+1443). |
| Callback | The job owns a callback object. It is called with (request id, user id, amount, currency id, external key, location hint) on results ❓ exact calls. |
| Request | One NP message with expire task 92 ❓ (vtable folded). "make id fail" has 3 crash paths. |
| Ack | OK rule of `14-jobs-b.md` §1. On OK and `notification` set: a second NP message with no transaction ❓. |
