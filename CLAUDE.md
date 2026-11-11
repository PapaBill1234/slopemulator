# bns-server

Rust port of the BnS 2020 server set (`Program/`) with specs (`Docs/`).

- Write all text in ASD-STE100.
- Never write keys, passwords or certificates. Write `keyhere`.
- Do not read a file over 40,000 bytes in full. Run `tools/spec_index.py <doc>` for its headings and line ranges, then read only those lines.
- Commit author: `Claude <noreply@anthropic.com>`.
- Slice work: `tools/slice_runner.py`. Queue: `tools/slices/queue.tsv`. Cards: `Docs/Plan/cards/`.
- Before you write Rust, read `Docs/Architecture/scale-rules.md` (structure, ownership and hot path rules).
