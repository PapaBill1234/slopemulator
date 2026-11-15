# Handoff gw-enter-2 (spec, GameDaemon 381 enter world, part 2)

## Done
- Rust card `Docs/Plan/cards/gw-enter-rust.md`: names, wire order, source rule, inputs with 05 line ranges, work steps, interfaces, files, 11 tests, abuse fix.
- 05 "Command 930 body": XML element facts from `ContentsRuleConfig::MakeListPacket` `0x140087D40` (header format `0x14091AC90` with 74/0; int `0x14091ADE8`; int64 `0x14091AEB0`; float `0x14091AEF8`; bool/text `0x14091AE58`; last `</config>`; UTF-8, no terminator, `b`).
- 06: rows 929 and 23/24, the 43 row, the appearance row and the gate table now have the 381 values. The "do not implement" line is replaced.
- 06: new section "Packet log contract" (`GAMED_PACKET_LOG`, `time dir id name size`, `hex` dump, `keyhere`).
- Abuse note `Docs/Plan/abuse-notes/gw-enter.md` (`exploit_enter_zone_client_pos`).

## Not done
- Step 4 (optional IDA): fragment `hi` prefix layout and new-PC defaults per property id. No IDA call in this slice. The ❓ stay.
- `play-1.md` §10 guesses (out of scope).

## ❓
- Fragment `hi` prefix layout (930, 32): golden 930/32 bytes depend on it. The card tests check the element text and field order, not the prefix bytes.
- New-PC default per property id (card: unknown defaults = 0, name = PC name).
- Bool descriptors with null default = false; `GetPcSpawn` argument 1; packet 4 for self.
- Deployed config values (ping, 1097, 912).

## Blocked
- None. No IDA save, no rename, no commit.
