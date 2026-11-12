//! Doc 22: shop and store entry functions. Build 381.8867.
//!
//! Zone tasks use `skills::queue`. Tables and opaque server state use
//! `Services::shop_check`. Async mileage/instant-payment posts are boundaries.
use super::skills::{counted, queue};
use super::*;

pub const SHOP_COMMANDS: &[u16] = &[
    ids::G_RebuyItemListReqCmd,
    ids::G_RestoreVitalityReqCmd,
    ids::G_DepositMileageReqCmd,
    ids::G_RetrieveMysteriousStoreReqCmd,
    ids::G_SellOldItemReqCmd,
    ids::G_MysteriousBuyReqCmd,
    ids::G_R_RestoreEnchantFailedItemReqCmd,
    ids::G_R_ResetEnchantRestoreInfoReqCmd,
    ids::G_RandomStoreInfoReqCmd,
    ids::G_RandomStoreDrawReqCmd,
    ids::G_ExpandRandomStoreSlotReqCmd,
    ids::G_BuyRandomStoreItemReqCmd,
    ids::G_TakeRandomStoreDrawRewardReqCmd,
    ids::G_BuyCollectionItemsReqCmd,
];

/// Each check names a server-owned predicate. Pass never completes a purchase.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShopCheck {
    /// Global byte +160 != 0.
    MileageEnabled,
    /// Global unsigned DWORD +164 >= the unsigned request count.
    MileageWithinLimit { count: u16 },
    /// PC user object DWORD +120 == 0. PC user presence is checked separately.
    UserNotFatigued { pc: u64 },
    /// PC qword +2216 == 0.
    NoLeaveAction { pc: u64 },
    /// Manager slot 1023, key bytes [store, slot], returns a record.
    ExpandRecord { store: u8, slot: u8 },
    /// The expansion record qword +32 != 0.
    ExpandPaymentEnabled { store: u8, slot: u8 },
    /// Daemon cfg byte +746 == 0.
    LiveServer,
    /// Global byte +2076 != 0.
    CollectionEnabled,
    /// Manager slot 507 has a group, charge key +16 != 0, paid key +24 == item.
    CollectionGroup { group: u64, item: u64 },
    /// Manager slot 1455 has item, with subtype 0, 1 or 4.
    CollectionItem { item: u64 },
    /// Subtype-specific group ID equals the low DWORD of group.
    CollectionGroupMatch { group: u64, item: u64 },
    /// Manager slot 1455 has the charge item from group +16.
    CollectionChargeItem { group: u64 },
}

