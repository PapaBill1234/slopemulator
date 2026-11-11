# Backlog

Items for later. They are not in the slice queue.

| Id | Item | Inputs | Notes |
| --- | --- | --- | --- |
| gmweb | Own GM web tool in Rust (axum), in place of NCSoft AdminWeb | `Docs/Server/ManagementDaemon/01-overview.md`; local decompile `private-project/decompiled/managementd/` (`AdminWebSession.cs`, 365 commands) | Speak the ManagementDaemon AdminWeb protocol (port 21101), or call our server directly. Start after the ManagementDaemon port. Scope: all 365 AdminWeb commands, plus extra features (search, audit log, Builder level page, live status). |

### gmweb: bans

| Feature | In AdminWeb | Source |
| --- | --- | --- |
| Block / unblock character | yes | `BlockCharacter`, `UnblockCharacter` |
| Kick (game, lobby) | yes | `Kick`, `KickToLobby` |
| Chat spam filter | yes | `ChatSpamFilter*` |
| Ranking black list | yes | `RankingBlackList` |
| White list (login) | yes | `WhiteList`, `InsertWhiteList` |
| Lock / unlock item | yes | `LockItem`, `UnlockItem` |
| Account ban | no ❓ (probably in NC account system / STS) | — |
| IP ban | no | — |
| Hardware ban | no (`pcid` = character id, not hardware) | — |
| Timed bans, ban reasons, ban history | no ❓ | — |

Add the missing rows in our tool. Account and IP bans go in our login server (StServers port).


### gmweb: decision 2026-11-11 — in-game GM panel first

| Part | Choice |
| --- | --- |
| Front end 1 | In-game GM panel as a client plugin (bnspatch loader, `GPluginInfo`). No `Client.exe` patch. |
| Drawing | Native client UI panel (same look as game windows). No overlay, no ImGui (user choice 2026-11-11). Needs a UI RE card first (see below). |
| Transport | Use the existing Builder protocol (`builder-port` 40100, `GameBuilderSession`, card gmt-1) where it covers the command. Add new GM packets only for the rest. |
| Auth | Server checks every GM packet against `Builderright` and account. The client panel is untrusted. Anyone can load a plugin. |
| Server side | One GM API in our Rust server (all AdminWeb commands + bans). The panel and a later web tool both use it. |
| Front end 2 | Web tool (axum): optional, later. Use it for offline characters, audit log and work without a game client. |
| Data (`datafile.bin`) | Live table edit in game, as the GM client `DataBrowser` does (memory only). Server must reload the same table, and other clients need the new file. ❓ Distribution path. |

### gmweb: UI RE card (needed before the panel)

Questions for the card (Client_gm PDB names, then Client 2020):

1. Where panel layouts come from. ❓ UI layout XML inside `xml.dat` / `xml64.dat` (mods edit these).
2. How a panel class is registered and created in code (factory, name to class map).
3. How button, list and text-box events reach code.
4. How a panel is opened (key, command, menu).
5. Can a plugin add a new panel (new layout + new handler), or must it take over an unused existing panel (for example a GM or debug panel left in the retail client)?
6. Which existing panels to copy: list, tabs, input box, buttons.
