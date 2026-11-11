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

### gmweb: what client RE already gives (2026-11-11)

Source: `Docs/Client/RE/14-ui.md` (§1), `14-ui-engine-vtables.tsv`, `12-infogate-arena.md` §3.1. Client 381.

| Fact | Source | Effect on the panel |
| --- | --- | --- |
| Widgets and layouts are in UE packages (world UI: package `00007979` / `00070471`, scene `Game_MainScene`). Code finds widgets by name (`UIPanel::GetWidgetId` `0x7fbe40`). | 14-ui §1.1, §1.3 | A new native panel needs a new layout in a UE package. |
| `UIEngineInterface` (501 slots) has no create or clone widget call. | 14-ui-engine-vtables.tsv | A plugin cannot build widgets at run time. |
| Widget events go to static `Panel::OnXxx(EngineEventHandler *, EUiEvent *)` callbacks through `EventTransmitter::Transmit` `0x55fbe0`. | 14-ui §1.5 | A plugin can hook buttons of an existing panel. |
| `InGameWebPanel` (33 functions) is a game window with a browser widget. Engine calls: `SetWebBrowserURL` (slot 206), `SetWebBrowserJavasciprt`, `SetWebBrowserCallJavascriptFunction`, URL filters. Web page → client calls: `BnsCommandDefinition` (72 types, method `CALLBACK`). | 14-ui §1.7, vtable TSV | A game window can show our own GM web page. |
| Web URLs come from a table: `InGameWeb::GetDomainUrl` `0xe31ae0` (index 1–75), `SetDomainUrl` `0xe31a80`. `InGameWeb::IsWebEnabled` gates use. | 12-infogate-arena §3.1 | A plugin can set one URL to our GM page. |

Paths:

| Path | Look | Work | Choice |
| --- | --- | --- | --- |
| B: `InGameWebPanel` + our GM web page (axum). Plugin sets the URL and handles `BnsCommandDefinition` callbacks for target, position and similar. | Game window frame, HTML inside | Low. One HTML GM tool for game and browser. | **First** |
| A1: take over an existing panel and hook its `OnXxx` callbacks | Fully native | Medium. Fixed widget set. | Later, for small tools |
| A2: new panel in a UE package | Fully native | High. UE package edit for `bsengine`. | Only if B is not good enough |

Open ❓: browser engine and version in `bsengine_Shipping.dll`; `BnsCommandDefinition` dispatch function (14-ui §4); `IsWebEnabled` condition; same functions in client 2020.
