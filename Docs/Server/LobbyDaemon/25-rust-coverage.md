# 25 — Rust coverage of LobbyDaemon (build 381)

Card `ld381-2`, 2026-11-12. Crate `Program/Server/LobbyDaemon` (package `lobbydaemon`) on branch `slice/ld381-2` (after `ld381-1-rust`, commit `1dc660d`).

Method: handler rows from `command-tables-381.json`; jobs from `12-job-map.md` §1; timers from `20-handlers.md` §9; wrappers from `21-lobbydb-wrappers.md` §3; config keys from `01-config.md` lines 51–211. Rust items from the public items of `src/` and the match arms of `Jobs::handle_fast` / `handle_slow` (`src/jobs.rs:403-532`). Tests: test function names only. No IDA. No test run (Bash was not available) ❓.

Source rule: logic = 2019 + PDB (specs `13` to `21`); packets = 381 (`command-tables-381.json`, `24-compare-381.md`); 381 logic that the client can see wins.

State rules:

| State | Meaning |
| --- | --- |
| `done` | Rust does the same job with the same data and result. Our own link protocol counts as done (card rule). |
| `partial` | Rust answers, but a step, a check or a data part is missing (often the feature-off path only). |
| `missing` | No Rust code. An unknown command gets no reply (`src/jobs.rs:521-525`). |
| `out of scope` | Not part of the port (runtime, pools, logs, licence or vendor boundary). |

Groups (cards): `r1` gamed link and character sync, `r2` account inventory link and its jobs, `r3` manage port, InfoGate, timers, registry, config load, `r4` NP (Sts), charge, goods-free NP jobs, grade, promotions, mileage, auction, `r5` database (guild, jumping state, name rules, error maps, time conversion, live SQL).

## 0. Counts

| Section | Rows | done | partial | missing | out of scope |
| --- | --- | --- | --- | --- | --- |
| 1 LobbyGate | 35 | 11 | 23 | 1 | 0 |
| 2 InfoGate | 3 | 0 | 0 | 3 | 0 |
| 3 Manage | 6 | 0 | 0 | 6 | 0 |
| 4 gamed | 53 | 7 | 2 | 44 | 0 |
| 5 Account inventory | 23 | 0 | 0 | 23 | 0 |
| 6 NP (Sts) | 43 | 3 | 0 | 40 | 0 |
| 7 Job classes | 52 | 11 | 25 | 16 | 0 |
| 8 Timers | 11 | 1 | 1 | 7 | 2 |
| 9 `LobbyDB` wrappers | 94 | 26 | 0 | 68 | 0 |
| 10 Config keys | 161 | 4 | 13 | 118 | 26 |
| 11 Cross-cutting | 14 | 3 | 6 | 5 | 0 |
| **Total** | **495** | **66** | **70** | **331** | **28** |

## 1. LobbyGate (`LobbyLobbyGate`, max command 110)

Rust: `Jobs::handle_fast` / `Jobs::handle_slow` (`src/jobs.rs`), `src/jobs/characters.rs`, `src/jobs/services.rs`, `src/jobs/world.rs`. Spec: `20-handlers.md` §4, §4.1; jobs `13`–`19`.