/// Test adapter. An absent result is Unknown, not Pass.
#[derive(Clone, Debug, Default)]
pub struct MemoryShop {
    pub results: BTreeMap<ShopCheck, Gate>,
    pub calls: Vec<ShopCheck>,
}
impl MemoryShop {
    pub(super) fn check(&mut self, check: &ShopCheck) -> Gate {
        self.calls.push(check.clone());
        self.results.get(check).copied().unwrap_or(Gate::Unknown)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShopTask {
    /// 80: kind 61, vtable 0x140BA7738, size 0x90.
    RebuyItemList,
    /// 944: kind 228, vtable 0x140B148E0, key qword +144.
    RetrieveMysteriousStore { key: u64 },
    /// 1008: kind 246, vtable 0x140B96C58. Depot +144, count +148, pairs +152.
    SellOldItem { depot: u8, items: Vec<(u16, u16)> },
    /// 1345: kind 229, vtable 0x140BC22F0. Seller +152, index +160,
    /// count DWORD +196, pairs +200; WORD +264 = 1.
    MysteriousBuy {
        seller: u64,
        index: u8,
        items: Vec<(u16, u16)>,
    },
    /// 1436: kind 347, vtable 0x140CF1800. Byte +144, shorts +146/+148, key +152.
    RestoreEnchantFailedItem {
        depot: u8,
        first: u16,
        second: u16,
        key: u64,
    },
    /// 1439: kind 346, vtable 0x140CF1630. Byte +144, short +146.
    ResetEnchantRestoreInfo { depot: u8, item: u16 },
    /// 1566: kind 171, vtable 0x140BC2000, size 0x90.
    RandomStoreInfo,
    /// 1569: kind 172, vtable 0x1409CEC68. Mode +144 = 2;
    /// bytes +168/+169/+170; pairs +172; count DWORD +236.
    RandomStoreDraw {
        store: u8,
        payment: u8,
        draws: u8,
        items: Vec<(u16, u16)>,
    },
    /// 1572: kind 173, vtable 0x1409CE468. Mode +144 = 2;
    /// bytes +168/+169/+170; pairs +172; count DWORD +236.
    ExpandRandomStoreSlot {
        store: u8,
        slot: u8,
        payment: u8,
        items: Vec<(u16, u16)>,
    },
    /// 1575: kind 174, vtable 0x140BB5CA8. Store +144, draw +145, slot +146,
    /// pairs +148, count DWORD +212.
    BuyRandomStoreItem {
        store: u8,
        draw: u8,
        slot: u8,
        items: Vec<(u16, u16)>,
    },
    /// 1578: kind 312, vtable 0x140BA8F80. Bytes +144/+145.
    TakeRandomStoreDrawReward { store: u8, reward: u8 },
}
impl From<ShopTask> for SkillTaskKind {
    fn from(task: ShopTask) -> Self {
        SkillTaskKind::Shop(task)
    }
}

pub(super) fn dispatch_shop(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_RebuyItemListReqCmd => handle_rebuy_item_list(ctx, req),
        ids::G_RestoreVitalityReqCmd => handle_restore_vitality(ctx, req),
        ids::G_DepositMileageReqCmd => handle_deposit_mileage(ctx, req),
        ids::G_RetrieveMysteriousStoreReqCmd => handle_retrieve_mysterious_store(ctx, req),
        ids::G_SellOldItemReqCmd => handle_sell_old_item(ctx, req),
        ids::G_MysteriousBuyReqCmd => handle_mysterious_buy(ctx, req),
        ids::G_R_RestoreEnchantFailedItemReqCmd => handle_r_restore_enchant_failed_item(ctx, req),
        ids::G_R_ResetEnchantRestoreInfoReqCmd => handle_r_reset_enchant_restore_info(ctx, req),
        ids::G_RandomStoreInfoReqCmd => handle_random_store_info(ctx, req),
        ids::G_RandomStoreDrawReqCmd => handle_random_store_draw(ctx, req),
        ids::G_ExpandRandomStoreSlotReqCmd => handle_expand_random_store_slot(ctx, req),
        ids::G_BuyRandomStoreItemReqCmd => handle_buy_random_store_item(ctx, req),
        ids::G_TakeRandomStoreDrawRewardReqCmd => handle_take_random_store_draw_reward(ctx, req),
        ids::G_BuyCollectionItemsReqCmd => handle_buy_collection_items(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn task(ctx: &Context<'_>, command: u16, zone: u64, kind: ShopTask) -> SkillTask {
    SkillTask {
        command,
        caster: ctx.pc().object_id,
        summon: false,
        zone,
        kind: kind.into(),
    }
}

fn checked(ctx: &mut Context<'_>, check: ShopCheck, fail: Outcome) -> Option<Outcome> {
    match ctx.services.shop_check(&check) {
        Gate::Pass => None,
        Gate::Fail => Some(fail),
        Gate::Unknown => Some(Outcome::unsupported("❓ shop server state / table adapter")),
    }
}

fn fatigue(ctx: &mut Context<'_>, fail: Outcome) -> Option<Outcome> {
    if !ctx.pc().user_present {
        return Some(fail);
    }
    checked(
        ctx,
        ShopCheck::UserNotFatigued {
            pc: ctx.pc().object_id,
        },
        fail,
    )
}

fn echo(command: u16, reason: u16, bytes: &[u8]) -> Outcome {
    let mut body = reason.to_le_bytes().to_vec();
    body.extend(bytes);
    Outcome::reply(command, body)
}

fn pair(r: &mut Reader<'_>) -> Option<(u16, u16)> {
    Some((r.short()?, r.short()?))
}

/// parity: 80 22-shop-handlers.md#80--g_rebuyitemlistreqcmd--partial partial
pub fn handle_rebuy_item_list(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(82, 1);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(82, 1);
    }
    let t = task(ctx, 80, zone, ShopTask::RebuyItemList);
    queue(ctx, t, Outcome::reason(82, 1))
}

/// parity: 827 22-shop-handlers.md#827--g_restorevitalityreqcmd--partial partial
pub fn handle_restore_vitality(_ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    Outcome::log("vitality: deprecated packet")
}

/// parity: 940 22-shop-handlers.md#940--g_depositmileagereqcmd--partial partial
pub fn handle_deposit_mileage(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DepositMileageFailCmd;
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        return Outcome::reason(F, 5);
    }
    if let Some(out) = checked(ctx, ShopCheck::MileageEnabled, Outcome::reason(F, 2177)) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(plan), Some(count)) = (r.short(), r.short()) else {
        return Outcome::reason(F, 2177);
    };
    if plan == 0 || count == 0 {
        return Outcome::reason(F, 2177);
    }
    if let Some(out) = checked(
        ctx,
        ShopCheck::MileageWithinLimit { count },
        Outcome::reason(F, 2179),
    ) {
        return out;
    }
    let Some(n) = r.short().filter(|n| (1..=5).contains(n)) else {
        return Outcome::reason(F, 2182);
    };
    for _ in 0..n {
        if pair(&mut r).is_none() {
            return Outcome::reason(F, 2177);
        }
    }
    let Some(n) = r.short().filter(|n| (1..=5).contains(n)) else {
        return Outcome::reason(F, 2184);
    };
    for _ in 0..n {
        if pair(&mut r).is_none() {
            return Outcome::reason(F, 2177);
        }
    }
    if ctx.pc().zone.is_none() || ctx.pc().native_kind != 1 {
        return Outcome::reason(F, 2);
    }
    Outcome::unsupported("❓ deposit mileage validator 0x1402AF440 and async post 0x140603BD0")
}

/// parity: 944 22-shop-handlers.md#944--g_retrievemysteriousstorereqcmd--partial partial
pub fn handle_retrieve_mysterious_store(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // Native ignores decode failure. The key starts at zero.
    let key = Reader(req).quad().unwrap_or(0);
    let failure = || {
        let mut body = 2u16.to_le_bytes().to_vec();
        body.extend(key.to_le_bytes());
        Outcome::reply(946, body)
    };
    let Some(zone) = ctx.pc().zone else {
        return failure();
    };
    // No native-kind gate.
    let t = task(ctx, 944, zone, ShopTask::RetrieveMysteriousStore { key });
    queue(ctx, t, failure())
}

/// parity: 1008 22-shop-handlers.md#1008--g_sellolditemreqcmd--partial partial
pub fn handle_sell_old_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_SellOldItemFailCmd;
    if let Some(out) = fatigue(ctx, Outcome::reason(F, 2291)) {
        return out;
    }
    let mut r = Reader(req);
    let Some(depot) = r.byte() else {
        return Outcome::reason(F, 2286);
    };
    if !matches!(depot, 2 | 3 | 6 | 8) {
        return Outcome::reason(F, 2286);
    }
    let Some(n) = r.short().filter(|n| *n < 16) else {
        return Outcome::reason(F, 2287);
    };
    let Some(items) = (0..n).map(|_| pair(&mut r)).collect::<Option<Vec<_>>>() else {
        return Outcome::reason(F, 2286);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(F, 2);
    }
    let t = task(ctx, 1008, zone, ShopTask::SellOldItem { depot, items });
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1345 22-shop-handlers.md#1345--g_mysteriousbuyreqcmd--partial partial
pub fn handle_mysterious_buy(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_MysteriousBuyFailCmd;
    if let Some(out) = fatigue(ctx, Outcome::reason(F, 355)) {
        return out;
    }
    let mut r = Reader(req);
    let (Some(seller), Some(index)) = (r.quad(), r.byte()) else {
        return Outcome::reason(F, 351);
    };
    let Some(items) = counted(&mut r, 16, pair) else {
        return Outcome::reason(F, 351);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(F, 2);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(F, 2);
    }
    let t = task(
        ctx,
        ids::G_MysteriousBuyReqCmd,
        zone,
        ShopTask::MysteriousBuy {
            seller,
            index,
            items,
        },
    );
    queue(ctx, t, Outcome::reason(F, 2))
}

/// parity: 1436 22-shop-handlers.md#1436--g_r_restoreenchantfaileditemreqcmd--partial partial
pub fn handle_r_restore_enchant_failed_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(depot), Some(first), Some(second), Some(key)) =
        (r.byte(), r.short(), r.short(), r.quad())
    else {
        return Outcome::reason(1438, 2);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1438, 2);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(1438, 2);
    }
    let t = task(
        ctx,
        ids::G_R_RestoreEnchantFailedItemReqCmd,
        zone,
        ShopTask::RestoreEnchantFailedItem {
            depot,
            first,
            second,
            key,
        },
    );
    queue(ctx, t, Outcome::reason(1438, 2))
}

