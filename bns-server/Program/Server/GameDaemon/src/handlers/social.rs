//! Doc 16 part 2: achievement and time-based play entry functions.
//! Build 381.8867.
//!
//! Title and ability installs queue a `ChatTask` through `Services::queue_skill`.
//! The lobby request (904), the reward-task initializer (1041) and the
//! time-based reward transaction (1047) go through `Services::chat`. The
//! achievement-service send (896) and the play-info reply body (1044) are
//! boundaries.
use super::skills::queue;
use super::*;

pub const SOCIAL_COMMANDS: &[u16] = &[
    ids::G_AchievementCompleteReqCmd,
    ids::G_AchievementTitleInstallReqCmd,
    ids::G_AchievementAbilityInstallReqCmd,
    ids::G_ActivateGameGradeReqCmd,
    ids::G_TakeAchievementRewardReqCmd,
    ids::G_TimeBasedPlayInfoReqCmd,
    ids::G_TakeTimeBasedPlayRewardReqCmd,
];

/// One achievement table record (DataManager slot 39), keyed by (id, step, job).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AchievementRecord {
    /// Record byte +24 ≠ 0.
    pub deprecated: bool,
    /// Record qword +176 (completed effect). 901 needs it non-zero.
    pub completed_effect: u64,
}

pub(super) fn dispatch_social(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_AchievementCompleteReqCmd => handle_achievement_complete(ctx, req),
        ids::G_AchievementTitleInstallReqCmd => handle_achievement_title_install(ctx, req),
        ids::G_AchievementAbilityInstallReqCmd => handle_achievement_ability_install(ctx, req),
        ids::G_ActivateGameGradeReqCmd => handle_activate_game_grade(ctx, req),
        ids::G_TakeAchievementRewardReqCmd => handle_take_achievement_reward(ctx, req),
        ids::G_TimeBasedPlayInfoReqCmd => handle_time_based_play_info(ctx, req),
        ids::G_TakeTimeBasedPlayRewardReqCmd => handle_take_time_based_play_reward(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn with_log(mut out: Outcome, text: &'static str) -> Outcome {
    out.logs.push(text);
    out
}

/// parity: 896 16-chat-social-handlers.md#896--g_achievementcompletereqcmd--partial partial
pub fn handle_achievement_complete(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    if !ctx.world.achievement_enabled {
        return Outcome::default();
    }
    // No client reply on any path.
    Outcome::unsupported("❓ register checks and achievement service send (daemon +5472)")
}

enum Install {
    Title,
    Ability,
}

fn install(ctx: &mut Context<'_>, req: &[u8], kind: Install) -> Outcome {
    let (command, fail, invalid, deprecated) = match kind {
        Install::Title => (ids::G_AchievementTitleInstallReqCmd, 900, 2090, 2091),
        Install::Ability => (ids::G_AchievementAbilityInstallReqCmd, 903, 2094, 2095),
    };
    if !ctx.world.achievement_enabled {
        return Outcome::default();
    }
    let mut r = Reader(req);
    let (Some(id), Some(step)) = (r.short(), r.short()) else {
        return with_log(
            Outcome::reason(fail, 2),
            "achievement install; cannot decompose",
        );
    };
    let (mut id, mut step) = (id, step);
    if id != 0 && step != 0 {
        let Some(table) = ctx.world.achievements.as_ref() else {
            return Outcome::unsupported("❓ achievement table (DataManager slot 39) not loaded");
        };
        let job = ctx.pc().job;
        let Some(record) = table
            .get(&(id, step, job))
            .or_else(|| table.get(&(id, step, 0)))
        else {
            return Outcome::reason(fail, invalid);
        };
        if record.deprecated {
            return Outcome::reason(fail, deprecated);
        }
        if matches!(kind, Install::Ability) && record.completed_effect == 0 {
            return Outcome::reason(fail, 2096);
        }
    } else {
        id = 0;
        step = 0;
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(fail, 2);
    };
    let task = match kind {
        Install::Title => ChatTask::AchievementTitleInstall { id, step },
        Install::Ability => ChatTask::AchievementAbilityInstall { id, step },
    };
    let task = SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    };
    queue(ctx, task, Outcome::reason(fail, 2))
}

/// parity: 898 16-chat-social-handlers.md#898--g_achievementtitleinstallreqcmd--partial partial
pub fn handle_achievement_title_install(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    install(ctx, req, Install::Title)
}

/// parity: 901 16-chat-social-handlers.md#901--g_achievementabilityinstallreqcmd--partial partial
pub fn handle_achievement_ability_install(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    install(ctx, req, Install::Ability)
}

/// parity: 904 16-chat-social-handlers.md#904--g_activategamegradereqcmd--partial partial
pub fn handle_activate_game_grade(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(key_type), Some(grade)) = (r.byte(), r.int()) else {
        return Outcome::log("ActivateGameGradeReq; cannot decompose");
    };
    let grade = grade as i32;
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        let mut body = 5u16.to_le_bytes().to_vec();
        body.push(key_type);
        body.extend(grade.to_le_bytes());
        return with_log(Outcome::reply(906, body), "ActivateGameGradeReq; arena");
    }
    let request = ChatRequest::ActivateGameGrade {
        world: ctx.pc().short_96,
        db_id: ctx.pc().db_id,
        key_type,
        grade,
    };
    match ctx.services.chat(&request) {
        Gate::Pass => Outcome::default(),
        Gate::Fail => Outcome::log("ActivateGameGradeReq; lobby request failed"),
        Gate::Unknown => Outcome::unsupported("❓ lobby channel (daemon +4928)"),
    }
}

