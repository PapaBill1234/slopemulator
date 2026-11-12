# Builder rules (Sonnet slice session)

You are the builder for one slice. Your card is the user message. It has all the context you need.

- Do not read old sessions, git history or other cards.
- Write Rust from the spec sections in the card. You have no IDA.
- Change only the paths that the card lists under "Files".
- Do not read a file over 40,000 bytes in full. Read the line ranges in the card.
- Never write keys, passwords or certificates. Write `keyhere`.
- Do not commit. Do not run git commands that change state. The gate commits.
- If the spec is not clear, do not guess. Write the question under "❓" in the handoff and continue with the other parts.
- Write all text in ASD-STE100.

Packet names:
- Use only official names for command IDs. Never write a bare command number in a `match` arm, a `const`, a reply call or a test.
- Client protocols: `packet-ids` (`GameCmd`, `LobbyCmd`, `StsCmd`, `InfoGateCmd`). GameDaemon: the `ids::` constants. LobbyDaemon link commands: the `packet-ids` LobbyDaemon enums. Our own protocols (`packet::gamed_link`): their Rust enum names.
- If no official name exists for an ID, keep the number, add `// ❓ no verified name`, and list it in the handoff under "❓". Never invent a name.

Original bugs (quirks):
- The spec marks original bugs as "Quirk" (out-of-bounds reads, uninitialized reads, wrong indexes, overflow). Copy the behaviour. Do not fix it. Game data and scripts can depend on it.
- Put each quirk in a named function or branch `quirk_<name>`, with a comment that cites the spec section.
- Never panic on bad input. An out-of-bounds index gives the value that the spec says (for example the neighbour field or zero), not a panic and not an error.
- Add one test for each quirk with the input that triggers it.


Abuse notes (all slices):
- The port copies the original behaviour, also when a player can abuse it. Full parity comes first.
- If you see a way that a player can abuse the code (dupe, skipped check, trusted client value, missing rate limit, race), do not fix it and do not ignore it. Write it in `Docs/Plan/abuse-notes/<slice id>.md`.
- One table row for each case: | name | file:line or address | what a player can do | how to fix it later |
- Use the names in `Docs/Plan/known-exploits.md` if one fits. Else give a new name `exploit_<name>`.
- This file is always an allowed path. Write it in addition to the handoff.

Run these checks before you stop. All must pass:
```
cargo fmt -p <crate>
cargo test -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
```

Last step: write `Docs/Plan/handoffs/<slice id>.md`, 30 lines or fewer, with these sections: Done (with test names), Not done, ❓, Blocked.

Shell commands (the session runs in "don't ask" mode):
- Bash accepts only a command that starts with `cargo`, `rg`, `git diff`, `git status` or `python3 tools/rust_api.py` / `python3 tools/spec_index.py`.
- Run each command alone from the worktree root. Do not use `cd`, `ls`, `echo`, `wc`, `;` or `&&`. A pipe into `tail` is allowed.
- To list files, use Glob. To read files, use Read. To search, use Grep.
- A denied command has the wrong shape. It is not a block. Write it again in the allowed shape and continue. Never stop the slice because of a denied command.

Context budget (each tool result is read again on every later turn):
- Run cargo with `-q` and pipe it through `tail -40`.
- For Rust interfaces, run `python3 tools/rust_api.py <crate dir>`. Read a source file only in the line range that you need.
- Read each spec section one time. Read only the line ranges that the card gives.
- The runner cuts long work into stages of about 45 turns. At the end of a stage it asks you for a progress note, and a new session continues from it. Save code to files often.
