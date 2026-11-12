//! Doc 12c part 2: ability passing (830), gem card stub (833), weapon
//! appearance (947), exchange (1010), custom dress (1050), item use and field
//! items (1233–1251) and pouches (1296–1321). Build 381.8867.
//!
//! Entry functions decode, gate and queue an `ItemTask` with an `ItemUseTask`
//! kind. Task actions, initializers and validators are adapter boundaries.
use super::items::reason_u64;
use super::items2::{count, pair_fail, pairs, post, u0_passes, KindCheck, Post};
use super::*;

pub const ITEM5_COMMANDS: &[u16] = &[
    ids::G_PassingItemAbilityReqCmd,
    ids::G_PickGemCardReqCmd,
    ids::G_ChangeWeaponAppearanceReqCmd,
    ids::G_ExchangeItemReqCmd,
    ids::G_MakeCustomDressReqCmd,
    ids::G_UseGroceryReqCmd,
    ids::G_UseGroceryToGroundReqCmd,
    ids::G_UseEquippedReqCmd,
    ids::G_UseEquippedToGroundReqCmd,
    ids::G_UseFieldItemReqCmd,
    ids::G_UseFieldItemToGroundReqCmd,
    ids::G_ThrowFieldItemReqCmd,
    ids::G_PickupFieldItemReqCmd,
    ids::G_PutdownFieldItemReqCmd,
    ids::G_PickupDeadBodyReqCmd,
    ids::G_OpenItemPouchReqCmd,
    ids::G_PrivateItemPouchListReqCmd,
    ids::G_CloseItemPouchReqCmd,
    ids::G_PickupItemReqCmd,
    ids::G_TakeContributionRewardReqCmd,
    ids::G_DistributeItemReqCmd,
    ids::G_CompleteDistributionReqCmd,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemUseTask {
    /// 830, kind 303. Qword prefix and three counted short-pair groups.
    PassingItemAbility {
        prefix: u64,
        groups: [Vec<(u16, u16)>; 3],
    },
    /// 1050, kind 287. First entries are two separate bytes.
    MakeCustomDress {
        target: u16,
        first: Vec<(u8, u8)>,
        pairs: Vec<(u16, u16)>,
    },
    /// 1233, 0xB8 task. Fields at +144, +148, +152, +160, +168.
    UseGrocery {
        a: u16,
        b: u32,
        c: u64,
        d: u16,
        e: u64,
    },
    /// 1234, same task. Target qword +152 = 0; shorts +176/+178/+180.
    UseGroceryToGround {
        a: u16,
        b: u32,
        position: [u16; 3],
        d: u16,
        e: u64,
    },
    /// 1236, 0xA8 task. Fields at +144, +152, +160.
    UseEquipped { a: u16, b: u64, c: u16 },
    /// 1237, same task. +144, +162, +164, +166, +160; target +152 = 0.
    UseEquippedToGround { a: u16, position: [u16; 3], c: u16 },
    /// 1242, 0xB0 task, mode 0. Fields at +152, +166, +168.
    UseFieldItem { target: u64, a: u16, b: u8 },
    /// 1247, inventory factory kind 23. Selector +152.
    PickupFieldItem { selector: u64 },
    /// 1251, inventory factory kind 25. Selector +152.
    PickupDeadBody { selector: u64 },
    /// 1296 (`close` false) and 1303 (`close` true): `0x1405188C0` last argument.
    ItemPouch { pouch: u64, close: bool },
    /// 1299, kind 75. Selector +144.
    PrivateItemPouchList { selector: u64 },
    /// 1306, kind 63.
    PickupItem {
        pouch: u64,
        selector: u8,
        entries: Vec<u8>,
    },
    /// 1311, kind 79.
    TakeContributionReward { selector: u64 },
    /// 1316, kind 231. The name is kept only in the task, never in logs.
    DistributeItem {
        pouch: u64,
        name: Vec<u16>,
        entries: Vec<u8>,
    },
    /// 1321, kind 232.
    CompleteDistribution { pouch: u64 },
}
impl From<ItemUseTask> for ItemTaskKind {
    fn from(task: ItemUseTask) -> Self {
        ItemTaskKind::Use(task)
    }
}

pub(super) fn dispatch_item5(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_PassingItemAbilityReqCmd => handle_passing_item_ability(ctx, req),
        ids::G_PickGemCardReqCmd => handle_pick_gem_card(ctx, req),
        ids::G_ChangeWeaponAppearanceReqCmd => handle_change_weapon_appearance(ctx, req),
        ids::G_ExchangeItemReqCmd => handle_exchange_item(ctx, req),
        ids::G_MakeCustomDressReqCmd => handle_make_custom_dress(ctx, req),
        ids::G_UseGroceryReqCmd => handle_use_grocery(ctx, req),
        ids::G_UseGroceryToGroundReqCmd => handle_use_grocery_to_ground(ctx, req),
        ids::G_UseEquippedReqCmd => handle_use_equipped(ctx, req),
        ids::G_UseEquippedToGroundReqCmd => handle_use_equipped_to_ground(ctx, req),
        ids::G_UseFieldItemReqCmd => handle_use_field_item(ctx, req),
        ids::G_UseFieldItemToGroundReqCmd
        | ids::G_ThrowFieldItemReqCmd
        | ids::G_PutdownFieldItemReqCmd => handle_u_codec_entry(ctx, command, req),
        ids::G_PickupFieldItemReqCmd => handle_pickup_field_item(ctx, req),
        ids::G_PickupDeadBodyReqCmd => handle_pickup_dead_body(ctx, req),
        ids::G_OpenItemPouchReqCmd => {
            handle_item_pouch(ctx, req, ids::G_OpenItemPouchReqCmd, false)
        }
        ids::G_CloseItemPouchReqCmd => {
            handle_item_pouch(ctx, req, ids::G_CloseItemPouchReqCmd, true)
        }
        ids::G_PrivateItemPouchListReqCmd => handle_private_item_pouch_list(ctx, req),
        ids::G_PickupItemReqCmd => handle_pickup_item(ctx, req),
        ids::G_TakeContributionRewardReqCmd => handle_take_contribution_reward(ctx, req),
        ids::G_DistributeItemReqCmd => handle_distribute_item(ctx, req),
        ids::G_CompleteDistributionReqCmd => handle_complete_distribution(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

const UNCHECKED_DECODE: &str =
    "❓ unchecked item-use decode (short body, field not zero-initialized)";

/// Two `hh` replies in order (wrong object kind paths).
fn two_reasons(first: (u16, u16), second: (u16, u16)) -> Outcome {
    let mut out = Outcome::reason(first.0, first.1);
    out.replies
        .extend(Outcome::reason(second.0, second.1).replies);
    out
}
/// Object kind: ID bits 48..63.
fn object_kind(id: u64) -> u16 {
    (id >> 48) as u16
}

// ---- Ability, appearance, exchange, dress ---------------------------------------

/// parity: 830 12c-item-handlers.md#command-830--g_passingitemabilityreqcmd partial
pub fn handle_passing_item_ability(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_PassingItemAbilityFailCmd;
    if !ctx.world.ability_passing_enabled {
        return reason_u64(F, 2013, 0);
    }
    if !u0_passes(ctx) {
        return reason_u64(F, 2030, 0);
    }
    if ctx.pc().zone.is_none() {
        return reason_u64(F, 2, 0);
    }
    let mut r = Reader(req);
    let Some(prefix) = r.quad() else {
        return reason_u64(F, 2013, 0);
    };
    let mut groups: [Vec<(u16, u16)>; 3] = Default::default();
    for (group, (max, count_reason)) in groups.iter_mut().zip([(1, 2017), (1, 2021), (8, 2023)]) {
        *group = match pairs(&mut r, 0, max) {
            Ok(p) => p,
            Err(true) => return reason_u64(F, count_reason, prefix),
            Err(false) => return reason_u64(F, 2013, prefix),
        };
    }
    // Initializer `0x140534270` failure sends no reply; the queue adapter
    // reports it as Unknown. Queue failure: reason 2 with the qword echo.
    let task = ItemUseTask::PassingItemAbility { prefix, groups };
    let fail = reason_u64(F, 2, prefix);
    post(ctx, 830, fail, KindCheck::None, Post::Queue, task)
}

/// parity: 833 12c-item-handlers.md#command-833--g_pickgemcardreqcmd ported
/// The full target logs the requester and returns true.
pub fn handle_pick_gem_card(_: &mut Context<'_>, _: &[u8]) -> Outcome {
    Outcome::log("pick gem card")
}

/// parity: 947 12c-item-handlers.md#command-947--g_changeweaponappearancereqcmd partial
pub fn handle_change_weapon_appearance(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ChangeWeaponAppearanceFailCmd;
    if !ctx.world.weapon_appearance_enabled {
        return Outcome::reason(F, 2189);
    }
    if ctx.world.arena || session::arena_blocked(ctx.pc()) {
        return Outcome::reason(F, 5);
    }
    let mut r = Reader(req);
    let (Some(_), Some(_), Some(_)) = (r.byte(), r.short(), r.short()) else {
        return Outcome::reason(F, 2190);
    };
    if pairs(&mut r, 0, 7).is_err() {
        return Outcome::reason(F, 2190);
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    Outcome::unsupported("❓ weapon appearance validator 0x1402B8C80")
}

/// parity: 1010 12c-item-handlers.md#command-1010--g_exchangeitemreqcmd partial
pub fn handle_exchange_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_ExchangeItemFailCmd;
    if !u0_passes(ctx) {
        return Outcome::reason(F, 2302);
    }
    if ctx.pc().zone.is_none() {
        return Outcome::reason(F, 2);
    }
    let mut r = Reader(req);
    // Mask 332 = {2, 3, 6, 8}.
    if !matches!(r.byte(), Some(2 | 3 | 6 | 8)) || r.quad().is_none() {
        return Outcome::reason(F, 2293);
    }
    if let Err(e) = pairs(&mut r, 0, 4) {
        return pair_fail(e, F, 2296, 2293);
    }
    if ctx.pc().native_kind != 1 {
        return Outcome::log("exchange item: caster is not a PC");
    }
    Outcome::unsupported("❓ exchange recipe table tm[1479] lookup")
}

/// parity: 1050 12c-item-handlers.md#command-1050--g_makecustomdressreqcmd partial
pub fn handle_make_custom_dress(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_MakeCustomDressFailCmd;
    if !ctx.world.custom_dress_enabled {
        return Outcome::reason(F, 2360);
    }
    let mut r = Reader(req);
    let Some(target) = r.short() else {
        return Outcome::reason(F, 2);
    };
    let Some(n) = count(&mut r, 0, 8) else {
        return Outcome::reason(F, 2);
    };
    let first = (0..n)
        .map(|_| Some((r.byte()?, r.byte()?)))
        .collect::<Option<Vec<_>>>();
    let Some(first) = first else {
        return Outcome::reason(F, 2);
    };
    let pairs = match pairs(&mut r, 0, 16) {
        Ok(p) => p,
        Err(e) => return pair_fail(e, F, 2363, 2),
    };
    let task = ItemUseTask::MakeCustomDress {
        target,
        first,
        pairs,
    };
    post(
        ctx,
        1050,
        Outcome::reason(F, 2),
        KindCheck::Fail,
        Post::Queue,
        task,
    )
}

// ---- Item use and field items ---------------------------------------------------

/// parity: 1233 12c-item-handlers.md#command-1233--g_usegroceryreqcmd partial
pub fn handle_use_grocery(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let f = (|| Some((r.short()?, r.int()?, r.quad()?, r.short()?, r.quad()?)))();
    let Some((a, b, c, d, e)) = f else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = ItemUseTask::UseGrocery { a, b, c, d, e };
    post(
        ctx,
        1233,
        Outcome::reason(1235, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1234 12c-item-handlers.md#command-1234--g_usegrocerytogroundreqcmd partial
pub fn handle_use_grocery_to_ground(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let f = (|| {
        Some((
            r.short()?,
            r.int()?,
            [r.short()?, r.short()?, r.short()?],
            r.short()?,
            r.quad()?,
        ))
    })();
    let Some((a, b, position, d, e)) = f else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = ItemUseTask::UseGroceryToGround {
        a,
        b,
        position,
        d,
        e,
    };
    post(
        ctx,
        1234,
        Outcome::reason(1235, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1236 12c-item-handlers.md#command-1236--g_useequippedreqcmd partial
pub fn handle_use_equipped(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(a), Some(b), Some(c)) = (r.short(), r.quad(), r.short()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = ItemUseTask::UseEquipped { a, b, c };
    post(
        ctx,
        1236,
        Outcome::reason(1238, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1237 12c-item-handlers.md#command-1237--g_useequippedtogroundreqcmd partial
pub fn handle_use_equipped_to_ground(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let f = (|| Some((r.short()?, [r.short()?, r.short()?, r.short()?], r.short()?)))();
    let Some((a, position, c)) = f else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = ItemUseTask::UseEquippedToGround { a, position, c };
    post(
        ctx,
        1237,
        Outcome::reason(1238, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1242 12c-item-handlers.md#command-1242--g_usefielditemreqcmd partial
pub fn handle_use_field_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    let (Some(target), Some(a), Some(b)) = (r.quad(), r.short(), r.byte()) else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    // Constructor mode 0 makes the task byte +144 test pass.
    let task = ItemUseTask::UseFieldItem { target, a, b };
    post(
        ctx,
        1242,
        Outcome::reason(1244, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1243 12c-item-handlers.md#command-1243--g_usefielditemtogroundreqcmd partial
/// parity: 1245 12c-item-handlers.md#command-1245--g_throwfielditemreqcmd partial
/// parity: 1249 12c-item-handlers.md#command-1249--g_putdownfielditemreqcmd partial
pub fn handle_u_codec_entry(_: &mut Context<'_>, _: u16, _: &[u8]) -> Outcome {
    Outcome::unsupported("❓ lowercase `u` request codec not resolved")
}

fn pickup(
    ctx: &mut Context<'_>,
    req: &[u8],
    command: u16,
    task: fn(u64) -> ItemUseTask,
) -> Outcome {
    let Some(selector) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let fail = Outcome::reason(command + 1, 1);
    post(
        ctx,
        command,
        fail,
        KindCheck::Fail,
        Post::Queue,
        task(selector),
    )
}

/// parity: 1247 12c-item-handlers.md#command-1247--g_pickupfielditemreqcmd partial
pub fn handle_pickup_field_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    pickup(ctx, req, 1247, |selector| ItemUseTask::PickupFieldItem {
        selector,
    })
}

/// parity: 1251 12c-item-handlers.md#command-1251--g_pickupdeadbodyreqcmd partial
pub fn handle_pickup_dead_body(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    pickup(ctx, req, 1251, |selector| ItemUseTask::PickupDeadBody {
        selector,
    })
}

// ---- Pouches ------------------------------------------------------------------------

/// parity: 1296 12c-item-handlers.md#command-1296--g_openitempouchreqcmd partial
/// parity: 1303 12c-item-handlers.md#command-1303--g_closeitempouchreqcmd partial
pub fn handle_item_pouch(ctx: &mut Context<'_>, req: &[u8], command: u16, close: bool) -> Outcome {
    let Some(pouch) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    // Initializer `0x1405188C0` and queue: any failure sends reason 2.
    let task = ItemUseTask::ItemPouch { pouch, close };
    let fail = Outcome::reason(command + 2, 2);
    post(ctx, command, fail, KindCheck::None, Post::Queue, task)
}

/// parity: 1299 12c-item-handlers.md#command-1299--g_privateitempouchlistreqcmd partial
pub fn handle_private_item_pouch_list(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let Some(selector) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    let task = ItemUseTask::PrivateItemPouchList { selector };
    post(
        ctx,
        1299,
        Outcome::reason(1301, 2),
        KindCheck::Fail,
        Post::Queue,
        task,
    )
}

/// Counted bytes 0..=max. Err(true): count. Err(false): entry decode.
fn counted_bytes(r: &mut Reader<'_>, max: u16) -> Result<Vec<u8>, bool> {
    let n = count(r, 0, max).ok_or(true)?;
    (0..n).map(|_| r.byte()).collect::<Option<_>>().ok_or(false)
}

/// Zone and PC kind checks that precede the object-kind test.
fn zone_and_kind(ctx: &Context<'_>, fail: (u16, u16)) -> Option<Outcome> {
    if ctx.pc().zone.is_none() {
        return Some(Outcome::reason(fail.0, fail.1));
    }
    if ctx.pc().native_kind != 1 {
        let mut out = Outcome::reason(fail.0, fail.1);
        out.logs.push("pouch task: caster is not a PC");
        return Some(out);
    }
    None
}

/// parity: 1306 12c-item-handlers.md#command-1306--g_pickupitemreqcmd partial
pub fn handle_pickup_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_PickupItemFailCmd;
    let mut r = Reader(req);
    let (Some(pouch), Some(selector)) = (r.quad(), r.byte()) else {
        return Outcome::reason(F, 505);
    };
    let entries = match counted_bytes(&mut r, 16) {
        Ok(e) => e,
        Err(e) => return pair_fail(e, F, 506, 505),
    };
    if let Some(out) = zone_and_kind(ctx, (F, 1)) {
        return out;
    }
    if !matches!(object_kind(pouch), 176 | 32 | 242) {
        return two_reasons((F, 507), (F, 1));
    }
    let task = ItemUseTask::PickupItem {
        pouch,
        selector,
        entries,
    };
    post(
        ctx,
        1306,
        Outcome::reason(F, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1311 12c-item-handlers.md#command-1311--g_takecontributionrewardreqcmd partial
pub fn handle_take_contribution_reward(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_TakeContributionRewardFailCmd;
    let Some(selector) = Reader(req).quad() else {
        return Outcome::unsupported(UNCHECKED_DECODE);
    };
    if let Some(out) = zone_and_kind(ctx, (F, 1)) {
        return out;
    }
    if object_kind(selector) != 176 {
        return two_reasons((F, 515), (F, 1));
    }
    let task = ItemUseTask::TakeContributionReward { selector };
    post(
        ctx,
        1311,
        Outcome::reason(F, 1),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1316 12c-item-handlers.md#command-1316--g_distributeitemreqcmd partial
pub fn handle_distribute_item(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DistributeItemFailCmd;
    let mut r = Reader(req);
    // ❓ String limit 32 UTF-16 units; the decoder's exact bound is modelled
    // as length ≤ 32 (task copy is `wcsncpy(..., 32)`).
    let (Some(pouch), Some(name)) = (r.quad(), r.string(33)) else {
        return Outcome::reason(F, 523);
    };
    let entries = match counted_bytes(&mut r, 15) {
        Ok(e) => e,
        Err(e) => return pair_fail(e, F, 524, 523),
    };
    if let Some(out) = zone_and_kind(ctx, (F, 2)) {
        return out;
    }
    if !matches!(object_kind(pouch), 176 | 32) {
        return two_reasons((1309, 507), (F, 2));
    }
    let task = ItemUseTask::DistributeItem {
        pouch,
        name,
        entries,
    };
    post(
        ctx,
        1316,
        Outcome::reason(F, 2),
        KindCheck::None,
        Post::Queue,
        task,
    )
}

/// parity: 1321 12c-item-handlers.md#command-1321--g_completedistributionreqcmd partial
pub fn handle_complete_distribution(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_CompleteDistributionFailCmd;
    let Some(pouch) = Reader(req).quad() else {
        return Outcome::reason(F, 2);
    };
    if let Some(out) = zone_and_kind(ctx, (F, 2)) {
        return out;
    }
    if object_kind(pouch) != 176 {
        return two_reasons((F, 536), (F, 2));
    }
    let task = ItemUseTask::CompleteDistribution { pouch };
    post(
        ctx,
        1321,
        Outcome::reason(F, 2),
        KindCheck::None,
        Post::Queue,
        task,
    )
}