| Cmd | Name | Spec | Rust item | Test | State | Note | Card |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `HandleLobbyLobbyGate_HelloReq` | `20` §2 | `handle_fast` HelloReq, `protocol::hello` | `r1_hello_version_format_order_and_duplicate` | partial | No `BLDaemonInfoTable` peer check. No mileage fever list (mileage off). | r3 |
| 6 | `HandleLobbyLobbyGate_InitSessionWithNPTokenReq` | `13` §3.1 | `Jobs::init` | `r2_init_rejections_echo_uniqueness_and_no_consumption` | done | | |
| 9 | `HandleLobbyLobbyGate_StartSessionWithNPTokenReq` | `18` §1 | `Jobs::start`, `authenticate` | `r4_rejection_missing_fields_and_presence_error_maps`, `r5_required_store_failures_and_best_effort_updates` | partial | Local subset: consume, presence, account, list. No slots, profile, game time, grade, bans, RMS, promotion steps. | r4 |
| 12 | `HandleLobbyLobbyGate_InitSessionWithLobbyTokenReq` | `13` §3.2 | `Jobs::init_retained` | `r13_retained_rebind_single_use_cache_and_link_checks` | done | | |
| 15 | `HandleLobbyLobbyGate_StartSessionWithLobbyTokenReq` | `18` §2, `10` §Correction | `Jobs::start_retained` | `r13_retained_start_cancels_rebound_init_deadline` | partial | No grade level reload (`getGradeLevelReq`, `listAllGradesReq`). | r4 |
| 18 | `HandleLobbyLobbyGate_LostAnonymousConnectionNotify` | `13` §3.3 | `Jobs::logout` | `r8_anonymous_close_uses_gate_key_and_preserves_authenticated_sessions` | done | | |
| 19 | `HandleLobbyLobbyGate_LostConnectionNotify` | `13` §3.3 | `Jobs::logout` | `logout_table` | done | | |
| 23 | `HandleLobbyLobbyGate_LogoutNotify` | `13` §3.3 | `Jobs::logout` | `logout_table`, `r7_close_lookup_logout_notice_and_idempotence` | done | | |
| 24 | `HandleLobbyLobbyGate_GetEnterWorldTokenReq` | `18` §3 | `Jobs::enter_world` (`jobs/world.rs`) | `success_single_use_and_identity_checks`, `restoration_store_failure_blocks_entry` | partial | No second password, get slot or charge checks (`18` §3.2). Restore of a stuck PC is local. | r4 |
| 28 | `HandleLobbyLobbyGate_GetWaitInfoReq` | `20` §4.1 | `Jobs::wait_info` | `wait_info_in_queue`, `wait_info_no_session` | done | The link has no queue yet (§11 X3). | |
| 31 | `HandleLobbyLobbyGate_GiveupWaitReq` | `13` §3.4 | `Jobs::giveup_wait` | `giveup_wait_ok`, `giveup_wait_wrong_state` | done | | |
| 35 | `HandleLobbyLobbyGate_GetCharacterInfoListReq` | `20` §4 | `Jobs::characters` | `r5_r6_register_required_loads_cache_and_fragmented_character_list` | done | | |
| 38 | `HandleLobbyLobbyGate_GetCharacterInfoReq` | `20` §4 | `Jobs::character_info` | `r14_character_info_checks_state_slot_session_and_link` | done | | |
| 41 | `HandleLobbyLobbyGate_CheckCharacterNameReq` | `20` §3, `24` §8 | no-op arm | `r14_check_name_is_a_no_op_with_any_body_or_session` | done | 381 handler has no reply too. | |
| 44 | `HandleLobbyLobbyGate_CreateCharacterReq` | `16` §1 | `character_mutation` | `game_create_rejection_rolls_back_and_ids_do_not_repeat`, `create_validation_and_store_errors_do_not_add_rows` | partial | No NP `GetSlot` (`16` §1.4). Slots and worlds come from the fixture `CharacterPolicy`. No jumping creation. | r4 |
| 47 | `HandleLobbyLobbyGate_DeleteCharacterReq` | `16` §2 | `character_mutation` | `delete_rejection_and_db_failure_recovery`, `loaded_character_can_be_deleted` | partial | gamed delete is local `Ok(1)` (`src/gamed_link.rs:490`). No second password (r4) or goods count (r2) step. | r1 |
| 51 | `HandleLobbyLobbyGate_CancelWaitCharacterDeletionReq` | `16` §3 | `character_mutation` | `lifecycle_lists_normal_waiting_and_restored_rows` | partial | gamed restore is local `Ok` (`src/gamed_link.rs:493`). | r1 |
| 54 | `HandleLobbyLobbyGate_RecoverCharacterReq` | `16` §4 | `character_mutation` | `restoration_failures_leave_recoverable_state_five` | partial | `restoreGameWorldPcReq` has no link message. | r1 |
| 57 | `HandleLobbyLobbyGate_BindCharacterSlotReq` | `15` §2, `20` §4.1 | — | — | missing | Falls to the unknown arm: no reply. | r4 |
| 60 | `HandleLobbyLobbyGate_GetChargeInfoReq` | `15` §1, `20` §4.1 | `Jobs::charge_info` (`jobs/services.rs`) | `charge_info_off`, `charge_info_on_is_not_ported` | partial | Off path only (code 7). | r4 |
| 64 | `HandleLobbyLobbyGate_GetGoodsCountReq` | `14` §3 | `Jobs::goods_unavailable` | `goods_requests_fail_with_account_inventory_unavailable` | partial | Off path only (codes 4/5/7). | r2 |
| 67 | `HandleLobbyLobbyGate_ListGoodsReq` | `14` §3 | `goods_unavailable` | same | partial | Off path only. | r2 |
| 70 | `HandleLobbyLobbyGate_TakeGoodsReq` | `14` §3 | `goods_unavailable` | same | partial | Off path only. | r2 |
| 73 | `HandleLobbyLobbyGate_ListLobbyServiceItemInfoReq` | `15` §1 | `goods_unavailable` | same | partial | Off path only. | r2 |
| 76 | `HandleLobbyLobbyGate_GetLobbyServiceItemInfoReq` | `15` §1 | `goods_unavailable` | same | partial | Off path only. | r2 |
| 79 | `HandleLobbyLobbyGate_GetServiceEntryListReq` | `20` §4 | `protocol::service_entries` | — ❓ | partial | Always an empty list (local policy). `FetchServiceEntryList` is not ported. | r2 |
| 82 | `HandleLobbyLobbyGate_ChangeCharacterAppearanceWithAccountInventoryItemReq` | `17` | `Jobs::account_item_service` | `item_service_off`, `item_service_no_session` | partial | Off path only (code 7). | r2 |
| 85 | `HandleLobbyLobbyGate_ChangeCharacterSexWithAccountInventoryItemReq` | `17` | `account_item_service` | `item_service_off` | partial | Off path only. | r2 |
| 88 | `HandleLobbyLobbyGate_ChangeCharacterRaceWithAccountInventoryItemReq` | `17` | `account_item_service` | `item_service_off` | partial | Off path only. | r2 |
| 91 | `HandleLobbyLobbyGate_RenameCharacterWithAccountInventoryItemReq` | `17` §4 | `account_item_service` | `rename_fail_command` | partial | Off path only. Fail command 93 (bug, not ported). | r2 |
| 94 | `HandleLobbyLobbyGate_AddSlotWithAccountInventoryItemReq` | `19` §1 | `account_item_service` | `item_service_off` | partial | Off path only. | r2 |
| 97 | `HandleLobbyLobbyGate_ChangeUserNameWithAccountInventoryItemReq` | `19` §2 | `account_item_service` | `item_service_off` | partial | Off path only. | r2 |
| 100 | `HandleLobbyLobbyGate_ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemReq` | `19` §5 | `account_item_service` | `item_service_zero_input` | partial | Off path only. | r2 |
| 103 | `HandleLobbyLobbyGate_CreateInitialUserNameReq` | `19` §3, §4 | `Jobs::initial_user_name` | `initial_name_success_and_all_platform_maps`, `initial_name_validation_maps` | done | | |
| 106 | `HandleLobbyLobbyGate_ActivateGradeReq` | `15` §3 | `Jobs::activate_grade` | `grade_off_before_session` | partial | Off path only (code 7). | r4 |

## 2. InfoGate (`LobbyInfoGate`, max command 10)

| Cmd | Name | Spec | Rust | Test | State | Note | Card |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `HandleLobbyInfoGate_HelloReq` | `20` §2 | — | — | missing | No InfoGate listener. | r3 |
| 4 | `HandleLobbyInfoGate_AuthTokenReq` | `20` §6 | — | — | missing | Needs the `_infoGateToken` of the session. | r3 |
| 7 | `HandleLobbyInfoGate_GetWorldInfoListReq` | `20` §6 | — | — | missing | | r3 |

## 3. Manage (`LobbyManage`, max command 16)

| Cmd | Name | Spec | Rust | Test | State | Note | Card |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `HandleLobbyManage_HelloReq` | `20` §2, §7 | — | — | missing | No manage listener. | r3 |
| 4 | `HandleLobbyManage_AnnounceNotify` | `20` §7 | — | — | missing | Broadcast to LobbyGate links. | r3 |
| 5 | `HandleLobbyManage_KickReq` | `20` §7, `18` §5 | (`Jobs::kick_world` exists) | — | missing | Manage demander of `KickJob`. | r3 |
| 8 | `HandleLobbyManage_CleanupGameAccountIdIndexReq` | `20` §7 | — | — | missing | | r3 |
| 11 | `HandleLobbyManage_CleanupPcidIndexReq` | `20` §7 | — | — | missing | | r3 |
| 14 | `HandleLobbyManage_RetryMileageDepositReq` | `20` §7, `15` §5 | — | — | missing | Needs `DepositMileageJob`. | r4 |

