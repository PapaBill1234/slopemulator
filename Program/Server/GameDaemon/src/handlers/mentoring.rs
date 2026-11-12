//! Doc 17: mentoring entry functions. Build 381.8867.
//!
//! Zone tasks queue a `SkillTask` with a `MentoringTask` kind through
//! `Services::queue_skill`. The mentoring registry (daemon +6200) goes
//! through `Services::mentoring_registry`. Task bodies are boundaries.
use super::skills::queue;
use super::*;

pub const MENTORING_COMMANDS: &[u16] = &[
    ids::G_ChangeMentoringTypeReqCmd,
    ids::G_InviteMentoringReqCmd,
    ids::G_ConfirmInvitationMentoringReqCmd,
    ids::G_RejectInvitationMentoringReqCmd,
    ids::G_LeaveMentoringReqCmd,
    ids::G_CallMentoringMemberReqCmd,
    ids::G_WarpToMentoringMemberReqCmd,
];

/// Doc 17 shared fields.
#[derive(Clone, Debug)]
pub struct MentoringWorld {
    /// DataManager signed short +32896 > 0.
    pub enabled: bool,
    /// DataManager qword +32888 record with short +8 > 0: byte +25 maximum mentee count.
    pub max_mentees: Option<i8>,
    /// Occupied PCs, keyed by their current object IDs. Keep one PC per slot.
    pub players: BTreeMap<u64, Pc>,
    /// NUL-prefix name → current object ID. Name lookup references the PC directly.
    pub names: BTreeMap<Vec<u16>, u64>,
    /// Native ID configuration +24 and +40. The adapter must set these values.
    pub slot_mask: u64,
    pub slot_shift: u32,
}
impl Default for MentoringWorld {
    fn default() -> Self {
        Self {
            enabled: false,
            max_mentees: None,
            players: BTreeMap::new(),
            names: BTreeMap::new(),
            // Model default for existing fixtures, not a verified native configuration.
            slot_mask: 0xffff,
            slot_shift: 0,
        }
    }
}
impl MentoringWorld {
    fn slot(&self, id: u64) -> Option<u32> {
        let slot = (id & self.slot_mask).checked_shr(self.slot_shift)? as u32;
        (slot < 20000).then_some(slot)
    }

    fn eligible(pc: &Pc, mask: u16) -> Option<&Pc> {
        (mask & 1 != 0 || pc.rights & mask != 0).then_some(pc)
    }

    /// `0x1403F7D20`: type, configured slot, bound, then rights. No full-ID recheck.
    pub fn by_id(&self, id: u64, mask: u16) -> Option<&Pc> {
        if id >> 48 != 1 {
            return None;
        }
        let slot = self.slot(id)?;
        let (_, pc) = self
            .players
            .iter()
            .find(|(stored_id, _)| self.slot(**stored_id) == Some(slot))?;
        Self::eligible(pc, mask)
    }

