//! Doc 29: NPC trade, board gacha and dungeon-step entry functions.
//! Build 381.8867. Zone tasks use `skills::queue`. Transactions and task
//! execution remain boundaries. Unknown server configuration never approves.
use super::skills::queue;
use super::*;

pub const NPC_TRADE_COMMANDS: &[u16] = &[
    ids::G_BuyReqCmd,
    ids::G_Buy2ReqCmd,
    ids::G_SellReqCmd,
    ids::G_RebuyReqCmd,
    ids::G_TradeWithNpcReqCmd,
    ids::G_BoardGachaDrawByItemReqCmd,
    ids::G_BoardGachaDrawByInstantPaymentReqCmd,
    ids::G_DungeonProgressToStepReqCmd,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NpcTradeTask {
    /// 1341: kind 68, vtable 0x140BB66B8, 0xE0 bytes.
    /// NPC +144, count DWORD +152, records +156 (native stride 4).
    Buy { npc: u64, items: Vec<(u8, u16)> },
    /// 1343: base type 122, vtable 0x140BA72B0, 0x118 bytes.
    /// NPC +152, index +160, store +162/+264, count +196, records +200,
    /// selector +272. +168/+176/+184/+192 are zero; +144 is the log label.
    Buy2 {
        npc: u64,
        index: u8,
        selector: u8,
        store: u16,
        items: Vec<(u16, u16)>,
    },
    /// 1348: kind 69, vtable 0x140BC26F8, 0xE0 bytes.
    /// NPC +144, count DWORD +152, records +156.
    Sell { npc: u64, items: Vec<(u16, u16)> },
    /// 1350: kind 70, vtable 0x140BA7C30, 0xA0 bytes. NPC +144, slot +152.
    Rebuy { npc: u64, slot: u8 },
    /// 1352: kind 71, vtable 0x140BABD80, 0xE0 bytes.
    /// NPC +144, selector +152, store +154, count +156, records +160.
    TradeWithNpc {
        npc: u64,
        selector: u8,
        store: u16,
        items: Vec<(u16, u16)>,
    },
    /// 1456: kind 121, vtable 0x140CAFE18, 0xE8 bytes.
    /// Shorts +144/+146, byte +148, NPC +152, count +160, pairs +164.
    DungeonProgressToStep {
        dungeon: u16,
        step: u16,
        selector: u8,
        npc: u64,
        items: Vec<(u16, u16)>,
    },
}
impl From<NpcTradeTask> for SkillTaskKind {
    fn from(task: NpcTradeTask) -> Self {
        SkillTaskKind::NpcTrade(task)
    }
}

/// Server-owned checks. Pass/Fail describes the named predicate, not a draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NpcTradeCheck {
    /// Global byte +2088 AND network cfg byte +404 are nonzero.
    BoardGachaEnabled,
    /// DataManager qword slot 10305 is non-null.
    BoardCostExists,
    /// Cost record byte +42 is nonzero.
    InstantPaymentEnabled,
    /// Slot 1455 resolves cost record qword +48 to an item record.
    InstantPaymentItemExists,
}
#[derive(Clone, Debug)]
pub struct MemoryNpcTrade {
    pub enabled: Gate,
    pub cost: Gate,
    pub instant: Gate,
    pub payment_item: Gate,
    pub calls: Vec<NpcTradeCheck>,
}
impl Default for MemoryNpcTrade {
    fn default() -> Self {
        Self {
            enabled: Gate::Unknown,
            cost: Gate::Unknown,
            instant: Gate::Unknown,
            payment_item: Gate::Unknown,
            calls: vec![],
        }
    }
}
impl MemoryNpcTrade {
    pub fn check(&mut self, check: &NpcTradeCheck) -> Gate {
        self.calls.push(check.clone());
        match check {
            NpcTradeCheck::BoardGachaEnabled => self.enabled,
            NpcTradeCheck::BoardCostExists => self.cost,
            NpcTradeCheck::InstantPaymentEnabled => self.instant,
            NpcTradeCheck::InstantPaymentItemExists => self.payment_item,
        }
    }
}