## 4. gamed (`LobbyGame`, max command 115)

Rust: `LinkGameDaemon` (`src/gamed_link.rs`), messages in `packet::gamed_link::Message` (`Program/Server/Packet/src/gamed_link.rs`). Spec: `07-gamed-link.md` lines 556–693, `20-handlers.md` §5.

| Cmd | Name | Rust | Test | State | Note | Card |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `HandleLobbyGame_HelloReq` | `Message::Hello`/`HelloOk`/`HelloFail` | `hello_unknown_world_is_refused_and_closed`, `second_hello_on_one_channel_closes_it` | partial | Identity checks done. `HelloOk` carries no time-based play season, no random store schedule, no mileage days (`20` §2). | r1 |
| 4 | `HandleLobbyGame_ServiceOpenNotify` | `Message::WorldLive` | `tests/link_identity.rs` (uses `WorldLive`) | done | | |
| 5 | `HandleLobbyGame_EnterWorldReq` | `Message::Enter`/`EnterOk`/`EnterFail`, `Server::consume_world` | `client_logs_in_through_stservers_to_the_character_list`, `exact_expiry_and_wrong_world_consume_cleanup` | partial | 381 lists `Cihqq;Chq;` are empty. No `UpdateLastPlayStart`. | r1 |
| 8 | `HandleLobbyGame_LeaveWorldReq` | `Message::Leave`/`LeaveOk`/`LeaveFail`, `Jobs::leave_world` | `leave_ok_issues_type2_token`, `leave_job_start_fail_sends_leave_fail` | done | DB step: see §7 `LeaveGameWorldJob`. | |
| 11 | `HandleLobbyGame_UpdateBotScoreNotify` | — | — | missing | `CliSession::UpdateBotScore`. | r1 |
| 12 | `HandleLobbyGame_LogoutNotify` | `Message::LogoutNotify`, `Jobs::logout_in_world` | `logout_in_world_ends_session`, `logout_in_world_wrong_pcid` | done | DB step: see §7 `LogoutInGameWorldJob`. | |
| 15 | `HandleLobbyGame_KickOk` | `Message::KickOk` | `kick_in_world_ok` | done | | |
| 16 | `HandleLobbyGame_KickFail` | `Message::KickFail` | `kick_in_world_fail_ends_session` | done | | |
| 17 | `HandleLobbyGame_NPPresenceKickReq` | — | — | missing | NP presence kick. | r4 |
| 21 | `HandleLobbyGame_CreateCharacterOk` | `Message::CreateOk` | `create_reply_from_another_world_is_ignored` | done | | |
| 22 | `HandleLobbyGame_CreateCharacterFail` | `Message::CreateFail` | `game_create_rejection_rolls_back_and_ids_do_not_repeat` | done | | |
| 24 | `HandleLobbyGame_DeleteCharacterOk` | — | — | missing | Delete request (23) not sent. | r1 |
| 25 | `HandleLobbyGame_DeleteCharacterFail` | — | — | missing | | r1 |
| 27 | `HandleLobbyGame_RestoreCharacterOk` | — | — | missing | Restore request (26) not sent. | r1 |
| 28 | `HandleLobbyGame_RestoreCharacterFail` | — | — | missing | | r1 |
| 30 | `HandleLobbyGame_StartChargeReq` | — | — | missing | `StartChargeJob`. | r4 |
| 33 | `HandleLobbyGame_StopChargeReq` | — | — | missing | `StopChargeJob`. | r4 |
| 38 | `HandleLobbyGame_AbandonChargeNotify` | — | — | missing | `AbandonChargeJob`. | r4 |
| 39 | `HandleLobbyGame_UpdateCharacterDataReq` | — | — | missing | Key check, `UpdateCharacterData`. | r1 |
| 42 | `HandleLobbyGame_UpdateAccountDataReq` | — | — | missing | 381 format `hqqqiCQiqqihcccc;qiqCihqq;Chq;`. | r1 |
| 45 | `HandleLobbyGame_TryRegisterGuildReq` | — | — | missing | | r5 |
| 48 | `HandleLobbyGame_FinishRegisterGuildNotify` | — | — | missing | | r5 |
| 49 | `HandleLobbyGame_CancelRegisterGuildNotify` | — | — | missing | | r5 |
| 50 | `HandleLobbyGame_TryRenameGuildReq` | — | — | missing | | r5 |
| 53 | `HandleLobbyGame_FinishRenameGuildNotify` | — | — | missing | | r5 |
| 54 | `HandleLobbyGame_CancelRenameGuildNotify` | — | — | missing | | r5 |
| 55 | `HandleLobbyGame_DeleteGuildNotify` | — | — | missing | | r5 |
| 58 | `HandleLobbyGame_ChangeCharacterAppearanceOk` | — | — | missing | Resume of the appearance job. | r2 |
| 59 | `HandleLobbyGame_ChangeCharacterAppearanceFail` | — | — | missing | | r2 |
| 61 | `HandleLobbyGame_ChangeCharacterSexOk` | — | — | missing | | r2 |
| 62 | `HandleLobbyGame_ChangeCharacterSexFail` | — | — | missing | | r2 |
| 64 | `HandleLobbyGame_ChangeCharacterRaceOk` | — | — | missing | | r2 |
| 65 | `HandleLobbyGame_ChangeCharacterRaceFail` | — | — | missing | | r2 |
| 67 | `HandleLobbyGame_RenameCharacterOk` | — | — | missing | | r2 |
| 68 | `HandleLobbyGame_RenameCharacterFail` | — | — | missing | | r2 |
| 69 | `HandleLobbyGame_ExtendJumpingCharacterCreationQuotaWithAccountInventoryItemReq` | — | — | missing | | r2 |
| 72 | `HandleLobbyGame_UpgradeJumpingCharacterStateWithCreationQuotaReq` | — | — | missing | | r5 |
| 75 | `HandleLobbyGame_UpdateJumpingCharacterStateReq` | — | — | missing | | r5 |
| 78 | `HandleLobbyGame_SetAppDataNotify` | — | — | missing | NP presence `SetAppData`. | r4 |
| 79 | `HandleLobbyGame_FriendMessageNotify` | — | — | missing | NP friend message. | r4 |
| 80 | `HandleLobbyGame_SecondPasswordGetStatusReq` | — | — | missing | | r4 |
| 83 | `HandleLobbyGame_SecondPasswordIsVerifiedReq` | — | — | missing | | r4 |
| 87 | `HandleLobbyGame_AuctionEscrowOk` | — | — | missing | | r4 |
| 88 | `HandleLobbyGame_AuctionEscrowFail` | — | — | missing | | r4 |
| 89 | `HandleLobbyGame_AuctionBidReq` | — | — | missing | | r4 |
| 92 | `HandleLobbyGame_GetGradeListReq` | — | — | missing | | r4 |
| 97 | `HandleLobbyGame_ActivateGradeReq` | — | — | missing | | r4 |
| 100 | `HandleLobbyGame_DepositGameMileageReq` | — | — | missing | `np::deposit_id` exists, no job. | r4 |
| 103 | `HandleLobbyGame_BoastEventInWorldNotify` | — | — | missing | InfoGate broadcast. | r3 |
| 104 | `HandleLobbyGame_TakeAccountTimeBasedPlayRewardReq` | — | — | missing | | r4 |
| 107 | `HandleLobbyGame_UpdateAccountExpReq` | — | — | missing | | r1 |
| 110 | `HandleLobbyGame_UpdateAccountContentQuotaReq` | — | — | missing | New in 381. Reply ids ❓ (`24` §9). | r1 |
| 113 | `HandleLobbyGame_UpdateAccountRecycleTimeReq` | — | — | missing | New in 381. Reply ids ❓. | r1 |

