//! Doc 16 part 1: chat, talk-session, social, chatting-channel, petition and
//! BoardGacha reward entry functions. Build 381.8867.
//!
//! Zone tasks queue a `SkillTask` with a `ChatTask` kind through
//! `Services::queue_skill`. Chatting-channel manager tasks and petition
//! backend requests go through `Services::chat`. Chat routines, the text
//! filter, the signature builder and petition transactions are boundaries.
use super::items3::BoardGachaRequest;
use super::skills::{queue, UNCHECKED_DECODE};
use super::*;

pub const CHAT_COMMANDS: &[u16] = &[
    ids::G_GodSayCmd,
    ids::G_MessageReqCmd,
    ids::G_MessageByItemReqCmd,
    ids::G_MessageByDiceReqCmd,
    ids::G_TalkSessionCloseReqCmd,
    ids::G_TalkSessionOpenReqCmd,
    ids::G_SetPcSocialReqCmd,
    ids::G_JoinChattingChannelReqCmd,
    ids::G_LeaveChattingChannelReqCmd,
    ids::G_ChattingServerLoginSignatureReqCmd,
    ids::G_CurrentPetitionReqCmd,
    ids::G_EndPetitionNotifyReqCmd,
    ids::G_SendPetitionChatNotifyReqCmd,
    ids::G_SubmitPetitionReqCmd,
    ids::G_CancelPetitionReqCmd,
    ids::G_FeedbackPetitionReqCmd,
    ids::G_BoardGachaTakeParticipationRewardReqCmd,
];

