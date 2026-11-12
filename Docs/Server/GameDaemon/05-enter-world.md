# 05 — Enter-world packets, build 381

## State

Partial static specification. No client test ran.
Use only the allowed build-381 client and GameDaemon databases for the addresses below.
The existing LobbyDaemon specification describes build 307. It supplies link context, not proof of identical build-381 code.
Connection and authentication are in [04-world-connect.md](04-world-connect.md).
Table dependencies are in [03-minimum-world.md](03-minimum-world.md).

## Packet identification

The client receive table starts at `0x149EC40` and uses command × 4.
Dispatcher `0xF44030` checks commands below `0x63F`.
GameDaemon dispatcher `0x14007BC00` reads the command at decoded packet +2.
Its unauthenticated table is `0x1407D6D50`.
Its authenticated-PC table is `0x1407D9F80`.
Server table entries use command × 8.

Client symbols are inconsistent. The named `HandleEnterZoneOkPacket` at `0xF3A860` contains unrelated UI code.
Use checked receive slots and command stores instead.
Names with ❓ below are descriptive names, not verified client symbols.
Packet-object offsets are not wire offsets.

Full format strings include the command `h`. They exclude the outer size field.
`h/u/E` use 2 bytes; `i` uses 4; `q` uses 8; `c/Q` use 1.
Scalars use little-endian order. Signedness depends on the field.
The existing BloodLib codec treats `K` as a nonnegative signed-16 byte count followed by bytes.
Source: `Program/Server/BloodLib/src/packet.rs:171` and `:179`.
Step 4 checked native `K` and `b` together in decoder `0x1406AA7B0`.
Both read a nonnegative signed-16 byte count, then that many bytes.
The decoder rejects a negative count, a short destination, or short input.
This matches the existing BloodLib byte-string codec.

## Ordered path

| Order | ID | Direction | Client symbol or checked target | Full format | Source and result |
| --- | ---: | --- | --- | --- | --- |
| 1 | 11 | Client → GameDaemon | Game-open callback `0xF2C0C0` | `hhqsqb` | Sends game Hello. See 04. |
| 2 | 12 | GameDaemon → client | `HandleHelloOkPacket`, `0xF70280` | `hb` | Processes opaque Hello reply. See 04. |
| 2a | 929 `G_ContentsRuleListReqCmd` | Client → GameDaemon | Constructor `0xF2BEF0`, called by the command-12 callback | `h` | Server `HandleGame_ContentsRuleListReq`, `0x140370250`. See "Commands 929–932". |
| 2b | 930 `G_ContentsRuleListOkCmd` | GameDaemon → client | Client fragment command, 4 fields | Fragment, descriptor `hihhiCb;` | Cached rule list. Builder `0x140087D40`. Failure: 931 `G_ContentsRuleListFailCmd`, `hE`. |
| 3 | 23 `G_LobbyTokenAuthReqCmd` | Client → GameDaemon | ❓ Exact client sender not recovered | `hhqq` | world_id:h, client_key:q, lobby_token:q; server `0x140371170`. |
| 4 | 24 `G_LobbyTokenAuthOkCmd` | GameDaemon → client | Receive slot → `0xF3AF90` | `hQiQih` | Charge state. Sender `0x14014ccb0`, call `0x14014EDA7`. See "Command 24". Failure: 25 `G_LobbyTokenAuthFailCmd`, `hh`. |
| 5 | 31 | Client → GameDaemon | Constructor in callback `0xF3AF90` | `hic` | Entry parameter:i, entry flag:c. Server `0x1403718A0`. |
| 6 | 32 | GameDaemon → client | Receive slot → `0xEF6860` → `World::EnterWorld`, `0x741A50` | Fragmented; checked prefix `hihqQihhhhhh…` | Initial PC composite. Sender `0x1405F5980`, fragment mode 2. |
| 7 | 41 | GameDaemon → client | Receive slot → `0xEF67D0` → world-interface prepare target `0x73E6C0` | `hqhiiccKuuuhhh` | Zone identity, destination and load data. Sender `0x1404BEED0`. |
| 8 | 42 | Client → GameDaemon | `BUiWorld::RequestEnterZone`, `0x7815C0` | `hiuuuh` | Spawn correlation:i, summoned-object x/y/z:uuu, summoned yaw:h. Server task `0x1402912F0`. |
| 6a | 87, 90, 423, 1122, 1126, 836 (+627, 1097, 912 if their condition is true) | GameDaemon → client | See "Packets after command 32" | Various | Sent by `Pc::SendEnterWorldOk` `0x1405F5980` after 32 and before 41. |
| 9 | 43 `G_EnterZoneOkCmd` | GameDaemon → client | Receive slot → `0xEF6790` → `World::EnterZone`, `0x74C850` | `hq` | q = enterSocialId = 0 for a first entry. See "Command 43". Failure: 44 `G_EnterZoneFailCmd`. |
| 10 | 4 `G_InSightCubeCmd`, 2 `G_EncounterOutOfSightBroadcastCmd`, 5 `G_OutSightCubeCmd`, 6 `G_BroadcastActionCmd` | GameDaemon → client | Zone broadcast cube packets | Fragmented | `Zone::moveTo` `0x14024B0C0` adds the creature to the cube EncounterList. The zone broadcast flush sends the cube packets later. See "Sight packets". |

This is a partial order, not a complete packet capture.
Command 12 does not prove a direct transition to command 23.
Supplementary packets can occur within and after the command-32 sender.
Do not infer their IDs from adjacent command numbers.

## Commands 929–932: contents rule list

Command names come from `Docs/Client/RE/game-commands.tsv`.

| ID | Name | Direction | Format |
| ---: | --- | --- | --- |
| 929 | `G_ContentsRuleListReqCmd` | Client → GameDaemon | `h` |
| 930 | `G_ContentsRuleListOkCmd` | GameDaemon → client | Fragment, descriptor `hihhiCb;` |
| 931 | `G_ContentsRuleListFailCmd` | GameDaemon → client | `hE`, reason |
| 932 | `G_ContentsRuleReloadNotifyCmd` | GameDaemon → client | `hi`, revision |

### Request handler

Handler: `HandleGame_ContentsRuleListReq`, `0x140370250`.

1. Let `last` be the session field +200 (the time of the last list send, in ms since server start). A new session has 0.
2. Let `now` be `GetTickCount64()` minus the server start tick.
3. Send the cached 930 packet when one of these is true:
   - `last <= 0`,
   - `last <= reload_time` (the time of the last reload; −1 before the first reload),
   - `now <= 30000` (the first 30 s of server uptime),
   - `now − last >= 30000`.
4. On send, set session +200 to `now`.
5. Else send 931 with reason 2167 (`0x877`).

Result: one list per 30 s per session. The first request of a session always gets 930.
The send copies the cached packet, encrypts it and sends it (Packet::Copy, then Session::send).

### Command 930 body

Builder: `ContentsRuleConfig::MakeListPacket`, `0x140087D40`.
The server builds the packet once at load (`LoadContentsRuleConfig`, `0x140087330`) and again at each reload.
Each request sends this cache.

| Order | Wire field | Value |
| --- | --- | --- |
| 1 | h | 930 |
| 2 | i, h | Fragment control. ❓ Layout: see "Fragment layout" under command 32. |
| 3 | h | Major version 74 (`0x140087E04`–`0x140087E1C`) |
| 4 | h | Minor version 0 |
| 5 | i | Revision, `dword_140EE586C` |
| 6 | C | Element count |
| 7 | b × count | UTF-8 text elements (code page 65001). Each element is at most 260 bytes. |

Element order:

1. The XML header element.
2. One `<option>` element for each row of the rule table `off_140ECAD88`, in table order (167 rows).
3. The element `</config>`.

The value text depends on the rule type:

| Type | Rows | Value text |
| ---: | ---: | --- |
| 1 (int32) | 98 | `%d` |
| 2 (int64) | 9 | `%I64d` |
| 3 (float) | 0 | `%f` |
| 4 (bool) | 56 | `yes` or `no` |
| 6 (text) | 4 | `%s` |

❓ The exact header and option element text was not copied. Get it from the format strings in `0x140087D40` before you write golden bytes.

Revision `dword_140EE586C`:

- The initial data value is `0xFFFFFFFF` (−1).
- Only `ReloadContentsRuleConfig` (`0x140087720`) changes it. It increments at `0x140087A1D` and decrements at `0x140087A35` when the reload fails.
- Load does not change it. A fresh server sends −1 (`FF FF FF FF`).

Command 932 carries the revision. `loadCharacterOk` (`0x140152bc0`) sends it only when `0 < session +200 < reload_time`. A fresh session does not get 932.

### Rule table

Config load: `DaemonConfig::Load` with 167 rows, 624-byte step, section `gamed-contents-rule-config`, then `CheckMinMaxBoundary` with 167 rows.
Table `0x140ECAD80`: 56-byte rows. +0 config offset, +8 name, +16 type, +24 min pointer, +32 max pointer, +40 zero, +48 default pointer.
A value in the config file replaces the default. The table below gives the defaults. A server with no config section sends these values.
Rust can generate the 930 body from this table.