## 5. Account inventory (`AccountInventoryLobby`, max command 36)

No Rust link. No Rust account inventory daemon exists (`Program/Server/*` has none). Spec: `02-links.md` lines 142–168, `14` §3, `17`, `19`, `20` §3. All rows: Rust —, test —, state `missing`, card `r2`.

| Cmd | Name | Note |
| --- | --- | --- |
| 2 | `HandleAccountInventoryLobby_HelloOk` | Log only. |
| 3 | `HandleAccountInventoryLobby_HelloFail` | Log only. |
| 5 | `HandleAccountInventoryLobby_GetGoodsCountOk` | |
| 6 | `HandleAccountInventoryLobby_GetGoodsCountFail` | |
| 8 | `HandleAccountInventoryLobby_ListGoodsOk` | Fragment. |
| 9 | `HandleAccountInventoryLobby_ListGoodsFail` | |
| 11 | `HandleAccountInventoryLobby_TakeGoodsOk` | Fragment. |
| 12 | `HandleAccountInventoryLobby_TakeGoodsFail` | |
| 14 | `HandleAccountInventoryLobby_RetrieveLobbyServiceItemListOk` | Fragment. |
| 15 | `HandleAccountInventoryLobby_RetrieveLobbyServiceItemListFail` | |
| 17 | `HandleAccountInventoryLobby_RetrieveSingleWarehouseItemOk` | |
| 18 | `HandleAccountInventoryLobby_RetrieveSingleWarehouseItemFail` | |
| 20 | `HandleAccountInventoryLobby_TryWarehouseItemConsumptionOk` | Form B resume. |
| 21 | `HandleAccountInventoryLobby_TryWarehouseItemConsumptionFail` | |
| 23 | `HandleAccountInventoryLobby_FinishWarehouseItemConsumptionOk` | |
| 24 | `HandleAccountInventoryLobby_FinishWarehouseItemConsumptionFail` | |
| 26 | `HandleAccountInventoryLobby_CancelWarehouseItemConsumptionOk` | Log only. |
| 27 | `HandleAccountInventoryLobby_CancelWarehouseItemConsumptionFail` | |
| 29 | `HandleAccountInventoryLobby_ConsumeWarehouseItemOk` | Form B resume. |
| 30 | `HandleAccountInventoryLobby_ConsumeWarehouseItemFail` | |
| 32 | `HandleAccountInventoryLobby_RetrieveCharacterGoodsCountOk` | Delete job goods count. |
| 33 | `HandleAccountInventoryLobby_RetrieveCharacterGoodsCountFail` | |
| 34 | `HandleAccountInventoryLobby_GoodsInventoryChangedNotify` | Fragment. |

## 6. NP (Sts) dispatch (`DispatchNPLobbyMessageHandler`, 381 `0x1400cb0e0`)

Rust: trait `Np` (`Program/Server/Shared/src/np_channel.rs:61`) with `consume`, `login_presence`, `logout_presence`, `world_entry`, `create_initial_user_name`, `expire`; binary `src/bin/lobbyd-sts.rs`. All rows not listed as done: Rust —, test —, state `missing`, card `r4`.

| Handler | State | Rust / test / note |
| --- | --- | --- |
| `Handle_SrvPres_Login` | done | `Np::login_presence`; `r4_presence_timeout_and_cancel_release_ownership` |
| `Handle_SrvPres_Logout` | done | `Np::logout_presence`; `r13_retained_and_rebound_cleanup_release_presence_once` |
| `Handle_SrvAuth_ChangeMyUserNameIncomplete` | done ❓ | `Np::create_initial_user_name`; `initial_name_success_and_all_platform_maps`. ❓ The message behind the trait call is not checked. |
| `Handle_CommonSlot_GetSlot` | missing | Create (44), bind (57), try enter (24). |
| `Handle_SrvPromotionStamp_ListCurrentPromotions` | missing | `SetupNpScheduleInfoJob`. |
| `Handle_SrvPromotionStamp_GetPromotionPolicy` | missing | `SetupNpScheduleInfoJob`. |
| `Handle_SrvPromotionStamp_GetUserPromotionStatus` | missing | Start sequence. |
| `Handle_SrvPromotionStamp_AttendPromotionSecured` | missing | |
| `Handle_CommonSecondPassword_GetStatus` | missing | |
| `Handle_CommonGrade_ListGrades` | missing | |
| `Handle_CommonGrade_GetLevel` | missing | Start sequence, retained start. |
| `Handle_CommonGrade_ListAllGrades` | missing | Start sequence, retained start. |
| `Handle_SrvAuth_IsUserNameChangeableIncomplete` | missing | `ChangeUserNameJob`. |
| `Handle_SrvAuth_GetUserNameChangeableIncomplete` | missing | `ChangeUserNameJob`. |
| `Handle_SrvPres_GetUserInfo` | missing | |
| `Handle_SrvPres_LogoutUser` | missing | |
| `Handle_SrvPres_SetAppData` | missing | gamed 78. |
| `Handle_SrvPres_Message` | missing | Log only (`20` §3). |
| `Handle_SrvPres_Kick` | missing | Log only (`20` §3). |
| `Handle_SrvGameAccount_ListUserLastPresences` | missing | Start sequence. |
| `Handle_SrvProfile_GetUserProfile` | missing | Start sequence. |
| `Handle_SrvGameTime_GetGameTime` | missing | Start sequence. |
| `Handle_SrvSlot_CreateSlots` | missing | Start sequence. |
| `Handle_SrvSlot_SetData` | missing | Start sequence. |
| `Handle_SrvRMS_Inquiry` | missing | Start sequence. |
| `Handle_SrvGate_RegisterService` | missing | Log only. |
| `Handle_SrvSecondPassword_IsVerified` | missing | |
| `Handle_SrvGrade_ActivateGrade` | missing | |
| `Handle_SrvGrade_NotifyGradeChanged` | missing | |
| `Handle_SrvGrade_NotifyScoreChanged` | missing | |
| `Handle_SrvFriend_SendMessage` | missing | gamed 79. |
| `Handle_SrvContentBan_ListUserBans` | missing | Start sequence. |
| `Handle_SrvCharge_GetInfo` | missing | LobbyGate 60, try enter. |
| `Handle_SrvCharge_Start` | missing | |
| `Handle_SrvCharge_Stop` | missing | |
| `Handle_SrvCharge_RequestStop` | missing | `20` §8. 381 address `0x1400cd0f0`. |
| `Handle_SrvCharge_Abandon` | missing | |
| `Handle_SrvCharge_NotifyRemainTime` | missing | `20` §8. |
| `Handle_SrvCharge_NotifyApplyNewChargeItem` | missing | `20` §8. 381 `0x1400cd700` (IDB name wrong, `24` §2.3). |
| `Handle_SrvAuction_Escrow` | missing | |
| `Handle_SrvAuction_RollbackEscrow` | missing | |
| `Handle_SrvAuction_Deliver` | missing | |
| `Handle_SrvAuction_Bid` | missing | |
| `Handle_SrvVirtualCurrency_Deposit` | missing | Mileage deposit. |

