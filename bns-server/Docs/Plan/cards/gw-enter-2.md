# Card gw-enter-2: finish the GameDaemon 381 enter world spec and write the Rust card (spec)

## Goal
Finish the open items of `gw-enter`. Write `Docs/Plan/cards/gw-enter-rust.md`. The loop cannot run `gw-enter-rust` until this card exists.

## Read first
| What | Path |
| --- | --- |
| Earlier handoff (state and open items) | `Docs/Plan/handoffs/gw-enter.md` |
| Earlier card (source rule, inputs, Rust card rules) | `Docs/Plan/cards/gw-enter.md` ("Source rule", "Inputs", "Work" step 9, "Architecture rules") |
| Spec | `Docs/Server/GameDaemon/05-enter-world.md` (large: run `tools/spec_index.py` first), `06-minimum-session.md` |

Use the same IDBs as `gw-enter`, with `boot(WT, stservers=False)`. Do not save. Do IDA work only for the items below.

## Work
1. Write the Rust card `Docs/Plan/cards/gw-enter-rust.md` FIRST (template `Docs/Plan/card-template.md`, max 3,000 tokens), as `gw-enter.md` "Work" step 9 says. Write it to disk before step 2.
2. Put the "Found, not yet in 05" facts of the handoff (930 `ContentsRuleConfig::MakeListPacket` XML elements) into `05-enter-world.md`.
3. `06-minimum-session.md`: rows 25, 26, 30, 31 and gate rows 123–129 with the 381 values. Add the section "Packet log contract" (`GAMED_PACKET_LOG`, one line `time dir id name size`, optional hex dump).
4. If turns are left: the fragment `hi` prefix layout (930, 32), and the default values for a new PC per property id of 32. Else keep the ❓.
5. Write the abuse note `Docs/Plan/abuse-notes/gw-enter.md` (client sends x/y/z in 42; fix: clamp on the server).

## Files
- `Docs/Server/GameDaemon/05-enter-world.md`
- `Docs/Server/GameDaemon/06-minimum-session.md`
- `Docs/Plan/cards/gw-enter-rust.md` (new)

## Out of scope
- No Rust change. No client IDA work. Do not edit `play-1.md`.