| n | Offset | Name | Type | Default | Min | Max |
| ---: | ---: | --- | ---: | --- | --- | --- |
| 1 | 8 | limit-money-market-sale-price | 2 | 15000000000 | 0 | 100000000000 |
| 2 | 16 | limit-money-market-bid-price | 2 | 15000000000 | 0 | 100000000000 |
| 3 | 24 | limit-money-private-trade | 2 | 3000000000 | 0 | 100000000000 |
| 4 | 32 | limit-money-party-auction-bid | 2 | 2000000000 | 0 | 100000000000 |
| 5 | 40 | limit-money-deliver-post | 2 | 2000000000 | 0 | 100000000000 |
| 6 | 48 | limit-money-shop-trade | 2 | 2000000000 | 0 | 100000000000 |
| 7 | 56 | limit-money-random-box-item-price | 2 | 2000000000 | 0 | 100000000000 |
| 8 | 64 | limit-money-guild-bank-deposit | 1 | 2000000000 | 0 | 2000000000 |
| 9 | 72 | limit-money-guild-bank-balance | 2 | 100000000000 | 0 | 100000000000 |
| 10 | 80 | teleport-duration | 1 | 10000 | 0 | 600000 |
| 11 | 84 | teleport-default-distance | 1 | 10 | 0 | 2000000000 |
| 12 | 88 | teleport-price-percent | 1 | 10 | 0 | 100 |
| 13 | 92 | teleport-limit-time | 1 | 180000 | 0 | 600000 |
| 14 | 96 | warp-to-guild-member-required-guild-level | 1 | 1 | 1 | 50 |
| 15 | 100 | warp-to-guild-member-casting-duration | 1 | 10000 | 0 | 600000 |
| 16 | 104 | warp-to-guild-member-cost-money | 1 | 0 | 0 | 2000000000 |
| 17 | 108 | warp-to-guild-member-cost-item-data-id | 1 | 0 | 0 | 2147483647 |
| 18 | 112 | warp-to-guild-member-cost-item-count | 1 | 0 | 0 | 9999 |
| 19 | 116 | warp-to-guild-member-cooltime-duration | 1 | 180000 | 0 | 600000 |
| 20 | 120 | call-guild-member-required-guild-level | 1 | 1 | 1 | 50 |
| 21 | 124 | call-guild-member-casting-duration | 1 | 10000 | 0 | 600000 |
| 22 | 128 | call-guild-member-cost-money | 1 | 0 | 0 | 2000000000 |
| 23 | 132 | call-guild-member-cost-item-data-id | 1 | 0 | 0 | 2147483647 |
| 24 | 136 | call-guild-member-cost-item-count | 1 | 0 | 0 | 9999 |
| 25 | 140 | call-guild-member-expire-duration | 1 | 7200 | 0 | 600000 |
| 26 | 144 | warp-to-guild-meeting-place-casting-duration | 1 | 1000 | 0 | 600000 |
| 27 | 148 | warp-to-guild-meeting-place-cooltime-duration | 1 | 180000 | 0 | 600000 |
| 28 | 152 | use-guild-uniform | 4 | yes | | |
| 29 | 153 | enable-free-closet | 4 | yes | | |
| 30 | 156 | auction-escrow-service-type | 1 | 0 | 0 | 2 |
| 31 | 160 | use-game-mileage | 4 | no | | |
| 32 | 164 | game-mileage-deposit-max-count | 1 | 10 | 1 | 1000 |
| 33 | 168 | duel-achievement-ability-enable | 4 | no | | |
| 34 | 172 | test-server-exp-bonus-percent-npc | 1 | 0 | 0 | 1000 |
| 35 | 176 | test-server-exp-bonus-percent-quest | 1 | 0 | 0 | 1000 |
| 36 | 180 | test-server-money-bonus-percent-npc | 1 | 0 | 0 | 1000 |
| 37 | 184 | test-server-money-bonus-percent-quest-epic | 1 | 0 | 0 | 1000 |
| 38 | 188 | test-server-money-bonus-percent-quest-normal | 1 | 0 | 0 | 1000 |
| 39 | 192 | test-server-item-drop-control-enable | 4 | no | | |
| 40 | 196 | test-server-item-drop-control-rare-prob | 1 | 0 | 0 | 100 |
| 41 | 200 | summoned-change-name-enable | 4 | no | | |
| 42 | 204 | summoned-change-name-cost-item-id | 1 | 0 | 0 | 2000000000 |
| 43 | 208 | use-weapon-appearance-change | 4 | no | | |
| 44 | 212 | ranking-list-refresh-interval | 1 | 600 | 0 | 6000 |
| 45 | 216 | my-ranking-refresh-interval | 1 | 10 | 10 | 6000 |
| 46 | 220 | enable-outer-world-economy-market-register-sale | 4 | yes | | |
| 47 | 221 | use-min-duration-only-for-market-register-sale | 4 | no | | |
| 48 | 224 | guild-flag-bearer-update-limit-duration | 1 | 60 | 0 | 51000 |
| 49 | 228 | create-exclusive-guild-cost | 1 | 1000 | 0 | 2000000000 |
| 50 | 232 | create-open-guild-cost | 1 | 10000 | 0 | 2000000000 |
| 51 | 236 | banish-open-guild-member-limit-count | 1 | 1 | 0 | 100 |
| 52 | 240 | banish-open-guild-member-count-daily-reset-time | 1 | 6 | 0 | 23 |
| 53 | 244 | rename-guild-enable | 4 | no | | |
| 54 | 246 | rename-guild-by-guild-name-prefix | 6 | (empty) | | |
| 55 | 768 | rename-guild-by-inventory-item-data-id | 1 | 0 | 0 | 2147483647 |
| 56 | 772 | rename-guild-rename-limit-duration | 1 | 2592000 | 0 | 31104000 |
| 57 | 776 | use-delegate-guildleader-forcibly | 4 | no | | |
| 58 | 780 | random-store-reset-time | 1 | 6 | 0 | 23 |
| 59 | 784 | random-store-1-enable | 4 | no | | |
| 60 | 785 | random-store-2-enable | 4 | no | | |
| 61 | 788 | craft-registration-quota | 1 | 3 | 0 | 10 |
| 62 | 792 | arena-dungeon-reinforcements-member-enable | 4 | no | | |
| 63 | 796 | arena-dungeon-reinforce-member-retry-sec | 1 | 5 | 1 | 86400 |
| 64 | 800 | arena-party-battle-occupation-war-enable | 4 | no | | |
| 65 | 801 | arena-party-battle-capture-the-flag-enable | 4 | no | | |
| 66 | 802 | arena-party-battle-lead-the-ball-enable | 4 | no | | |
| 67 | 803 | arena-random-party-battle-field-enable | 4 | no | | |
| 68 | 804 | arena-guild-combat-enable | 4 | no | | |
| 69 | 805 | arena-guild-combat-limit-enable | 4 | no | | |
| 70 | 808 | arena-guild-combat-limit-duration | 1 | 168 | 0 | 720 |
| 71 | 812 | arena-public-raid-enable | 4 | no | | |
| 72 | 813 | arena-party-battle-field-no-game-result-win-and-draw | 4 | no | | |
| 73 | 814 | arena-party-battle-field-no-game-result-lose | 4 | no | | |
| 74 | 815 | arena-party-battle-field-no-game-reward-win-and-draw | 4 | no | | |
| 75 | 816 | arena-party-battle-field-no-game-reward-lose | 4 | no | | |
| 76 | 817 | arena-guild-combat-no-game-reward-win-and-draw | 4 | no | | |
| 77 | 818 | arena-guild-combat-no-game-reward-lose | 4 | no | | |
| 78 | 819 | arena-spectate-enable | 4 | no | | |
| 79 | 820 | arena-spectate-max-game-spectator-count | 1 | 10 | 0 | 20 |
| 80 | 824 | arena-spectate-max-arenad-spectator-count | 1 | 2000 | 0 | 3000 |
| 81 | 828 | arena-spectate-rate-limit-sec | 1 | 30 | 0 | 86400 |
| 82 | 832 | arena-spectate-duel-1vs1-top-scorer-room-count | 1 | 10 | 0 | 100 |
| 83 | 836 | arena-spectate-duel-1vs1-guild-member-room-count | 1 | 10 | 0 | 100 |
| 84 | 840 | arena-duel-1vs1-min-rating-score | 1 | 130000 | 0 | 500000 |
| 85 | 844 | arena-duel-1vs1-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 86 | 848 | arena-duel-3vs3-min-rating-score | 1 | 130000 | 0 | 500000 |
| 87 | 852 | arena-duel-3vs3-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 88 | 856 | arena-party-battle-occupation-war-min-rating-score | 1 | 130000 | 0 | 500000 |
| 89 | 860 | arena-party-battle-occupation-war-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 90 | 864 | arena-party-battle-capture-the-flag-min-rating-score | 1 | 130000 | 0 | 500000 |
| 91 | 868 | arena-party-battle-capture-the-flag-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 92 | 872 | arena-party-battle-lead-the-ball-min-rating-score | 1 | 130000 | 0 | 500000 |
| 93 | 876 | arena-party-battle-lead-the-ball-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 94 | 880 | arena-guild-combat-min-rating-score | 1 | 130000 | 0 | 500000 |
| 95 | 884 | arena-guild-combat-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 96 | 888 | arena-battle-royal-min-rating-score | 1 | 0 | 0 | 500000 |
| 97 | 892 | arena-battle-royal-max-rating-score | 1 | 500000 | 0 | 1000000 |
| 98 | 896 | arena-ingame-championship-enable | 4 | no | | |
| 99 | 897 | arena-battle-royal-field-matching-enable | 4 | no | | |
| 100 | 898 | arena-battle-royal-field-jobstyle-restriction | 6 | (empty) | | |
| 101 | 1418 | arena-battle-royal-field-job-restriction | 6 | (empty) | | |
| 102 | 1940 | transfer-faction-score-percent-by-pc-killed | 1 | 100 | 0 | 100 |
| 103 | 1944 | enable-help-resurrect-enemy-faction | 4 | yes | | |
| 104 | 1945 | use-boast-event | 4 | no | | |
| 105 | 1946 | use-time-based-play-schedule | 4 | no | | |
| 106 | 1948 | update-time-based-play-info-period-minute | 1 | 0 | 0 | 10080 |
| 107 | 1952 | item-transform-failure-upper-limit-enable | 4 | no | | |
| 108 | 1956 | item-transform-failure-mileage-max | 1 | 100 | 0 | 30000 |
| 109 | 1960 | item-transform-failure-mileage-care-value | 1 | 2 | 1 | 100 |
| 110 | 1964 | item-transform-failure-mileage-invariable-min | 1 | 50 | 0 | 1000 |
| 111 | 1968 | item-transform-failure-mileage-invariable-max | 1 | 150 | 0 | 1000 |
| 112 | 1972 | item-transform-retry-enable | 4 | no | | |
| 113 | 1976 | game-time-schedule-limit-duration | 1 | 30 | 0 | 1440 |
| 114 | 1980 | enable-skill-skin-system | 4 | no | | |
| 115 | 1981 | enable-personal-customize | 4 | no | | |
| 116 | 1982 | enable-challenge-party | 4 | no | | |
| 117 | 1983 | enable-challenge-party-banish | 4 | no | | |
| 118 | 1984 | challenge-party-confirm-invitation-duration | 1 | 60 | 0 | 51000 |
| 119 | 1988 | challenge-party-rejoin-duration | 1 | 1440 | 0 | 10080 |
| 120 | 1992 | use-passing-item-ability | 4 | no | | |
| 121 | 1996 | passing-item-ability-grade-min | 1 | 0 | 0 | 8 |
| 122 | 2000 | enable-passing-item-weapon-type | 4 | no | | |
| 123 | 2001 | enable-fish-system | 4 | no | | |
| 124 | 2004 | max-skill-build-up-point | 1 | 30000 | 0 | 30000 |
| 125 | 2008 | enable-account-contents | 4 | no | | |
| 126 | 2012 | account-contents-required-pc-level | 1 | 0 | 0 | 1000 |
| 127 | 2016 | account-contents-required-pc-mastery-level | 1 | 0 | 0 | 1000 |
| 128 | 2020 | account-contents-required-account-top-pc-level | 1 | 0 | 0 | 1000 |
| 129 | 2024 | account-contents-required-account-top-pc-mastery-level | 1 | 0 | 0 | 1000 |
| 130 | 2028 | account-exp-quota-per-day | 1 | 0 | 0 | 2147483647 |
| 131 | 2032 | account-exp-quota-reset-time | 1 | 6 | 0 | 23 |
| 132 | 2036 | account-level-max | 1 | 50 | 0 | 1000 |
| 133 | 2040 | enable-badge | 4 | no | | |
| 134 | 2041 | enable-guild-join-renewal | 4 | no | | |
| 135 | 2044 | guild-join-renewal-retry-limit-period | 1 | 20 | 0 | 86400 |
| 136 | 2048 | guild-application-applied-expiration-duration | 1 | 86400 | 0 | 604800 |
| 137 | 2052 | guild-application-accepted-expiration-duration | 1 | 86400 | 20 | 604800 |
| 138 | 2056 | guild-application-quota-per-pc | 1 | 10 | 1 | 50 |
| 139 | 2060 | guild-application-quota-per-guild | 1 | 10 | 1 | 50 |
| 140 | 2064 | guild-invitation-expiration-duration | 1 | 86400 | 20 | 604800 |
| 141 | 2068 | guild-invitation-quota-per-pc | 1 | 10 | 1 | 50 |
| 142 | 2072 | guild-invitation-quota-per-guild | 1 | 10 | 1 | 50 |
| 143 | 2076 | use-buy-collection-item-by-instant-payment | 4 | no | | |
| 144 | 2080 | account-recycle-time-db-id-min | 1 | 21 | 21 | 21 |
| 145 | 2084 | account-recycle-time-db-id-max | 1 | 25 | 25 | 25 |
| 146 | 2088 | board-gacha-enable | 4 | no | | |
| 147 | 2089 | enable-contents-reset-system | 4 | no | | |
| 148 | 2092 | slate-open-page-count | 1 | 0 | 0 | 2 |
| 149 | 2096 | slate-open-pc-level | 1 | 55 | 0 | 1000 |
| 150 | 2100 | slate-open-pc-mastery-level | 1 | 10 | 0 | 1000 |
| 151 | 2104 | enable-dungeon-additional-reward-system | 4 | no | | |
| 152 | 2105 | enable-faction | 4 | yes | | |
| 153 | 2106 | enable-guild-bank | 4 | yes | | |
| 154 | 2107 | enable-guild-point | 4 | no | | |
| 155 | 2108 | guild-point-percent-per-exp | 1 | 0 | 0 | 100 |
| 156 | 2112 | guild-point-percent-per-account-exp | 1 | 0 | 0 | 100 |
| 157 | 2116 | guild-point-accumulate-retry-limit-period | 1 | 1800 | 0 | 86400 |
| 158 | 2120 | guild-point-per-character-max-limit | 1 | 2000000000 | 0 | 2147483647 |
| 159 | 2128 | guild-point-per-guild-max-limit | 2 | 9000000000000000000 | 0 | 9223372036854775807 |
| 160 | 2136 | enable-guild-benefit | 4 | no | | |
| 161 | 2137 | enable-open-guild | 4 | no | | |
| 162 | 2138 | enable-guild-warp-renewal | 4 | no | | |
| 163 | 2140 | guild-warp-renewal-required-completion-quest-id | 1 | 0 | 0 | 32767 |
| 164 | 2144 | enable-guild-alliance | 4 | no | | |
| 165 | 2148 | guild-alliance-max-ally-count | 1 | 3 | 1 | 10 |
| 166 | 2152 | enable-job | 6 | `;` ❓ | | |
| 167 | 2672 | special-skill-usable-level | 1 | 0 | 0 | 1000 |

