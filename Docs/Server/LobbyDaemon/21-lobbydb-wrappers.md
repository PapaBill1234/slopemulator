# 21 — Batch F: `LobbyDB::*` wrappers

Source: build 348 (PDB). Procedure binding (parameters, result columns) of build 307: `db-binding.json`, `08-db-binding.md`. This file adds the wrapper names, the callers and the wrapper rule. All facts are ❓ for build 307 until the bodies are compared.

## 1. Totals

| Fact | Value |
| --- | --- |
| `LobbyDB::*` wrappers in build 348 | 94 |
| Wrappers in build 307 | 90 (`db-binding.json` has 87; it does not have `LoadGameWorldCluster`, `LoadGameWorld`, `LoadMileage`) |
| Wrappers only in build 348 | 4: `LoadGameAccountContentQuotaList`, `UpdateGameAccountContentQuota`, `LoadGameAccountRecycleTimeList`, `UpdateGameAccountRecycleTime` |
| Procedures in build 307 with no 348 wrapper | 0 |

Do not port the 4 wrappers of build 348. Source of this correction: `22-compare-307.md` §2.

## 2. Wrapper rule

All wrappers have the same form. Example read: `LobbyDB::FinishGuildRegistration` (`0x1400a6310`).

| Step | Rule | On failure |
| --- | --- | --- |
| 1 | Check the parameters (only the wrappers with "yes" in the column Check). | Log "invalid parameter". Return false. `dbrtn` is not written ❓. |
| 2 | `Db::AllocTransaction`. Timeout 10 s. | Return false. |
| 3 | `DbTransaction::Prepare("{?=CALL dbo.usp_…(?, …)}")`. | Release. Return false. |
| 4 | `*dbrtn = 0`. Bind parameter 1 as the return value (output, int). Bind the other parameters in order. | Release. Return false. |
| 5 | `DbTransaction::Execute`. | Release. Return false. |
| 6 | Wrappers with result columns: bind the columns and fetch the rows. | Release. Return false ❓. |
| 7 | `*dbrtn == 0` → release, return true. Else release, return false. The caller reads `dbrtn` for the reason. | — |

A caller cannot tell an ODBC failure (steps 2 to 5, `dbrtn` = 0 or not written) from a procedure refusal without the value of `dbrtn`. ❓ The caller maps from `dbrtn` to the wire code are in the caller functions. They are not read yet, except the paths in `db-binding.json` (`caller_error_maps_step8`).

Wrappers with a compare of `dbrtn` or a count to a value other than 0 and -1 ❓ meaning:

| Wrapper | Value |
| --- | --- |
| `LoadGameAccountTimeBasedPlayInfo`, `UpdateGameAccountTimeBasedPlayInfo`, `WriteUserRenameServiceLog`, `TryAuctionResultDelivery` | 2 |
| `LoadJumpingCharacterCreationQuota` | 1 |
| `LoadMileage`, `LoadMileageFeverTime` | 86400 (seconds in a day) |
| `LoadMileageAccumulatedTimePerDay` | 10 |

## 3. Wrapper table

Procedure = `usp_` + wrapper name, unless the column Procedure gives another name. Cols = count of bind-column and fetch calls in the body (0 = no result set). `CS` = `CliSession`, `Ch` = `Character`.

