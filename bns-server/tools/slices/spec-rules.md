# Spec rules (Opus IDA slice session)

You reverse one slice in IDA and write its spec. Your card is the user message.

- Open only the `.i64` copy that the card names. Never open another copy.
- Every behaviour claim cites an address or function. Mark claims that you did not verify with ❓.
- Do not copy keys, passwords, certificates or Hex-Rays code. Describe behaviour.
- Change only the paths that the card lists under "Files".
- Never read or probe the IDA licence. If IDA fails to open, write Blocked and stop.
- Do not commit. The gate commits.
- Write all text in ASD-STE100.

Context budget (each tool result is read again on every later turn):
- Each turn reads the full context again. The number of turns is the main cost. Target: 40 IDA calls or fewer for the whole slice.
- Work in batches. For each topic, first run 1 survey script (use `survey` and `hrs`) that covers the whole topic: all handlers, their callees, xrefs, strings, constants and enums. Then run only the follow-up calls that the survey shows you need.
- Send independent tool calls in the same turn. Example: open all databases of the card in one turn, then set up all of them in one turn (one `execute_python` per database).
- In each database, run this once: `WT="<worktree>"; exec(open(WT + "/tools/ida_lib.py").read()); print(boot(WT))`. For StServers it also applies the framework names and loads `recipe.py`. For other binaries use `boot(WT, stservers=False)`.
- Use the `ida_lib.py` helpers: `summ`/`survey` (callers, calls, strings, constants), `hr`/`hrs` (filtered pseudocode with line numbers), `xstr`, `callers`, `callsites`, `drefs`, `callees`, `arg`, `swmap`, `vslots`, `dis`. Do not write your own copies of them. Write a local helper only when none fits.
- Survey many functions in one call (`survey([...])`, `hrs([...])`), not one call per function.
- Keep each IDA result under 1,500 tokens (about 3,500 characters). Only the first survey of a topic can use 3,000 tokens.
- Never print a full function, string table or xref list. Filter in Python and print counts plus the rows you need. If a list is long, print the first 20 rows and the count.
- Do not print full pseudocode. Filter it in Python: print only the lines with calls, strings, constants or the field that you need.
- Write the chapter in large edits (one topic per edit), not one fact per edit.
- Do not read files outside the worktree and the `.i64` copy.
- Put temporary work files only in `.scratch/` in the worktree. Git ignores it.
- For Rust interfaces, run `python3 tools/rust_api.py <crate dir>`. Do not read Rust source files.
- Write facts into the spec file as soon as you verify them. Do not keep them only in the conversation.
- The runner cuts long work into stages of about 45 turns. At the end of a stage it asks you for a progress note, and a new session continues from it. Facts that are only in the conversation are lost.

## Original bugs (quirks)
- The original code has bugs, for example out-of-bounds reads, reads of uninitialized memory, wrong indexes and integer overflow. Game data and scripts can depend on what these bugs return. A port that fixes them silently breaks that content.
- Record the bug in the spec. Do not hide it. Write a "Quirk" row: address, what the code does, the value it actually gets (for example the next field in the struct, the old buffer contents, or zero), and which data or script can reach it.
- Give the quirk a short name (`quirk_<name>`), so that the Rust code and the tests can cite it.

- If a quirk looks like a player exploit (see `Docs/Plan/known-exploits.md`), add "exploit" and the list name to its row.
- The Rust port does not copy a bug or an exploit (user rule, 2026-11-11). Each quirk row gets a column "Port": `fixed` (write the correct behaviour) or `keep value` (game data or scripts need the value that the bug returns; the port gives this value in a safe way, with no memory error).

Abuse notes (all slices):
- The port copies the original behaviour, also when a player can abuse it. Full parity comes first.
- If you see a way that a player can abuse the code (dupe, skipped check, trusted client value, missing rate limit, race), do not fix it and do not ignore it. Write it in `Docs/Plan/abuse-notes/<slice id>.md`.
- One table row for each case: | name | file:line or address | what a player can do | how to fix it later |
- Use the names in `Docs/Plan/known-exploits.md` if one fits. Else give a new name `exploit_<name>`.
- This file is always an allowed path. Write it in addition to the handoff.

Last steps:
1. Unless the card says "No Rust card", write the Rust card for the next slice with `Docs/Plan/card-template.md`. Path: `Docs/Plan/cards/<slice id>-rust.md`. Give exact spec line ranges, Rust signatures and test cases.
2. Write `Docs/Plan/handoffs/<slice id>.md`, 30 lines or fewer: Done, Not done, ❓, Blocked.

## Order of work
- Write each verified fact into the spec at once. Do not keep facts only in the conversation.
- Write the Rust card early, before the last topic. Update it at the end. A slice without the card cannot chain to Rust.
- You have no general Bash. Do not try `tools/slice_runner.py`. Check the gate rules by hand.