## 7. Job classes (`12-job-map.md` §1)

| Job | Spec | Rust | Test | State | Note | Card |
| --- | --- | --- | --- | --- | --- | --- |
| AbandonChargeJob | `14` §2 | — | — | missing | | r4 |
| ActivateGradeJob | `15` §3 | `activate_grade` | `grade_off_with_session_and_malformed` | partial | Off path only. | r4 |
| AddSlotJob | `19` §1 | `account_item_service` | `item_service_off` | partial | Off path only. | r2 |
| AuctionBidJob | `14` §2 | — | — | missing | | r4 |
| AuctionDeliverJob | `15` §4.3 | — | — | missing | Post office link. | r4 |
| AuctionEscrowJob | `15` §4.1 | — | — | missing | | r4 |
| AuctionRollbackEscrowJob | `15` §4.2 | — | — | missing | Post office link. | r4 |
| BindCharacterSlotJob | `15` §2 | — | — | missing | | r4 |
| CancelWaitCharacterDeletionJob | `16` §3 | `character_mutation` | `deletion_wait_across_links` | partial | gamed restore local. | r1 |
| ChangeCharacterAppearanceJob | `17` | `account_item_service` | `item_service_off` | partial | Off path only. | r2 |
| ChangeCharacterRaceJob | `17` | same | same | partial | Off path only. | r2 |
| ChangeCharacterSexJob | `17` | same | same | partial | Off path only. | r2 |
| ChangeUserNameJob | `19` §2 | same | same | partial | Off path only. | r2 |
| CliSessionJob | `13` §1 | `Jobs` core (decision 2) | `step6_packets_of_one_session_keep_order_when_the_first_is_slow` | done | | |
| CreateCharacterJob | `16` §1 | `character_mutation` | `create_validation_and_store_errors_do_not_add_rows` | partial | No NP `GetSlot`; fixture policy. | r4 |
| CreateInitialUserNameJob | `19` §3 | `initial_user_name` | `initial_name_success_and_all_platform_maps` | done | | |
| DeleteCharacterJob | `16` §2 | `character_mutation` | `immediate_delete_reply_and_list_are_consistent` | partial | gamed delete local; second password and goods steps missing. | r1 |
| DepositMileageJob | `15` §5, `24` §3 | `np::deposit_id` only | `deposit_id_pc` | missing | | r4 |
| DepositPcCafeMileageJobManageCallback | `19` §9 | — | — | missing | | r4 |
| EndWaitGameWorldJob | `13` §3.5 | — | — | missing | Needs the world queue (§11 X3). | r3 |
| EnterGameWorldJob | `18` §4 | `Server::consume_world` | `success_single_use_and_identity_checks` | partial | No `UpdateLastPlayStart`; empty 381 lists. | r1 |
| ExtendJumpingCharacterCreationQuotaJob | `19` §5 | `account_item_service` | `item_service_zero_input` | partial | Off path only. | r2 |
| GetChargeInfoJob | `15` §1 | `charge_info` | `charge_info_off` | partial | Off path only. | r4 |
| GetGoodsCountJob | `14` §3 | `goods_unavailable` | `goods_requests_fail_with_account_inventory_unavailable` | partial | Off path only. | r2 |
| GetLobbyServiceItemJob | `15` §1 | `goods_unavailable` | same | partial | Off path only. | r2 |
| GiveupWaitGameWorldJob | `13` §3.4 | `giveup_wait` | `giveup_wait_ok` | done | | |
| InitCliSessionWlobbyTokenJob | `13` §3.2 | `init_retained` | `r13_retained_binding_collision_and_missing_session` | done | | |
| InitCliSessionWnpTokenJob | `13` §3.1 | `init` | `r2_init_rejections_echo_uniqueness_and_no_consumption` | done | | |
| KickJob | `18` §5 | `Jobs::kick_world`, `GameDaemon::kick` | `kick_in_world_ok`, `kick_in_world_timeout` | partial | World kick done. Manage demander missing. | r3 |
| LeaveGameWorldJob | `13` §3.6 | `Jobs::leave_world` | `leave_playing_checks_and_token_failure` | partial | No `EndGameWorldPlay` (`UpdateLastPlayEnd`). | r1 |
| ListGoodsJob | `14` §3 | `goods_unavailable` | same as GetGoodsCountJob | partial | Off path only. | r2 |
| ListLobbyServiceItemJob | `15` §1 | `goods_unavailable` | same | partial | Off path only. | r2 |
| LobbyJob | `13` §1 | `Jobs::checkout` / `merge` | `step6_slow_np_of_one_session_does_not_delay_another_session_on_the_link` | done | | |
| LobbyJobManager | `13` §1 | `Jobs`, `Jobs::slow` | `r9_lanes_keep_order_without_blocking_other_links_and_share_capacity` | done | | |
| LobbyJobTaskEvent | `13` §1 | job task in `LinkTask` | same | done | | |
| LobbyJobTransaction | `13` §1 | transaction map in `LinkGameDaemon` | `kick_in_world_timeout` | done | Local 10 s limit ❓ (`src/gamed_link.rs:9`). | |
| LogoutInGameWorldJob | `13` §3.7 | `Jobs::logout_in_world` | `logout_in_world_ends_session` | partial | No `EndGameWorldPlay` (`UpdateLastPlayEnd`). | r1 |
| LogoutJob | `13` §3.3 | `Jobs::logout` | `logout_table` | done | | |
| NPLobbyJobTransaction | `19` §9 | `Settings::np_timeout` | `r4_np_timeout_is_explicit_local_duration` | done | Local 5 s ❓. | |
| RecoverPcJob | `16` §4 | `character_mutation` | `restoration_failures_leave_recoverable_state_five` | partial | gamed restore local. | r1 |
| RenameCharacterJob | `17` §4 | `account_item_service` | `rename_fail_command` | partial | Off path only. | r2 |
| SecondPasswordGetStatusJob | `14` §2 | — | — | missing | | r4 |
| SecondPasswordIsVerifiedJob | `14` §2 | — | — | missing | | r4 |
| SetupNpScheduleInfoJob | `19` §8 | — | — | missing | | r4 |
| StartChargeJob | `14` §2 | — | — | missing | | r4 |
| StartCliSessionWlobbyTokenJob | `18` §2 | `start_retained` | `r13_retained_start_cancels_rebound_init_deadline` | partial | No grade steps. | r4 |
| StartCliSessionWnpTokenJob | `18` §1 | `start`, `authenticate` | `r5_r6_register_required_loads_cache_and_fragmented_character_list` | partial | Local subset. | r4 |
| StopChargeJob | `14` §2 | — | — | missing | | r4 |
| TakeAccountTimeBasedPlayRewardJob | `19` §7 | — | — | missing | | r4 |
| TakeGoodsJob | `14` §3 | `goods_unavailable` | same | partial | Off path only. | r2 |
| TryEnterGameWorldJob | `18` §3 | `enter_world` | `rejection_does_not_reserve_and_can_retry` | partial | No NP checks. | r4 |
| UpgradeJumpingCharacterStateJob | `19` §6 | — | — | missing | | r5 |

