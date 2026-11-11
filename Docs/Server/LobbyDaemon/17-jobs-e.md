# 17 — Jobs, batch B2: rename, appearance, sex, race (account-inventory item services)

Source: build 348 (PDB). All facts are ❓ for build 307 until the bodies are compared. Index: `12-job-map.md`. Terms: `16-jobs-d.md`.

The 4 jobs have one skeleton. Each job uses one account-inventory item to change one character through gamed.

## 1. Skeleton

```text
0 init ─► 1 check item ─► 2 register service entry ─► 3 try consume ─► 4 gamed request ─► 5 finish consume ─► 6 finish service entry ─► 7 fin
              AccInv                (local)                AccInv            gamed              AccInv              (local, DB)
```

| State | Task 0 (send) | Ok | Fail | Expire | Call |
| --- | --- | --- | --- | --- | --- |
| 1 | request | 156 | 157 | 158 | `AccountInventoryChannel::SendRetrieveSingleWarehouseItemReq(tran, item instance id, account id, 0)` |
| 2 | `registerServiceEntry` | — | — | — | `<Service>::Start(…)`, add to the session lobby services |
| 3 | request | 159 | 160 | 161 | `SendTryWarehouseItemConsumptionReq(tran, label id, item instance id, account id)`; Ok → `<Service>::ConsumeItemTried` |
| 4 | request | job | job | job | `GamedChannel::Send<Change>Req(tran, service id, pcid, …)`; Ok → `<Service>::TaskDone` |
| 5 | request | 162 | 163 | 164 | `SendFinishWarehouseItemConsumptionReq(tran, label id, item instance id, account id, …)`; Ok → `<Service>::ConsumeItemFinished` |
| 6 | `finServiceEntry` | — | — | — | `<Service>::Finish`, then the Ok reply |

Each failure sends `Send<X>WithAccountInventoryItemFail(clikey, slot id, label id, item instance id, code)` and moves to state 7.

## 2. Per job

| Job | Start task | Gamed Ok / Fail / Expire | Service class | Item type | Code base B |
| --- | --- | --- | --- | --- | --- |
| ChangeCharacterAppearanceJob | 24 | 123 / 124 / 125 | `CharacterCosmesisService` | `SERVICEITEMTYPE_CHANGE_CHARACTER_APPEARANCE` | 172 |
| RenameCharacterJob | 25 | 126 / 127 / 128 | `RenameCharacterService` | `…RENAME_CHARACTER` or `…RENAME_CHARACTER_BY_SYSTEM` | 193 |
| ChangeCharacterSexJob | 31 | 136 / 137 / 138 | `ChangeCharacterSexService` | `…CHANGE_CHARACTER_SEX` | 179 |
| ChangeCharacterRaceJob | 32 | 139 / 140 / 141 | `ChangeCharacterRaceService` | `…CHANGE_CHARACTER_RACE` | 186 |

## 3. Reply codes

| Code | Step | Rule |
| --- | --- | --- |
| no reply | init | Bad input: channel, slot id, label id, item instance id must not be 0 |
| 7 | init | Account-inventory channel absent. Original bug ❓: `RenameCharacterJob` sends this reply with `SendChangeCharacterAppearanceWithAccountInventoryItemFail` |
| B+4 | init | `LobbyDaemon::CheckServiceItemDataId(item type, item data id)` false |
| 5 | init | Session not 2.0 |
| B+0 | init, register | No session character with the slot id |
| B+1 | init, register | `Character::IsPlayable` false. In register: `block` set, or state ≠ 2, or status bit 0, or `in_service` |
| 9 | init, register | `queryLobbyService(session, character)` ≠ none |
| B+5 (rename: 202) | init | World refused ❓ (log line empty) |
| 2 | init | World / gamed channel not found |
| 7 | state 1 | Request unsent, or expire |
| 2 | state 1 Ok | No parameter |
| B+3 | state 1 Ok | Item not usable |
| B+4 | state 1 Ok | Item data id ≠ the input item data id |
| B+2 | state 1 Fail | One ack value ❓; other values → 2 |
| 3 or 2 ❓ | register | `<Service>::Start` fails or the add fails (rename: §4) |
| B+6 (rename: 203) | states 3–6 | Each failure after the service entry exists: unsent, bad ack, Fail, expire, service step fails |
| 2 | state 6 | Last step fails after the Ok reply ❓ |

Ok reply: `Send<X>WithAccountInventoryItemOk(clikey, slot id, …, label id, …)`. The appearance, sex and race jobs also call `LobbyDB::UpdateCharacterData` and `Character::SetShowcase` in state 4 Ok or state 6 ❓.

## 4. RenameCharacterJob differences

Input: channel (+0), slot id (+8), new name (+24, UTF-16, max 33 units with the zero), label id (+96), item instance id (+104), item data id (+224).

| Step | Rule | Code |
| --- | --- | --- |
| init | `_tencentUICEnable` on and no external account key | 2 |
| init | Tencent name filter refuses | 199 |
| init | `RuleSet::CheckName` fails (same config keys as `16-jobs-d.md` §1.3) | 198; forbidden → 199 |
| init | `LobbyDB::CheckValidityOfCharacterName(name, multilingual ❓)` fails | DB return 1 → 6, 2 → 200, 3 → 201 |
| register | `RenameCharacterService::Start(session, item data id, new name, _characterNameReuseLimitDuration)` fails | 201, 200 or 2 by the error ❓ |
| register | Add of the service fails | 2 |
| register | `Character::SetName(new name)` fails | 2 |
| state 4 | `GamedChannel::SendRenameCharacterReq(tran, service id, pcid, old name, new name)` | — |
| state 6 | `_enableFriendMessageNotification` on → `sendFriendMessageToNP`: NP `SendRequestFriendSendMessage(account, _friendMessageRecipientAppIdList, …)`. A send failure is a log only | — |

Ok reply: `SendRenameCharacterWithAccountInventoryItemOk(clikey, slot id, new name, label id, …)`.

Note: the DB return map here (1 → 6, 2 → 200, 3 → 201) gives the probable map for create: 2 → 124, 3 → 125 (`16-jobs-d.md` §1.3 rule 11).