/// parity: 1439 22-shop-handlers.md#1439--g_r_resetenchantrestoreinforeqcmd--partial partial
pub fn handle_r_reset_enchant_restore_info(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(depot), Some(item)) = (r.byte(), r.short()) else {
        return Outcome::reason(1441, 2);
    };
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1441, 2);
    };
    if ctx.pc().native_kind != 1 {
        return Outcome::reason(1441, 2);
    }
    let t = task(
        ctx,
        ids::G_R_ResetEnchantRestoreInfoReqCmd,
        zone,
        ShopTask::ResetEnchantRestoreInfo { depot, item },
    );
    queue(ctx, t, Outcome::reason(1441, 2))
}

/// parity: 1566 22-shop-handlers.md#1566--g_randomstoreinforeqcmd--partial partial
pub fn handle_random_store_info(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    let Some(zone) = ctx.pc().zone else {
        return Outcome::reason(1568, 2);
    };
    // No native-kind gate.
    let t = task(ctx, 1566, zone, ShopTask::RandomStoreInfo);
    queue(ctx, t, Outcome::reason(1568, 2))
}

/// parity: 1569 22-shop-handlers.md#1569--g_randomstoredrawreqcmd--partial partial
pub fn handle_random_store_draw(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let store = r.byte().unwrap_or(0);
    let fail = |reason| echo(1571, reason, &[store]);
    let (Some(payment), Some(draws), Some(_goods), Some(_price)) =
        (r.byte(), r.byte(), r.int(), r.blob(8))
    else {
        return fail(2115);
    };
    let Some(n) = r.short().filter(|n| *n <= 16) else {
        return fail(2115);
    };
    let mut items = Vec::new();
    for _ in 0..n {
        let Some(item) = pair(&mut r) else {
            return fail(1553);
        };
        if item.1 as i16 <= 0 {
            return fail(1553);
        }
        items.push(item);
    }
    if !(1..=2).contains(&store) || !(1..=4).contains(&payment) {
        return fail(2116);
    }
    if payment == 2 && items.is_empty() {
        return fail(2116);
    }
    if ctx.pc().instant_payment {
        return fail(2126);
    }
    if let Some(out) = checked(
        ctx,
        ShopCheck::NoLeaveAction {
            pc: ctx.pc().object_id,
        },
        fail(2125),
    ) {
        return out;
    }
    if let Some(out) = fatigue(ctx, fail(2118)) {
        return out;
    }
    let Some(zone) = ctx.pc().zone else {
        return fail(2);
    };
    if draws != 1 && draws != 5 {
        return fail(2116);
    }
    if payment == 4 {
        if let Some(out) = checked(ctx, ShopCheck::LiveServer, fail(15)) {
            return out;
        }
        // Native tests daemon arena configuration only, not the PC arena state.
        if ctx.world.arena {
            return fail(5);
        }
        return Outcome::unsupported(
            "❓ random store draw instant-payment transaction and async post",
        );
    }
    // No native-kind gate in 0x1402A4CC0.
    let t = task(
        ctx,
        ids::G_RandomStoreDrawReqCmd,
        zone,
        ShopTask::RandomStoreDraw {
            store,
            payment,
            draws,
            items,
        },
    );
    queue(ctx, t, fail(2))
}