/// parity: 1041 16-chat-social-handlers.md#1041--g_takeachievementrewardreqcmd--partial partial
pub fn handle_take_achievement_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TakeAchievementRewardFailCmd;
    let mut r = Reader(req);
    let (Some(id), Some(step)) = (r.short(), r.short()) else {
        return Outcome::log("TakeAchievementRewardReq; cannot decompose");
    };
    if !ctx.pc().user_present {
        return Outcome::log("TakeAchievementRewardReq; cannot find user");
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    if !ctx.world.achievement_enabled || !ctx.world.achievement_service_usable {
        return Outcome::reason(F, 2260);
    }
    let request = ChatRequest::AchievementRewardInit {
        zone,
        db_id: ctx.pc().db_id,
        id,
        step,
    };
    match ctx.services.chat(&request) {
        Gate::Pass => {}
        Gate::Fail => return Outcome::reason(F, 2),
        Gate::Unknown => return Outcome::unsupported("❓ reward task initializer 0x1402C15A0"),
    }
    let task = SkillTask {
        command: ids::G_TakeAchievementRewardReqCmd,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: ChatTask::TakeAchievementReward { id, step }.into(),
    };
    queue(
        ctx,
        task,
        with_log(
            Outcome::reason(F, 1),
            "TakeAchievementRewardReq; queue failed",
        ),
    )
}

/// parity: 1044 16-chat-social-handlers.md#1044--g_timebasedplayinforeqcmd--partial partial
pub fn handle_time_based_play_info(ctx: &mut Context<'_>, _: &[u8]) -> Outcome {
    const F: u16 = ids::G_TimeBasedPlayInfoFailCmd;
    if !ctx.world.time_play_enabled || ctx.world.arena {
        return Outcome::reason(F, 2274);
    }
    if !ctx.pc().user_present {
        return Outcome::log("TimeBasedPlayInfoReq; cannot find user");
    }
    Outcome::unsupported("❓ play-state record (user +1288) and reply 1045 body")
}

/// parity: 1047 16-chat-social-handlers.md#1047--g_taketimebasedplayrewardreqcmd--partial partial
pub fn handle_take_time_based_play_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TakeTimeBasedPlayRewardFailCmd;
    if !ctx.world.time_play_enabled || ctx.world.arena {
        return Outcome::reason(F, 2275);
    }
    let Some(target) = Reader(req).byte() else {
        return Outcome::log("TakeTimeBasedPlayRewardReq; cannot decompose");
    };
    if !ctx.pc().user_present {
        return Outcome::log("TakeTimeBasedPlayRewardReq; cannot find user");
    }
    if target > 1 {
        return Outcome::reason(F, 2276);
    }
    let Some((start, end)) = ctx.world.time_play_schedules[target as usize] else {
        return Outcome::reason(F, 2277);
    };
    let now = ctx.world.utc_seconds;
    if now < start || now > end {
        return Outcome::reason(F, 2278);
    }
    let request = ChatRequest::TimeBasedPlayReward {
        db_id: ctx.pc().db_id,
        target,
    };
    match ctx.services.chat(&request) {
        Gate::Pass => Outcome::default(),
        Gate::Fail => Outcome::reason(F, 1),
        Gate::Unknown => Outcome::unsupported("❓ user transaction 0x3F210A, event 376"),
    }
}