## Command 24: token success

Sender: `LobbyTokenAuthTran::work`, `0x14014ccb0`. Send call: `0x14014EDA7`. After the send, the transaction goes to state 17.
Full format: `hQiQih`. The values come from the transaction object. The 2019 PDB names are at offset −8.

| Wire field | Width | 381 offset | 2019 name | Fresh-PC value |
| --- | ---: | ---: | --- | --- |
| Charge apply type | Q (1) | +1980 | `_chargeApplyType` | 0 for free-to-play, 1 for trial |
| Remain play time | i | +1984 | `_remainPlayTime` | −1 |
| Charge base type | Q (1) | +1988 | `_chargeBaseType` | 0 |
| Charge item id | i | +1992 | `_chargeItemId` | 0 |
| ❓ 381 field | h | +1996 | ❓ (2019 `_reservations`?) | 0 |

`loadCharacterOk` (`0x140152bc0`, decompiled lines 562–618) reads the user charge apply type (+140):

- 2 (trial): set +1980 to 1.
- 3 (free-to-play): set +1980 to 0.
- Other values: go to state 15. `tryStartCharge` and `startChargeOk` fill the fields from the reply (+132, +136, +140, +144).
- For 2 and 3, if session +238 is 0, set it to 2, and copy it to PC word 10336. ❓ Meaning. Command 32 sends this value as its "Session +238" field.

Free-to-play golden body after the command: `00 FF FF FF FF 00 00 00 00 00 00 00` (12 bytes). The command bytes are `18 00`.
Failure: 25 `G_LobbyTokenAuthFailCmd`, `hh`, reason. Many branches send it.

## Command 31: enter-world request

| Field | Width | Checked source | Meaning |
| --- | --- | --- | --- |
| Entry parameter | i | Client packet-object +0x10; server decode in `0x1403718A0` | Client starts with zero. A conditional security-version getter can change it. ❓ Server log calls it characterId. The conflict is unresolved. It is not a verified character DB ID. |
| Entry flag | c | Client packet-object +0x14, set to zero in the checked branch | ❓ Meaning not verified. |

The server rejects duplicate requests.
It calls the command-32 sender first.
It then selects a destination with `0x1405F1230` or an alternate route.
It calls world-entry helper `0x1403F4A60` after destination selection.
Thus a later destination failure can follow command 32.
The failure reply is command 33, full format `hh`, with reason:h.
Client command-33 receive slot points to `0xF2BEA0`.

The security getter is a boundary. Calls at `0xF3B00C` and `0xF3B1F6` are out of scope.
Do not inspect or reproduce that getter.

## Command 32: initial player composite

Sender: `0x1405F5980`. Format literal: `0x140DAA240`.
The literal starts `hihqQihhhhhh…`. This is a prefix, not an encodable complete format.
The sender uses fragment mode 2.
❓ The fragment-control field layout and limits were not recovered here.
Use the existing Packet fragment layer only after a compatibility check.
Do not send this payload as a header-only success.

| Checked prefix field | Native source | Provenance |
| --- | --- | --- |
| command:h | 32 | Protocol |
| fragment field:i | Fragment writer | Protocol; ❓ bit layout open |
| short:h | PC +96 | Runtime PC; ❓ semantic name and DB origin open |
| object_id:q | PC +40 | Runtime object identity; not a table ID |
| byte:Q | PC +13740 | Runtime PC; ❓ semantic name open |
| integer:i | Configuration +296 | Server configuration; ❓ semantic name open |
| short:h | Configuration +304 | Server configuration; ❓ meaning open |
| short:h | Runtime/config-selected local value | ❓ Meaning and original source open |
| short:h | Configuration +152 if +160 is enabled, else zero | Server configuration |
| short:h | World/daemon +86858 | Runtime world configuration; ❓ meaning open |
| short:h | Session +238, or zero | Runtime session; ❓ meaning open |

The descriptor contains six `h` fields after the configuration integer.
The initial compose call emits only the five independent shorts above.
❓ The next `h` appears to start the property section. Its property mapping is not verified.
Do not invent a sixth configuration field.

### Remaining ordered groups

Source: build-381 GameDaemon sender `0x1405F5980`, literal `0x140DAA240`.
This is a group inventory, not a complete encodable format.

| Order after prefix | Format / checked source | Provenance and limit |
| ---: | --- | --- |
| 1 | Property IDs 1..`0x1A6`; PC property container +48; writer `0x140697E50` | 422 property IDs. Descriptor advance is 373, not 422 wire fields. Custom `?` entries occur. ❓ Full dynamic layout and DB columns remain open. |
| 2 | `b`; helper `0x14025FB20` | Runtime PC byte block. ❓ Content identity open. |
| 3 | `K`; daemon pointer +3096, length +3104 | Daemon byte block. ❓ Original source and meaning open. |
| 4 | `C[ci]`; helper `0x140618730`, PC +3912 | Runtime collection. ❓ Element meanings open. |
| 5 | `C[Qb]`; count 2, tags 1 then 2 | Two property containers at PC +29808, stride 3352; property ID 2. ❓ Nested byte contract open. |
| 6 | `cccccq` | Four configuration bytes at +704/+708/+712/+716; helper byte `0x1405ED470`; PC q at +20744. ❓ Meanings open. |
| 7 | `C[Qqq]`; up to nine entries | Session +872/+880, stride 16. Emit nonzero pairs when configuration enables them. ❓ Semantic names open. |
| 8 | `Q` | Configuration +746 != 0. ❓ Option meaning open. |
| 9 | `C[ch]`; up to two entries | PC +39784/+39786, stride 4. Include only types 1..14. ❓ Meaning open. |
| 10 | ❓ `C[Qiqqihcccc]` from descriptor | Helper `0x1403E8C00`, session +1288. Helper-to-field mapping was not checked. |
| 11 | Final counted group | Helper `0x1401BB5E0`, PC +20616. ❓ Full element mapping remains open. |

A PC memory source does not prove a character DB column.
Keep the full command-32 body raw and unsupported.
The generic BloodLib codec cannot encode the custom `?` fields.
The existing fragment tests do not prove native mode-2 metadata compatibility.

The sender also composes nested data with `hchcccchchchch`.
A conditional composer uses `hqciqh`.
❓ These strings do not establish independent packet IDs or complete optional-group contracts.
Other send helpers run from this function.
❓ Inventory, skills, quests and social data were not separated from required initialization.