/// parity: 1572 22-shop-handlers.md#1572--g_expandrandomstoreslotreqcmd--partial partial
pub fn handle_expand_random_store_slot(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let store = r.byte().unwrap_or(0);
    let slot = r.byte().unwrap_or(0);
    let fail = |reason| echo(1574, reason, &[store, slot]);
    let (Some(payment), Some(_goods), Some(_price)) = (r.byte(), r.int(), r.blob(8)) else {
        return echo(1574, 2130, &[store, 0xFF]);
    };
    if !(1..=2).contains(&store) {
        return fail(2131);
    }
    if let Some(out) = checked(ctx, ShopCheck::ExpandRecord { store, slot }, fail(2139)) {
        return out;
    }
    if !ctx.pc().user_present || ctx.pc().zone.is_none() {
        return fail(2);
    }
    let zone = ctx.pc().zone.unwrap();
    if payment == 4 {
        if let Some(out) = checked(ctx, ShopCheck::LiveServer, fail(15)) {
            return out;
        }
        if let Some(out) = checked(
            ctx,
            ShopCheck::ExpandPaymentEnabled { store, slot },
            fail(15),
        ) {
            return out;
        }
        if ctx.world.arena {
            return fail(5);
        }
        return Outcome::unsupported(
            "❓ random store expansion instant-payment transaction and async post",
        );
    }
    if payment != 2 {
        return fail(2131);
    }
    let Some(n) = r.short().filter(|n| (1..=16).contains(n)) else {
        return fail(2133);
    };
    let Some(items) = (0..n).map(|_| pair(&mut r)).collect::<Option<Vec<_>>>() else {
        return fail(2130);
    };
    let t = task(
        ctx,
        ids::G_ExpandRandomStoreSlotReqCmd,
        zone,
        ShopTask::ExpandRandomStoreSlot {
            store,
            slot,
            payment,
            items,
        },
    );
    queue(ctx, t, fail(2))
}

