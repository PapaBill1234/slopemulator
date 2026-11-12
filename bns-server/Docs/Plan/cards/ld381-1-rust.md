# Card ld381-1-rust: gamed link formats and NP deposit id for LobbyDaemon 381

## Goal
The gamed link messages `HelloOk` and `EnterOk` use the 381 formats, the gamed protocol value 83 is marked as verified, and `lobbydaemon` has a sans-I/O function that makes the 381 NP mileage deposit id.

## Source rule (user decision, 2026-11-12)
| What | Source | Why |
| --- | --- | --- |
| Logic: jobs, checks, order of steps, database calls, timers | 2019 + PDB build (full names and types) | Readable; the specs `13` to `21` already use it |
| Packets: command ids, handler table flags, formats, field order and sizes, reply codes | 381 build | The 381 client is the target. The client must accept every byte. |
| 381 logic that differs from 2019 and the client can see it | 381 build | 381 wins over 2019 for client-visible behaviour |

## Architecture rules
- `Docs/Architecture/scale-rules.md`: §1 (tiers: the client-visible part is fixed, the rest is modern), §3 (layers; domain code is sans-I/O), §4 (ownership), §5 (hot path), §7 (database), §11 (check before each commit).
- `Docs/Plan/step6-part2.md` §3: the job core of part 1b stays; the gamed link stays our own protocol (new messages at both ends); original bugs are fixed and marked "bug, not ported".

## Context
- No command id changes. The client tables (LobbyLobbyGate, InfoGate, Manage) are equal to 307. Do not change `packet-ids`.
- Gamed link commands (names from `Docs/Server/LobbyDaemon/command-tables-381.json`, LobbyGame table): 1 `HandleLobbyGame_HelloReq` (reply `HelloOk`, command 2), 6 `EnterWorldOk` (`Message::EnterOk`).
- 381 formats (verified in the 381 IDB):
  - `HelloOk` `hCQiiiqqChiiii;;qqqi` (307: `hCQiiiqqChii;;qqqi`). The 2 new `i` are in the inner rule entry.
  - `EnterWorldOk` `hiqqocbbbbssQhhiQCQiqqihcccc;Cqhs;iQQQsCiiiii;iCiiQiqq;Ciqq;qqiqCihqq;Chq;` (307: `…ihcc;…qqiq`). Changes: list entry `ihcc` → `ihcccc`; 2 new lists at the end: `Cihqq;` (content quota: i, h, q, q) and `Chq;` (recycle time: h, q).
- `PACKET_VERSION = 83` is verified in 381 (`HandleLobbyGame_HelloReq` `0x140083ee0`).
- NP deposit id (381 `DepositMileageJob::depositReq` `0x1400495f0`). Prefix = config key `np-game-code` (`BnsCommonConfig`, default `bnsprog`). `LobbyGate` already loads this key (`Program/Server/LobbyGate/src/config.rs:220`); `lobbydaemon` does not. The `--game-code` argument in `src/main.rs:44` is a different value (STS game code); do not use it as the prefix.
  - location 0: `{prefix}_{request_id}`
  - location ≠ 0, not type 1: `{prefix}_{location}{kind:02}+_{request_id}`
  - deposit type 1: `{prefix}_{location}{kind:02}+{pc_db_id}+{pc_job}+{request_id}_{sub_id}` (381 drops sex and race; 348 had them).
- Quirk `quirk_np_game_code_prefix` (keep value): 381 converts the code into 7 bytes and does not check the result. Port: prefix = first 7 bytes of the ASCII code. ❓ Not tested on Windows. Reject a non-ASCII code at config load (fixed, no memory error).

