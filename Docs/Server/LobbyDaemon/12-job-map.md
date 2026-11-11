# 12 — Job map (step 1 inventory)

Source: build 348 with PDB, used as the guide for build 307. Tool: IDA script (section 4). All rows are ❓ for 307 until the body is compared.

## 1. Job classes (52)

| Job | Created by (table+command) | Tasks | State methods | Size (tokens) | Spec |
| --- | --- | --- | --- | --- | --- |
| AbandonChargeJob | Game38 | 58 59 122 | Initialize init requestChargeAbandon responseChargeAbandon expireChargeAbandon | 1047 | open |
| ActivateGradeJob | Lobb106 | 28 80 81 | ActivateGradeJob ~ActivateGradeJob initFromLobbyGate initFromGame activateGradeReq activateGradeResponse activateGradeExpire sendFail | 2774 | open |
| AddSlotJob | Lobb94 | 26 | AddSlotJob ~AddSlotJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout checkSlotCapacity checkSlotCapacityAck checkSlotCapacityExpire registerServiceEntry tryConsumeAccountInventoryItem tryConsumeAccountInventoryItemOk tryConsumeAccountInventoryItemFail tryConsumeAccountInventoryItemTimeout createSlot createSlotAck createSlotExpire finConsumeAccountInventoryItem finConsumeAccountInventoryItemOk finConsumeAccountInventoryItemFail finConsumeAccountInventoryItemTimeout finServiceEntry | 6197 | open |
| AuctionBidJob | — | 85 86 135 | init auctionBidReq auctionBidResponse auctionBidExpire | 1153 | open |
| AuctionDeliverJob | — | 82 | AuctionDeliverJob ~AuctionDeliverJob init registerPostOk registerPostFail registerPostTimeout | 3265 | open |
| AuctionEscrowJob | — | 83 | ~AuctionEscrowJob escrow escrowOk escrowFail escrowExpire | 1888 | open |
| AuctionRollbackEscrowJob | — | 84 | AuctionRollbackEscrowJob ~AuctionRollbackEscrowJob rollbackEscrow registerPostOk registerPostFail registerPostExpire | 2252 | open |
| BindCharacterSlotJob | — | 33 | init reqGetSlot responseGetSlot expireGetSlot bindCharacterSlot | 1669 | open |
| CancelWaitCharacterDeletionJob | Lobb51 | 12 | CancelWaitCharacterDeletionJob ~CancelWaitCharacterDeletionJob init restore restoreCharacterReq restoreCharacterOk restoreCharacterFail restoreCharacterExpire | 3720 | open |
| ChangeCharacterAppearanceJob | Lobb82 | 24 | ChangeCharacterAppearanceJob ~ChangeCharacterAppearanceJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout registerServiceEntry tryConsumeAccountInventoryItem tryConsumeAccountInventoryItemOk tryConsumeAccountInventoryItemFail tryConsumeAccountInventoryItemTimeout changeCharacterAppearance changeCharacterAppearanceOk changeCharacterAppearanceFail changeCharacterAppearanceTimeout finConsumeAccountInventoryItem finConsumeAccountInventoryItemOk finConsumeAccountInventoryItemFail finConsumeAccountInventoryItemTimeout finServiceEntry | 7083 | open |
| ChangeCharacterRaceJob | Lobb88 | 32 | ChangeCharacterRaceJob ~ChangeCharacterRaceJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout registerServiceEntry tryConsumeAccountInventoryItem tryConsumeAccountInventoryItemOk tryConsumeAccountInventoryItemFail tryConsumeAccountInventoryItemTimeout changeCharacterRace changeCharacterRaceOk changeCharacterRaceFail changeCharacterRaceTimeout finConsumeAccountInventoryItem finConsumeAccountInventoryItemOk finConsumeAccountInventoryItemFail finConsumeAccountInventoryItemTimeout finServiceEntry releasePcServiceStatus | 7332 | open |
| ChangeCharacterSexJob | Lobb85 | 31 | ChangeCharacterSexJob ~ChangeCharacterSexJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout registerServiceEntry tryConsumeAccountInventoryItem tryConsumeAccountInventoryItemOk tryConsumeAccountInventoryItemFail tryConsumeAccountInventoryItemTimeout changeCharacterSex changeCharacterSexOk changeCharacterSexFail changeCharacterSexTimeout finConsumeAccountInventoryItem finConsumeAccountInventoryItemOk finConsumeAccountInventoryItemFail finConsumeAccountInventoryItemTimeout finServiceEntry | 7133 | open |
| ChangeUserNameJob | Lobb97 | 29 | ChangeUserNameJob ~ChangeUserNameJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout changeUserName changeUserNameResponse changeUserNameExpire consumeLobbyServiceItem consumeLobbyServiceItemOk consumeLobbyServiceItemFail consumeLobbyServiceItemTimeout finLobbyService releaseService | 5627 | open |
| CliSessionJob | — |  | CliSessionJob ~CliSessionJob dispatch_work | 294 | `13-jobs-a.md` |
| CreateCharacterJob | Lobb44 |  | CreateCharacterJob ~CreateCharacterJob init reqGetSlot responseGetSlot expireGetSlot tryCreateCharacter createCharacterOk createCharacterFail createCharacterExpire | 6640 | open |
| CreateInitialUserNameJob | — | 27 70 71 | ~CreateInitialUserNameJob init changeUserNameReq changeUserNameAck changeUserNameExpire | 1807 | open |
| DeleteCharacterJob | Lobb47 | 11 | DeleteCharacterJob ~DeleteCharacterJob init checkSecondPasswordStatus checkSecondPasswordStatusResponse checkSecondPasswordStatusExpire checkSecondPasswordVerified checkSecondPasswordVerifiedResponse checkSecondPasswordVerifiedExpire getCharacterGoodsReq getCharacterGoodsCountOk getCharacterGoodsCountFail getCharacterGoodsCountExpire deleteReq deleteOk deleteFail deleteExpire directDelete scheduleDelete | 7906 | open |
| DepositMileageJob | Mana14 Game100 | 5 91 92 | DepositMileageJob ~DepositMileageJob init depositReq depositAck depositExpire Callback | 3098 | open |
| DepositPcCafeMileageJobManageCallback | — |  | OnError OnSuccess OnFail OnTimeout | 234 | open |
| EndWaitGameWorldJob | — | 0 3 | init endWait | 696 | `13-jobs-a.md` |
| EnterGameWorldJob | Game5 | 0 117 | Initialize init enterGameWorld | 3337 | open |
| ExtendJumpingCharacterCreationQuotaJob | Lobb100 Game69 | 30 | ExtendJumpingCharacterCreationQuotaJob ~ExtendJumpingCharacterCreationQuotaJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout registerServiceEntry tryConsumeAccountInventoryItem tryConsumeAccountInventoryItemOk tryConsumeAccountInventoryItemFail tryConsumeAccountInventoryItemTimeout extendJumpingCharacterCreationQuota finConsumeAccountInventoryItem finConsumeAccountInventoryItemOk finConsumeAccountInventoryItemFail finConsumeAccountInventoryItemTimeout finServiceEntry sendFailReason | 4826 | open |
| GetChargeInfoJob | Lobb60 | 17 52 53 | GetChargeInfoJob init requestChargeGetInfo responseChargeGetInfo expireChargeGetInfo | 1424 | open |
| GetGoodsCountJob | Lobb64 | 19 | GetGoodsCountJob ~GetGoodsCountJob init getGoodsCountReq getGoodsCountOk getGoodsCountFail getGoodsCountTimeout | 1162 | open |
| GetLobbyServiceItemJob | Lobb76 | 23 | GetLobbyServiceItemJob ~GetLobbyServiceItemJob init getLobbyServiceItemReq getLobbyServiceItemOk getLobbyServiceItemFail getLobbyServiceItemTimeout | 1266 | open |
| GiveupWaitGameWorldJob | Lobb31 | 0 16 | Initialize init giveupWait | 411 | `13-jobs-a.md` |
| InitCliSessionWlobbyTokenJob | Lobb12 | 0 14 | Initialize init initOk | 574 | `13-jobs-a.md` |
| InitCliSessionWnpTokenJob | — | 7 | init | 584 | `13-jobs-a.md` |
| KickJob | Mana5 | 1 | Demander KickJob ~KickJob init kick worldKickReq worldKickOk worldKickFail worldKickExpire | 1877 | open |
| LeaveGameWorldJob | Game8 | 0 119 | Initialize init leaveGameWorld | 823 | `13-jobs-a.md` |
| ListGoodsJob | Lobb67 | 20 | ListGoodsJob ~ListGoodsJob init listGoodsReq listGoodsOk listGoodsFail listGoodsTimeout | 1169 | open |
| ListLobbyServiceItemJob | Lobb73 | 22 | ListLobbyServiceItemJob ~ListLobbyServiceItemJob init listLobbyServiceItemReq listLobbyServiceItemOk listLobbyServiceItemFail listLobbyServiceItemTimeout | 1281 | open |
| LobbyJob | — |  | ~LobbyJob dispatch_work transit execute resume | 495 | `13-jobs-a.md` |
| LobbyJobManager | — |  | ~LobbyJobManager Initialize Register Clear Start Postpone Execute ExecuteW ResumeW add remove find | 2658 | `13-jobs-a.md` |
| LobbyJobTaskEvent | — |  | ~LobbyJobTaskEvent HandleEvent | 231 | `13-jobs-a.md` |
| LobbyJobTransaction | — |  | GetTid Expire Set | 180 | `13-jobs-a.md` |
| LogoutInGameWorldJob | Game12 | 0 118 | Initialize init logout | 418 | `13-jobs-a.md` |
| LogoutJob | Lobb18 Lobb19 Lobb23 | 0 9 | LogoutJob init logout | 618 | `13-jobs-a.md` |
| NPLobbyJobTransaction | — |  | Expire Set | 158 | open |
| RecoverPcJob | Lobb54 | 18 | RecoverPcJob ~RecoverPcJob Initialize init restoreGameWorldPcReq restoreGameWorldPcOk restoreGameWorldPcFail restoreGameWorldPcExpired __restoreStuckPc | 2788 | open |
| RenameCharacterJob | Lobb91 | 25 | RenameCharacterJob ~RenameCharacterJob init checkAccountInventoryItem checkAccountInventoryItemOk checkAccountInventoryItemFail checkAccountInventoryItemTimeout registerServiceEntry tryConsumeAccountInventoryItem tryConsumeAccountInventoryItemOk tryConsumeAccountInventoryItemFail tryConsumeAccountInventoryItemTimeout renameCharacter renameCharacterOk renameCharacterFail renameCharacterTimeout finConsumeAccountInventoryItem finConsumeAccountInventoryItemOk finConsumeAccountInventoryItemFail finConsumeAccountInventoryItemTimeout finServiceEntry sendFriendMessageToNP | 8535 | open |
| SecondPasswordGetStatusJob | — | 62 63 129 | init requestGetStatus responseGetStatus expireGetStatus | 991 | open |
| SecondPasswordIsVerifiedJob | — | 64 65 130 | init requestIsVerified responseIsVerified expireIsVerified | 1011 | open |
| SetupNpScheduleInfoJob | — | 0 6 | SetupNpScheduleInfoJob ~SetupNpScheduleInfoJob tryListCurrentPromotions listCurrentPromotionsAck tryGetPromotionPolicy getPromotionPolicyAck | 3920 | open |
| StartChargeJob | Game30 | 54 55 120 | Initialize init requestStartCharge responseStartCharge expireStartCharge | 1156 | open |
| StartCliSessionWlobbyTokenJob | Lobb15 | 15 | Initialize init getGradeLevelReq getGradeLevelResponse getGradeLevelExpire listAllGradesReq listAllGradesResponse listAllGradesExpire start | 2518 | open |
| StartCliSessionWnpTokenJob | — | 8 | StartCliSessionWnpTokenJob ~StartCliSessionWnpTokenJob init consumeToken consumeTokenResponse consumeTokenExpire listUserLastPresencesReq listUserLastPresencesResponse listUserLastPresencesExpire listSlotsReq listSlotsResponse listSlotsExpire createSlotsReq createSlotsResponse createSlotsExpire setSlotDataReq setSlotDataResponse setSlotDataExpire presenceLoginReq presenceLoginResponse presenceLoginExpire getUserProfileReq getUserProfileResponse getUserProfileExpire getGameTimeReq getGameTimeResponse getGameTimeExpire getGradeLevelReq getGradeLevelResponse getGradeLevelExpire listAllGradesReq listAllGradesResponse listAllGradesExpire listUserBansReq listUserBansResponse listUserBansExpire getRMSInquiryReq getRMSInquiryResponse getRMSInquiryExpire getUserPromotionStatusReq getUserPromotionStatusResponse getUserPromotionStatusExpire loginAccount checkIntialUserName | 16469 | open |
| StopChargeJob | Game33 | 56 57 121 | Initialize init requestChargeStop responseChargeStop expireChargeStop | 1089 | open |
| TakeAccountTimeBasedPlayRewardJob | — | 103 104 143 | init takeAccountTimeBasedPlayRewardReq takeAccountTimeBasedPlayRewardResponse takeAccountTimeBasedPlayRewardExpire sendOk sendFail | 3174 | open |
| TakeGoodsJob | Lobb70 | 21 | TakeGoodsJob ~TakeGoodsJob init takeGoodsReq takeGoodsOk takeGoodsFail takeGoodsTimeout | 1196 | open |
| TryEnterGameWorldJob | Lobb24 | 13 | TryEnterGameWorldJob ~TryEnterGameWorldJob Initialize init restorePcReq restorePcOk restorePcFail restorePcExpired checkSecondPasswordStatus checkSecondPasswordStatusResponse checkSecondPasswordStatusExpire checkSecondPasswordVerified checkSecondPasswordVerifiedResponse checkSecondPasswordVerifiedExpire requestGetSlot responseGetSlot expireGetSlot requestChargeGetInfo responseChargeGetInfo expireChargeGetInfo tryEnterGameWorld issueGameWorldToken | 7138 | open |
| UpgradeJumpingCharacterStateJob | Game72 Game75 | 0 | UpgradeJumpingCharacterStateJob init upgradeJumpingCharacterState | 1453 | open |

