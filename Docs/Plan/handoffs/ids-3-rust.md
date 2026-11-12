# Handoff ids-3-rust

## Done
- `packet-ids`: `gen-ids` writes `src/generated/lobbyd.rs` (`LobbyGateLobbydCmd` 35, `GameLobbydCmd` 15, `AccountInventoryLobbydCmd` 3; ✅ rows only). Tests: `lobbyd_names_from_table`, `lobbyd_unverified_absent`, `generated_is_current` (covers `lobbyd.rs`). New dep: `serde_json`.
- LobbyDaemon: `build.rs` + `src/ids.rs` (constants are `i16`, because `wire::command` returns `i16`). Request numbers in `jobs.rs`, `jobs/services.rs`, `jobs/characters.rs` now use names.
- LobbyGate: request IDs in `daemon.rs` (1, 6, 12) and `login.rs` (9, 15) use `LobbyGateLobbydCmd::X.id()`.
- GameDaemon: all 99 `const F: u16 = <n>;` now `ids::G_...FailCmd`. `play.rs` (`packet()`, `token_reply`, tests) and `kick.rs` use `ids::`. `handlers/npc_trade.rs` `gacha_prefix` (1178, 1181) uses `ids::`.
- Check `rg 'const F: u16 = [0-9]' Program/Server/GameDaemon/src`: no output.
- Check `rg -n '^\s*[0-9]+( *\| *[0-9]+)* *=>' LobbyDaemon/src/jobs.rs src/jobs`: no output (no ❓ arm exists).
- `cargo fmt`, `cargo test -q`, `cargo clippy --all-targets -- -D warnings` pass for packet-ids, lobbydaemon, lobbygate, gamedaemon. Existing tests unchanged, except the `play.rs` unit tests (names instead of numbers).

## Not done
- Reply IDs (LobbyDaemon → LobbyGate) stay as numbers, as the card says (lines approximate):
  - LobbyDaemon `jobs.rs`: 20/21, 14/17/8/11/37, 13, 7, 3, 66/69/72/75/78. `jobs/characters.rs`: 45-49, 52, 53, 55, 56, 48, 104, 105. `jobs/services.rs`: 62, 108, 84/87/90/93/96/99/102. `jobs/world.rs`: 26, 29, 30, 32, 33. `protocol.rs`: 22, 25, 27, 39, 40.
  - LobbyGate `login.rs`: l.186 (7|13), l.251 (8|11|14|17), l.274 (10|16). `daemon.rs` l.231-268 (2, 3). `server.rs` l.389-448 (2, 4, 5, 22, 25, 36, 68, 71, 74, 80). `goods.rs` l.143-145.
- Tests in `GameDaemon/tests` (for example `gn1_npc_trade.rs`, `distance_kick.rs`) keep bare numbers. The card says they pass unchanged.

## ❓
- The card says prefix `HandleLobby<Table>_`. The AccountInventory table uses `HandleAccountInventoryLobby_`. The generator removes all text up to the first `_` after `Handle`.
- Game table rows 110 and 113 have no `name_status`. I treated them as not ✅.
- `login.rs` 7/13 (daemon → gate, with tokens) are replies, not requests. Please confirm.
- Step 7 (number and `GameCmd` differ): no mismatch found.

## Blocked
- Nothing.