## Inputs
| What | Path | Lines or section |
| --- | --- | --- |
| Compare spec | `Docs/Server/LobbyDaemon/24-compare-381.md` | §2.1 lines 40–50; §3 line 65 (deposit id); §7 lines 96–104 (quirk); §8 lines 106–143 |
| Gamed link spec | `Docs/Server/LobbyDaemon/07-gamed-link.md` | `HelloOk`, `EnterWorldOk` sections |
| Gamed link code | `Program/Server/Packet/src/gamed_link.rs` | lines 1–17 (header, consts), `encode` (`HelloOk`, `EnterOk`), `decode`, tests line 382+ |
| NP code | `Program/Server/LobbyDaemon/src/np.rs` | add the function here |

## Work
1. `gamed_link.rs`: set `HELLO_OK` and `ENTER_OK` to the 381 strings. Update the header: remove the ❓ lines 5 and 9 about 381; cite LobbyDaemon 381 string addresses `0x14017ba60` (`HelloOk`) and `0x140179cd0` (`EnterWorldOk`).
2. `HelloOk` encode/decode: a rule entry has 5 values (h, i, i, i, i). The rule lists stay empty.
3. `EnterOk` encode/decode: the `ihcccc` entry has 2 more `c` values; append 2 empty lists at the end. Decode must accept and skip non-empty new lists.
4. Update the comment "10 for each rule" in `round_trip` to 18. Keep the test name. Run it; if the size changes, fix the assert and write the new value in the handoff.
5. `np.rs`: add `deposit_id` (see Interfaces) and its config input. Add `np_game_code: String` (default `bnsprog`) to the lobbydaemon config or start arguments.
6. Run `cargo test -p packet`, `cargo test -p lobbydaemon` and `cargo test -p gamedaemon` (GameDaemon decodes the same messages).

## Interfaces
```rust
// Program/Server/LobbyDaemon/src/np.rs
pub enum DepositKind { Account, Pc { pc_db_id: u64, pc_job: u8 } }
pub struct DepositIdInput<'a> {
    pub np_game_code: &'a str, // ASCII; first 7 bytes used (quirk_np_game_code_prefix)
    pub location: i32,
    pub location_kind: i32,     // printed with 2 digits
    pub kind: DepositKind,      // Pc = deposit type 1
    pub request_id: u64,
    pub sub_id: u64,
}
pub fn deposit_id(input: &DepositIdInput<'_>) -> String;
```

## Files
- `Program/Server/Packet/src/gamed_link.rs`
- `Program/Server/LobbyDaemon/src/np.rs`
- `Program/Server/LobbyDaemon/src/main.rs` (config input only)
- `Program/Server/LobbyDaemon/tests/np_deposit_id.rs` (new)

## Acceptance
| Test name | Input | Expected result |
| --- | --- | --- |
| `round_trip` (existing) | all messages | decode(encode(m)) == m; size assert passes |
| `hello_ok_381_format` | `HELLO_OK` | equals `hCQiiiqqChiiii;;qqqi` |
| `enter_ok_381_tail` | `Message::EnterOk` encode | decode with the 381 string succeeds; 2 empty lists at the end |
| `deposit_id_location_zero` | `bnsprog`, location 0, request 77 | `bnsprog_77` |
| `deposit_id_location` | `bnsprog`, location 3, kind 2, request 77 | `bnsprog_302+_77` |
| `deposit_id_pc` | `bnsprog`, 3/2, Pc{5001, job 4}, request 77, sub 9 | `bnsprog_302+5001+4+77_9` |
| `deposit_id_long_code` | code `bnsprog2` | prefix `bnsprog` |

`cargo test -p lobbydaemon`

## Out of scope
- Gamed commands 110 `HandleLobbyGame_UpdateAccountContentQuotaReq` and 113 `HandleLobbyGame_UpdateAccountRecycleTimeReq` (reply ids ❓), the 4 database wrappers (`24` §6) and the deposit job flow: audit `ld381-2` puts them into `ld381-r1`, `ld381-r4` and `ld381-r5`.
- Do not fill the new `EnterOk` lists with data (needs the 110/113 work).
- No change to LobbyGate or to client formats.