| Wrapper | Procedure | Check | Cols | Callers |
| --- | --- | --- | --- | --- |
| `LoadGameAccount` |  | yes | 9 | `CS::login` |
| `LoadGameAccountTimeBasedPlayInfo` |  | yes | 12 | `CS::loadDB` |
| `UpdateGameAccountTimeBasedPlayInfo` |  | yes | 0 | `CS::UpdateAccountData` |
| `RegisterGameAccount` |  | yes | 0 | `CS::login` |
| `UpdateLastLogin` |  | yes | 0 | `CS::login` |
| `UpdateLastLogout` |  | yes | 0 | `CS::logout` |
| `LoadCharacterList` |  | yes | 30 | `CS::loadDB` |
| `LoadGameWorldCluster` (348 only ❓) |  |  | 16 | `GameWorldRegistry::Setup`, `GameWorldRegistry::Update` |
| `LoadGameWorld` (348 only ❓) |  |  | 26 | `GameWorldRegistry::Setup`, `GameWorldRegistry::Update` |
| `CheckValidityOfCharacterName` |  |  | 0 | `CreateCharacterJob::init`, `RenameCharacterJob::init` |
| `TryCharacterCreation` |  | yes | 0 | `Ch::TryCreation` |
| `FinishCharacterCreation` |  | yes | 0 | `Ch::FinishCreation` |
| `CancelCharacterCreation` |  |  | 0 | `Ch::CancelCreation` |
| `TryCharacterDeletion` |  |  | 0 | `Ch::TryDeletion` |
| `FinishCharacterDeletion` |  |  | 0 | `Ch::FinishDeletion` |
| `CancelCharacterDeletion` |  |  | 0 | `Ch::CancelDeletion` |
| `TryCharacterRestoration` |  |  | 0 | `Ch::TryRestore` |
| `FinishCharacterRestoration` |  |  | 0 | `Ch::FinishRestore` |
| `RetrieveExpiredDeletionWaitingList` |  |  | 8 | `LobbyCharacterDeletionScheduler::process` |
| `ExpireCharacterDeletionWaiting` |  |  | 0 | `LobbyCharacterDeletionScheduler::process` |
| `UpdateLastPlayStart` |  |  | 0 | `Ch::StartGameWorldPlay` |
| `UpdateLastPlayEnd` |  | yes | 0 | `Ch::EndGameWorldPlay` |
| `UpdateCharacterData` |  | yes | 0 | `ChangeCharacterAppearanceJob::changeCharacterAppearanceOk`, `ChangeCharacterRaceJob::changeCharacterRaceOk`, `ChangeCharacterSexJob::changeCharacterSexOk`, `CS::UpdateCharacterData` |
| `UpdateCharacterLevel` |  | yes | 0 | `CS::UpdateCharacterLevel` |
| `TryGuildRegistration` |  | yes | 0 | `HandleLobbyGame_TryRegisterGuildReq` |
| `FinishGuildRegistration` |  |  | 0 | `HandleLobbyGame_FinishRegisterGuildNotify` |
| `CancelGuildRegistration` |  |  | 0 | `HandleLobbyGame_CancelRegisterGuildNotify` |
| `DeleteGuild` |  |  | 0 | `HandleLobbyGame_DeleteGuildNotify` |
| `TryRenameGuild` |  | yes | 0 | `HandleLobbyGame_TryRenameGuildReq` |
| `FinishRenameGuild` |  | yes | 0 | `HandleLobbyGame_FinishRenameGuildNotify` |
| `CancelRenameGuild` |  | yes | 0 | `HandleLobbyGame_CancelRenameGuildNotify` |
| `LoadJumpingCharacterCreationQuota` |  | yes | 6 | `CS::login` |
| `BindCharacterSlot` |  | yes | 0 | `CS::BindCharacterSlot` |
| `UpdateJumpingCharacterStateComplete` | `usp_UpdateJumpingCharacterStateComplete` or `usp_UpdateJumpingCharacterStateCompleteWithoutQuota` |  | 0 | `Ch::UpgradeJumpingCharacterState` |
| `LoadGameAccounExpInfo` | `usp_LoadGameAccountExp` | yes | 10 | `CS::loadDB` |
| `RegisterGameAccountExpInfo` | `usp_InsertGameAccountExp` | yes | 0 | `CS::loadDB` |
| `UpdateGameAccountExpInfo` | `usp_UpdateGameAccountExp` | yes | 0 | `CS::UpdateAccountExp` |
| `LoadGameAccountContentQuotaList` (348 only ❓) | `usp_LoadGameAccountContentQuota` | yes | 12 | `CS::fetchAccountContentQuotaList` |
| `UpdateGameAccountContentQuota` (348 only ❓) |  | yes | 0 | `CS::UpdateAccountContentQuota` |
| `LoadGameAccountRecycleTimeList` (348 only ❓) | `usp_LoadGameAccountRecycleTime` | yes | 8 | `CS::fetchAccountRecycleTimeList` |
| `UpdateGameAccountRecycleTime` (348 only ❓) |  | yes | 0 | `CS::UpdateAccountRecycleTime` |
| `LoadRandomStoreInitializationSchedule` |  |  | 0 | `LobbyDaemon::initialize` |
| `LoadIncompleteLobbyService` |  | yes | 14 | `CS::loadDB` |
| `LoadCharacterCosmesisService` |  | yes | 8 | `CS::loadDB` |
| `TryCharacterCosmesisService` |  | yes | 0 | `CharacterCosmesisService::Start` |
| `UpdateLobbyServiceStateToItemConsumptionTryState_CharacterCosmesisService` |  |  | 0 | `CharacterCosmesisService::ConsumeItemTried` |
| `UpdateLobbyServiceStateToTaskDoneState_CharacterCosmesisService` |  | yes | 0 | `CharacterCosmesisService::TaskDone` |
| `UpdateLobbyServiceStateToItemConsumptionFinishState_CharacterCosmesisService` |  |  | 0 | `CharacterCosmesisService::ConsumeItemFinished` |
| `FinishCharacterCosmesisService` |  | yes | 0 | `CharacterCosmesisService::Finish` |
| `LoadChangeCharacterSexService` |  | yes | 10 | `CS::loadDB` |
| `TryChangeCharacterSexService` |  | yes | 0 | `ChangeCharacterSexService::Start` |
| `UpdateLobbyServiceStateToItemConsumptionTryState_ChangeCharacterSexService` |  |  | 0 | `ChangeCharacterSexService::ConsumeItemTried` |
| `UpdateLobbyServiceStateToTaskDoneState_ChangeCharacterSexService` |  | yes | 0 | `ChangeCharacterSexService::TaskDone` |
| `UpdateLobbyServiceStateToItemConsumptionFinishState_ChangeCharacterSexService` |  |  | 0 | `ChangeCharacterSexService::ConsumeItemFinished` |
| `FinishChangeCharacterSexService` |  | yes | 0 | `ChangeCharacterSexService::Finish` |
| `LoadChangeCharacterRaceService` |  | yes | 12 | `CS::loadDB` |
| `TryChangeCharacterRaceService` |  | yes | 0 | `ChangeCharacterRaceService::Start` |
| `UpdateLobbyServiceStateToItemConsumptionTryState_ChangeCharacterRaceService` |  |  | 0 | `ChangeCharacterRaceService::ConsumeItemTried` |
| `UpdateLobbyServiceStateToTaskDoneState_ChangeCharacterRaceService` |  | yes | 0 | `ChangeCharacterRaceService::TaskDone` |
| `UpdateLobbyServiceStateToItemConsumptionFinishState_ChangeCharacterRaceService` |  |  | 0 | `ChangeCharacterRaceService::ConsumeItemFinished` |
| `FinishChangeCharacterRaceService` |  | yes | 0 | `ChangeCharacterRaceService::Finish` |
| `LoadRenameCharacterService` | `usp_LoadCharacterRenameService` | yes | 7 | `CS::loadDB` |
| `TryCharacterRenameService` |  | yes | 0 | `RenameCharacterService::Start` |
| `UpdateLobbyServiceStateToItemConsumptionTryState_CharacterRenameService` |  |  | 0 | `RenameCharacterService::ConsumeItemTried` |
| `UpdateLobbyServiceStateToTaskDoneState_CharacterRenameService` |  | yes | 0 | `RenameCharacterService::TaskDone` |
| `UpdateLobbyServiceStateToItemConsumptionFinishState_CharacterRenameService` |  |  | 0 | `RenameCharacterService::ConsumeItemFinished` |
| `FinishCharacterRenameService` |  | yes | 0 | `RenameCharacterService::Finish` |
| `LoadSlotAdditionService` |  | yes | 5 | `CS::loadDB` |
| `TrySlotAdditionService` |  | yes | 0 | `AddSlotService::Start` |
| `UpdateLobbyServiceStateToItemConsumptionTryState_SlotAdditionService` |  |  | 0 | `AddSlotService::ConsumeItemTried` |
| `UpdateLobbyServiceStateToTaskDoneState_SlotAdditionService` |  | yes | 0 | `AddSlotService::TaskDone` |
| `UpdateLobbyServiceStateToItemConsumptionFinishState_SlotAdditionService` |  |  | 0 | `AddSlotService::ConsumeItemFinished` |
| `FinishSlotAdditionService` |  | yes | 0 | `AddSlotService::Finish` |
| `WriteUserRenameServiceLog` |  | yes | 0 | `RenameUserService::Finish` |
| `TryExtendJumpingCharacterCreationQuota` |  | yes | 0 | `ExtendJumpingCharacterCreationQuotaService::Start` |
| `UpdateLobbyServiceStateToItemConsumptionTryState_ExtendJumpingCharacterCreationQuota` |  | yes | 0 | `ExtendJumpingCharacterCreationQuotaService::ConsumeItemTried` |
| `UpdateLobbyServiceStateToTaskDoneState_ExtendJumpingCharacterCreationQuota` |  |  | 0 | `ExtendJumpingCharacterCreationQuotaService::TaskDone` |
| `UpdateLobbyServiceStateToItemConsumptionFinishState_ExtendJumpingCharacterCreationQuota` |  | yes | 0 | `ExtendJumpingCharacterCreationQuotaService::ConsumeItemFinished` |
| `FinishExtendJumpingCharacterCreationQuota` |  | yes | 0 | `ExtendJumpingCharacterCreationQuotaService::Finish` |
| `TryAuctionEscrow` |  | yes | 0 | `AuctionEscrowJob::escrow` |
| `CancelAuctionEscrow` |  | yes | 0 | `AuctionEscrowJob::escrowFail` |
| `FinishAuctionEscrow` |  | yes | 0 | `AuctionEscrowJob::escrowOk` |
| `TryAuctionEscrowRollback` |  | yes | 0 | `AuctionRollbackEscrowJob::rollbackEscrow` |
| `FinishAuctionEscrowRollback` |  | yes | 0 | `AuctionRollbackEscrowJob::registerPostOk`, `AuctionRollbackEscrowJob::registerPostFail` |
| `TryAuctionResultDelivery` |  | yes | 0 | `AuctionDeliverJob::init` |
| `FinishAuctionResultDelivery` |  | yes | 0 | `AuctionDeliverJob::registerPostOk`, `AuctionDeliverJob::registerPostFail` |
| `LoadMileage` (348 only ❓) |  |  | 30 | `MileageManager::Initialize` |
| `LoadMileagePolicy` |  | yes | 12 | `MileageManager::Initialize` |
| `LoadMileageFeverTime` |  | yes | 22 | `MileageManager::Initialize` |
| `LoadMileageAccumulatedTimePerDay` |  | yes | 7 | `MileageManager::CreateSlot` |
| `LoadRetryMileageDeposit` |  | yes | 13 | `MileageBatchRetryDeposit::Tick` |
| `RetrieveMileageDeposit` |  | yes | 18 | `HandleLobbyManage_RetryMileageDepositReq` |
| `TryMileageDepost` | `usp_TryMileageDeposit` | yes | 0 | `Mileage::deposit` |
| `FinishMileageDeposit` |  | yes | 0 | `DepositMileageCallback::OnSuccess`, `DepositMileageCallback::OnFail` |

