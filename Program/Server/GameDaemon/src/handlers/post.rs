//! Doc 23: post entry functions. Build 381.8867.
//!
//! These entries use cached requests and account transactions, not zone tasks.
//! `Services::post` owns the cached channel, user DB ID, money limit and
//! transaction registration. Unknown is never approval.
use super::*;

pub const POST_COMMANDS: &[u16] = &[
    ids::G_RetrievePostCountReqCmd,
    ids::G_RetrievePostBriefListReqCmd,
    ids::G_RetrievePostReqCmd,
    ids::G_RetrievePcDbIdReqCmd,
    ids::G_ArchivePostReqCmd,
    ids::G_MarkExpressPostReqCmd,
    ids::G_DetachPostAttachmentReqCmd,
    ids::G_DeliverPostReqCmd,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostKey {
    pub kind: u8,
    pub id: u64,
}

/// Fields decoded into the deliver transaction. Names not in the logs stay neutral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostDelivery {
    /// Byte +88.
    pub kind: u8,
    /// Qword +96. ❓ Meaning.
    pub reference: u64,
    /// Short +104. ❓ Meaning.
    pub value: u16,
    /// UTF-16 buffers +106 (33), +172 (65), +302 (1025).
    pub receiver: Vec<u16>,
    pub subject: Vec<u16>,
    pub content: Vec<u16>,
    /// Signed qword +2352.
    pub money: i64,
    /// Count +2360, parallel short arrays +2364 / +2370 (maximum 3).
    pub attached: Vec<(u16, u16)>,
    /// Count +2376, parallel short arrays +2380 / +2392 (maximum 6).
    pub charged: Vec<(u16, u16)>,
}