## 2. Handlers without a job

| Table | Command | Handler | Sends |
| --- | --- | --- | --- |
| LobbyManage | 1 | LobbyManage_HelloReq | Send |
| LobbyManage | 4 | LobbyManage_AnnounceNotify |  |
| LobbyManage | 8 | LobbyManage_CleanupGameAccountIdIndexReq | SendCleanupGameAccountIdIndexFail SendCleanupGameAccountIdIndexOk |
| LobbyManage | 11 | LobbyManage_CleanupPcidIndexReq | SendCleanupPcidIndexFail SendCleanupPcidIndexOk |
| LobbyLobbyG | 1 | LobbyLobbyGate_HelloReq | Send |
| LobbyLobbyG | 6 | LobbyLobbyGate_InitSessionWithNPTokenReq | SendInitSessionWithNPTokenFail |
| LobbyLobbyG | 9 | LobbyLobbyGate_StartSessionWithNPTokenReq | SendStartSessionWithNPTokenFail |
| LobbyLobbyG | 28 | LobbyLobbyGate_GetWaitInfoReq | SendGetWaitInfoFail SendGetWaitInfoOk |
| LobbyLobbyG | 35 | LobbyLobbyGate_GetCharacterInfoListReq | Send SendGetCharacterInfoListFail |
| LobbyLobbyG | 38 | LobbyLobbyGate_GetCharacterInfoReq | Send SendGetCharacterInfoFail |
| LobbyLobbyG | 41 | LobbyLobbyGate_CheckCharacterNameReq |  |
| LobbyLobbyG | 57 | LobbyLobbyGate_BindCharacterSlotReq | SendBindCharacterSlotFail |
| LobbyLobbyG | 79 | LobbyLobbyGate_GetServiceEntryListReq | Send SendGetServiceEntryListFail |
| LobbyLobbyG | 103 | LobbyLobbyGate_CreateInitialUserNameReq | SendCreateInitialUserNameFail |
| LobbyInfoGa | 1 | LobbyInfoGate_HelloReq | Send |
| LobbyInfoGa | 4 | LobbyInfoGate_AuthTokenReq | SendAuthTokenFail SendAuthTokenOk |
| LobbyInfoGa | 7 | LobbyInfoGate_GetWorldInfoListReq | Send SendGetWorldInfoListFail |
| LobbyGamePa | 1 | LobbyGame_HelloReq | Send |
| LobbyGamePa | 4 | LobbyGame_ServiceOpenNotify |  |
| LobbyGamePa | 11 | LobbyGame_UpdateBotScoreNotify |  |
| LobbyGamePa | 15 | LobbyGame_KickOk |  |
| LobbyGamePa | 16 | LobbyGame_KickFail |  |
| LobbyGamePa | 17 | LobbyGame_NPPresenceKickReq | Send SendRequestPresenceKick |
| LobbyGamePa | 21 | LobbyGame_CreateCharacterOk |  |
| LobbyGamePa | 22 | LobbyGame_CreateCharacterFail |  |
| LobbyGamePa | 24 | LobbyGame_DeleteCharacterOk |  |
| LobbyGamePa | 25 | LobbyGame_DeleteCharacterFail |  |
| LobbyGamePa | 27 | LobbyGame_RestoreCharacterOk |  |
| LobbyGamePa | 28 | LobbyGame_RestoreCharacterFail |  |
| LobbyGamePa | 39 | LobbyGame_UpdateCharacterDataReq | SendUpdateCharacterDataFail SendUpdateCharacterDataOk |
| LobbyGamePa | 42 | LobbyGame_UpdateAccountDataReq | SendUpdateAccountDataFail SendUpdateAccountDataOk |
| LobbyGamePa | 45 | LobbyGame_TryRegisterGuildReq | SendTryRegisterGuildFail SendTryRegisterGuildOk |
| LobbyGamePa | 48 | LobbyGame_FinishRegisterGuildNotify |  |
| LobbyGamePa | 49 | LobbyGame_CancelRegisterGuildNotify |  |
| LobbyGamePa | 50 | LobbyGame_TryRenameGuildReq | SendTryRenameGuildFail SendTryRenameGuildOk |
| LobbyGamePa | 53 | LobbyGame_FinishRenameGuildNotify |  |
| LobbyGamePa | 54 | LobbyGame_CancelRenameGuildNotify |  |
| LobbyGamePa | 55 | LobbyGame_DeleteGuildNotify |  |
| LobbyGamePa | 58 | LobbyGame_ChangeCharacterAppearanceOk |  |
| LobbyGamePa | 59 | LobbyGame_ChangeCharacterAppearanceFail |  |
| LobbyGamePa | 61 | LobbyGame_ChangeCharacterSexOk |  |
| LobbyGamePa | 62 | LobbyGame_ChangeCharacterSexFail |  |
| LobbyGamePa | 64 | LobbyGame_ChangeCharacterRaceOk |  |
| LobbyGamePa | 65 | LobbyGame_ChangeCharacterRaceFail |  |
| LobbyGamePa | 67 | LobbyGame_RenameCharacterOk |  |
| LobbyGamePa | 68 | LobbyGame_RenameCharacterFail |  |
| LobbyGamePa | 78 | LobbyGame_SetAppDataNotify | SendMsg |
| LobbyGamePa | 79 | LobbyGame_FriendMessageNotify | SendRequestFriendSendMessage |
| LobbyGamePa | 80 | LobbyGame_SecondPasswordGetStatusReq | SendSecondPasswordGetStatusFail |
| LobbyGamePa | 83 | LobbyGame_SecondPasswordIsVerifiedReq | SendSecondPasswordIsVerifiedFail |
| LobbyGamePa | 87 | LobbyGame_AuctionEscrowOk |  |
| LobbyGamePa | 88 | LobbyGame_AuctionEscrowFail |  |
| LobbyGamePa | 89 | LobbyGame_AuctionBidReq | SendAuctionBidFail |
| LobbyGamePa | 92 | LobbyGame_GetGradeListReq | SendGetGradeListFail |
| LobbyGamePa | 97 | LobbyGame_ActivateGradeReq | SendActivateGradeFail |
| LobbyGamePa | 103 | LobbyGame_BoastEventInWorldNotify |  |
| LobbyGamePa | 104 | LobbyGame_TakeAccountTimeBasedPlayRewardReq | SendTakeAccountTimeBasedPlayRewardFail |
| LobbyGamePa | 107 | LobbyGame_UpdateAccountExpReq | SendUpdateAccountExpFail SendUpdateAccountExpOk |
| AccountInve | 2 | AccountInventoryLobby_HelloOk |  |
| AccountInve | 3 | AccountInventoryLobby_HelloFail |  |
| AccountInve | 5 | AccountInventoryLobby_GetGoodsCountOk |  |
| AccountInve | 6 | AccountInventoryLobby_GetGoodsCountFail |  |
| AccountInve | 8 | AccountInventoryLobby_ListGoodsOk |  |
| AccountInve | 9 | AccountInventoryLobby_ListGoodsFail |  |
| AccountInve | 11 | AccountInventoryLobby_TakeGoodsOk |  |
| AccountInve | 12 | AccountInventoryLobby_TakeGoodsFail |  |
| AccountInve | 14 | AccountInventoryLobby_RetrieveLobbyServiceItemListOk |  |
| AccountInve | 15 | AccountInventoryLobby_RetrieveLobbyServiceItemListFail |  |
| AccountInve | 17 | AccountInventoryLobby_RetrieveSingleWarehouseItemOk |  |
| AccountInve | 18 | AccountInventoryLobby_RetrieveSingleWarehouseItemFail |  |
| AccountInve | 20 | AccountInventoryLobby_TryWarehouseItemConsumptionOk |  |
| AccountInve | 21 | AccountInventoryLobby_TryWarehouseItemConsumptionFail |  |
| AccountInve | 23 | AccountInventoryLobby_FinishWarehouseItemConsumptionOk |  |
| AccountInve | 24 | AccountInventoryLobby_FinishWarehouseItemConsumptionFail |  |
| AccountInve | 26 | AccountInventoryLobby_CancelWarehouseItemConsumptionOk |  |
| AccountInve | 27 | AccountInventoryLobby_CancelWarehouseItemConsumptionFail |  |
| AccountInve | 29 | AccountInventoryLobby_ConsumeWarehouseItemOk |  |
| AccountInve | 30 | AccountInventoryLobby_ConsumeWarehouseItemFail |  |
| AccountInve | 32 | AccountInventoryLobby_RetrieveCharacterGoodsCountOk |  |
| AccountInve | 33 | AccountInventoryLobby_RetrieveCharacterGoodsCountFail |  |
| AccountInve | 34 | AccountInventoryLobby_GoodsInventoryChangedNotify |  |

## 3. Batches

| Batch | Content | State |
| --- | --- | --- |
| A | Jobs under 3,000 tokens | open |
| B | Character jobs (create, delete, cancel, recover, rename, appearance, sex, race) | open |
| C | Session jobs (init, start, enter, leave, wait, kick, logout) | open |
| D | Account inventory, slot, grade, charge, auction, mileage jobs | open |
| E | Handlers without a job, timers, manage port | open |

## 4. Tool

The script decompiles each job method, removes log calls and dead log variables, and writes one file per job to the scratchpad. The output stays out of the repository (clean-room rule).