## 8. Timers (`20-handlers.md` §9)

| Timer | Rust | Test | State | Note | Card |
| --- | --- | --- | --- | --- | --- |
| `LobbyTick` | `Jobs::next_expiry` / `expire` (token desk only) | `r11_generated_token_expiry_rejects_at_boundary_and_after_sweep` | partial | `LobbyTokenDesk::Tick` done. Mileage listeners missing. | r3 |
| `LobbyCharacterDeletionScheduler` | `scheduler::run`, `Server::spawn_deletion_scheduler` | `scheduler_run_ticks_each_period_and_stops`, `scheduler_expires_wait` | done | An open session keeps the old row until relogin (`lp2-5-rust` ❓). | |
| `GameTimeScheduler` | — | — | missing | 168 hourly queues. | r3 |
| `MileageInterimDepositTimer` | — | — | missing | | r4 |
| `MileageBatchRetryDeposit` | — | — | missing | | r4 |
| `LobbydReloadInvalidRestoreTimer` | — | — | missing | Body ❓ (`20` §10). | r3 |
| `NPPing` | — | — | out of scope | Our NP channel (`np_channel.rs`) owns its keepalive ❓. | |
| `ReconnectTimer` | — | — | missing | Reconnect of the account inventory and post office links. | r3 |
| `ObjPoolTimer` | — | — | out of scope | Pool statistics. Rust has no object pools. | |
| Stop play (`CliSession::TimeoutStopPlay`) | — | — | missing | Charge end. | r4 |
| `GameWorldRegistry::Update` | — | — | missing | `LoadGameWorldCluster`, `LoadGameWorld`; can start `EndWaitGameWorldJob`. | r3 |

## 9. `LobbyDB` wrappers (`21-lobbydb-wrappers.md` §3)

Rust: `Store` / `SyncStore` (`src/store.rs:200-364`), `MemoryStore`, `MssqlStore` (`src/mssql.rs`). Tests: `tests/store.rs`, `tests/store_growth.rs`, `tests/characters.rs`, unit tests in `src/mssql.rs` (`mssql_reads_times`, `procedure_code_remains_typed`, `missing_status_is_not_success`), operator tests `tests/mssql.rs` (not run). Live SQL is §11 X10.

Done (26), with the `Store` method:

| Wrapper | `Store` method |
| --- | --- |
| `LoadGameAccount` | `load_game_account` |
| `LoadGameAccountTimeBasedPlayInfo` | `load_time_based_play_info` |
| `RegisterGameAccount` | `register_game_account` |
| `UpdateLastLogin` | `update_last_login` |
| `UpdateLastLogout` | `update_last_logout` |
| `LoadCharacterList` | `load_character_list` |
| `CheckValidityOfCharacterName` | `check_character_name` |
| `TryCharacterCreation` / `FinishCharacterCreation` / `CancelCharacterCreation` | `try_character_creation` / `finish_character_creation` / `cancel_character_creation` |
| `TryCharacterDeletion` / `FinishCharacterDeletion` / `CancelCharacterDeletion` | `try_character_deletion` / `finish_character_deletion` / `cancel_character_deletion` |
| `TryCharacterRestoration` / `FinishCharacterRestoration` | `try_character_restoration` / `finish_character_restoration` |
| `RetrieveExpiredDeletionWaitingList` / `ExpireCharacterDeletionWaiting` | `retrieve_expired_deletion_waiting_list` / `expire_character_deletion_waiting` |
| `LoadJumpingCharacterCreationQuota` | `load_jumping_creation_quota` |
| `LoadGameAccounExpInfo` / `RegisterGameAccountExpInfo` | `load_account_exp` / `insert_account_exp` |
| `LoadIncompleteLobbyService` | `load_incomplete_lobby_service` |
| `LoadCharacterCosmesisService`, `LoadChangeCharacterSexService`, `LoadChangeCharacterRaceService`, `LoadRenameCharacterService`, `LoadSlotAdditionService` | `load_service_detail(id, ServiceKind)` |