/// Native account transactions, NOT zone tasks. No post entry calls skills::queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostTask {
    /// 0x70 bytes, vtable 0x1409E63D8, DWORD +8 = 0xB210A.
    /// +88 = 0xFF, +96/+104 = 0. The entry does not copy the decoded key.
    MarkExpress,
    /// 0x90 bytes, vtable 0x1409E6BE8, DWORD +8 = 0xC210A.
    /// +88 = 0, +96 = 0xFF, +104/+112/+120/+128 = 0, DWORD +136 = 0x20000.
    /// The entry does not copy the decoded key or attachment fields.
    DetachAttachment,
    /// 0x9A8 bytes, vtable 0x1409E6810, DWORD +8 = 0x3C210A.
    Deliver(Box<PostDelivery>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PostOperation {
    /// The adapter resolves user qword +216. It must not substitute PC::db_id.
    /// 69/72: hqq [object ID, user DB ID].
    /// 75/93: hqQqq [object ID, kind, user DB ID, post ID].
    Cached {
        command: u16,
        routing: u64,
        object_id: u64,
        post: Option<PostKey>,
    },
    /// Global qword_140EE58A0 qword index 5, signed.
    MoneyLimit,
    /// Account transaction manager +5080, not a zone queue.
    Register { object_id: u64, task: PostTask },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PostResult {
    #[default]
    Unknown,
    /// Cached send attempted, or transaction registered.
    Submitted,
    /// Transaction registration failed. For cached sends the result is ignored.
    Rejected,
    /// User +216 is zero. The cached request is not sent.
    InvalidUserDbId,
    MoneyLimit(i64),
}

/// Test boundary. Defaults do not permit a send or transaction.
#[derive(Clone, Debug, Default)]
pub struct MemoryPost {
    pub cached_result: PostResult,
    pub register_result: PostResult,
    pub money_limit: Option<i64>,
    pub calls: Vec<PostOperation>,
}
impl MemoryPost {
    pub(super) fn check(&mut self, operation: &PostOperation) -> PostResult {
        self.calls.push(operation.clone());
        match operation {
            PostOperation::Cached { .. } => self.cached_result,
            PostOperation::MoneyLimit => self
                .money_limit
                .map(PostResult::MoneyLimit)
                .unwrap_or_default(),
            PostOperation::Register { .. } => self.register_result,
        }
    }
}

pub(super) fn dispatch_post(ctx: &mut Context<'_>, command: u16, req: &[u8]) -> Outcome {
    match command {
        ids::G_RetrievePostCountReqCmd => handle_retrieve_post_count(ctx, req),
        ids::G_RetrievePostBriefListReqCmd => handle_retrieve_post_brief_list(ctx, req),
        ids::G_RetrievePostReqCmd => handle_retrieve_post(ctx, req),
        ids::G_RetrievePcDbIdReqCmd => handle_retrieve_pc_db_id(ctx, req),
        ids::G_ArchivePostReqCmd => handle_archive_post(ctx, req),
        ids::G_MarkExpressPostReqCmd => handle_mark_express_post(ctx, req),
        ids::G_DetachPostAttachmentReqCmd => handle_detach_post_attachment(ctx, req),
        ids::G_DeliverPostReqCmd => handle_deliver_post(ctx, req),
        _ => unreachable!("membership checked"),
    }
}

fn arena_b(ctx: &Context<'_>) -> bool {
    ctx.world.arena || session::arena_blocked(ctx.pc())
}

fn has_user(ctx: &Context<'_>) -> bool {
    ctx.pc().user_present && !ctx.pc().party_user_blocked
}

fn cached(ctx: &mut Context<'_>, command: u16, post: Option<PostKey>) -> Outcome {
    let operation = PostOperation::Cached {
        command,
        routing: ctx.pc().routing,
        object_id: ctx.pc().object_id,
        post,
    };
    match ctx.services.post(&operation) {
        PostResult::Submitted | PostResult::Rejected => Outcome::default(),
        PostResult::InvalidUserDbId => Outcome::log("post: cached user DB ID is invalid"),
        _ => Outcome::unsupported("❓ post cached channel and user +216 DB ID"),
    }
}

fn key(r: &mut Reader<'_>) -> Option<PostKey> {
    Some(PostKey {
        kind: r.byte()?,
        id: r.quad()?,
    })
}

fn reason_key(command: u16, reason: u16, key: PostKey) -> Outcome {
    let mut body = reason.to_le_bytes().to_vec();
    body.push(key.kind);
    body.extend(key.id.to_le_bytes());
    Outcome::reply(command, body)
}

fn register(ctx: &mut Context<'_>, task: PostTask, fail: Outcome) -> Outcome {
    let operation = PostOperation::Register {
        object_id: ctx.pc().object_id,
        task,
    };
    match ctx.services.post(&operation) {
        PostResult::Submitted => Outcome::default(),
        PostResult::Rejected => fail,
        _ => Outcome::unsupported("❓ post account transaction registration and continuation"),
    }
}

/// parity: 368 23-post-handlers.md#368--g_retrievepostcountreqcmd--partial partial
pub fn handle_retrieve_post_count(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    if arena_b(ctx) {
        return Outcome::reason(370, 5);
    }
    if !has_user(ctx) {
        return Outcome::log("retrieve post count: user is not found");
    }
    cached(ctx, 69, None)
}

/// parity: 371 23-post-handlers.md#371--g_retrievepostbrieflistreqcmd--partial partial
pub fn handle_retrieve_post_brief_list(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    if arena_b(ctx) {
        return Outcome::reason(373, 5);
    }
    if !has_user(ctx) {
        return Outcome::log("retrieve post brief list: user is not found");
    }
    cached(ctx, 72, None)
}

/// parity: 374 23-post-handlers.md#374--g_retrievepostreqcmd--partial partial
pub fn handle_retrieve_post(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if arena_b(ctx) {
        // 0x140396C1C sets command/reason only, before the request decode.
        return Outcome::unsupported("❓ post reply 376 reason 5: native hEQq tail is unset");
    }
    let post = key(&mut Reader(req)); // Native ignores the decode result.
    if !has_user(ctx) {
        return Outcome::log("retrieve post: user is not found");
    }
    let Some(post) = post else {
        return Outcome::unsupported(
            "❓ retrieve post: unchecked decode leaves native fields unset",
        );
    };
    cached(ctx, 75, Some(post))
}

/// parity: 785 23-post-handlers.md#785--g_retrievepcdbidreqcmd--partial partial
pub fn handle_retrieve_pc_db_id(ctx: &mut Context<'_>, _req: &[u8]) -> Outcome {
    // Native hb: a u16 byte count, followed by eight raw bytes (not a bare q).
    let mut body = 8u16.to_le_bytes().to_vec();
    body.extend(ctx.pc().db_id.to_le_bytes());
    Outcome::reply(786, body)
}

/// parity: 1500 23-post-handlers.md#1500--g_archivepostreqcmd--partial partial
pub fn handle_archive_post(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    if arena_b(ctx) {
        return Outcome::unsupported("❓ post reply 1502 reason 5: native hEQq tail is unset");
    }
    if ctx.pc().t2_blocked {
        return Outcome::unsupported("❓ post reply 1502 reason 7: native hEQq tail is unset");
    }
    let post = key(&mut Reader(req)); // Native ignores the decode result.
    if !has_user(ctx) {
        return Outcome::log("archive post: user is not found");
    }
    let Some(post) = post else {
        return Outcome::unsupported(
            "❓ archive post: unchecked decode leaves native fields unset",
        );
    };
    cached(ctx, 93, Some(post))
}

/// parity: 1503 23-post-handlers.md#1503--g_markexpresspostreqcmd--partial partial
pub fn handle_mark_express_post(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    // This unchecked decode comes BEFORE arena and trial gates.
    let post = key(&mut Reader(req));
    let fail = |reason| match post {
        Some(post) => reason_key(1505, reason, post),
        None => {
            Outcome::unsupported("❓ mark express post: unchecked decode leaves reply fields unset")
        }
    };
    if arena_b(ctx) {
        return fail(5);
    }
    if ctx.pc().t2_blocked {
        return fail(7);
    }
    if !has_user(ctx) {
        return Outcome::log("mark express post: user is not found");
    }
    if post.is_none() {
        return fail(2);
    }
    register(ctx, PostTask::MarkExpress, fail(2))
}

/// parity: 1506 23-post-handlers.md#1506--g_detachpostattachmentreqcmd--partial partial
pub fn handle_detach_post_attachment(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    let mut r = Reader(req);
    // Both native 128-bit temporaries start at zero. Keep partial decoded values.
    let mut post = PostKey { kind: 0, id: 0 };
    let decoded = (|| {
        post.kind = r.byte()?;
        post.id = r.quad()?;
        r.quad()?; // First attachment qword, logged as instid.
        r.quad()?; // Second attachment qword. ❓ Meaning.
        Some(())
    })()
    .is_some();
    if !decoded {
        return reason_key(1508, 2, post);
    }
    if arena_b(ctx) {
        return reason_key(1508, 5, post);
    }
    if ctx.pc().t2_blocked {
        return reason_key(1508, 7, post);
    }
    if !has_user(ctx) {
        return Outcome::log("detach post attachment: user is not found");
    }
    register(ctx, PostTask::DetachAttachment, reason_key(1508, 2, post))
}

fn delivery(r: &mut Reader<'_>) -> Option<PostDelivery> {
    Some(PostDelivery {
        kind: r.byte()?,
        reference: r.quad()?,
        value: r.short()?,
        receiver: r.string(33)?,
        subject: r.string(65)?,
        content: r.string(1025)?,
        money: r.quad()? as i64,
        attached: Vec::new(),
        charged: Vec::new(),
    })
}

fn positions(r: &mut Reader<'_>, count: u16) -> Option<Vec<(u16, u16)>> {
    (0..count).map(|_| Some((r.short()?, r.short()?))).collect()
}

fn duplicate_position(items: &[(u16, u16)]) -> bool {
    items
        .iter()
        .enumerate()
        .any(|(i, (position, _))| items[i + 1..].iter().any(|(other, _)| position == other))
}

/// parity: 1509 23-post-handlers.md#1509--g_deliverpostreqcmd--partial partial
pub fn handle_deliver_post(ctx: &mut Context<'_>, req: &[u8]) -> Outcome {
    const F: u16 = ids::G_DeliverPostFailCmd;
    if arena_b(ctx) {
        return Outcome::reason(F, 5);
    }
    if ctx.pc().t2_blocked {
        return Outcome::reason(F, 7);
    }
    if !has_user(ctx) {
        return Outcome::log("deliver post: user is not found");
    }
    let mut r = Reader(req);
    let Some(mut delivery) = delivery(&mut r) else {
        return Outcome::log("deliver post: invalid packet");
    };
    // sub_14069DA00 returns -1 on a short count. Native rejects that as unsigned.
    let Some(count) = r.short().filter(|&n| n <= 3) else {
        return Outcome::reason(F, 1287);
    };
    let Some(items) = positions(&mut r, count) else {
        return Outcome::log("deliver post: invalid packet");
    };
    if duplicate_position(&items) {
        return Outcome::reason(F, 1292);
    }
    delivery.attached = items;
    let Some(count) = r.short().filter(|&n| n <= 6) else {
        return Outcome::reason(F, 1292);
    };
    let Some(items) = positions(&mut r, count) else {
        return Outcome::log("deliver post: invalid packet");
    };
    if duplicate_position(&items) {
        return Outcome::reason(F, 1292);
    }
    delivery.charged = items;
    if delivery.money < 0 {
        return Outcome::reason(F, 33);
    }
    let PostResult::MoneyLimit(limit) = ctx.services.post(&PostOperation::MoneyLimit) else {
        return Outcome::unsupported("❓ post money limit: global qword index 5");
    };
    if delivery.money > limit {
        return Outcome::reason(F, 33);
    }
    register(
        ctx,
        PostTask::Deliver(Box::new(delivery)),
        Outcome::reason(F, 2),
    )
}