/// parity: 844 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 849 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 852 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 855 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 857 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 860 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 863 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 866 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 869 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 873 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 877 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 881 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 884 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 890 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// parity: 893 16-chat-social-handlers.md#844--893--qq-group--partial partial
/// QQ commands that share target `0x1400DE550` with 887 (doc 10).
pub const QQ_COMMANDS: &[u16] = &[
    ids::G_QQLoginReqCmd,
    ids::G_QQUserStatusReqCmd,
    ids::G_QQGetUnreadMsgReqCmd,
    ids::G_QQConfirmUnreadMsgReqCmd,
    ids::G_QQC2CReqCmd,
    ids::G_QQGroupCreateReqCmd,
    ids::G_QQGroupListReqCmd,
    ids::G_QQGroupBindingReqCmd,
    ids::G_QQGroupUnbindingReqCmd,
    ids::G_QQGroupMemberInviteReqCmd,
    ids::G_QQGroupMemberJoinReqCmd,
    ids::G_QQGroupMessageReqCmd,
    ids::G_QQUserChangeStatusReqCmd,
    ids::G_QQUserNameReqCmd,
    ids::G_QQGroupMemberNameReqCmd,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatTask {
    /// 58, kind 268. min +144, max +146.
    Dice { min: i16, max: i16 },
    /// 61, kind 9. q +144.
    TalkSessionClose { session: u64 },
    /// 1216, kind 8. q +144.
    TalkSessionOpen { session: u64 },
    /// 68, kind 19. Social +144.
    SetPcSocial { social: i16 },
    /// 898, kind 146, vtable `0x140BA00D8`. id +144, step +146.
    AchievementTitleInstall { id: u16, step: u16 },
    /// 901, kind 225, vtable `0x140B95610`.
    AchievementAbilityInstall { id: u16, step: u16 },
    /// 1041, 0x1A8-byte task `0x1402C13F0`.
    TakeAchievementReward { id: u16, step: u16 },
}
impl From<ChatTask> for SkillTaskKind {
    fn from(task: ChatTask) -> Self {
        SkillTaskKind::Chat(task)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatRequest {
    /// 342/345: 0x88-byte manager task, mode 2, vtable `0x140D02470`,
    /// submitted by daemon +5824 `0x1403FB2D0`. Leave: DWORD +112 = 1, channel 0xFF.
    ChattingChannel {
        command: u16,
        object_id: u64,
        leave: bool,
        channel: u8,
    },
    /// 390: petition backend request 173 (`u1i8`).
    CurrentPetition { world: i16, db_id: u64 },
    /// 1560: petition backend request 154 (`u1s2i8s2i8`).
    /// Both pairs use the PC +194 string and PC +20744 DB ID.
    CancelPetition {
        world: i16,
        identity: Vec<u16>,
        db_id: u64,
    },
    /// 904: lobby request 97 `hqqQi` (0, db_id, key type, grade) to world short_96.
    ActivateGameGrade {
        world: i16,
        db_id: u64,
        key_type: u8,
        grade: i32,
    },
    /// 1041: initializer `0x1402C15A0(task, zone, pc, db_id, id, step)`.
    AchievementRewardInit {
        zone: u64,
        db_id: u64,
        id: u16,
        step: u16,
    },
    /// 1047: user transaction (DWORD +8 = 0x3F210A), started with event 376.
    TimeBasedPlayReward { db_id: u64, target: u8 },
}

impl ChatRequest {
    /// Backend 154 payload, without the outer size. Route it to `world`.
    /// Native `0x14018D030` writes each s2 string up to the first NUL.
    pub fn cancel_petition_packet(&self) -> Option<Vec<u8>> {
        let Self::CancelPetition {
            identity, db_id, ..
        } = self
        else {
            return None;
        };
        let mut body = vec![154];
        for _ in 0..2 {
            super::party::string_body(&mut body, identity);
            body.extend(db_id.to_le_bytes());
        }
        Some(body)
    }
}

pub(super) fn dispatch_chat(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_GodSayCmd => handle_god_say(ctx, req),
        ids::G_MessageReqCmd => handle_message(ctx, req),
        ids::G_MessageByItemReqCmd => handle_message_by_item(ctx, req),
        ids::G_MessageByDiceReqCmd => handle_message_by_dice(ctx, req),
        ids::G_TalkSessionCloseReqCmd => handle_talk_session_close(ctx, req),
        ids::G_TalkSessionOpenReqCmd => handle_talk_session_open(ctx, req),
        ids::G_SetPcSocialReqCmd => handle_set_pc_social(ctx, req),
        ids::G_JoinChattingChannelReqCmd => handle_join_chatting_channel(ctx, req),
        ids::G_LeaveChattingChannelReqCmd => handle_leave_chatting_channel(ctx, req),
        ids::G_ChattingServerLoginSignatureReqCmd => {
            handle_chatting_server_login_signature(ctx, req)
        }
        ids::G_CurrentPetitionReqCmd => handle_current_petition(ctx, req),
        ids::G_EndPetitionNotifyReqCmd => handle_end_petition_notify(ctx, req),
        ids::G_SendPetitionChatNotifyReqCmd => handle_send_petition_chat_notify(ctx, req),
        ids::G_SubmitPetitionReqCmd => handle_submit_petition(ctx, req),
        ids::G_CancelPetitionReqCmd => handle_cancel_petition(ctx, req),
        ids::G_FeedbackPetitionReqCmd => handle_feedback_petition(ctx, req),
        ids::G_BoardGachaTakeParticipationRewardReqCmd => {
            handle_board_gacha_take_participation_reward(ctx, req)
        }
        _ => unreachable!("membership checked"),
    }
}

fn arena_config(ctx: &Context<'_>) -> bool {
    // World +2736 config form A (+237 byte and +248 DWORD).
    ctx.world.arena
}

fn chat_task(ctx: &Context<'_>, command: u16, zone: u64, task: ChatTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    }
}

fn chat_request(ctx: &mut Context<'_>, request: ChatRequest, fail: Outcome) -> Outcome {
    match ctx.services.chat(&request) {
        Gate::Pass => Outcome::default(),
        Gate::Fail => fail,
        Gate::Unknown => Outcome::unsupported("❓ chatting-channel manager / petition channel"),
    }
}

/// parity: 50 16-chat-social-handlers.md#50--g_godsaycmd--done ported
pub fn handle_god_say(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    // The command-only decoder cannot fail once dispatch has the command.
    Outcome::log("pcId, GodSay")
}

/// parity: 51 16-chat-social-handlers.md#51--g_messagereqcmd--partial partial
pub fn handle_message(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    const F: u16 = ids::G_MessageFailCmd;
    // Decode result ignored; no decoded field reaches a gate.
    if !ctx.world.chat_rule && ctx.pc().chat_state != 1 {
        let mut out = Outcome::reason(F, 90);
        out.logs.push("slience state");
        return out;
    }
    if ctx.world.text_filter_present {
        if !ctx.pc().user_present {
            let mut out = Outcome::reason(F, 90);
            out.logs.push("cannot find user");
            return out;
        }
        return Outcome::unsupported("❓ target lookup 0x1403F7EF0 and text filter vfunc +8");
    }
    Outcome::unsupported("❓ chat routine 0x1405F89E0")
}

/// parity: 55 16-chat-social-handlers.md#55--g_messagebyitemreqcmd--partial partial
pub fn handle_message_by_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_MessageByItemFailCmd;
    if !ctx.world.item_message_enabled {
        return Outcome::log("message by item disabled");
    }
    if arena_config(ctx) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    if r.string(400).is_none() || r.byte().is_none() || r.short().is_none() {
        return Outcome::reason(F, 96);
    }
    if ctx.pc().chat_state != 1 {
        return Outcome::reason(F, 93);
    }
    if ctx.world.text_filter_present {
        if !ctx.pc().user_present {
            return Outcome::reason(F, 93);
        }
        return Outcome::unsupported("❓ text filter vfunc +8");
    }
    Outcome::unsupported("❓ item message routine 0x1405F97B0")
}

/// parity: 58 16-chat-social-handlers.md#58--g_messagebydicereqcmd--partial partial
pub fn handle_message_by_dice(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_MessageByDiceFailCmd;
    let mut r = Reader(req);
    let (Some(min), Some(max)) = (r.short(), r.short()) else {
        return Outcome::reason(F, 2);
    };
    let (min, max) = (min as i16, max as i16);
    if ctx.pc().chat_state != 1 {
        return Outcome::reason(F, 97);
    }
    if min < 0 || max < 0 || min > max || max > 10000 {
        return Outcome::reason(F, 98);
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let task = chat_task(ctx, 58, zone, ChatTask::Dice { min, max });
    queue(ctx, task, Outcome::reason(F, 2))
}

/// parity: 61 16-chat-social-handlers.md#61--g_talksessionclosereqcmd--partial partial
pub fn handle_talk_session_close(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return Outcome::default();
    };
    let Some(session) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = chat_task(ctx, 61, zone, ChatTask::TalkSessionClose { session });
    // The queue result is ignored.
    queue(ctx, task, Outcome::default())
}

/// parity: 1216 16-chat-social-handlers.md#1216--g_talksessionopenreqcmd--partial partial
pub fn handle_talk_session_open(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TalkSessionOpenFailCmd;
    let Some(zone) = ctx.pc().zone else {
        let mut out = Outcome::reason(F, 2);
        out.logs.push("TalkSessionOpenReq; cannot find zone");
        return out;
    };
    let Some(session) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = chat_task(ctx, 1216, zone, ChatTask::TalkSessionOpen { session });
    queue(ctx, task, Outcome::reason(F, 2))
}

/// parity: 68 16-chat-social-handlers.md#68--g_setpcsocialreqcmd--partial partial
pub fn handle_set_pc_social(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SetPcSocialFailCmd;
    let Some(zone) = ctx.pc().zone else {
        let mut out = Outcome::reason(F, 2);
        out.logs.push("SetPcSocialReq failed; cannot find zone");
        return out;
    };
    let Some(social) = Reader(req).short() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = chat_task(
        ctx,
        ids::G_SetPcSocialReqCmd,
        zone,
        ChatTask::SetPcSocial {
            social: social as i16,
        },
    );
    let mut fail = Outcome::reason(F, 2);
    fail.logs.push("SetPcSocialReq ExecuteSerializedJob Fail");
    queue(ctx, task, fail)
}

/// parity: 342 16-chat-social-handlers.md#342--g_joinchattingchannelreqcmd--partial partial
pub fn handle_join_chatting_channel(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(channel) = Reader(req).byte() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let request = ChatRequest::ChattingChannel {
        command: 342,
        object_id: ctx.pc().object_id,
        leave: false,
        channel,
    };
    chat_request(ctx, request, Outcome::reason(344, 2))
}

/// parity: 345 16-chat-social-handlers.md#345--g_leavechattingchannelreqcmd--partial partial
pub fn handle_leave_chatting_channel(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    let request = ChatRequest::ChattingChannel {
        command: 345,
        object_id: ctx.pc().object_id,
        leave: true,
        channel: 0xFF,
    };
    chat_request(ctx, request, Outcome::reason(347, 2))
}

/// parity: 387 16-chat-social-handlers.md#387--g_chattingserverloginsignaturereqcmd--partial partial
pub fn handle_chatting_server_login_signature(_: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(server_type) = Reader(req).byte() else {
        return Outcome::log("WriteLatencyLogReq decompose failed");
    };
    if !matches!(server_type, 1 | 2) {
        return Outcome::reason(389, 2);
    }
    Outcome::unsupported("❓ arena lobby 0x14033ED20, BlowFish key and signature 0x1403A0800")
}

/// parity: 390 16-chat-social-handlers.md#390--g_currentpetitionreqcmd--partial partial
pub fn handle_current_petition(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    let request = ChatRequest::CurrentPetition {
        world: ctx.pc().short_96,
        db_id: ctx.pc().db_id,
    };
    // The result is ignored.
    chat_request(ctx, request, Outcome::default())
}

/// parity: 393 16-chat-social-handlers.md#393--g_endpetitionnotifyreqcmd--partial partial
pub fn handle_end_petition_notify(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::unsupported("❓ petition map daemon +2832, 0x14018CB90 / 0x14018CA00")
}

/// parity: 395 16-chat-social-handlers.md#395--g_sendpetitionchatnotifyreqcmd--done ported
pub fn handle_send_petition_chat_notify(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::log("[petition], HandleGame_FeedbackPetitionReq not supported")
}

fn petition_reply(command: u16, reason: u16) -> Outcome {
    let mut body = reason.to_le_bytes().to_vec();
    body.extend([0, 0]);
    Outcome::reply(command, body)
}

/// parity: 1557 16-chat-social-handlers.md#1557--g_submitpetitionreqcmd--partial partial
pub fn handle_submit_petition(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    const F: u16 = ids::G_SubmitPetitionFailCmd;
    if arena_config(ctx) {
        let mut out = petition_reply(F, 5);
        out.logs.push("SubmitPetitionReq: cannot process in arena");
        return out;
    }
    // Native clears only the first string buffer (0x642 bytes), then ignores
    // the decode result. The integer and second string have no zero defaults.
    // Keep malformed values at the boundary; do not invent decoded fields.
    if !ctx.pc().user_present {
        let mut out = petition_reply(F, 2);
        out.logs
            .push("[petition], SendSubmitPetitionReqPacket user is null");
        return out;
    }
    Outcome::unsupported("❓ petition channel, transaction 0x14018C2D0 and map add")
}

/// parity: 1560 16-chat-social-handlers.md#1560--g_cancelpetitionreqcmd--partial partial
pub fn handle_cancel_petition(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if arena_config(ctx) {
        let mut out = petition_reply(1562, 5);
        out.logs.push("CancelPetitionReq: cannot process in arena");
        return out;
    }
    let request = ChatRequest::CancelPetition {
        world: ctx.pc().short_96,
        identity: ctx.pc().name.clone(),
        db_id: ctx.pc().db_id,
    };
    chat_request(ctx, request, Outcome::default())
}

/// parity: 1563 16-chat-social-handlers.md#1563--g_feedbackpetitionreqcmd--partial partial
pub fn handle_feedback_petition(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if arena_config(ctx) {
        return Outcome::reason(1565, 5);
    }
    Outcome::unsupported("❓ petition map lookup, score list codec and channel send")
}

/// parity: 1185 16-chat-social-handlers.md#1185--g_boardgachatakeparticipationrewardreqcmd--partial partial
pub fn handle_board_gacha_take_participation_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_BoardGachaTakeParticipationRewardFailCmd;
    if !ctx.world.board_gacha_enabled {
        return Outcome::reason(F, 2);
    }
    let pc = ctx.pc();
    if !pc.user_present || pc.party_user_state != 0 || pc.trial_jumping {
        return Outcome::reason(F, 2);
    }
    let Some(count) = Reader(req).short() else {
        return Outcome::reason(F, 2);
    };
    let count = count as i16;
    if count <= 0 {
        return Outcome::reason(F, 3);
    }
    let request = BoardGachaRequest::RewardTransaction {
        command: 1185,
        count,
        object_id: ctx.pc().object_id,
    };
    match ctx.services.board_gacha(&request) {
        Gate::Pass => Outcome {
            changes: vec![StateChange::Queued(1185)],
            ..Outcome::default()
        },
        Gate::Fail => Outcome::reason(F, 2),
        Gate::Unknown => Outcome::unsupported("❓ BoardGacha reward transaction adapter"),
    }
}