Missing (68), state `missing`:

| Card | Wrappers |
| --- | --- |
| r1 (11) | `UpdateLastPlayStart`, `UpdateLastPlayEnd`, `UpdateCharacterData`, `UpdateCharacterLevel`, `UpdateGameAccountTimeBasedPlayInfo`, `UpdateGameAccountExpInfo`, `LoadGameAccountContentQuotaList`, `UpdateGameAccountContentQuota`, `LoadGameAccountRecycleTimeList`, `UpdateGameAccountRecycleTime`, `LoadRandomStoreInitializationSchedule` |
| r2 (31) | For each of `CharacterCosmesisService`, `ChangeCharacterSexService`, `ChangeCharacterRaceService`, `CharacterRenameService`, `SlotAdditionService`: `Try…`, `UpdateLobbyServiceStateToItemConsumptionTryState_…`, `UpdateLobbyServiceStateToTaskDoneState_…`, `UpdateLobbyServiceStateToItemConsumptionFinishState_…`, `Finish…` (25); `TryExtendJumpingCharacterCreationQuota` and its 3 state updates and `FinishExtendJumpingCharacterCreationQuota` (5); `WriteUserRenameServiceLog` (1) |
| r3 (2) | `LoadGameWorldCluster`, `LoadGameWorld` |
| r4 (16) | `BindCharacterSlot`; `TryAuctionEscrow`, `CancelAuctionEscrow`, `FinishAuctionEscrow`, `TryAuctionEscrowRollback`, `FinishAuctionEscrowRollback`, `TryAuctionResultDelivery`, `FinishAuctionResultDelivery`; `LoadMileage`, `LoadMileagePolicy`, `LoadMileageFeverTime`, `LoadMileageAccumulatedTimePerDay`, `LoadRetryMileageDeposit`, `RetrieveMileageDeposit`, `TryMileageDepost` (`usp_TryMileageDeposit`), `FinishMileageDeposit` |
| r5 (8) | `TryGuildRegistration`, `FinishGuildRegistration`, `CancelGuildRegistration`, `DeleteGuild`, `TryRenameGuild`, `FinishRenameGuild`, `CancelRenameGuild`, `UpdateJumpingCharacterStateComplete` (2 procedures) |

The 4 wrappers that `21` §1 says "do not port" (348 only) are in 381 (`24` §6). The source rule makes them r1 work: gamed 110/113 and the `EnterWorldOk` lists need them.

## 10. Config keys (`01-config.md` lines 51–211)

Rust has no config file loader. Values come from `main.rs` arguments (`local-port`, `--world-port`, `--deletion-period`, `--np-game-code`, `--gamegate`, `--game-code`, `--world`) and `Settings` defaults (`src/jobs.rs:86-142`). `done` = an argument sets the value. `partial` = a `Settings` field exists, the value is a local default.

| Key | State | Rust / note | Card |
| --- | --- | --- | --- |
| `lobby-gate-port` | done | `local-port` argument | |
| `world-port` | done | `--world-port` | |
| `np-game-code` | done | `--np-game-code` (not yet in `Settings`, see §11 X11) | |
| `character-deletion-scheduler-period` | done | `--deletion-period` → `deletion_scheduler_period` | |
| `character-deletion-scheduler-concurrent-processing-count` | partial | `deletion_scheduler_batch` = 16 ❓ | r3 |
| `check-ip-mismatch` | partial | `check_retained_ipv4` ❓ key match | r3 |
| `lobby-job-transaction-life-time` | partial | `np_timeout`, `world_timeout` 5 s, link 10 s (local) | r3 |
| `lobby-token-life-time` | partial | `lobby_token_lifetime` = 120 s ❓ | r3 |
| `max-lobby-gate-session-count` | partial | `max_links` = 64 | r3 |
| `apply-charge` | partial | `charge` (config +400) ❓ key match | r4 |
| `initial-user-name-enable` | partial | local initial-name control | r4 |
| `use-grade` | partial | `grade` (config +413) | r4 |
| `use-account-inventory` | partial | always off | r2 |
| `allowed-min-character-name-length`, `allowed-max-character-name-length` | partial (2) | `CharacterPolicy::min_name_units` / `max_name_units` | r5 |
| `short-term-character-deletion-time`, `wait-character-deletion` | partial (2) | `CharacterPolicy::deletion_wait_seconds` | r5 |

Missing keys (118), by card:

| Card | Keys |
| --- | --- |
| r1 (3) | `enable-account-contents` ❓, `game-mileage-effective-duration-in-days`, `use-time-based-play-schedule` |
| r2 (18) | `account-inventory-adress`, `account-inventory-port`, `account-inventory-max-session-count`, `add-slot-item-data-ids`, `change-character-appearance-item-data-ids`, `change-character-race-item-data-ids`, `change-character-sex-item-data-ids`, `change-user-name-item-data-ids`, `enable-change-user-name-service`, `extend-jumping-character-creation-quota-item-data-ids`, `rename-character-item-data-ids`, `rename-character-by-system-item-data-ids`, `min-lobby-service-item-data-id`, `max-lobby-service-item-data-id`, `check-slot-capacity`, `check-character-goods-for-character-deletion`, `permit-enter-world-in-change-character-appearance`, `max-slot-count` |
| r3 (21) | `max-game-world-session-count`, `info-gate-port`, `info-gate-public-address`, `info-gate-public-port`, `use-info-gate`, `max-info-gate-session-count`, `manage-port`, `max-management-session-count`, `lobby-world-id`, `world-info-list-update-period`, `enable-game-time-schedule`, `reserved-game-world-wid-range-min`, `reserved-game-world-wid-range-max`, `reserved-info-gate-wid-range-min`, `reserved-info-gate-wid-range-max`, `reserved-lobby-gate-wid-range-min`, `reserved-lobby-gate-wid-range-max`, `use-bot-priority-queue`, `min-bot-score-to-apply-bot-priority-queue`, `use-grade-priority-queue`, `grade-priority-queue-game-grade-key` |
| r4 (48) | `np-address`, `np-port`, `np-app-id`, `np-client-app-id`, `np-program-id`, `max-np-session-count`, `post-office-address`, `post-office-port`, `max-postoffice-session-count`, `free-play-type`, `limit-not-charge-pc-cafe`, `p-type` ❓, `stop-play-game-world-timeout`, `enable-second-password`, `second-password-type`, `enable-rms-inquiry`, `enable-content-ban`, `content-ban-id-for-jumping-character`, `enable-friend-message-notification`, `friend-message-recipient-app-id-list`, `enable-activity-status`, `activity-pre-resting-status-period`, `activity-resting-status-period`, `game-grade-key-for-get-level`, `grade-level-group-id`, `use-grade-level`, `enable-reload-grade-while-return-to-lobby`, `enable-auction-escrow-service`, `auction-escrow-min-money`, `auction-escrow-money-item-key`, `auction-escrow-unit-money`, `enable-mileage`, `enable-game-mileage`, `enable-mileage-batch-retry-on-startup`, `enable-mileage-deposit-notification`, `mileage-batch-retry-concurrent-count`, `mileage-deposit-period`, `mileage-deposit-reason`, `mileage-deposit-request-code`, `mileage-schedule-week-count`, `game-mileage-deposit-reason-code`, `game-mileage-deposit-request-code`, `game-mileage-virtual-currency-id`, `game-cash-virtual-currency-id`, `initial-user-name-prefix-list`, `ranking-service-mode` ❓, `teen-age-boundary` ❓, `trial-play-limit-level` ❓ |
| r5 (22 + 6 = 28) | `allowed-character-name-space-amount`, `allowed-guild-name-space-amount`, `allowed-min-guild-name-length`, `allowed-max-guild-name-length`, `allowed-min-user-name-length`, `allowed-max-user-name-length`, `character-name-reuse-limit-duration`, `guild-name-reuse-limit-duration`, `enable-multilingual-for-character-name`, `enable-multilingual-for-guild-name`, `enable-multilingual-for-user-name`, `enable-numeric-for-character-name`, `enable-numeric-for-guild-name`, `forbidden-characters-for-character-name`, `forbidden-characters-for-guild-name`, `force-sentence-case-for-character-name`, `force-sentence-case-for-guild-name`, `force-sentence-case-for-user-name`, `store-game-account-name`, `store-user-ip-address`, `long-term-character-deletion-criteria-level`, `long-term-character-deletion-time` |
| r5, more (6) | `enable-jumping-character-creation`, `use-jumping-character-creation-quota`, `use-jumping-character-creation-quota-in-world`, `enable-guild-join-renewal` ❓, `enable-play-warrior` ❓, `support-languages` ❓ |

Out of scope (26): `account-hash-count`, `allow-client-ip-address-in-logs`, `cert`, `cli-session-hash-count`, `config-path`, `data-path`, `dummy-data`, `dump-type`, `enable-np-recv-completion-log`, `export-lobby-stats-file-path`, `export-lobby-stats-file-period`, `init-client-session-count`, `init-lobby-job-count`, `lobby-job-hash-count`, `log-path`, `np-ping-interval`, `root-path`, `tencent-uic-config`, `tencent-uic-enable`, `tencent-uic-inverval`, `timer-interval`, `timer-max-time`, `type-for-failed-event-object-log`, `use-perf-monitor`, `wait-time-for-reuse-pool-object`, `worker-thread`.

Check: missing 3 + 18 + 21 + 48 + 28 = 118; section 4 + 13 + 118 + 26 = 161. The ❓ keys have no code match yet; the card that owns them must find the use or mark the key "no use found".

## 11. Cross-cutting items

| # | Item | Spec | Rust | Test | State | Card |
| --- | --- | --- | --- | --- | --- | --- |
| X1 | Config file loader (`lobbyd.config.xml` → `Settings`) | `01-config.md` | — (arguments only) | — | missing | r3 |
| X2 | World registry from the database (`GameWorldRegistry::Setup`/`Update`) | `20` §9, `21` §4 | `--world` fixture, `protocol::WorldInfo` | `empty_registry_refuses_every_hello` | partial | r3 |
| X3 | World wait queue (admission with a wait) | `13` §3.4–3.5, `06-world-entry.md` | `LinkGameDaemon::admit` has no queue (`src/gamed_link.rs:509`) | `queued_reply_order_promotion_and_cleanup` (stub only) | missing | r3 |
| X4 | LobbyGate peer check (`BLDaemonInfoTable`) | `20` §2 | — | — | missing | r3 |
| X5 | Account inventory link transport (our protocol) | `02-links.md` 142–168 | — | — | missing | r2 |
| X6 | Post office link (auction posts) | `15` §4 | — | — | missing | r4 |
| X7 | NP transport: `Np` trait and `lobbyd-sts` | `18` §1.2, `20` §3 | `Np` (5 calls) | `shared_stub_keeps_presence_and_token_behavior` | partial | r4 |
| X8 | `dbrtn` → wire code maps of the callers | `21` §2, §5; `08-db-binding.md` lines 76–145 | `ProcedureError`, `procedure_error`, local codes (R15) | `memory_store_preserves_procedure_codes_and_enforces_states` | partial | r5 |
| X9 | SQL time conversion (`GETDATE()` local time against UTC read) | `08` lines 116–132; `lp2-5-rust` handoff | `SqlTimestamp::to_system_time`, UTC read | `sql_timestamp_reads_as_utc`, `mssql_reads_times` | partial | r5 |
| X10 | Live SQL run of `tests/mssql.rs` (operator) | `08` lines 179–199 | `MssqlStore` | `sql_character_lifecycle_outputs_and_cleanup` (not run) | partial | r5 |
| X11 | `np-game-code` into the job (`Settings`) | `24` §3, §7 | argument only, `main.rs` logs it | `deposit_id_long_code` | partial | r4 |
| X12 | Job core: link task, job task on a copy, `Jobs::slow` | `13` §1, decision 2 | `Jobs::checkout`/`merge`/`slow` | `step6_slow_np_of_one_session_does_not_delay_another_session_on_the_link` | done | |
| X13 | Async store (no lock across `.await`) | `scale-rules.md` §7 | `Store`, `Shared<S>` | `r5_required_store_failures_and_best_effort_updates` | done | |
| X14 | Session states (12 spec states) | `13` §2 | `jobs::State` (8 local) | `logout_table`, `logout_world_state_is_kept` | done | |

## 12. Notes

- `lp2-4-rust` "Blocked" (`main.rs` must call `LinkGameDaemon::attach`) is resolved: `src/main.rs:183` calls `gamed.attach(server.clone())`.
- `lp2-4-rust` open items stay open: 4th `q` of `Kick` ❓, `GetGameWorldKickReason` ❓, `Server::kick` returns `Ok(())` for result 2. They go to `ld381-r1`.
- No abuse case was found in this audit (no new code was read for that purpose).
