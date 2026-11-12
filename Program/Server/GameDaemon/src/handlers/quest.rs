//! Doc 20: quest lists, discovery and survey entry functions. Build 381.8867.
//!
//! Zone tasks use `Services::queue_skill`. Quest tables, survey data and task
//! bodies remain adapter boundaries. Configuration is server-owned.
use super::skills::{counted, queue};
use super::*;

pub const QUEST_COMMANDS: &[u16] = &[
    ids::G_QuestListReqCmd,
    ids::G_QuestCompleteListReqCmd,
    ids::G_DiscoveryAreaListReqCmd,
    ids::G_AddDiscoveryAreaReqCmd,
    ids::G_RegisterSurveyReqCmd,
    ids::G_AnswerSurveyReqCmd,
    ids::G_AnswerExtraSurveyReqCmd,
    ids::G_RefreshSimpleQuestPackReqCmd,
    ids::G_R_SkipQuestTeleportReqCmd,
    ids::G_ChangeToReplayQuestReqCmd,
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum QuestConfig {
    #[default]
    Unknown,
    Known(QuestRules),
}

/// Doc 20: daemon configuration, not request fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestRules {
    /// Daemon word +2752. Command 1205 requires 2.
    pub region: u16,
    /// Rule config (daemon +2744) byte +1196.
    pub refresh_enabled: bool,
    /// Rule config signed DWORD +40.
    pub skip_max_level: i32,
    /// Rule config byte +44.
    pub skip_mastery_enabled: bool,
    /// Rule config signed DWORD +48.
    pub skip_max_mastery: i32,
}

/// Test/default configuration boundary. Unknown is never approval.
#[derive(Clone, Debug, Default)]
pub struct MemoryQuest {
    pub config: QuestConfig,
    /// Optional ordered reads, including the second 1017 feature check.
    pub configs: std::collections::VecDeque<QuestConfig>,
    pub config_reads: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestTask {
    /// 86, kind 100, vtable `0x140C35A78`, size 0x90.
    List,
    /// 89, kind 99, vtable `0x140BC2500`, size 0x90.
    CompleteList,
    /// 93, kind 29, vtable `0x140C3FC10`, size 0x90.
    DiscoveryAreaList,
    /// 96, kind 30, vtable `0x140C3D180`, short +144.
    AddDiscoveryArea { area: i16 },
    /// 398, kind 39, vtable `0x140B20018`, mode DWORD +144 = 0.
    /// Bytes +3952 (at most 9), count short +3962.
    RegisterSurvey { orders: Vec<u8> },
    /// 401, same task, mode +144 = 1, bytes +148/+149, UTF-16 +150.
    AnswerSurvey {
        survey: u8,
        answer: u8,
        text: Vec<u16>,
    },
    /// 1017, kind 257, vtable `0x140A9CEC8`, byte +144 = 1.
    RefreshSimpleQuestPack,
    /// 1205, kind 360, vtable `0x140AF0170`, bytes +144/+145, short +146 = 0.
    SkipQuestTeleport { level: i8, mastery: i8 },
    /// 1420, kind 185, vtable `0x140BC2288`, short +144.
    ChangeToReplayQuest { quest: i16 },
}
impl From<QuestTask> for SkillTaskKind {
    fn from(task: QuestTask) -> Self {
        SkillTaskKind::Quest(task)
    }
}

pub(super) fn dispatch_quest(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_QuestListReqCmd => handle_quest_list(ctx, req),
        ids::G_QuestCompleteListReqCmd => handle_quest_complete_list(ctx, req),
        ids::G_DiscoveryAreaListReqCmd => handle_discovery_area_list(ctx, req),
        ids::G_AddDiscoveryAreaReqCmd => handle_add_discovery_area(ctx, req),
        ids::G_RegisterSurveyReqCmd => handle_register_survey(ctx, req),
        ids::G_AnswerSurveyReqCmd => handle_answer_survey(ctx, req),
        ids::G_AnswerExtraSurveyReqCmd => handle_answer_extra_survey(ctx, req),
        ids::G_RefreshSimpleQuestPackReqCmd => handle_refresh_simple_quest_pack(ctx, req),
        ids::G_R_SkipQuestTeleportReqCmd => handle_r_skip_quest_teleport(ctx, req),
        ids::G_ChangeToReplayQuestReqCmd => handle_change_to_replay_quest(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, task: QuestTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    }
}

fn list(ctx: &mut Context<'_>, command: u16, fail: u16, kind: QuestTask) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(fail, 2);
    };
    // These three native initializers do not test the PC kind.
    let t = task(ctx, command, zone, kind);
    queue(ctx, t, Outcome::reason(fail, 2))
}

/// parity: 86 20-quest-handlers.md#86--g_questlistreqcmd--partial partial
pub fn handle_quest_list(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    list(ctx, ids::G_QuestListReqCmd, 88, QuestTask::List)
}

/// parity: 89 20-quest-handlers.md#89--g_questcompletelistreqcmd--partial partial
pub fn handle_quest_complete_list(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    list(
        ctx,
        ids::G_QuestCompleteListReqCmd,
        91,
        QuestTask::CompleteList,
    )
}

/// parity: 93 20-quest-handlers.md#93--g_discoveryarealistreqcmd--partial partial
pub fn handle_discovery_area_list(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    list(
        ctx,
        ids::G_DiscoveryAreaListReqCmd,
        95,
        QuestTask::DiscoveryAreaList,
    )
}

