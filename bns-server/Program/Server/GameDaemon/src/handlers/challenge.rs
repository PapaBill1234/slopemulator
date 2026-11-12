//! Doc 18: challenge-party entry functions. Build 381.8867.
//!
//! Zone tasks queue through `skills::queue`. Task bodies are boundaries.
//! Season state and name checks use `Services::challenge_party`.
use super::skills::queue;
use super::*;

pub const CHALLENGE_COMMANDS: &[u16] = &[
    ids::G_CreateChallengePartyReqCmd,
    ids::G_InviteChallengePartyReqCmd,
    ids::G_ConfirmInvitationChallengePartyReqCmd,
    ids::G_RejectInvitationChallengePartyReqCmd,
    ids::G_LeaveChallengePartyReqCmd,
];

/// Opaque server checks. Pass means that the check permits the request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChallengeCheck {
    /// daemon +3192, DWORD +112 bit 12: Pass = clear; Fail = set.
    SeasonOpen,
    /// `0x14012F9F0`, with rule config +1212..+1232. Missing config = Fail.
    NameValid { name: Vec<u16> },
    /// Text-filter virtual +0. The adapter resolves the requester's user.
    NameAllowed { requester: u64, name: Vec<u16> },
}

/// Test boundary results. Unknown is never approval.
#[derive(Clone, Debug)]
pub struct MemoryChallenge {
    pub season: Gate,
    pub name_rules: Gate,
    pub name_filter: Gate,
    pub calls: Vec<ChallengeCheck>,
}
impl Default for MemoryChallenge {
    fn default() -> Self {
        Self {
            season: Gate::Unknown,
            name_rules: Gate::Unknown,
            name_filter: Gate::Unknown,
            calls: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChallengeTask {
    /// 1068, kind 295, vtable `0x140BB3458`. Name buffer +144..+209.
    Create { name: Vec<u16> },
    /// 1071, kind 296, vtable `0x140BA9750`. Name buffer +144..+209.
    Invite { name: Vec<u16> },
    /// 1075, kind 298, vtable `0x140C414E8`. Wire value +144.
    /// ❓ Exact invitation-ID meaning.
    Confirm { invitation: u64 },
    /// 1078, kind 299, vtable `0x140C3FCB8`. Value +144, byte +152 = 0.
    /// The wire byte is ignored. ❓ Exact invitation-ID meaning.
    Reject { invitation: u64 },
    /// 1080, kind 301, vtable `0x140C40AF8`. Byte +144.
    /// Qwords +152/+168/+176/+184 and short +160 = 0. Padding is not modeled.
    /// ❓ Option meaning.
    Leave { option: u8 },
}
impl From<ChallengeTask> for SkillTaskKind {
    fn from(task: ChallengeTask) -> Self {
        Self::ChallengeParty(task)
    }
}

pub(super) fn dispatch_challenge(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_CreateChallengePartyReqCmd => handle_create_challenge_party(ctx, req),
        ids::G_InviteChallengePartyReqCmd => handle_invite_challenge_party(ctx, req),
        ids::G_ConfirmInvitationChallengePartyReqCmd => {
            handle_confirm_invitation_challenge_party(ctx, req)
        }
        ids::G_RejectInvitationChallengePartyReqCmd => {
            handle_reject_invitation_challenge_party(ctx, req)
        }
        ids::G_LeaveChallengePartyReqCmd => handle_leave_challenge_party(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn arena_b(ctx: &Context<'_>) -> bool {
    ctx.world.arena || session::arena_blocked(ctx.pc())
}

/// The four reply-producing entries check feature, arena, then season.
fn preflight(ctx: &mut Context<'_>, failure: u16) -> Option<Outcome> {
    if !ctx.world.challenge_party_enabled {
        return Some(Outcome::reason(failure, 2));
    }
    if arena_b(ctx) {
        return Some(Outcome::reason(failure, 5));
    }
    match ctx.services.challenge_party(&ChallengeCheck::SeasonOpen) {
        Gate::Pass => None,
        Gate::Fail => Some(Outcome::reason(failure, 2)),
        Gate::Unknown => Some(Outcome::unsupported("❓ challenge-party season state")),
    }
}

fn logged(mut out: Outcome, text: &'static str) -> Outcome {
    out.logs.push(text);
    out
}

/// Zone, native initializer, then queue. Keep all counted name units in the task.
fn queue_task(
    ctx: &mut Context<'_>,
    command: u16,
    kind: ChallengeTask,
    failure: Outcome,
) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return logged(failure, "ChallengeParty; pc has no zone");
    };
    let valid = match &kind {
        ChallengeTask::Create { name } | ChallengeTask::Invite { name } => {
            name.first().is_some_and(|unit| *unit != 0)
        }
        ChallengeTask::Confirm { invitation } => *invitation != 0,
        ChallengeTask::Reject { .. } | ChallengeTask::Leave { .. } => true,
    };
    if ctx.pc().native_kind != 1 || !valid {
        return logged(failure, "crash: challenge-party task initializer");
    }
    let task = SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: kind.into(),
    };
    queue(ctx, task, logged(failure, "ChallengeParty; queue failed"))
}

/// parity: 1068 18-challenge-party-handlers.md#1068--g_createchallengepartyreqcmd--partial partial
pub fn handle_create_challenge_party(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_CreateChallengePartyFailCmd;
    if let Some(out) = preflight(ctx, F) {
        return out;
    }
    let Some(name) = Reader(req).string(33) else {
        return Outcome::reason(F, 2404);
    };
    // Native helpers see the NUL prefix, but the task copies the full buffer.
    let prefix = party_model::name_prefix(&name).to_vec();
    match ctx.services.challenge_party(&ChallengeCheck::NameValid {
        name: prefix.clone(),
    }) {
        Gate::Pass => {}
        Gate::Fail => return Outcome::reason(F, 2405),
        Gate::Unknown => {
            return Outcome::unsupported("❓ challenge-party name rules and rule config")
        }
    }
    if ctx.world.text_filter_present {
        if !ctx.pc().user_present || ctx.pc().party_user_blocked {
            return Outcome::reason(F, 2);
        }
        let check = ChallengeCheck::NameAllowed {
            requester: ctx.pc().object_id,
            name: prefix,
        };
        match ctx.services.challenge_party(&check) {
            Gate::Pass => {}
            Gate::Fail => return Outcome::reason(F, 2405),
            Gate::Unknown => return Outcome::unsupported("❓ challenge-party text filter"),
        }
    }
    queue_task(
        ctx,
        ids::G_CreateChallengePartyReqCmd,
        ChallengeTask::Create { name },
        Outcome::reason(F, 2),
    )
}

/// parity: 1071 18-challenge-party-handlers.md#1071--g_invitechallengepartyreqcmd--partial partial
pub fn handle_invite_challenge_party(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_InviteChallengePartyFailCmd;
    if let Some(out) = preflight(ctx, F) {
        return out;
    }
    let Some(name) = Reader(req).string(33) else {
        return Outcome::reason(F, 2411);
    };
    if ctx.world.party.by_name(&name).is_none() {
        return Outcome::reason(F, 2413);
    }
    queue_task(
        ctx,
        ids::G_InviteChallengePartyReqCmd,
        ChallengeTask::Invite { name },
        Outcome::reason(F, 2),
    )
}

/// parity: 1075 18-challenge-party-handlers.md#1075--g_confirminvitationchallengepartyreqcmd--partial partial
pub fn handle_confirm_invitation_challenge_party(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ConfirmInvitationChallengePartyFailCmd;
    if let Some(out) = preflight(ctx, F) {
        return out;
    }
    let Some(invitation) = Reader(req).quad() else {
        return Outcome::reason(F, 2421);
    };
    queue_task(
        ctx,
        ids::G_ConfirmInvitationChallengePartyReqCmd,
        ChallengeTask::Confirm { invitation },
        Outcome::reason(F, 2),
    )
}

/// parity: 1078 18-challenge-party-handlers.md#1078--g_rejectinvitationchallengepartyreqcmd--partial partial
pub fn handle_reject_invitation_challenge_party(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !ctx.world.challenge_party_enabled {
        return Outcome::default();
    }
    if arena_b(ctx) {
        return Outcome::log("RejectInvitationChallengeParty; arena blocked");
    }
    // Native has no season check. It decodes the byte, then forces task +152 to 0.
    let mut r = Reader(req);
    let (Some(invitation), Some(_)) = (r.quad(), r.byte()) else {
        return Outcome::log("RejectInvitationChallengeParty; packet decompose failed");
    };
    queue_task(
        ctx,
        ids::G_RejectInvitationChallengePartyReqCmd,
        ChallengeTask::Reject { invitation },
        Outcome::default(),
    )
}

/// parity: 1080 18-challenge-party-handlers.md#1080--g_leavechallengepartyreqcmd--partial partial
pub fn handle_leave_challenge_party(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_LeaveChallengePartyFailCmd;
    if let Some(out) = preflight(ctx, F) {
        return out;
    }
    let Some(option) = Reader(req).byte() else {
        return Outcome::reason(F, 2421);
    };
    queue_task(
        ctx,
        ids::G_LeaveChallengePartyReqCmd,
        ChallengeTask::Leave { option },
        Outcome::reason(F, 2),
    )
}
