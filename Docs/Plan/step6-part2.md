# Step 6 part 2: LobbyDaemon job port — plan

Date: 2026-11-12. Before: `handoff-step6.md` (parts 1a and 1b are done). Rules: `Docs/Architecture/scale-rules.md`.

## 1. Gap list (code against specs `13` to `22`)

| Area | Commands | State today | Spec |
| --- | --- | --- | --- |
| Session states | all | Local `State` enum, 8 values. Spec has 12 (`13-jobs-a.md` §2). | `13` §2 |
| Logout | LobbyGate 18, 19, 23 | Partial. No per-state table. | `13` §3.3 |
| World queue | LobbyGate 28, 31 | Unsupported (error, link closes ❓). | `13` §3.4, `20` §4.1 |
| Feature-off replies | LobbyGate 60, 106 | Unsupported. | `20` §4.1 |
| Account-inventory item jobs | LobbyGate 82–100 | Unsupported. Account inventory is off (no link). | `17`, `19`, `20` §4.1 |
| Slot bind | LobbyGate 57 | Unsupported. Needs NP slot service and a DB bind ❓. | `15` §2 |
| Return to lobby | client 26 → gamed 8 → LobbyGate 12/15 | GameDaemon: no transaction. LobbyDaemon: local `retain` only. | `10-session-handlers.md` §26 (partial), `13` §3.6, `07` lines 819–851 |
| Logout in world, kick | gamed 12, 14/15/16, link close | Not on the link. `kick` is local. | `13` §3.7, `18` §5 |
| Character lifecycle | 44, 47, 51, 54 | `character_states` and `deletion_deadlines` live in the `Jobs` of one link. Loaded rows reject mutation. | `16`, `21` |
| Delete and restore on GameDaemon | gamed 23–28 | Stub. | `16` §2–4, `07` |
| Full NP start sequence | 9 | Local subset (consume, presence, account, list). | `18` §1 |
| Try enter world checks | 24 | Local subset. No second password, charge, restore. | `18` §3 |
| Gamed account requests | gamed 11, 39, 42, 80, 83, 92, 97, 104, 107 | Not on the link. GameDaemon does not send them yet. | `14`, `20` §5 |
| Account inventory link, auction, mileage, NP schedule | — | Not ported (StServers WarehouseSrv not ported). | `14`, `15`, `19` |

## 2. Slices

One Sonnet builder at a time (`tools/run_queue.sh`). Each Rust slice keeps all 84 tests and adds its own.

| Slice | Model | Work | Deps |
| --- | --- | --- | --- |
| `lp2-1-rust` | Sonnet | Spec session states; LogoutJob table; 28 GetWaitInfo; 31 GiveupWait | — |
| `lp2-2-rust` | Sonnet | Feature-off and channel-absent replies: 60, 106, 82–100 | `lp2-1-rust` |
| `lp2-3` | Opus (IDA) | Spec: GameDaemon 381 `ReturnToLobbyTran`, client 27/28, link 8/9/10 order. Writes card `lp2-3-rust`. | — |
| `lp2-3-rust` | Sonnet | Return to lobby on both ends | `lp2-3` |
| `lp2-4-rust` | Sonnet | Gamed 12 LogoutNotify, link close, KickJob over the link (14/15/16) | `lp2-3-rust` |
| `lp2-5-rust` | Sonnet | Character lifecycle from the database; remove the per-link maps | `lp2-2-rust` |

Later (not carded): delete and restore on GameDaemon, full NP start sequence (`18` §1), try-enter checks (`18` §3), gamed account requests, slot bind 57, account inventory link. Write their cards when `lp2-5-rust` is done and the user has tested login.

## 3. Decisions

| # | Decision | Reason |
| --- | --- | --- |
| 1 | Account inventory, charge and grade are off (config). The jobs send the original "off" codes (7). | No WarehouseSrv or NP charge port yet. The client gets a reply, not a closed link. |
| 2 | The job core stays as part 1b (link task, job task on a copy). A slow job is an `async fn` on the copy. | Rule O1 holds. A per-job state machine type gives no client-visible gain. |
| 3 | The gamed link stays our own protocol (`packet::gamed_link::Message`). New messages are added at both ends. | Tier 2 in `scale-rules.md` §1. |
| 4 | Original bugs are fixed and marked "bug, not ported" in the spec (example: `20-handlers.md` §4.1, rename Fail command). | `scale-rules.md` §1. |