/// parity: 1575 22-shop-handlers.md#1575--g_buyrandomstoreitemreqcmd--partial partial
pub fn handle_buy_random_store_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    // Each decoded prefix byte survives a later decode failure.
    let store = r.byte().unwrap_or(0);
    let draw = r.byte().unwrap_or(0);
    let slot = r.byte();
    let fail = |reason| echo(1577, reason, &[store, draw, slot.unwrap_or(0)]);
    let Some(slot) = slot else { return fail(2145) };
    if !(1..=2).contains(&store) || !(1..=8).contains(&slot) {
        return fail(2146);
    }
    if !(1..=5).contains(&draw) {
        return fail(2146);
    }
    if let Some(out) = fatigue(ctx, fail(2148)) {
        return out;
    }
    let Some(items) = counted(&mut r, 16, pair) else {
        return fail(2145);
    };
    let Some(zone) = ctx.pc().zone else {
        return fail(2);
    };
    let t = task(
        ctx,
        ids::G_BuyRandomStoreItemReqCmd,
        zone,
        ShopTask::BuyRandomStoreItem {
            store,
            draw,
            slot,
            items,
        },
    );
    queue(ctx, t, fail(2))
}

/// parity: 1578 22-shop-handlers.md#1578--g_takerandomstoredrawrewardreqcmd--partial partial
pub fn handle_take_random_store_draw_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let store = r.byte().unwrap_or(0);
    let reward = r.byte();
    let fail = |reason| echo(1580, reason, &[store, reward.unwrap_or(0)]);
    let Some(reward) = reward else {
        return fail(2157);
    };
    if !(1..=2).contains(&store) || (reward as i8) < 0 {
        return fail(2158);
    }
    let Some(zone) = ctx.pc().zone else {
        return fail(2);
    };
    let t = task(
        ctx,
        ids::G_TakeRandomStoreDrawRewardReqCmd,
        zone,
        ShopTask::TakeRandomStoreDrawReward { store, reward },
    );
    queue(ctx, t, fail(2))
}

/// parity: 1584 22-shop-handlers.md#1584--g_buycollectionitemsreqcmd--partial partial
pub fn handle_buy_collection_items(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_BuyCollectionItemsFailCmd;
    // Arena form A here: configuration only.
    if ctx.world.arena {
        return Outcome::reason(F, 5);
    }
    if !ctx.world.account_inventory_enabled {
        return Outcome::reason(F, 15);
    }
    if let Some(out) = checked(ctx, ShopCheck::LiveServer, Outcome::reason(F, 15)) {
        return out;
    }
    if let Some(out) = checked(ctx, ShopCheck::CollectionEnabled, Outcome::reason(F, 20)) {
        return out;
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    if ctx.pc().instant_payment {
        return Outcome::reason(F, 29);
    }
    if !ctx.pc().user_present {
        return Outcome::reason(F, 2);
    }
    let mut r = Reader(req);
    if r.blob(8).is_none() {
        return Outcome::reason(F, 2504);
    }
    let Some(n) = r.short().filter(|n| *n <= 10) else {
        return Outcome::reason(F, 2505);
    };
    let mut items = Vec::new();
    for _ in 0..n {
        let (Some(goods), Some(_price), Some(group), Some(item)) =
            (r.int(), r.blob(8), r.quad(), r.quad())
        else {
            return Outcome::reason(F, 2504);
        };
        items.push((goods as i32, group, item));
    }
    // The decoder has already limited all price lengths to eight bytes.
    for (goods, group, item) in items {
        if goods <= 0 {
            return Outcome::reason(F, 2506);
        }
        if group == 0 {
            return Outcome::reason(F, 2508);
        }
        if item == 0 {
            return Outcome::reason(F, 2509);
        }
        if let Some(out) = checked(
            ctx,
            ShopCheck::CollectionGroup { group, item },
            Outcome::reason(F, 2508),
        ) {
            return out;
        }
        if let Some(out) = checked(
            ctx,
            ShopCheck::CollectionItem { item },
            Outcome::reason(F, 2509),
        ) {
            return out;
        }
        if let Some(out) = checked(
            ctx,
            ShopCheck::CollectionGroupMatch { group, item },
            Outcome::reason(F, 2510),
        ) {
            return out;
        }
        if let Some(out) = checked(
            ctx,
            ShopCheck::CollectionChargeItem { group },
            Outcome::reason(F, 2508),
        ) {
            return out;
        }
    }
    Outcome::unsupported("❓ collection purchase transaction, charge item mapping and async post")
}
