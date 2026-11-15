# Lift rules (Opus IDA + C++ slice session, repo bns-lift)

You lift one part of a 2019 + PDB server exe to C++ source that compiles. Your card is the user message. Your cwd is a worktree of the PRIVATE repo `bns-lift`.

## Hard rules
- Read `Docs/Plan/plan.md` (sections "Toolchain", "Repo layout", "Lift method") before you start.
- Open only the `.i64` copy that the card names (`private-project/ida/lift/<Server>/<Server>.exe`). Never open another copy.
- Never read or probe the IDA licence. If IDA fails to open, write Blocked in the handoff and stop.
- Never write keys, passwords or certificates. Write `keyhere`.
- Never create binaries in the worktree, except in `build/` (git ignores it).
- Change only the paths that the card lists under "Files", plus `Docs/Plan/handoffs/<card id>.md`.
- Do not commit. The gate commits.
- Write all text in ASD-STE100.

## Lift rules
- Use the PDB names for functions, classes, members, enums and globals. If a name is not in the PDB, use the IDA name and add `// ❓`.
- Use the PDB types. Do not use `__int64`, `_DWORD` or `v12` style names in the final code.
- Keep the original logic in phase 1. Do not fix bugs. Mark each bug with `// BUG: <one line>` and the address.
- Put each function in the `.cpp` file of its PDB module (`Port.obj` → `src/common/Port.cpp`). Put a `// 0x<rva>` comment above each function.
- Put inlined STL code back as STL calls. Use `include/vs2010/` (VS2010 object layout). If a container is missing, add it there with the layout from the PDB.
- Put calls to code from a later card behind a declaration in the header. Do not write a stub body unless the link needs it. A stub body has `// TODO(lift-<card>)`.
- Each function that you finish must compile. Run `tools/gate.sh` after each module.

## Context budget
- Each turn reads the full context again. The number of turns is the main cost.
- In each database, run this once: `WT="<worktree>"; exec(open(WT + "/tools/ida_lib.py").read()); print(boot(WT, stservers=False))`.
- Use the `ida_lib.py` helpers (`survey`, `hr`, `hrs`, `callees`, `vslots`, `dis`). Do not write your own copies.
- Decompile many functions in one call. Write the pseudocode to `.scratch/<module>.c` in the worktree, then read only the line ranges you need. Do not print full pseudocode into the conversation.
- Keep each IDA result under 1,500 tokens (about 3,500 characters).
- Write one module per edit (one `Write` of the full `.cpp`), not one function per edit.
- Put temporary files only in `.scratch/`.

## Stages
- The runner cuts long work into stages of about 45 turns. At the end of a stage it asks you for a progress note. A new session continues from that note.
- Write the source files to disk at once. Code that is only in the conversation is lost.

## Last step
Write `Docs/Plan/handoffs/<card id>.md`, 30 lines or fewer: Done (modules, function count), Not done (functions with addresses), ❓, Blocked, Gate result.
