# Handoff gw-enter (spec, GameDaemon 381 enter world)

## Done
- 05: 930 body and request rules, 167-row rule table with defaults, revision −1 on a fresh server.
- 05: 24 success body (free-to-play golden `18 00` + `00 FF FF FF FF 00 00 00 00 00 00 00`).
- 05: 32 header fields, property blob rules, 422-row property table (type, wire size, min, max, offset, sync).
- 05: packets after 32 (order 32, 627?, 87, 90, 423, 1097?, 1122, 1126, 836, 912?).
- 05: 41 field sources, 42 checks, 43 q = `enterSocialId` = 0 (golden `2B 00` + 8 zero bytes), sight packets 2/4/5/6.
- 06: ping option defaults (ping off; 60 s; count 3), spawn wait 1,800,000 ms, spawn position rule.

## Found, not yet in 05 (381 `ContentsRuleConfig::MakeListPacket` 0x140087D40)
- Header element: wide format at 0x14091AC90, `<?xml version="1.0" encoding="utf-8"?><config major-version="%d" minor-version="%d" module="contents-rule-config" release-module="LocalizationData" release-side="client">`, args 74 and 0.
- Option formats: int 0x14091ADE8 `<option name='%s' value='%d'/>`; int64 0x14091AEB0 (`%I64d`); float 0x14091AEF8 (`%f`); bool and text 0x14091AE58 (`%s`, bool = `yes`/`no`).
- Each element: UTF-8, no terminator, sent as `b` (i16 length + bytes). Last element `</config>`.

## Not done
- 06 rows 25, 26, 30, 31 and gate rows 123–129 still have old text.
- 06 "Packet log contract" section (`GAMED_PACKET_LOG`, one line `time dir id name size`, optional hex dump).
- play-1.md §10 guesses not replaced.
- Rust card `Docs/Plan/cards/gw-enter-rust.md` not written. The slice cannot chain to Rust yet.
- Abuse note `Docs/Plan/abuse-notes/gw-enter.md` (client summoned x/y/z in 42; fix: server-side clamp).
- 32: default values for a new PC per property id (not traced).

## ❓
- Fragment `hi` prefix layout (930, 32); needed for golden 930/32 bytes.
- Bool descriptors with null default = false; `GetPcSpawn` argument 1; packet 4 for self.
- Deployed config values (ping, 1097, 912).

## Blocked
- None. No IDA save, no rename.
