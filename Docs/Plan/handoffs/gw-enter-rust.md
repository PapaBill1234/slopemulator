# Handoff gw-enter-rust

## Done
- `entry.rs`: builders 24 (`token_ok`), 25, 32 (own fragments, 381 blob 1,024 + 2n), 87, 90, 423, 1122, 1126, 836, 41, 43, 44, 33, 931, 17; `rule_list` (167 rows, config override, revision -1).
- `rules381.rs`, `property381.rs` (generated tables; declared in `entry.rs` with `#[path]`).
- `play.rs`: new flow 32, 87, 90, 423, 1122, 1126, 836, 41; 929 rate rule; 42 with key and state 2, clamp, 44/1150; 43 q=0; second 31 gives 33; ping tick and echo 18.
- `conn.rs`: `Dir`, `LogMode`, `log_line`, `log_lines`; Hello 11/12 logged; 41 key and Hello body not in the dump.
- `shard.rs`: clock, spawn key counter (skips 0), `World::with_options` (ping, start hour), ping on tick; default rules when the list is empty.
- Tests in `tests/gw_enter.rs`: `loopback_order`, `golden_930`, `rule_list_rate`, `golden_24`, `golden_32`, `golden_41`, `enter_zone_bad_key`, `enter_zone_clamp`, `golden_43`, `packet_log_line`, `packet_log_off`. Unit tests: `ping_follows_the_table`, `summon_is_clamped`, `spawn_keys_skip_zero`, `rule_config_overrides_the_default`.
- fmt, test (all of `gamedaemon`) and clippy pass.

## Not done
- `Packet/src/game.rs` and `game381.json` codecs for 24, 32, 930 (not needed).
- `loopback_order` runs the session layer, not a socket: Hello 11/12 is not in it.
- The `gamedaemon` bin is outside "Files": it still sends no 930 when `--contents-rule` is missing (the default list comes only from `World::new` with an empty list, so it works), and it has no option for ping or start hour.
- Operator step (5 minutes in zone 1101, packet log).

## ❓
- 423 unit (seconds used). 90 empty length (three empty `b`). 87/1122/1126 value of the field before the count (0).
- 836: meaning of the 13 values (all 0). Header fields 4-9 of 32 (chat 0/1, bias 0, field 9 = 2).
- Zone key size and algorithm (`keyhere`, 7 bytes); zone object id (= zone id); `SUMMON_RANGE` (500) and yaw units.
- Clock start hour (0); ping policy flags and close action; row 166 default `;`; max-hp 303 and speed 62.
- Formats of 33 and 44 (`hh`); reason 2 of 33 for a second 31. Packet 4 for self may be needed.

## Blocked
- None.