Client callback `0xEF6860` passes packet-object +0x500 to session virtual slot +0x38.
It then calls world-interface slot +4, which resolves to `World::EnterWorld`, `0x741A50`.
Stop the world-function path at the restricted reference `0x741AB2`.
The callback also reaches a security interface at `0xEF68C8`; do not follow that path.

### Command 32 layout, gw-enter stage 1 (381)

Sender: `Pc::SendEnterWorldOk`, `0x1405F5980`. Caller: `HandleGame_EnterWorldReq`, `0x1403718A0`.
Format literal: `0x140DAA240`, 448 characters (442 without `;`). It starts `hihqQihhhhhhccc`.
The fragment prefix is `hi`. The sender sets fragment value 1 and command 32. The 930 descriptor uses the same prefix.
❓ The meaning of the `hi` fragment fields (fragment mode) is open.

Header fields, in order (one Compose call, 9 values):

| # | Type | Source in 381 |
| ---: | --- | --- |
| 1 | h | Pc word +48 |
| 2 | q | Pc qword +40 (object id) |
| 3 | Q | Pc byte +13740 |
| 4 | i | controller +342 → +296 |
| 5 | h | +304 |
| 6 | h | Chat server id. If flags +492 and +512 are set: +248 + 10000 when +237 is set, else controller word 43426. Else `GameDaemon::GetServerIdForChat(1)`. |
| 7 | h | +152 if +160 is set, else 0 |
| 8 | h | controller word 43429 |
| 9 | h | session (Pc +2587) word +238; 0 if no session or Pc +20752 is set |

Property blob: the sender loops over ids 1..422 (`< 0x1A7`).
For each id it calls `Property::GetPureData` (`0x140697E50`) with pure mode 1.
It concatenates the results into a 4,000-byte buffer and copies it raw. Then it skips 373 format characters.

`Property::GetPureData` rules (`0x140697E50`):

- The field table comes from virtual slot 5 (FieldDefine). The count comes from slot 6 (FieldMax).
- One Def entry is 184 bytes: +4 type, +138 object offset, +160/+162 bit range, +164 byte offset, +166 bit index.
- Type 0: not sent.
- Types 10 (bit field) and 11 (bool): an entry with bit index ≠ 0 sends 0 bytes. The bit-0 entry sends the full byte.
- Type 12: wide string, u16 character count + 2 × count bytes.
- Types 13 and 14: i16 length + bytes.
- Other types: size from table `0x1407E54A0` = 0, 1, 2, 4, 8, 16, 1, 2, 4, 8, 1, 1 (index = type). ❓ Types 1–5 signed, 6–9 unsigned.

Def table source: the runtime table `CreatureProperty::fieldDefine` (`0x140EE5418`) is empty in the file.
`GameDaemon::initialize` (`0x14033A4BA`) fills it with `Property::initInstance` (`0x1406983A0`).
Arguments: count 422 (`0x1A6`), static FieldDef table `0x1407CA330`, 4, SyncDef table `0x1407C8340`, outputs `0x140EE5530`, `0x140EE5418`, `0x140EE55B0`, `0x140EE5350`.
`Property::initFieldDefine` (`0x1406985C0`) builds the 184-byte Def entries from the static FieldDef entries.

#### Static FieldDef entry (381, `0x1407CA330`, 422 entries, stride 48)

| Offset | Size | Meaning | Evidence |
| ---: | ---: | --- | --- |
| +0 | 2 | Group 1..6. A bit run stops at a group change. | `0x1406985C0` line 124 |
| +2 | 2 | Property id 1..422 (row index + 1) | to Def +2 |
| +4 | 1 | Type | to Def +4 |
| +8 | 8 | Pointer to the wide name | `wcsncpy` to Def +6 |
| +16, +17, +18 | 1 each | Flags to Def +134, +135, +136. ❓ Meaning open. | lines 86–93 |
| +24 | 8 | Minimum value | to Def +160 (types 1–9) |
| +32 | 8 | Maximum value; for types 12 and 14 the maximum length | to Def +168 |
| +40 | 2 | Byte offset of the value in the Property object | to Def +138 |
| +42..+45 | 1 each | Four sync flags, copied to the size-table bytes 8..11. ❓ Meaning open (who gets the update). | lines 205–211 |

Groups (FieldDef +0): 1 = ids 1–97, 2 = 98–243, 3 = 244–343, 4 = 344–367, 5 = 368–401, 6 = 402–422.

Types in the table: 1 i8, 2 i16, 3 i32, 4 i64, 7, 8, 9 (stat values, wire size 2, 4, 8), 10 bit, 11 bool, 12 wide string, 14 byte blob. Types 5, 6 and 13 do not occur.
Types 7–9 keep more than one value in the object (object stride 8, 16, 32). Their min can be negative, so they are signed on the wire.
`GetPureData` returns them with `Property::get`. ❓ Which value of the stat (base or total) it returns is not checked.

Wire rules for pure mode 1 (`Property::GetPureData`, `0x140697E50`):

- Types 1–4 and 7–9: the value, little-endian, size from table `0x1407E54A0`.
- Type 12 (`name`, id 97): u16 character count n, then 2n bytes. No terminator.
- Type 14 (`survey`, id 150): i16 length n, then n bytes.
- Types 10 and 11 make bit runs. A run is a sequence of type-10/11 rows in the same group. The bit index counts 0..7, then a new byte starts. The row with bit 0 sends one byte. The other rows send 0 bytes. Bit k of the byte is the value of row (first row of the byte + k): for type 11 the value is "byte ≠ 0", for type 10 the stored bit.
- Wire size of the blob: 1,020 bytes + (2 + 2 × name length) + (2 + survey length). For a new PC with an empty survey: 1,024 + 2 × name length.

Column "Sync" is FieldDef +42..+45 as four digits. Column "Off" is FieldDef +40. Source of all rows: dump of `0x1407CA330` (381).
Rows with a range ("5–96") have the same type, min, max and sync. The name has the number id − 4 (or id − 177), and the offset increases by the type size.

#### Command 32 property table (381)