    /// `0x1403F7EF0`: nonempty NUL prefix, direct name entry, then rights.
    pub fn by_name(&self, name: &[u16], mask: u16) -> Option<&Pc> {
        let key = party_model::name_prefix(name);
        if key.is_empty() {
            return None;
        }
        Self::eligible(self.players.get(self.names.get(key)?)?, mask)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MentoringTask {
    /// 1098, kind 304, vtable `0x140B9A840`. Type +144.
    ChangeType { mentoring_type: u8 },
    /// 1100, kind 305, vtable `0x140C39BB0`, attached to the target.
    /// +144 inviter DB ID, +152 inviter object ID, +160 type.
    Invite {
        target: u64,
        target_zone: u64,
        inviter_db_id: u64,
        inviter_object_id: u64,
        mentoring_type: u8,
    },
    /// 1103, kind 309, vtable `0x140C413F8`, attached to the mentee.
    Confirm {
        /// +144: requester PC ID, separate from the mentee task owner at +128.
        requester_object_id: u64,
        /// +152: original wire ID, even when it aliases the inviter's slot.
        inviter_object_id: u64,
        inviter_db_id: u64,
        mentor_db_id: u64,
        mentee_db_id: u64,
        max_mentees: i8,
        /// +186/+188: signed PC job bytes widened to signed shorts.
        mentor_job: i16,
        mentee_job: i16,
        /// +190/+256: wcsncpy snapshots, 33 UTF-16 units each.
        mentor_name: [u16; 33],
        mentee_name: [u16; 33],
    },
    /// 1105, kind 307, vtable `0x140BC1930`. +144, +152, +160 = 0.
    Reject { first: u64, second: u64 },
    /// 1111, kind 310, vtable `0x140BC29E8`. Target +144, +152 = 0.
    Call { target: u64 },
    /// 1115, kind 311, vtable `0x140B98E58`. Target +144, +152 = 0.
    Warp { target: u64 },
}
impl From<MentoringTask> for SkillTaskKind {
    fn from(task: MentoringTask) -> Self {
        SkillTaskKind::Mentoring(task)
    }
}

/// Result of the 1103 registry checks (daemon +6200).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MentoringRegistry {
    /// Neither check rejects.
    Clear,
    /// The mentee already has a mentoring (2452).
    AlreadyMentored,
    /// The mentor count reached the maximum (2453).
    MentorFull,
    #[default]
    Unknown,
}

pub(super) fn dispatch_mentoring(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_ChangeMentoringTypeReqCmd => handle_change_mentoring_type(ctx, req),
        ids::G_InviteMentoringReqCmd => handle_invite_mentoring(ctx, req),
        ids::G_ConfirmInvitationMentoringReqCmd => handle_confirm_invitation_mentoring(ctx, req),
        ids::G_RejectInvitationMentoringReqCmd => handle_reject_invitation_mentoring(ctx, req),
        ids::G_LeaveMentoringReqCmd => handle_leave_mentoring(ctx, req),
        ids::G_CallMentoringMemberReqCmd => handle_call_mentoring_member(ctx, req),
        ids::G_WarpToMentoringMemberReqCmd => handle_warp_to_mentoring_member(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn arena_b(ctx: &Context<'_>) -> bool {
    ctx.world.arena || session::arena_blocked(ctx.pc())
}

fn target_arena_c(ctx: &Context<'_>, pc: &Pc) -> bool {
    // cfg +237 ≠ 0 and +248 = 0, and target arena state 2/3/5/6.
    ctx.world.party_config.arena_enabled && !ctx.world.arena && session::arena_blocked(pc)
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, task: MentoringTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: task.into(),
    }
}

fn name_snapshot(name: &[u16]) -> [u16; 33] {
    let mut snapshot = [0; 33];
    for (to, from) in snapshot.iter_mut().zip(party_model::name_prefix(name)) {
        *to = *from;
    }
    snapshot
}

fn logged(mut out: Outcome, text: &'static str) -> Outcome {
    out.logs.push(text);
    out
}

/// parity: 1098 17-mentoring-handlers.md#1098--g_changementoringtypereqcmd--partial partial
pub fn handle_change_mentoring_type(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChangeMentoringTypeFailCmd;
    if !ctx.world.mentoring.enabled {
        return Outcome::reason(F, 2436);
    }
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    let Some(mentoring_type) = Reader(req).byte() else {
        return Outcome::reason(F, 2);
    };
    if mentoring_type > 2 {
        return Outcome::reason(F, 2435);
    }
    if ctx.pc().mentoring_type == mentoring_type {
        return Outcome::reason(F, 2434);
    }
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    let t = task(
        ctx,
        ids::G_ChangeMentoringTypeReqCmd,
        zone,
        MentoringTask::ChangeType { mentoring_type },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1100 17-mentoring-handlers.md#1100--g_invitementoringreqcmd--partial partial
pub fn handle_invite_mentoring(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_InviteMentoringFailCmd;
    if !ctx.world.mentoring.enabled {
        return Outcome::reason(F, 2446);
    }
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    let (Some(mentoring_type), Some(name)) = (r.byte(), r.string(33)) else {
        return Outcome::reason(F, 2);
    };
    if !(1..=2).contains(&mentoring_type) {
        return Outcome::reason(F, 3);
    }
    let world = ctx.world;
    let Some(target) = world.mentoring.by_name(&name, 6) else {
        return Outcome::reason(F, 2447);
    };
    let (object_id, db_id) = (ctx.pc().object_id, ctx.pc().db_id);
    if target.object_id == object_id || target.db_id == db_id {
        return Outcome::reason(F, 2447);
    }
    if target_arena_c(ctx, target) {
        return Outcome::reason(F, 2448);
    }
    let Some(target_zone) = target.zone else {
        return Outcome::reason(F, 2449);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    if target.native_kind != 1 {
        return logged(Outcome::reason(F, 2), "crash: mentoring invite target kind");
    }
    let t = task(
        ctx,
        ids::G_InviteMentoringReqCmd,
        zone,
        MentoringTask::Invite {
            target: target.object_id,
            target_zone,
            inviter_db_id: db_id,
            inviter_object_id: object_id,
            mentoring_type,
        },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1103 17-mentoring-handlers.md#1103--g_confirminvitationmentoringreqcmd--partial partial
pub fn handle_confirm_invitation_mentoring(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ConfirmInvitationMentoringFailCmd;
    let Some(max_mentees) = ctx.world.mentoring.max_mentees else {
        return Outcome::reason(F, 2454);
    };
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    let (Some(inviter_object_id), Some(inviter_db_id)) = (r.quad(), r.quad()) else {
        return Outcome::reason(F, 2);
    };
    let world = ctx.world;
    let Some(inviter) = world.mentoring.by_id(inviter_object_id, 6) else {
        return Outcome::reason(F, 2);
    };
    let pc = ctx.pc();
    if inviter.db_id != inviter_db_id
        || inviter.object_id == pc.object_id
        || inviter.db_id == pc.db_id
    {
        return Outcome::reason(F, 2);
    }
    let (mentor, mentee) = match pc.mentoring_type {
        2 => (pc, inviter),
        1 => (inviter, pc),
        _ => return Outcome::reason(F, 2),
    };
    let (mentor_db_id, mentee_db_id) = (mentor.db_id, mentee.db_id);
    let (mentee_zone, mentee_kind) = (mentee.zone, mentee.native_kind);
    let mentee_object_id = mentee.object_id;
    // Native snapshots these fields before either registry check.
    let confirm = MentoringTask::Confirm {
        requester_object_id: pc.object_id,
        inviter_object_id,
        inviter_db_id,
        mentor_db_id,
        mentee_db_id,
        max_mentees,
        mentor_job: i16::from(mentor.job as i8),
        mentee_job: i16::from(mentee.job as i8),
        mentor_name: name_snapshot(&mentor.name),
        mentee_name: name_snapshot(&mentee.name),
    };
    match ctx
        .services
        .mentoring_registry(mentor_db_id, mentee_db_id, max_mentees)
    {
        MentoringRegistry::Clear => {}
        MentoringRegistry::AlreadyMentored => return Outcome::reason(F, 2452),
        MentoringRegistry::MentorFull => return Outcome::reason(F, 2453),
        MentoringRegistry::Unknown => {
            return Outcome::unsupported("❓ mentoring registry (daemon +6200)")
        }
    }
    let Some(zone) = mentee_zone else {
        return Outcome::reason(F, 2);
    };
    if mentee_kind != 1 || inviter_db_id == 0 {
        return Outcome::reason(F, 2);
    }
    let mut t = task(ctx, ids::G_ConfirmInvitationMentoringReqCmd, zone, confirm);
    t.caster = mentee_object_id;
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1105 17-mentoring-handlers.md#1105--g_rejectinvitationmentoringreqcmd--partial partial
pub fn handle_reject_invitation_mentoring(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if !ctx.world.mentoring.enabled {
        return Outcome::log("RejectInvitationMentoring; invalid mentoring");
    }
    if arena_b(ctx) {
        // The native handler uses the confirm failure command.
        return Outcome::reason(1104, 5);
    }
    let mut r = Reader(req);
    let (Some(first), Some(second)) = (r.quad(), r.quad()) else {
        return Outcome::log("RejectInvitationMentoring; packet decompose failed");
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::log("RejectInvitationMentoring; pc has no zone");
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::log("RejectInvitationMentoring; invalid pc kind");
    }
    let t = task(
        ctx,
        ids::G_RejectInvitationMentoringReqCmd,
        zone,
        MentoringTask::Reject { first, second },
    );
    queue(
        ctx,
        t,
        Outcome::log("RejectInvitationMentoring; queue failed"),
    )
}

/// parity: 1107 17-mentoring-handlers.md#1107--g_leavementoringreqcmd--partial partial
pub fn handle_leave_mentoring(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_LeaveMentoringFailCmd;
    if !ctx.world.mentoring.enabled {
        return Outcome::reason(F, 2455);
    }
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    if r.byte().is_none() || r.byte().is_none() || r.quad().is_none() {
        return Outcome::reason(F, 3);
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    Outcome::unsupported("❓ leave task 308 initializer vfunc +88 and field mapping")
}

fn call_or_warp(ctx: &mut Context<'_>, req: &[u8], warp: bool) -> Outcome {
    let (command, fail) = if warp {
        (ids::G_WarpToMentoringMemberReqCmd, 1117)
    } else {
        (ids::G_CallMentoringMemberReqCmd, 1113)
    };
    let mut r = Reader(req);
    let (target, decoded) = match (r.quad(), r.short()) {
        (Some(t), Some(_)) => (t, true),
        (t, _) => (t.unwrap_or(0), false),
    };
    let reply = |reason: u16| {
        let mut body = reason.to_le_bytes().to_vec();
        body.extend(target.to_le_bytes());
        body.extend([0, 0]);
        Outcome::reply(fail, body)
    };
    if !decoded {
        return reply(3);
    }
    if !ctx.world.mentoring.enabled {
        return reply(2455);
    }
    if arena_b(ctx) {
        return reply(5);
    }
    if warp {
        let pc = ctx.pc();
        if pc.achievement_reward || pc.jumping_ticket {
            return logged(reply(2), "WarpToMentoringMember; taking achievement reward");
        }
        if pc.instant_payment {
            return logged(reply(2), "WarpToMentoringMember; try instant payment");
        }
    }
    let world = ctx.world;
    let found = world.mentoring.by_id(target, 6);
    let Some(member) = found else {
        return reply(2458);
    };
    if target_arena_c(ctx, member) {
        return reply(6);
    }
    let Some(zone) = ctx.pc().zone else {
        return reply(2);
    };
    if ctx.pc().native_kind != 1 {
        return reply(2);
    }
    let kind = if warp {
        MentoringTask::Warp { target }
    } else {
        MentoringTask::Call { target }
    };
    let t = task(ctx, command, zone, kind);
    queue(ctx, t, reply(2))
}

/// parity: 1111 17-mentoring-handlers.md#1111--g_callmentoringmemberreqcmd--partial partial
pub fn handle_call_mentoring_member(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    call_or_warp(ctx, req, false)
}

/// parity: 1115 17-mentoring-handlers.md#1115--g_warptomentoringmemberreqcmd--partial partial
pub fn handle_warp_to_mentoring_member(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    call_or_warp(ctx, req, true)
}