## 4. Caller groups

| Group | Callers | Spec |
| --- | --- | --- |
| Login and load | `CS::login`, `CS::loadDB`, `CS::logout` | `18-jobs-f.md` §1 |
| Character life cycle | `Ch::TryCreation` … `Ch::FinishRestore`, `Ch::StartGameWorldPlay`, `Ch::EndGameWorldPlay` | `16-jobs-d.md` |
| Lobby service entries (5 states each) | `…Service::Start`, `ConsumeItemTried`, `TaskDone`, `ConsumeItemFinished`, `Finish` | `17-jobs-e.md`, `19-jobs-g.md` |
| Guild names | gamed handlers | `20-handlers.md` §5.1 |
| Auction | `AuctionEscrowJob`, `AuctionRollbackEscrowJob`, `AuctionDeliverJob` | `15-jobs-c.md` |
| Mileage | `MileageManager`, `Mileage::deposit`, `DepositMileageCallback`, `MileageBatchRetryDeposit::Tick` | `15-jobs-c.md`, `20-handlers.md` §9 |
| World registry | `GameWorldRegistry::Setup`, `Update` | `20-handlers.md` §9 |
| Scheduler | `LobbyCharacterDeletionScheduler::process` | `20-handlers.md` §9 |

The service entry state order in the DB is: Try (`Start`) → item consumption try state → task done state → item consumption finish state → Finish. Each lobby service class has one wrapper for each step.

## 5. Open ❓

| Item | Where |
| --- | --- |
| Caller maps from `dbrtn` to wire codes (`Ch::*`, `…Service::*`, `CS::*`) | §2 |
| Meaning of the values 1, 2, 10 | §2 |
| World list load path of build 307 | §1 |
| Fetch failure rule and row limits of the wrappers with result columns | §2 |