pub(super) fn dispatch_npc_trade(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_BuyReqCmd => handle_buy(ctx, req),
        ids::G_Buy2ReqCmd => handle_buy2(ctx, req),
        ids::G_SellReqCmd => handle_sell(ctx, req),
        ids::G_RebuyReqCmd => handle_rebuy(ctx, req),
        ids::G_TradeWithNpcReqCmd => handle_trade_with_npc(ctx, req),
        ids::G_BoardGachaDrawByItemReqCmd => handle_board_gacha_draw_by_item(ctx, req),
        ids::G_BoardGachaDrawByInstantPaymentReqCmd => {
            handle_board_gacha_draw_by_instant_payment(ctx, req)
        }
        ids::G_DungeonProgressToStepReqCmd => handle_dungeon_progress_to_step(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn user_ready(pc: &Pc) -> bool {
    pc.user_present && !pc.party_user_blocked && pc.party_user_state == 0
}
fn queue_task(
    ctx: &mut Context<'_>,
    command: u16,
    zone: u64,
    kind: NpcTradeTask,
    fail: u16,
    reason: u16,
) -> Outcome {
    // Every task in this module tests native kind after the zone check.
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(fail, reason);
    }
    let t = SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: kind.into(),
    };
    queue(ctx, t, Outcome::reason(fail, reason))
}
/// Keep count failure separate from element failure. The missing count reads -1.
#[allow(clippy::result_large_err)]
fn records<T>(
    r: &mut Reader<'_>,
    fail: u16,
    decode_reason: u16,
    count_reason: u16,
    mut element: impl FnMut(&mut Reader<'_>) -> Option<T>,
) -> Result<Vec<T>, Outcome> {
    let count = r.short().map(|v| v as i16).unwrap_or(-1);
    if !(0..16).contains(&count) {
        return Err(Outcome::reason(fail, count_reason));
    }
    (0..count)
        .map(|_| element(r).ok_or_else(|| Outcome::reason(fail, decode_reason)))
        .collect()
}
fn shorts(r: &mut Reader<'_>) -> Option<(u16, u16)> {
    Some((r.short()?, r.short()?))
}

/// parity: 1341 29-npc-trade-handlers.md#1341--g_buyreqcmd--partial partial
pub fn handle_buy(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_BuyFailCmd;
    if !user_ready(ctx.pc()) {
        return Outcome::reason(F, 314);
    }
    let mut r = Reader(req);
    let Some(npc) = r.quad() else {
        return Outcome::reason(F, 303);
    };
    let items = match records(&mut r, F, 303, 304, |r| Some((r.byte()?, r.short()?))) {
        Ok(items) => items,
        Err(out) => return out,
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    queue_task(
        ctx,
        ids::G_BuyReqCmd,
        zone,
        NpcTradeTask::Buy { npc, items },
        F,
        2,
    )
}

/// parity: 1343 29-npc-trade-handlers.md#1343--g_buy2reqcmd--partial partial
pub fn handle_buy2(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_Buy2FailCmd;
    if !user_ready(ctx.pc()) {
        return Outcome::reason(F, 350);
    }
    let mut r = Reader(req);
    let (Some(npc), Some(index), Some(selector), Some(store)) =
        (r.quad(), r.byte(), r.byte(), r.short())
    else {
        return Outcome::reason(F, 348);
    };
    if index >= 120 {
        return Outcome::reason(F, 348);
    }
    let items = match records(&mut r, F, 348, 349, shorts) {
        Ok(items) => items,
        Err(out) => return out,
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    queue_task(
        ctx,
        ids::G_Buy2ReqCmd,
        zone,
        NpcTradeTask::Buy2 {
            npc,
            index,
            selector,
            store,
            items,
        },
        F,
        2,
    )
}

/// parity: 1348 29-npc-trade-handlers.md#1348--g_sellreqcmd--partial partial
pub fn handle_sell(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SellFailCmd;
    if !user_ready(ctx.pc()) {
        return Outcome::reason(F, 322);
    }
    let mut r = Reader(req);
    let Some(npc) = r.quad() else {
        return Outcome::reason(F, 315);
    };
    let items = match records(&mut r, F, 315, 316, shorts) {
        Ok(items) => items,
        Err(out) => return out,
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    queue_task(
        ctx,
        ids::G_SellReqCmd,
        zone,
        NpcTradeTask::Sell { npc, items },
        F,
        2,
    )
}

/// parity: 1350 29-npc-trade-handlers.md#1350--g_rebuyreqcmd--partial partial
pub fn handle_rebuy(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_RebuyFailCmd;
    if !user_ready(ctx.pc()) {
        return Outcome::reason(F, 332);
    }
    let mut r = Reader(req);
    // The native entry ignores decode failure. Both locals have no initial value.
    let (npc, slot) = (r.quad(), r.byte());
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 1);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(F, 1);
    }
    let (Some(npc), Some(slot)) = (npc, slot) else {
        return Outcome::unsupported("❓ rebuy: unchecked decode, unset NPC or slot");
    };
    queue_task(
        ctx,
        ids::G_RebuyReqCmd,
        zone,
        NpcTradeTask::Rebuy { npc, slot },
        F,
        1,
    )
}

/// parity: 1352 29-npc-trade-handlers.md#1352--g_tradewithnpcreqcmd--partial partial
pub fn handle_trade_with_npc(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TradeWithNpcFailCmd;
    if !user_ready(ctx.pc()) {
        return Outcome::reason(F, 347);
    }
    let mut r = Reader(req);
    let (Some(npc), Some(selector), Some(store)) = (r.quad(), r.byte(), r.short()) else {
        return Outcome::reason(F, 337);
    };
    let items = match records(&mut r, F, 337, 338, shorts) {
        Ok(items) => items,
        Err(out) => return out,
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    queue_task(
        ctx,
        ids::G_TradeWithNpcReqCmd,
        zone,
        NpcTradeTask::TradeWithNpc {
            npc,
            selector,
            store,
            items,
        },
        F,
        2,
    )
}

#[allow(clippy::result_large_err)]
fn check(ctx: &mut Context<'_>, check: NpcTradeCheck) -> Result<bool, Outcome> {
    match ctx.services.npc_trade(&check) {
        Gate::Pass => Ok(true),
        Gate::Fail => Ok(false),
        Gate::Unknown => Err(Outcome::unsupported(
            "❓ NPC trade / board gacha server data",
        )),
    }
}
/// Preserve enable, record, payment-mode order for both draw entries.
#[allow(clippy::result_large_err)]
fn gacha_prefix(ctx: &mut Context<'_>, instant: bool) -> Result<(), Outcome> {
    let fail = if instant {
        ids::G_BoardGachaDrawByInstantPaymentFailCmd
    } else {
        ids::G_BoardGachaDrawByItemFailCmd
    };
    if !check(ctx, NpcTradeCheck::BoardGachaEnabled)? {
        return Err(Outcome::reason(fail, 2));
    }
    if !check(ctx, NpcTradeCheck::BoardCostExists)? {
        return Err(Outcome::reason(fail, 2));
    }
    if check(ctx, NpcTradeCheck::InstantPaymentEnabled)? != instant {
        return Err(Outcome::reason(fail, if instant { 2 } else { 2518 }));
    }
    if !user_ready(ctx.pc()) {
        return Err(Outcome::reason(fail, 2520));
    }
    Ok(())
}

/// parity: 1176 29-npc-trade-handlers.md#1176--g_boardgachadrawbyitemreqcmd--partial partial
pub fn handle_board_gacha_draw_by_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_BoardGachaDrawByItemFailCmd;
    if let Err(out) = gacha_prefix(ctx, false) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(_board), Some(_draw)) = (r.quad(), r.byte()) else {
        return Outcome::reason(F, 2519);
    };
    let Some(count) = r.short().map(|v| v as i16) else {
        return Outcome::reason(F, 2519);
    };
    if !(1..=10).contains(&count) {
        return Outcome::reason(F, 2519);
    }
    for _ in 0..count {
        if shorts(&mut r).is_none() {
            return Outcome::reason(F, 2519);
        }
    }
    // Native reacquires the user here. The pure PC snapshot cannot change mid-call.
    if !ctx.pc().user_present || ctx.pc().party_user_blocked {
        return Outcome::reason(F, 2);
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    Outcome::unsupported("❓ board gacha item transaction registration and task scheduler")
}

/// parity: 1179 29-npc-trade-handlers.md#1179--g_boardgachadrawbyinstantpaymentreqcmd--partial partial
pub fn handle_board_gacha_draw_by_instant_payment(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_BoardGachaDrawByInstantPaymentFailCmd;
    if let Err(out) = gacha_prefix(ctx, true) {
        return out;
    }
    if ctx.pc().trial_jumping {
        return Outcome::reason(F, 32);
    }
    let mut r = Reader(req);
    let (Some(_board), Some(_draw), Some(_value), Some(_payment)) =
        (r.quad(), r.byte(), r.int(), r.blob(8))
    else {
        return Outcome::reason(F, 2);
    };
    match check(ctx, NpcTradeCheck::InstantPaymentItemExists) {
        Ok(true) => {}
        Ok(false) => return Outcome::reason(F, 2),
        Err(out) => return out,
    }
    // Native has no zone check here. A missing second user read logs only.
    if !ctx.pc().user_present || ctx.pc().party_user_blocked {
        return Outcome::log("board gacha instant payment: user is null");
    }
    Outcome::unsupported("❓ board gacha instant-payment transaction registration")
}

/// parity: 1456 29-npc-trade-handlers.md#1456--g_dungeonprogresstostepreqcmd--partial partial
pub fn handle_dungeon_progress_to_step(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DungeonProgressToStepFailCmd;
    let mut r = Reader(req);
    let (Some(dungeon), Some(step), Some(selector), Some(npc)) =
        (r.short(), r.short(), r.byte(), r.quad())
    else {
        return Outcome::reason(F, 1067);
    };
    let items = match records(&mut r, F, 1067, 1068, shorts) {
        Ok(items) => items,
        Err(out) => return out,
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    // Native quirk: task kind/init/queue failures use 1451, not 1457.
    queue_task(
        ctx,
        ids::G_DungeonProgressToStepReqCmd,
        zone,
        NpcTradeTask::DungeonProgressToStep {
            dungeon,
            step,
            selector,
            npc,
            items,
        },
        1451,
        2,
    )
}