/// parity: 96 20-quest-handlers.md#96--g_adddiscoveryareareqcmd--partial partial
pub fn handle_add_discovery_area(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // Native ignores decode failure and does not initialize the short.
    let area = Reader(req).short();
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(98, 2);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(98, 2);
    }
    let Some(area) = area else {
        return Outcome::unsupported("❓ add discovery area: unchecked decode, unset short");
    };
    let t = task(
        ctx,
        ids::G_AddDiscoveryAreaReqCmd,
        zone,
        QuestTask::AddDiscoveryArea { area: area as i16 },
    );
    queue(ctx, t, Outcome::reason(98, 2))
}

fn register_failure() -> Outcome {
    // hEc: reason 2, order -1. Do not drop the last byte.
    Outcome::reply(400, vec![2, 0, 0xFF])
}

/// parity: 398 20-quest-handlers.md#398--g_registersurveyreqcmd--partial partial
pub fn handle_register_survey(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(orders) = counted(&mut Reader(req), 9, |r| r.byte()) else {
        return register_failure();
    };
    let Some(zone) = ctx.pc().zone else {
        return register_failure();
    };
    if ctx.pc().native_kind != 1 {
        return register_failure();
    }
    let t = task(
        ctx,
        ids::G_RegisterSurveyReqCmd,
        zone,
        QuestTask::RegisterSurvey { orders },
    );
    queue(ctx, t, register_failure())
}

/// parity: 401 20-quest-handlers.md#401--g_answersurveyreqcmd--partial partial
pub fn handle_answer_survey(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(survey), Some(answer), Some(mut text)) = (r.byte(), r.byte(), r.string(1900)) else {
        return Outcome::reason(403, 2);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(403, 2);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(403, 2);
    }
    // The decoder retains embedded NULs. Native wcscpy_s stops at the first one.
    if let Some(nul) = text.iter().position(|&c| c == 0) {
        text.truncate(nul);
    }
    let t = task(
        ctx,
        ids::G_AnswerSurveyReqCmd,
        zone,
        QuestTask::AnswerSurvey {
            survey,
            answer,
            text,
        },
    );
    queue(ctx, t, Outcome::reason(403, 2))
}

/// parity: 404 20-quest-handlers.md#404--g_answerextrasurveyreqcmd--partial partial
pub fn handle_answer_extra_survey(_ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    if r.byte().is_none() || r.string(1900).is_none() {
        return Outcome::reason(406, 2);
    }
    let mut out = Outcome::reply(405, Vec::new());
    out.logs.push("answer extra survey: client text log");
    out
}

/// parity: 1017 20-quest-handlers.md#1017--g_refreshsimplequestpackreqcmd--partial partial
pub fn handle_refresh_simple_quest_pack(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RefreshSimpleQuestPackFailCmd;
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        return Outcome::reason(F, 5);
    }
    match ctx.services.quest_config() {
        QuestConfig::Known(rules) if !rules.refresh_enabled => return Outcome::reason(F, 2),
        QuestConfig::Known(_) => {}
        QuestConfig::Unknown => {
            return Outcome::unsupported("❓ quest refresh configuration +1196")
        }
    }
    // Descriptor h reads only the command header. Dispatch already supplies it.
    // Native reads the feature flag again after decode.
    match ctx.services.quest_config() {
        QuestConfig::Known(rules) if !rules.refresh_enabled => return Outcome::reason(F, 2),
        QuestConfig::Known(_) => {}
        QuestConfig::Unknown => {
            return Outcome::unsupported("❓ quest refresh configuration +1196")
        }
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    // Native has no PC-kind gate here.
    let t = task(
        ctx,
        ids::G_RefreshSimpleQuestPackReqCmd,
        zone,
        QuestTask::RefreshSimpleQuestPack,
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1205 20-quest-handlers.md#1205--g_r_skipquestteleportreqcmd--partial partial
pub fn handle_r_skip_quest_teleport(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_R_SkipQuestTeleportFailCmd;
    let rules = match ctx.services.quest_config() {
        QuestConfig::Known(rules) => rules,
        QuestConfig::Unknown => return Outcome::unsupported("❓ skip quest configuration"),
    };
    if rules.region != 2 {
        return Outcome::reason(F, 2627);
    }
    let mut r = Reader(req);
    let (Some(level), Some(mastery)) = (r.byte(), r.byte()) else {
        return Outcome::reason(F, 2627);
    };
    let (level, mastery) = (level as i8, mastery as i8);
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    if level <= 0 || i32::from(level) > rules.skip_max_level {
        return Outcome::reason(F, 2630);
    }
    if rules.skip_mastery_enabled && (mastery < 0 || i32::from(mastery) > rules.skip_max_mastery) {
        return Outcome::reason(F, 2630);
    }
    // Task initialization imposes fixed limits, after the configuration gates.
    if ctx.pc().native_kind != 1 || !(1..=70).contains(&level) || !(0..=50).contains(&mastery) {
        return Outcome::reason(F, 2);
    }
    let t = task(
        ctx,
        ids::G_R_SkipQuestTeleportReqCmd,
        zone,
        QuestTask::SkipQuestTeleport { level, mastery },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1420 20-quest-handlers.md#1420--g_changetoreplayquestreqcmd--partial partial
pub fn handle_change_to_replay_quest(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // Native ignores decode failure and does not initialize the short.
    let quest = Reader(req).short();
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1422, 1);
    };
    let Some(quest) = quest else {
        return Outcome::unsupported("❓ change to replay quest: unchecked decode, unset short");
    };
    // Native has no PC-kind gate here.
    let t = task(
        ctx,
        ids::G_ChangeToReplayQuestReqCmd,
        zone,
        QuestTask::ChangeToReplayQuest {
            quest: quest as i16,
        },
    );
    queue(ctx, t, Outcome::reason(1422, 1))
}