| Id | Name | Grp | Type | Wire | Min | Max | Off | Sync | Bits |
| ---: | --- | ---: | --- | --- | ---: | ---: | ---: | --- | --- |
| 1 | `world-id` | 1 | i16 (2) | 2 | -32768 | 32767 | 48 | 1100 |  |
| 2 | `race` | 1 | i8 (1) | 1 | -128 | 127 | 50 | 1101 |  |
| 3 | `sex` | 1 | i8 (1) | 1 | -128 | 127 | 51 | 1101 |  |
| 4 | `job` | 1 | i8 (1) | 1 | -128 | 127 | 52 | 1101 |  |
| 5–96 | `appearance1`..`appearance92` | 1 | i8 (1) | 1 each | -128 | 127 | 53..144 | 1100 |  |
| 97 | `name` | 1 | wstr (12) | 2+2n | 0 | 32 | 146 | 1101 | n ≤ 32 |
| 98 | `geo-zone` | 2 | i32 (3) | 4 | 1 | 65535 | 216 | 1010 |  |
| 99 | `x` | 2 | i16 (2) | 2 | -32000 | 31999 | 220 | 1011 |  |
| 100 | `y` | 2 | i16 (2) | 2 | -32000 | 31999 | 222 | 1011 |  |
| 101 | `z` | 2 | i16 (2) | 2 | -32000 | 31999 | 224 | 1011 |  |
| 102 | `yaw` | 2 | i16 (2) | 2 | 0 | 359 | 226 | 1011 |  |
| 103 | `level` | 2 | i8 (1) | 1 | 1 | 70 | 228 | 1010 |  |
| 104 | `exp` | 2 | i32 (3) | 4 | 0 | 2000000000 | 232 | 1010 |  |
| 105 | `mastery-level` | 2 | i8 (1) | 1 | 0 | 50 | 236 | 1010 |  |
| 106 | `mastery-exp` | 2 | i64 (4) | 8 | 0 | 1000000000000 | 240 | 1000 |  |
| 107 | `hp` | 2 | i64 (4) | 8 | 0 | 1000000000000 | 248 | 1011 |  |
| 108 | `guard-gauge` | 2 | i64 (4) | 8 | 0 | 1000000000000 | 256 | 1011 |  |
| 109 | `money` | 2 | i64 (4) | 8 | 0 | 20000000000 | 264 | 1000 |  |
| 110 | `money-diff` | 2 | i64 (4) | 8 | -20000000000 | 20000000000 | 272 | 1000 |  |
| 111 | `faction` | 2 | i8 (1) | 1 | 0 | 100 | 280 | 1110 |  |
| 112 | `faction2` | 2 | i8 (1) | 1 | 0 | 100 | 281 | 1010 |  |
| 113 | `guild-id` | 2 | i64 (4) | 8 | i64 | i64 | 288 | 0010 |  |
| 114 | `activated-faction` | 2 | i8 (1) | 1 | 0 | 100 | 296 | 0010 |  |
| 115 | `faction-reputation` | 2 | i32 (3) | 4 | 0 | 200000 | 300 | 1010 |  |
| 116 | `achievement-id` | 2 | i16 (2) | 2 | i16 | i16 | 304 | 1010 |  |
| 117 | `achievement-step` | 2 | i16 (2) | 2 | i16 | i16 | 306 | 1010 |  |
| 118 | `ability-achievement-id` | 2 | i16 (2) | 2 | i16 | i16 | 308 | 1000 |  |
| 119 | `ability-achievement-step` | 2 | i16 (2) | 2 | i16 | i16 | 310 | 1000 |  |
| 120 | `sp` | 2 | i16 (2) | 2 | 0 | 32767 | 312 | 0011 |  |
| 121 | `sp2` | 2 | i16 (2) | 2 | 0 | 32767 | 314 | 0011 |  |
| 122 | `stance` | 2 | i8 (1) | 1 | 0 | 127 | 316 | 0011 |  |
| 123 | `link-target` | 2 | i64 (4) | 8 | i64 | i64 | 320 | 0011 |  |
| 124 | `link-type` | 2 | i8 (1) | 1 | i8 | i8 | 328 | 0011 |  |
| 125 | `link-state` | 2 | i8 (1) | 1 | i8 | i8 | 329 | 0011 |  |
| 126 | `link-stage` | 2 | i8 (1) | 1 | i8 | i8 | 330 | 0011 |  |
| 127 | `target` | 2 | i64 (4) | 8 | i64 | i64 | 336 | 0010 |  |
| 128 | `action-type` | 2 | i8 (1) | 1 | i8 | i8 | 344 | 0010 |  |
| 129 | `action-target` | 2 | i64 (4) | 8 | i64 | i64 | 352 | 0010 |  |
| 130 | `additional-hp` | 2 | i32 (3) | 4 | 0 | 2147483647 | 360 | 0010 |  |
| 131 | `effect-attribute` | 2 | i64 (4) | 8 | i64 | i64 | 368 | 0011 |  |
| 132 | `effect-attribute-2` | 2 | i64 (4) | 8 | i64 | i64 | 376 | 0011 |  |
| 133 | `effect-attribute-3` | 2 | i64 (4) | 8 | i64 | i64 | 384 | 0011 |  |
| 134 | `immune-effect-attribute` | 2 | i64 (4) | 8 | i64 | i64 | 392 | 0010 |  |
| 135 | `immune-effect-attribute-2` | 2 | i64 (4) | 8 | i64 | i64 | 400 | 0010 |  |
| 136 | `immune-effect-attribute-3` | 2 | i64 (4) | 8 | i64 | i64 | 408 | 0010 |  |
| 137 | `effect-flag` | 2 | i64 (4) | 8 | i64 | i64 | 416 | 0011 |  |
| 138 | `effect-flag-2` | 2 | i64 (4) | 8 | i64 | i64 | 424 | 0011 |  |
| 139 | `effect-flag-3` | 2 | i64 (4) | 8 | i64 | i64 | 432 | 0010 |  |
| 140 | `effect-flag-4` | 2 | i64 (4) | 8 | i64 | i64 | 440 | 0010 |  |
| 141 | `field-item` | 2 | i64 (4) | 8 | i64 | i64 | 448 | 0010 |  |
| 142 | `summoned` | 2 | i64 (4) | 8 | i64 | i64 | 456 | 0011 |  |
| 143 | `convoy-id` | 2 | i8 (1) | 1 | i8 | i8 | 464 | 0010 |  |
| 144 | `top-contribution-group-id` | 2 | i64 (4) | 8 | i64 | i64 | 472 | 0010 |  |
| 145 | `inventory-size` | 2 | i8 (1) | 1 | 0 | 96 | 480 | 1000 |  |
| 146 | `depository-size` | 2 | i8 (1) | 1 | 0 | 48 | 481 | 1000 |  |
| 147 | `wardrobe-size` | 2 | i8 (1) | 1 | 0 | 80 | 482 | 1000 |  |
| 148 | `premium-depository-size` | 2 | i8 (1) | 1 | 0 | 80 | 483 | 1000 |  |
| 149 | `builder-right` | 2 | i8 (1) | 1 | -1 | 10 | 484 | 1000 |  |
| 150 | `survey` | 2 | blob (14) | 2+n | 0 | 10 | 486 | 1000 | n ≤ 10 |
| 151 | `style-score` | 2 | i16 (2) | 2 | i16 | i16 | 498 | 0000 |  |
| 152 | `immune-breaker-attribute` | 2 | i16 (2) | 2 | i16 | i16 | 500 | 0010 |  |
| 153 | `immune-breaker-count` | 2 | i16 (2) | 2 | i16 | i16 | 502 | 0010 |  |
| 154 | `faction-score` | 2 | i32 (3) | 4 | 0 | 1000000 | 504 | 0010 |  |
| 155 | `faction-killed-count` | 2 | i8 (1) | 1 | i8 | i8 | 508 | 0000 |  |
| 156 | `combat-side` | 2 | i8 (1) | 1 | i8 | i8 | 509 | 0010 |  |
| 157 | `duel-point` | 2 | i32 (3) | 4 | 0 | 2147483647 | 512 | 1000 |  |
| 158 | `party-battle-point` | 2 | i32 (3) | 4 | 0 | 2147483647 | 516 | 1000 |  |
| 159 | `field-play-point` | 2 | i32 (3) | 4 | 0 | 2147483647 | 520 | 1000 |  |
| 160 | `life-contents-point` | 2 | i32 (3) | 4 | 0 | 2147483647 | 524 | 1000 |  |
| 161 | `duel-grade` | 2 | i8 (1) | 1 | 0 | 127 | 528 | 0010 |  |
| 162 | `duel-grade-type` | 2 | i8 (1) | 1 | 0 | 127 | 529 | 0010 |  |
| 163 | `dead` | 2 | i8 (1) | 1 | i8 | i8 | 530 | 0011 |  |
| 164 | `gm-mode` | 2 | i8 (1) | 1 | i8 | i8 | 531 | 0010 |  |
| 165 | `grade-level` | 2 | i8 (1) | 1 | 0 | 20 | 532 | 0010 |  |
| 166–173 | `grade-broadcast-benefit-1`..`-8` | 2 | bit (10) | 1 at 166, else 0 | 0 | 1 | 533 | 0010 | bits 0–7 |
| 174 | `logout-time` | 2 | i64 (4) | 8 | i64 | i64 | 536 | 1000 |  |
| 175 | `instant-heart-count` | 2 | i8 (1) | 1 | 0 | 50 | 544 | 0000 |  |
| 176 | `heart-count` | 2 | i8 (1) | 1 | 0 | 50 | 545 | 1000 |  |
| 177 | `skill-score` | 2 | i32 (3) | 4 | i32 | i32 | 548 | 0000 |  |
| 178–194 | `bound-raid-dungeon-id-1`..`-17` | 2 | i64 (4) | 8 each | i64 | i64 | 552..680 | 0000 |  |
| 195 | `boss-second-gauge` | 2 | i16 (2) | 2 | -1000 | 1000 | 688 | 0010 |  |
| 196 | `accumulated-damage-slot-tao` | 2 | i64 (4) | 8 | 0 | i64 max | 696 | 0000 |  |
| 197–199 | `accumulated-damage-slot-1`..`-3` | 2 | i64 (4) | 8 each | 0 | i64 max | 704..720 | 0010 |  |
| 200 | `skill-skin-id` | 2 | i16 (2) | 2 | 0 | 3000 | 728 | 1010 |  |
| 201 | `pvp-mode-cooltime` | 2 | i64 (4) | 8 | i64 | i64 | 736 | 1010 |  |
| 202 | `ghost-type` | 2 | i8 (1) | 1 | i8 | i8 | 744 | 0011 |  |
| 203 | `challenge-party-id` | 2 | i64 (4) | 8 | i64 | i64 | 752 | 0000 |  |
| 204 | `challenge-party-out-time` | 2 | i64 (4) | 8 | i64 | i64 | 760 | 1000 |  |
| 205 | `acquired-skill-build-up-point` | 2 | i16 (2) | 2 | -32768 | 30000 | 768 | 1000 |  |
| 206 | `usable-skill-build-up-point` | 2 | i16 (2) | 2 | -32768 | 30000 | 770 | 0000 |  |
| 207 | `job-style` | 2 | i8 (1) | 1 | 0 | 10 | 772 | 0010 |  |
| 208 | `account-exp-to-add` | 2 | i32 (3) | 4 | 0 | 2147483647 | 776 | 1000 |  |
| 209 | `account-exp-added` | 2 | i32 (3) | 4 | 0 | 2000000000 | 780 | 1000 |  |
| 210 | `account-exp-added-time` | 2 | i64 (4) | 8 | 0 | i64 max | 784 | 1000 |  |
| 211 | `account-exp-by-pc` | 2 | i64 (4) | 8 | 0 | i64 max | 792 | 1000 |  |
| 212 | `account-level` | 2 | i16 (2) | 2 | 0 | 32767 | 800 | 0010 |  |
| 213 | `account-exp` | 2 | i64 (4) | 8 | 0 | i64 max | 808 | 0000 |  |
| 214 | `account-exp-quota-per-day` | 2 | i32 (3) | 4 | 0 | 2147483647 | 816 | 0000 |  |
| 215 | `account-exp-last-update-time` | 2 | i64 (4) | 8 | 0 | i64 max | 824 | 0000 |  |
| 216 | `activated-badge-page` | 2 | i8 (1) | 1 | 1 | 3 | 832 | 1000 |  |
| 217 | `equip-badge-gear-score` | 2 | i32 (3) | 4 | 0 | 2147483647 | 836 | 0010 |  |
| 218–225 | `combat-mode`, `falling`, `convoy`, `attackable`, `casting`, `immune-breaker-disable`, `npc-interactive`, `npc-talkable` | 2 | bit (10) | 1 at 218, else 0 | 0 | 1 | 840 | 0010 | bits 0–7 |
| 226–233 | `special-skill-gauge-full`, `pvp-mode` (sync 1010), `pvp-safe-area`, `zone-move-state`, `public-reserve-5`..`-8` | 2 | bit (10) | 1 at 226, else 0 | 0 | 1 | 840 | 0010 / 0000 | bits 0–7 |
| 234–241 | `detect-hiding` (sync 0001), `guild-invitation-refusal` (sync 1000), `private-reserve-3`..`-8` | 2 | bit (10) | 1 at 234, else 0 | 0 | 1 | 840 | 0000 | bits 0–7 |
| 242 | `slate-page` | 2 | i8 (1) | 1 | 1 | 2 | 843 | 1000 |  |
| 243 | `guild-point` | 2 | i32 (3) | 4 | 0 | 2000000000 | 844 | 1000 |  |
| 244 | `max-hp` | 3 | stat64 (9) | 8 | 1 | 1000000000000 | 848 | 0011 |  |
| 245 | `max-hp-equip` | 3 | stat32 (8) | 4 | 0 | 2000000000 | 880 | 0011 |  |
| 246 | `max-guard-gauge` | 3 | stat64 (9) | 8 | 0 | 1000000000000 | 896 | 0011 |  |
| 247 | `max-guard-gauge-equip` | 3 | stat32 (8) | 4 | 0 | 2000000000 | 928 | 0011 |  |
| 248 | `max-sp` | 3 | stat16 (7) | 2 | 0 | 10000 | 944 | 0011 |  |
| 249 | `max-sp2` | 3 | stat16 (7) | 2 | 0 | 10000 | 952 | 0011 |  |
| 250 | `speed` | 3 | stat16 (7) | 2 | 0 | 1000 | 960 | 0011 |  |
| 251 | `modify-cast-speed-percent` | 3 | stat16 (7) | 2 | -1000 | 1000 | 968 | 0000 |  |
| 252–255 | `hp-regen`, `hp-regen-equip`, `hp-regen-combat`, `hp-regen-combat-equip` | 3 | stat32 (8) | 4 each | 0 | 2000000000 | 976..1024 | 0000 |  |
| 256 | `attack-hit-base-percent` | 3 | stat16 (7) | 2 | -1000 | 10000 | 1040 | 0000 |  |
| 257 | `attack-hit-value` | 3 | stat16 (7) | 2 | -30000 | 30000 | 1048 | 0000 |  |
| 258 | `attack-hit-value-equip` | 3 | stat16 (7) | 2 | -30000 | 30000 | 1056 | 0000 |  |
| 259 | `attack-pierce-base-percent` | 3 | stat16 (7) | 2 | -1000 | 10000 | 1064 | 0000 |  |
| 260 | `attack-parry-pierce-percent` | 3 | stat16 (7) | 2 | -1000 | 10000 | 1072 | 0000 |  |
| 261 | `attack-pierce-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1080 | 0000 |  |
| 262 | `attack-pierce-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1088 | 0000 |  |
| 263 | `attack-critical-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1096 | 0000 |  |
| 264 | `attack-critical-damage-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1104 | 0000 |  |
| 265 | `attack-critical-value` | 3 | stat32 (8) | 4 | 0 | 300000 | 1112 | 0000 |  |
| 266 | `attack-critical-value-equip` | 3 | stat32 (8) | 4 | 0 | 300000 | 1128 | 0000 |  |
| 267 | `attack-critical-damage-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1144 | 0000 |  |
| 268 | `attack-critical-damage-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1152 | 0000 |  |
| 269 | `defend-critical-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1160 | 0000 |  |
| 270 | `defend-critical-damage-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1168 | 0000 |  |
| 271 | `defend-critical-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1176 | 0000 |  |
| 272 | `defend-critical-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1184 | 0000 |  |
| 273 | `defend-bounce-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1192 | 0000 |  |
| 274 | `defend-dodge-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1200 | 0001 |  |
| 275 | `defend-dodge-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1208 | 0000 |  |
| 276 | `defend-dodge-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1216 | 0000 |  |
| 277 | `defend-parry-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1224 | 0000 |  |
| 278 | `defend-parry-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1232 | 0000 |  |
| 279 | `defend-parry-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1240 | 0000 |  |
| 280 | `defend-parry-reduce-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1248 | 0000 |  |
| 281 | `defend-parry-reduce-diff` | 3 | stat16 (7) | 2 | 0 | 30000 | 1256 | 0000 |  |
| 282 | `defend-perfect-parry-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1264 | 0000 |  |
| 283 | `defend-immune-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1272 | 0001 |  |
| 284 | `attack-power-creature-min` | 3 | stat32 (8) | 4 | 0 | 300000 | 1280 | 0001 |  |
| 285 | `attack-power-creature-max` | 3 | stat32 (8) | 4 | 0 | 300000 | 1296 | 0001 |  |
| 286 | `attack-power-equip-min` | 3 | stat16 (7) | 2 | 0 | 30000 | 1312 | 0001 |  |
| 287 | `attack-power-equip-max` | 3 | stat16 (7) | 2 | 0 | 30000 | 1320 | 0001 |  |
| 288 | `defend-power-creature-value` | 3 | stat32 (8) | 4 | 0 | 300000 | 1328 | 0000 |  |
| 289 | `defend-power-equip-value` | 3 | stat32 (8) | 4 | 0 | 300000 | 1344 | 0000 |  |
| 290 | `defend-resist-power-creature-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1360 | 0000 |  |
| 291 | `defend-resist-power-equip-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1368 | 0000 |  |
| 292 | `defend-physical-damage-reduce-percent` | 3 | stat16 (7) | 2 | -10000 | 1000 | 1376 | 0000 |  |
| 293 | `defend-force-damage-reduce-percent` | 3 | stat16 (7) | 2 | -10000 | 1000 | 1384 | 0000 |  |
| 294 | `attack-damage-modify-percent` | 3 | stat16 (7) | 2 | -3000 | 10000 | 1392 | 0001 |  |
| 295 | `attack-damage-modify-diff` | 3 | stat32 (8) | 4 | -1000000 | 1000000 | 1400 | 0001 |  |
| 296 | `defend-damage-modify-percent` | 3 | stat16 (7) | 2 | -10000 | 3000 | 1416 | 0000 |  |
| 297 | `defend-damage-modify-diff` | 3 | stat32 (8) | 4 | -1000000 | 1000000 | 1424 | 0000 |  |
| 298 | `defend-miss-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1440 | 0000 |  |
| 299 | `attack-stiff-duration-base-percent` | 3 | stat16 (7) | 2 | -1000 | 10000 | 1448 | 0000 |  |
| 300 | `attack-stiff-duration-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1456 | 0000 |  |
| 301 | `defend-stiff-duration-base-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1464 | 0000 |  |
| 302 | `defend-stiff-duration-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1472 | 0000 |  |
| 303 | `cast-duration-base-percent` | 3 | stat16 (7) | 2 | -10000 | 1000 | 1480 | 0000 |  |
| 304 | `cast-duration-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1488 | 0000 |  |
| 305 | `attack-concentrate-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1496 | 0000 |  |
| 306 | `attack-concentrate-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1504 | 0000 |  |
| 307 | `defend-perfect-parry-reduce-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1512 | 0000 |  |
| 308 | `defend-counter-reduce-percent` | 3 | stat16 (7) | 2 | 0 | 1000 | 1520 | 0000 |  |
| 309–314 | `pve-boss-level-npc-` + `attack-power-creature-min`, `attack-power-creature-max`, `attack-power-equip-min`, `attack-power-equip-max`, `defend-power-creature-value`, `defend-power-equip-value` | 3 | stat16 (7) | 2 each | 0 | 30000 | 1528..1568 | 0000 |  |
| 315–320 | `pvp-` + the same six names | 3 | stat16 (7) | 2 each | 0 | 30000 | 1576..1616 | 0000 |  |
| 321 | `job-ability-1` | 3 | stat32 (8) | 4 | 0 | 300000 | 1624 | 0000 |  |
| 322 | `job-ability-2` | 3 | stat32 (8) | 4 | 0 | 300000 | 1640 | 0000 |  |
| 323 | `heal-power-base-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1656 | 0000 |  |
| 324 | `heal-power-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1664 | 0000 |  |
| 325 | `heal-power-diff` | 3 | stat32 (8) | 4 | 0 | 2000000000 | 1672 | 0000 |  |
| 326 | `aoe-defend-base-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1688 | 0000 |  |
| 327 | `aoe-defend-power-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1696 | 0000 |  |
| 328 | `abnormal-attack-base-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1704 | 0000 |  |
| 329 | `abnormal-attack-power-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1712 | 0000 |  |
| 330 | `abnormal-attack-power-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1720 | 0000 |  |
| 331 | `abnormal-defend-base-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1728 | 0000 |  |
| 332 | `abnormal-defend-power-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1736 | 0000 |  |
| 333 | `hate-base-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1744 | 0000 |  |
| 334 | `hate-power-creature-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1752 | 0000 |  |
| 335 | `hate-power-equip-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1760 | 0000 |  |
| 336 | `additional-exp-diff-by-kill` | 3 | stat32 (8) | 4 | 0 | 300000 | 1768 | 0000 |  |
| 337 | `additional-exp-percent-by-kill` | 3 | stat16 (7) | 2 | 0 | 1000 | 1784 | 0000 |  |
| 338 | `additional-mastery-exp-diff-by-kill` | 3 | stat32 (8) | 4 | 0 | 300000 | 1792 | 0000 |  |
| 339 | `additional-mastery-exp-percent-by-kill` | 3 | stat16 (7) | 2 | 0 | 1000 | 1808 | 0000 |  |
| 340 | `additional-faction-score-max-percent` | 3 | stat16 (7) | 2 | 0 | 30000 | 1816 | 0000 |  |
| 341 | `attack-attribute-value` | 3 | stat16 (7) | 2 | 0 | 30000 | 1824 | 0000 |  |
| 342 | `attack-attribute-value-equip` | 3 | stat16 (7) | 2 | 0 | 30000 | 1832 | 0000 |  |
| 343 | `attack-attribute-base-percent` | 3 | stat16 (7) | 2 | 0 | 10000 | 1840 | 0000 |  |
| 344–351 | `invisible` (sync 0011), `block-move`, `block-skill`, `block-physical-skill`, `block-force-skill`, `immune-damage`, `immune-death`, `immune-debuff` | 4 | bool (11) | 1 at 344, else 0 | i8 | i8 | 1848..1855 | 0001 | bits 0–7 |
| 352–359 | `exception-target`, `exception-detect`, `exception-hostile-target`, `immune-casting-delay`, `block-dodge`, `block-bounce`, `block-parry`, `block-perfect-parry` | 4 | bool (11) | 1 at 352, else 0 | i8 | i8 | 1856..1863 | 0001 | bits 0–7 |
| 360–367 | `front-perfect-parry`, `back-perfect-parry`, `front-bounce`, `back-bounce`, `debug-invincible`, `debug-invisible`, `abnormality-reserve-7` (sync 0000), `abnormality-reserve-8` (sync 0000) | 4 | bool (11) | 1 at 360, else 0 | i8 | i8 | 1864..1871 | 0001 | bits 0–7 |
| 368 | `equip-hand` | 5 | i32 (3) | 4 | i32 | i32 | 1872 | 0011 |  |
| 369 | `equip-hand-level` | 5 | i8 (1) | 1 | i8 | i8 | 1876 | 0011 |  |
| 370 | `equip-hand-appearance-item` | 5 | i32 (3) | 4 | i32 | i32 | 1880 | 0010 |  |
| 371 | `equip-hand-appearance-item-level` | 5 | i8 (1) | 1 | i8 | i8 | 1884 | 0010 |  |
| 372 | `equip-hand-alt` | 5 | i32 (3) | 4 | i32 | i32 | 1888 | 0010 |  |
| 373 | `equip-body` | 5 | i32 (3) | 4 | i32 | i32 | 1892 | 0010 |  |
| 374 | `equip-body-guild-id` | 5 | i64 (4) | 8 | i64 | i64 | 1896 | 0010 |  |
| 375–382 | `equip-body-custom1`..`8` | 5 | i8 (1) | 1 each | i8 | i8 | 1904..1911 | 0010 |  |
| 383 | `equip-ear` | 5 | i32 (3) | 4 | i32 | i32 | 1912 | 0010 |  |
| 384 | `equip-eye` | 5 | i32 (3) | 4 | i32 | i32 | 1916 | 0010 |  |
| 385–387 | `equip-eye-custom1`..`3` | 5 | i8 (1) | 1 each | i8 | i8 | 1920..1922 | 0010 |  |
| 388 | `equip-head` | 5 | i32 (3) | 4 | i32 | i32 | 1924 | 0010 |  |
| 389–391 | `equip-head-custom1`..`3` | 5 | i8 (1) | 1 each | i8 | i8 | 1928..1930 | 0010 |  |
| 392 | `equip-body-attach` | 5 | i32 (3) | 4 | i32 | i32 | 1932 | 0010 |  |
| 393–395 | `equip-body-attach-custom1`..`3` | 5 | i8 (1) | 1 each | i8 | i8 | 1936..1938 | 0010 |  |
| 396 | `equip-weapon-gem` | 5 | i32 (3) | 4 | i32 | i32 | 1940 | 0010 |  |
| 397 | `equip-pet-1` | 5 | i32 (3) | 4 | i32 | i32 | 1944 | 0010 |  |
| 398 | `equip-pet-1-appearance-item` | 5 | i32 (3) | 4 | i32 | i32 | 1948 | 0010 |  |
| 399 | `equip-pet-2` | 5 | i32 (3) | 4 | i32 | i32 | 1952 | 0000 |  |
| 400 | `equip-pet-2-appearance-item` | 5 | i32 (3) | 4 | i32 | i32 | 1956 | 0000 |  |
| 401 | `equip-badge-appearance-item` | 5 | i32 (3) | 4 | i32 | i32 | 1960 | 0010 |  |
| 402 | `attack-stiff-duration-equip-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 1968 | 0000 |  |
| 403 | `defend-stiff-duration-equip-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 1976 | 0000 |  |
| 404 | `aoe-defend-power-value-equip` | 6 | stat16 (7) | 2 | 0 | 30000 | 1984 | 0000 |  |
| 405 | `heal-power-equip-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 1992 | 0000 |  |
| 406 | `defend-strength-creature-value` | 6 | stat32 (8) | 4 | 0 | 300000 | 2000 | 0000 |  |
| 407 | `defend-strength-equip-value` | 6 | stat32 (8) | 4 | 0 | 300000 | 2016 | 0000 |  |
| 408 | `attack-precise-creature-value` | 6 | stat32 (8) | 4 | 0 | 300000 | 2032 | 0000 |  |
| 409 | `attack-precise-equip-value` | 6 | stat32 (8) | 4 | 0 | 300000 | 2048 | 0000 |  |
| 410 | `attack-aoe-pierce-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 2064 | 0000 |  |
| 411 | `attack-aoe-pierce-value-equip` | 6 | stat16 (7) | 2 | 0 | 30000 | 2072 | 0000 |  |
| 412 | `attack-abnormal-hit-base-percent` | 6 | stat16 (7) | 2 | -1000 | 10000 | 2080 | 0000 |  |
| 413 | `attack-abnormal-hit-value` | 6 | stat16 (7) | 2 | -30000 | 30000 | 2088 | 0000 |  |
| 414 | `attack-abnormal-hit-equip-value` | 6 | stat16 (7) | 2 | -30000 | 30000 | 2096 | 0000 |  |
| 415 | `defend-abnormal-dodge-base-percent` | 6 | stat16 (7) | 2 | -1000 | 10000 | 2104 | 0000 |  |
| 416 | `defend-abnormal-dodge-value` | 6 | stat16 (7) | 2 | -30000 | 30000 | 2112 | 0000 |  |
| 417 | `defend-abnormal-dodge-equip-value` | 6 | stat16 (7) | 2 | -30000 | 30000 | 2120 | 0000 |  |
| 418 | `support-power-base-percent` | 6 | stat16 (7) | 2 | -1000 | 10000 | 2128 | 0000 |  |
| 419 | `support-power-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 2136 | 0000 |  |
| 420 | `support-power-equip-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 2144 | 0000 |  |
| 421 | `hypermove-power-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 2152 | 0000 |  |
| 422 | `hypermove-power-equip-value` | 6 | stat16 (7) | 2 | 0 | 30000 | 2160 | 0000 |  |

In Min/Max, `i8`, `i16`, `i32`, `i64` mean the full range of that type. `i64 max` is 9223372036854775807.
Bit-run rows with two sync values: the row text gives the exceptions. The full raw dump is reproducible from `0x1407CA330`.

Sections after the property blob, in order:

1. ExtraProperty blob (`Creature::FetchAllExtraProperty`, max 0xFA0) with a 2-byte length.
2. Two more length-prefixed blobs.
3. A loop over 2 slots at Pc +26456 + 3352 × k, property id 2.
4. Compose of 4 values from a record (+704, +708, …).
5. Compose of 2 values: PcCafe publisher code, Pc qword +2593.
6. A loop of 9 entries.
7. Compose of 1 flag.
8. A repeat of 2 entries at Pc +39784.
9. `TimeBasedPlayInfoList::ComposeToClient`.
10. An empty repeat.
11. `SlateContainer::ComposePureSync`.

Group terminators at the format end: `…bKCci;CQb;cccccq`.
Other format strings in the sender: `hChcccchchch…ch;`, `hQCiqh;`, `s50qs64ts20iqs150hiiiqitihhhh`.
After command 32 the sender sends more packets. See "Packets after command 32" below.

### Packets after command 32 (381, verified)

Source: 381 `Pc::SendEnterWorldOk` `0x1405F5980`, called by `HandleGame_EnterWorldReq` `0x1403718A0` line 136.
All sends use the Pc virtual send slot +584 (lines 459, 534, 608) or the sub-sender's own send.
The order below is the call order in the function. Each row is one packet.

| Order | ID and name | Format | Condition | Value source for a new level-1 PC |
| ---: | --- | --- | --- | --- |
| 1 | 32 (initial PC composite) | Fragmented | Always | See "Command 32 property table (381)". Sent at line 459. |
| 2 | 627 `G_SummonedEnterWorldOkCmd` | Fragmented | Only if Pc +36880 (summoned object) is not 0 (line 460). `Pc::SendSummonedEnterWorldOk` `0x1405F6E80` | Not sent for a new PC (no summoned object). |
| 3 | 87 `G_QuestListOkCmd` | `hChcccch…` repeated `hc` pairs, end `;` (string `aHchcccchchchch`) | Always. Compose `0x1405F647B`, id 0x57. Body from `QuestJournal::FetchList` (line 517). Sent at line 534 | Count C = number of active quests. ❓ A new PC can have a start quest from PcInitialSetting. With no quest: count 0. |
| 4 | 90 `G_QuestCompleteListOkCmd` | `hbbb` | Always. `Pc::SendQuestCompleteListOk` `0x1405F9A00` (line 537) | Completed-quest bit list. New PC: empty list ❓ (exact empty length not traced). |
| 5 | 423 `G_CurrentUTCTimeOkCmd` | `hq` | Always. `Pc::SendCurrentUTCTimeOk` `0x1405F88C0` (line 538), compose `Packet::Compose(…, 2, 423, v2)` | q = current UTC time. ❓ Unit (seconds or FILETIME) not checked. |
| — | none | `hqih` | `Pc::SendGameBuilderAuth` `0x1405F52F0` (line 540) sends only if Pc +532 (builder level) > 0 or 0xFF (line 23) | Not sent for a normal account. |
| 6 | 1097 `G_MentoringPropertySyncCmd` | `hhcc` | `Pc::SendMentoringPropertySync` `0x1405F9BA0` (line 541). Only if a server config value > 0 (line 22) | ❓ Config value for the default server not traced. |
| 7 | 1122 `G_CollectingListOkCmd` | `hiCiq;` | Always. `CollectingContainer::SendCollectingList` `0x14007FD60` (line 542; id at line 38) | New PC: count 0. |
| 8 | 1126 `G_FishingListOkCmd` | `hiChiiq;` | Always. `FishingContainer::SendFishingList` `0x1400B98C0` (line 543; id at line 38) | New PC: count 0. |
| 9 | 836 `G_EnergyPropertySyncCmd` | `hiiccqiiiiiiii` | Always. `0x1405FE760` (line 544), compose id 836 | Energy properties from the Pc property block (`Property::GetPureData`, 13 values in a loop to 0xE). |
| 10 | 912 `G_RaidDungeonUnbindListNotifyCmd` | `hqciqh` | Only if config byte (ctrlCode +343 → +1028) is not 0 (line 545). Compose line 565 `0x1405F66A9`, id 0x390. Sent at line 608 | Raid unbind list (`RaidDungeonPropertyContainer::FetchUnbindList`, line 590). New PC: empty ❓. |

After these, line 613 calls `Pc::writeGameLogEnterWorld` (log only, no packet).
Then `HandleGame_EnterWorldReq` line 199 calls `Pc::TranslateZoneOnEnterWorld`, and line 215 calls `World::Enter` `0x1403F4A60`.
`World::Enter` → `Zone::Enter` `0x14022EC20` → spawn action → command 41 (sender `0x1404BEED0`).

Each "Always" row is a packet that the 381 server sends. The port sends rows 1, 3, 4, 5, 7, 8, 9 for a new PC.
Rows 6 and 10 depend on server config. The port sends them only if its config enables the feature. Default for the port: do not send (❓ until the 381 config values are known).

## Command 41: prepare destination

The actual format bytes at `0x140CF1EF8` are `hqhiiccKuuuhhh`.
The IDA symbol spelling uses lowercase letters. The format byte is uppercase `K`.
Sender `0x1404BEED0` runs from PcSpawnAction start `0x1404BEAE0`.

| Body order | Type | Checked source | Field meaning / origin |
| ---: | --- | --- | --- |
| 1 | q | zone +40 | Zone runtime object ID; server runtime, not table key |
| 2 | h | zone +16600 | Zone channel. The first channel of a geo-zone is 1 (2019 `World::createZone2` 0x1403fab80, `CreateZone` 0x1403f1b30). New-PC value: 1. |
| 3 | i | First zone data record +8 | Zone table ID; table dependency below |
| 4 | i | Spawn action +208 | spawnActionKey. 2019 `PcSpawnAction::Initialize` 0x1404b47e0: global interlocked counter (dword_14101E34C), +1 for each action, skips 0. Server runtime. |
| 5 | c | World configuration object +56 | World clock hour, 0..23 (2019 `PcSpawnAction::prepareSpawn` 0x1404b4e50). See "World clock" below. |
| 6 | c | Spawn action +193 != 0 | arenaWaitCancel. `Initialize` sets it false, so the value is 0 on normal entry. |
| 7 | K | Pointer at zone +16200; length at +16208 | Zone broadcast-crypt key bytes and size. These are two native arguments for one wire field. The key is per zone and made at run time (2019 `Zone::Open` 0x140228cb0 → `BroadcastCrypt::Initialize` 0x1406a03d0, BROADCAST_LOCAL, `GenerateKey`). Write `keyhere` in fixtures. ❓ Key size and algorithm for the port. |
| 8–10 | uuu | Action +152/+154/+156 | Destination x/y/z, read as signed shorts by sender; ❓ units and table conversion open |
| 11 | h | Action +160 | Destination yaw; ❓ units open |
| 12 | h | Incremented PC +3480 | PC transition counter |
| 13 | h | Incremented summoned-object +3480, or zero | Summoned transition counter |

2019 `prepareSpawn` 0x1404b4e50 (2019 wire id 39; the 381 id is 41) also does these steps before the send:
it sets properties 0x63/0x64/0x65 (x, y, z) on the PC and on the summoned object, and it calls `Creature::SetZone`.

#### World clock (2019)

- `World::Start(hour, min)` is called at 0x1403ef847 from `GameDaemon::startGameDaemon` 0x140341890. ❓ Source of the arguments.
- `WorldClock::start` 0x1403ec0a0 sets clock = hour. The first tick comes after 7,500 × (60 − min) ms.
- `WorldClock::HandleEvent` 0x1403ec040 sets clock = (clock + 1) % 24 every 450,000 ms (7.5 minutes for one game hour).

Entry helper `0x1403F4A60` resolves the destination zone and schedules PcSpawnAction through `0x14022EC20`.
The action stores position/yaw and generates the correlation key.
Its start checks or adjusts the position with `0x1404C1A70` before sending command 41.
❓ The saved-character location versus new-character table-selection branch is not fully mapped.

Client prepare target `0x73E6C0` requires a player object.
It caches destination/correlation data, calls `BPlayer::InitPos`, queries local map data, and notifies observers and `BParty::PrepareEnterZone`.
This establishes a player-initialization dependency before zone preparation.

## Loading and command 42

`UiTransitControl::enterWorld`, `0x7D8E70`, checks `World::IsDestZoneResource` and calls `UiTransitControl::loadZoneMap`, `0x78FED0`.
It creates the world UI and calls `0x78E8F0`.
That function reaches `BUiWorld::RequestEnterZone` at `0x78EC10` or `0x78EC39`.
A cinematic branch can defer this request.
Other checked callers are `0x7DA146` and `0x7DB4C3`.

The server decodes `hiuuuh` in `0x140371EE0` and queues task `0x1402912F0`.

| Body order | Type | Checked correspondence | Source class |
| ---: | --- | --- | --- |
| 1 | i | Task +144 must equal current spawn action +208, in action state 2. A mismatch sends command 44, error 1150. | Echo of command-41 runtime correlation. Not a character DB ID. |
| 2–4 | uuu | Task +148/+150/+152 pass to `0x1404BF580` for summoned-object x/y/z. | Client summoned-object state; not the local-player destination echo. |
| 5 | h | Task +154 passes to the same summoned-object branch as yaw. | Client summoned-object state. ❓ Units and valid ranges open. |

Client `0x7815C0` initializes the four shorts to zero and calls
`PresentationWorld::GetSummonedEnterZoneInfo`, `0xB523E0`.
The checked ordinary helper branch copies summoned-object shorts +0x94/+0x96/+0x98/+0x9A.
❓ No-summon and special-mode branches are not fully mapped.
The server can correct summoned position against terrain and permitted distance.
Keep raw scalar bits in the codec. Do not enforce an invented coordinate range.

Command 42 is the checked request in the load/enter path.
❓ It is not proof of a separate universal loading-finished acknowledgement.

## Command 43 and appearance order

Full format: `hq`. Server sender: `0x1402304A0`.
The only application field is a q value from spawn-action +184.
The source chain is `0x1402912F0` → `0x1404BF580` → `0x1404BF1E0` → `0x1402304A0`.
The field is `enterSocialId` (verified).
381 `Zone::Enter` `0x14022EC20` line 253 writes action +184 from argument 9.
2019 `Zone::Enter` `0x14022e170` has the PDB type; argument 9 is `unsigned __int64 enterSocialId`.
381 `World::Enter` `0x1403F4A60` line 56 calls `Zone::Enter` with argument 9 = 0 (call `0x1403F4B3F`).
Thus for a first world entry, command 43 q = 0. Golden body: `2B 00` + 8 zero bytes (after the size and command header).
Other callers give other values: `World::ReturnToGame` `0x1403F5860` (reason 8) and `Zone::transferCreature` `0x140230D20`. They are out of scope.
2019 `PcSpawnAction::spawn` 0x1404b5160 lines 81–92: if enterSocialId is not 0, it sets creature->enterSocialId and passes it to `Zone::enter`.
2019 `SpawnCreature` 0x1404b5500 calls `Pc::SendFriendNotiLocation(FRIENDNOTILOCATION_ENTER_WORLD)`.
This is runtime action state, not a verified table key or character DB column.

The sender (381 `Zone::enter` `0x1402304A0`) calls `Zone::moveTo` `0x14024B0C0` before it sends command 43.
`Zone::moveTo` only puts the creature in the cube lists. The cube packets go out later, on the zone broadcast flush.
It calls `Zone::OnEnterZone` `0x140254770` after command 43. That function does quest progress only and sends no packet.
Thus the wire order for a first entry is: 43, then the cube packets (see "Sight packets").
❓ The body of packet 4 for the first appearance is not traced.

### Sight packets

381 `ZoneBroadcastCube` constructor `0x1402581A0` makes 2 sets of 4 fragment packets per cube (lines 56–85).

| Cube list | Command | Format string (start) | Builder |
| --- | --- | --- | --- |
| +96 | 2 `G_EncounterOutOfSightBroadcastCmd` | `hiqCqQ;…` | `Zone::makeEnterLeavePacket` `0x14024C2C0` |
| +104 | 4 `G_InSightCubeCmd` | `hiqCqihhhh…` (long creature appearance format) | `Zone::makeEnterPacket` `0x14024C570`, `Zone::makeEncounterPacket` `0x14024E7A0` |
| +112 | 5 `G_OutSightCubeCmd` | `hiqCqC…` | `Zone::makeOutOfSightPacket` `0x14024E4B0` |
| +120 | 6 `G_BroadcastActionCmd` | `hics` | broadcast actions |

2019 `Zone::enter` `0x14022FDC0` adds the creature to `EnterList` (line 122), calls `Zone::moveTo` (line 130) and `Zone::OnEnterZone` (line 142).
2019 `Zone::moveTo` `0x14024B480` line 170 adds the creature to the cube `EncounterList` with `enterSocialId`.
`Zone::OnEnterZone` `0x140256030` (2019) does quest progress only; it sends no packet.
❓ Not verified: whether the cube packet 4 includes the receiving PC itself, and the full field list of the 4 format.
For the first Rust slice: a PC that is alone in the zone needs no packet 4 for other creatures.
The operator step checks if the client stands without packet 4. If it does not, packet 4 for self is the next work item.

Client slot 43 selects `0xEF6790`, then `World::EnterZone` at `0x74C850`.
That target requires an existing player and starts installing it in the world object map.
❓ Final load-screen exit and a standing-client state remain unverified.

## Table and DB sources

| Entry data | Source class | Link to minimum-world specification | Limit |
| --- | --- | --- | --- |
| Admission token and client key | Lobby/session | Not game tables | Command 23 verified; command-24 body open |
| PC identity, appearance and properties | DB property blob + table-derived stats. 2019 `Pc::SetAllDbData` 0x1405de2f0 copies the blob with `Property::set` 0x140689c70, then `Pc::LoadData` 0x1405dfc40 calls `Creature::InitLevel` 0x14026a320, `CreateCreatureJobRule` 0x14009a830 and `Pc::ApplyAbility` 0x1405dfd20 | Callers: `LobbyTokenAuthTran::loadCharacterOk` 0x140155610, `World::ReplacePc` 0x1403f30a0 | ❓ Numeric defaults (max-hp, speed) are data-driven; no values copied |
| New-character destination candidate | PcInitialSetting → zone | [Standing chain](03-minimum-world.md#byte-evidence-and-standing-chain) | ❓ Binary selection branch not complete |
| Client terrain resource | zone → Terrain | Same standing chain | Terrain ID is not a separately verified field of command 41 |
| Spawn position and yaw candidate | ZonePcSpawn; or saved-character state | Same standing chain | ❓ Initial-spawn choice and conversion to wire shorts open; never choose the first record by default |
| Respawn/reentry | ZoneRespawn → ZoneArea; zone reentry relation | Same standing chain | ❓ Not proven required for first standing entry |
| Environment and nearby objects | ZoneEnv/ZoneEnv2/ZoneEnv2Place/ZoneEnv2Spawn | [Table scope](03-minimum-world.md#table-scope) | ❓ Appearance commands and mandatory subset open |
| Initial quests and scripts | PcInitialSetting quest link; script/quest catalogs | Same table scope | ❓ Not proven required or optional for standing |

## Rust codec contract (step 4)

| Item | Contract |
| --- | --- |
| Table | `Program/Server/Packet/tables/game381.json`; protocol 1278; separate client/server maps. This is a verified subset, not the complete native table. |
| Codec | `Program/Server/Packet/src/game.rs`; commands 11/12, 23, 31, 41/42/43, 16/17/18. Formats match the tables above. |
| Unknown payloads | Commands 24, 32, 929 and unknown IDs return `Error::Unsupported(RawPacket)`. The error retains command and all application bytes. Raw encoding also returns unsupported. |
| Packet boundary | One logical size/command/body packet; maximum 4000 bytes. No outer-frame crypto, native mode-2 assembly, dispatch authentication or world state machine is added. |
| Scalars | Unverified signed fields use unsigned raw-bit storage. Text retains UTF-16 units. No coordinate or game-value defaults are added. |
| Validation | Reject wrong directions, truncation, invalid counts and trailing fields. Generated tests check independent little-endian fixtures and round trips. |

The table has no LobbyGate relay mapping.
The codec does not verify admission tokens or process handshake secrets.
Do not infer safe state transitions from a successful codec operation.

No game record, account name, address or secret value is included.
See [06-minimum-session.md](06-minimum-session.md) for the conservative standing-session limit.
