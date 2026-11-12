#!/usr/bin/env python3
"""Run one slice in a new session, gate it, and merge it into main.

Usage: slice_runner.py status
       slice_runner.py run [ID] [--dry-run] [--no-push]

Queue: tools/slices/queue.tsv (tab-separated)
  id     slice id, also the card name: Docs/Plan/cards/<id>.md
  kind   rust (Sonnet builder, no IDA) or spec (Opus, IDA)
  model  sonnet or opus
  crate  cargo package to check (rust), or - (spec)
  paths  comma-separated path prefixes the session may change
  deps   comma-separated slice ids that must be done first, or -
  turns  turn limit
  status ready | running | done | blocked
  title  short title (commit subject)

Flow: account check -> worktree outside private-project -> session (stages of 45 turns) -> gate
      -> on fail: 1 repair (rust: fresh session; spec: same session) -> gate
      -> pass: commit, fast-forward main, push; fail: blocked (worktree kept).
Ledger: notes/slice-usage.jsonl (one row per session).
Exit codes: 0 done, 1 blocked, 2 nothing to run, 75 no account with quota.
"""
import argparse, json, os, re, subprocess, sys, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]           # bns-server
PROJECT = REPO.parent                                # private-project
QUEUE = REPO / "tools/slices/queue.tsv"
LEDGER = PROJECT / "notes/slice-usage.jsonl"
WT_ROOT = PROJECT.parent / "bns-wt"                   # outside private-project: no supervisor CLAUDE.md or memory
TARGET = Path.home() / ".cache/bns-wt-target"         # one builder at a time, so one shared target dir
MCP = PROJECT / ".mcp.json"
GATEWAY_KEY = Path.home() / ".local/state/nl-parts-pipeline/operator-access/home-gateway/codex-handover/worker-api-key"
TOOLS = "Read,Edit,Write,Bash,Grep,Glob"
ALLOW = {
    "rust": "Read,Grep,Glob,Bash(cargo:*),Bash(rg:*),Bash(git diff:*),Bash(git status:*),Bash(python3 tools/spec_index.py:*),Bash(python3 tools/rust_api.py:*)",
    "spec": "Read,Grep,Glob,mcp__ida__*,Bash(rg:*),Bash(git diff:*),Bash(git status:*),Bash(python3 tools/spec_index.py:*),Bash(python3 tools/rust_api.py:*)",
}
RULES = {"rust": REPO / "tools/slices/builder-rules.md", "spec": REPO / "tools/slices/spec-rules.md"}
BAD_EXT = re.compile(r"\.(exe|dll|pdb|i64|idb|bin|dat|pem|key|blob|mdf|ldf|rar|pfx|cer|crt)$", re.I)
TAIL = 40
WAIT_STEP_S, WAIT_MAX_S = 900, 12 * 3600   # loop: poll for quota every 15 min, up to 12 h
# Extra gate commands per crate (cross builds).
EXTRA_GATE = {"clientdbg": [["cargo", "build", "-q", "-p", "clientdbg", "--release", "--target", "i686-pc-windows-gnu"],
                           ["bash", "-c", "i686-w64-mingw32-objdump -p \"${CARGO_TARGET_DIR:-target}/i686-pc-windows-gnu/release/clientdbg.dll\" | grep -q GPluginInfo"]],
              # The SQL store compiles only with the feature; tests against LobbyDB stay manual (secret URL).
              "lobbydaemon": [["cargo", "clippy", "-q", "-p", "lobbydaemon", "--all-targets", "--features", "mssql", "--", "-D", "warnings"]]}
LIMIT_5H, LIMIT_7D = 90, 99   # cswap rule (memory: cswap-account-switching)


# ---------- queue ----------
def load():
    lines = QUEUE.read_text().splitlines()
    head = lines[0].split("\t")
    return head, [dict(zip(head, l.split("\t"))) for l in lines[1:] if l.strip()]


def save(head, rows):
    QUEUE.write_text("\n".join(["\t".join(head)] + ["\t".join(r[h] for h in head) for r in rows]) + "\n")


def set_status(sid, status):
    head, rows = load()
    for r in rows:
        if r["id"] == sid:
            r["status"] = status
    save(head, rows)


def pick(rows, sid):
    done = {r["id"] for r in rows if r["status"] == "done"}
    for r in rows:
        if sid and r["id"] != sid:
            continue
        deps = [d for d in r["deps"].split(",") if d and d != "-"]
        if r["status"] == "paused" and r["id"] in paused():
            return r
    for r in rows:
        if sid and r["id"] != sid:
            continue
        deps = [d for d in r["deps"].split(",") if d and d != "-"]
        if r["status"] == "ready" and all(d in done for d in deps):
            return r
    return None


# A session stopped by a usage limit is "paused": worktree and session id are kept, the next run resumes it.
PAUSED = Path.home() / ".cache/bns-slices/paused.json"
RESUME_PROMPT = "A usage limit stopped you. Continue the card from where you stopped. Do not redo finished work."
# Staged sessions: cost grows with turns x context, so a long session is cut into fresh stages.
# At the stage turn limit, the same session writes a progress note (few turns), then a new session
# starts from the card + note. The spec file on disk keeps the verified facts.
STAGE_TURNS = 45
STAGE_MAX = 4
WRAP_TURNS = "4"
WRAP_PROMPT = ("Stop work now. This stage is at its turn limit. A new session continues from your note. "
               "First write all verified facts that are not yet in the spec or code files. Then write {note}, "
               "40 lines or fewer: topics done, files changed, next steps with exact addresses or functions, "
               "open questions, and checks still to run. Do not start new work.")
# Spec stages lose all analysis that is only in the conversation. The note must keep the open threads.
WRAP_PROMPT_SPEC = ("Stop work now. This stage is at its turn limit. A new session continues from your note, "
                    "and it does not see this conversation. "
                    "1. Write all verified facts into the spec now, with addresses. "
                    "2. Write {note}, 60 lines or fewer, with these sections: "
                    "Done (topics complete in the spec); "
                    "In progress (the analysis you are in now: function addresses, what you know, what is left); "
                    "Hypotheses (not verified: claim, address, how to check); "
                    "Ruled out (addresses or ideas already checked and not relevant, so they are not checked again); "
                    "Next (exact addresses or functions, in order); "
                    "IDA state (renames or types that are only in memory). "
                    "Do not start new work.")
NEXT_PROMPT = ("This card continues from an earlier session. Do not redo finished work. "
               "Read the files that the note names only in the line ranges you need.\n\n"
               "## Progress note\n{note}\n\n## Card\n{card}")


REPAIR_TURNS = "30"
REPAIR_FRESH = ("An earlier session did the work of the card below in this worktree, but the gate failed. "
                "Fix only the gate failure. Run `git status` and `git diff --stat` to see the work. "
                "Read only the files and line ranges that the error names. Do not redo or extend the work. "
                "Then run the failed check again and add one line about the fix to {handoff}.\n\n## Gate failure\n")


# Cards that need a person: the runner writes them to one short file, so no chat session must watch the loop.
ATTN = PROJECT / "notes/slices-blocked.md"
ATTN_DB = Path.home() / ".cache/bns-slices/attention.json"
STOP = Path.home() / ".cache/bns-slices/stop"   # tools/run_queue.sh stop: the loop stops after the current card


def attention(sid, state, text="", wt=None):
    """Record (state) or clear (state=None) a card that needs a person. Then write ATTN again."""
    try:
        d = json.loads(ATTN_DB.read_text())
    except (OSError, ValueError):
        d = {}
    if state:
        d[sid] = {"state": state, "time": time.strftime("%Y-%m-%d %H:%M"), "text": text.strip()[-2500:],
                  "wt": str(wt) if wt else ""}
    else:
        d.pop(sid, None)
    ATTN_DB.parent.mkdir(parents=True, exist_ok=True)
    ATTN_DB.write_text(json.dumps(d, indent=1))
    lines = ["# Slice cards that need attention", "",
             "The slice runner writes this file. A card leaves it when it is done.", ""]
    if not d:
        lines.append("Nothing needs attention.")
    for k, v in sorted(d.items()):
        lines += [f"## {k}: {v['state']} ({v['time']})", ""]
        if v["wt"]:
            lines += [f"- Worktree: `{v['wt']}`", f"- Handoff: `{v['wt']}/Docs/Plan/handoffs/{k}.md`"]
        lines += ["", "```", v["text"] or "-", "```", ""]
    ATTN.write_text("\n".join(lines) + "\n")


def paused():
    try:
        return json.loads(PAUSED.read_text())
    except (OSError, ValueError):
        return {}


def set_paused(sid, session):
    d = paused()
    if session:
        d[sid] = session
    else:
        d.pop(sid, None)
    PAUSED.parent.mkdir(parents=True, exist_ok=True)
    PAUSED.write_text(json.dumps(d))


def hit_limit(rec):
    r = (rec.get("result") or "").lower()
    return "hit your" in r and "limit" in r


# ---------- accounts ----------
def accounts():
    out = subprocess.run(["cswap", "list"], capture_output=True, text=True, timeout=60).stdout
    accts, cur = [], None
    for line in out.splitlines():
        m = re.match(r"\s*(\d+): .*?(\(active\))?", line)
        if m and "·" in line:
            cur = {"n": int(m.group(1)), "active": "(active)" in line, "5h": None, "7d": None}
            accts.append(cur)
        m = re.search(r"(5h|7d):\s+(\d+)%", line)
        if m and cur:
            cur[m.group(1)] = int(m.group(2))
    return accts


def ensure_account():
    """Apply the cswap rule. Return the active account row, or None if no account has quota."""
    accts = accounts()
    ok = lambda a: a["5h"] is not None and a["7d"] is not None and a["5h"] < LIMIT_5H and a["7d"] < LIMIT_7D
    active = next((a for a in accts if a["active"]), None)
    if active and ok(active):
        return active
    target = next((a for a in sorted(accts, key=lambda a: a["n"]) if ok(a)), None)
    if not target:
        return None
    subprocess.run(["cswap", "switch", str(target["n"])], check=True, capture_output=True, timeout=60)
    return target


# ---------- session ----------
def session_cmd(row, prompt, resume=None, max_turns=None):
    cmd = ["claude", "-p", "--model", row["model"], "--max-turns", str(max_turns or row["turns"]), "--output-format", "json",
           "--disable-slash-commands", "--tools", TOOLS, "--permission-mode", "dontAsk",
           "--allowedTools", ALLOW[row["kind"]] + f",Edit(/{WT_ROOT}/{row['id']}/**),Write(/{WT_ROOT}/{row['id']}/**)", "--append-system-prompt-file", str(RULES[row["kind"]])]
    cmd += ["--mcp-config", str(MCP), "--strict-mcp-config"] if row["kind"] == "spec" else ["--strict-mcp-config"]
    if resume:
        cmd += ["--resume", resume]
    return cmd + [prompt]


def api_calls(session, t0):
    """Count real API calls in the transcript since t0. num_turns is not the same number."""
    files = list((Path.home() / ".claude/projects").glob(f"*/{session}.jsonl")) if session else []
    if not files:
        return None
    ids = set()
    for line in files[0].read_text(errors="replace").splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("type") != "assistant":
            continue
        ts = m.get("timestamp", "")
        try:
            t = time.mktime(time.strptime(ts[:19], "%Y-%m-%dT%H:%M:%S")) - time.timezone
        except ValueError:
            t = t0
        if t >= t0 - 1:
            ids.add(m.get("message", {}).get("id"))
    return len(ids)


def run_session(row, wt, prompt, resume=None, max_turns=None, role="work"):
    """role: work (new session), next (new stage), resume (after a usage limit), wrap (stage note), repair."""
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET))
    t0 = time.time()
    p = subprocess.run(session_cmd(row, prompt, resume, max_turns), cwd=wt, capture_output=True, text=True, env=env, timeout=4 * 3600)
    try:
        data = json.loads(p.stdout)
        if isinstance(data, list):
            data = [x for x in data if x.get("type") == "result"][-1]
    except (ValueError, IndexError):
        data = {"subtype": "no_json", "result": (p.stderr or p.stdout)[-500:]}
    u = data.get("usage", {})
    rec = {"ts": int(t0), "slice": row["id"], "kind": row["kind"], "model": row["model"],
           "role": role, "repair": role == "repair", "session": data.get("session_id"), "subtype": data.get("subtype"),
           "turns": data.get("num_turns"), "calls": api_calls(data.get("session_id"), t0), "s": int(time.time() - t0),
           "input": u.get("input_tokens", 0), "cache_write": u.get("cache_creation_input_tokens", 0),
           "cache_read": u.get("cache_read_input_tokens", 0), "output": u.get("output_tokens", 0),
           "cost_usd_equiv": data.get("total_cost_usd"), "result": str(data.get("result") or "")[-300:]}
    return rec


# ---------- gate ----------
def sh(cmd, cwd):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, env=dict(os.environ, CARGO_TARGET_DIR=str(TARGET)))
    return p.returncode, (p.stdout + p.stderr)


def changed(wt):
    _, out = sh(["git", "status", "--porcelain", "--untracked-files=all"], wt)
    return [l[3:].split(" -> ")[-1] for l in out.splitlines() if l.strip()]


ABUSE_PRIVATE = PROJECT / "notes" / "abuse-notes"  # local only; never in git (public repo)


def move_abuse_notes(wt):
    """Move abuse notes out of the worktree, so they are never committed or pushed."""
    src = Path(wt) / "Docs/Plan/abuse-notes"
    for f in sorted(src.glob("*.md")) if src.is_dir() else []:
        if f.name == "README.md":
            continue
        ABUSE_PRIVATE.mkdir(parents=True, exist_ok=True)
        with (ABUSE_PRIVATE / f.name).open("a") as out:
            out.write(f.read_text(errors="replace").rstrip() + "\n")
        f.unlink()
        print(f"abuse note moved to {ABUSE_PRIVATE / f.name}")


def gate(row, wt):
    """Return '' when all checks pass, else the failure text (tail only)."""
    move_abuse_notes(wt)
    files = changed(wt)
    handoff = f"Docs/Plan/handoffs/{row['id']}.md"
    if handoff not in files:
        return f"missing {handoff}"
    allowed = [p.strip() for p in row["paths"].split(",") if p.strip()] + ["Docs/Plan/handoffs/", "Docs/Plan/cards/", "Docs/Plan/abuse-notes/"]
    outside = [f for f in files if not any(f.startswith(a) for a in allowed)]
    if outside:
        return "changed files outside the card paths:\n" + "\n".join(outside)
    bad = [f for f in files if BAD_EXT.search(f)]
    if bad:
        return "binary or key files:\n" + "\n".join(bad)
    sh(["git", "add", "-A"], wt)
    _, diff = sh(["git", "diff", "--cached"], wt)
    sh(["git", "reset", "-q"], wt)
    if re.search(r"^\+.*BEGIN [A-Z ]*PRIVATE KEY", diff, re.M):
        return "private key in diff"
    if GATEWAY_KEY.is_file():
        key = GATEWAY_KEY.read_text().strip()
        if key and key in diff:
            return "gateway key in diff"
    if row["kind"] == "rust":
        # The crate column can list several crates, comma-separated.
        for c in [x.strip() for x in row["crate"].split(",") if x.strip()]:
            for cmd in (["cargo", "fmt", "-p", c, "--", "--check"], ["cargo", "test", "-q", "-p", c],
                        ["cargo", "clippy", "-q", "-p", c, "--all-targets", "--", "-D", "warnings"],
                        *EXTRA_GATE.get(c, [])):
                rc, out = sh(cmd, wt)
                if rc:
                    return f"$ {' '.join(cmd)}\n" + "\n".join(out.splitlines()[-TAIL:])
    return ""


# ---------- main flow ----------
def run(sid, dry, push):
    head, rows = load()
    row = pick(rows, sid)
    if not row:
        print("nothing to run (no ready slice with done deps)")
        return 2
    card = REPO / f"Docs/Plan/cards/{row['id']}.md"
    prompt = card.read_text()
    wt = WT_ROOT / row["id"]
    if dry:
        print(" ".join(session_cmd(row, f"<{card.name}>")))
        print(f"cwd={wt}")
        return 0
    acct = ensure_account()
    if not acct:
        print("no account under 5h 90 % and 7d 99 %; stop")
        return 75
    WT_ROOT.mkdir(exist_ok=True)
    branch = f"slice/{row['id']}"
    if not wt.exists():
        rc, out = sh(["git", "worktree", "add", "-q", "-b", branch, str(wt), "main"], REPO)
        if rc:
            print(out)
            return 1
    set_status(row["id"], "running")
    log = []
    resume = paused().get(row["id"])
    note_rel = f"Docs/Plan/handoffs/{row['id']}-progress.md"
    note = wt / note_rel
    budget = int(row["turns"])
    stage_prompt = RESUME_PROMPT if resume else (
        NEXT_PROMPT.format(note=note.read_text(), card=prompt) if note.exists() else prompt)
    for stage in range(1, STAGE_MAX + 1):
        last = stage == STAGE_MAX or budget <= STAGE_TURNS + 10
        turns = budget if last else STAGE_TURNS
        role = "resume" if resume else ("work" if stage == 1 and not note.exists() else "next")
        rec = run_session(row, wt, stage_prompt, resume=resume, max_turns=turns, role=role)
        rec["stage"] = stage
        rec["account"], rec["acct_5h"], rec["acct_7d"] = acct["n"], acct["5h"], acct["7d"]
        log.append(rec)
        if hit_limit(rec):
            set_paused(row["id"], rec.get("session") or resume)
            for r in log:
                r["gate"] = "paused"
            with LEDGER.open("a") as f:
                for r in log:
                    f.write(json.dumps(r) + "\n")
            set_status(row["id"], "paused")
            print(f"PAUSED {row['id']} stage {stage} (usage limit). Resume later in session {rec.get('session') or resume}")
            attention(row["id"], "paused", f"Usage limit in stage {stage}. The loop resumes it when an account has quota.", wt)
            return 75
        set_paused(row["id"], None)
        resume = None
        if last or rec.get("subtype") != "error_max_turns" or not rec.get("session"):
            break
        wp = WRAP_PROMPT_SPEC if row["kind"] == "spec" else WRAP_PROMPT
        wrap = run_session(row, wt, wp.format(note=note_rel), resume=rec["session"], max_turns=WRAP_TURNS, role="wrap")
        wrap["stage"] = stage
        log.append(wrap)
        if not note.exists():
            break  # no note: let the gate judge what exists
        budget -= int(rec.get("turns") or turns)
        print(f"STAGE {stage} of {row['id']} done ({rec.get('turns')} turns); next stage from {note_rel}")
        stage_prompt = NEXT_PROMPT.format(note=note.read_text(), card=prompt)
    if note.exists():
        note.unlink()  # working note only; the final handoff is the record
    fail = gate(row, wt)
    if fail and (rec.get("session") or row["kind"] == "rust"):
        if row["kind"] == "rust" or not rec.get("session"):
            # Fresh session: a resumed one reads the whole old context again on every call.
            # A Rust gate error is concrete, so the card, the diff and the error are enough.
            rp = (REPAIR_FRESH.format(handoff=f"Docs/Plan/handoffs/{row['id']}.md") + fail
                  + "\n\n## Card\n" + prompt)
            rep = run_session(row, wt, rp, max_turns=REPAIR_TURNS, role="repair")
        else:
            rep = run_session(row, wt, "The gate failed. Fix only this. Then run the checks again and update the handoff.\n\n" + fail,
                              resume=rec["session"], role="repair")
        log.append(rep)
        fail = gate(row, wt)
    with LEDGER.open("a") as f:
        for r in log:
            r["gate"] = "fail" if fail else "pass"
            f.write(json.dumps(r) + "\n")
    tokens = sum(r["input"] + r["cache_write"] + r["cache_read"] + r["output"] for r in log)
    if fail:
        set_status(row["id"], "blocked")
        print(f"BLOCKED {row['id']} ({tokens:,} tokens). Worktree kept: {wt}\n{fail}")
        attention(row["id"], "blocked", f"Gate failed ({tokens:,} tokens).\n{fail}", wt)
        return 1
    sh(["git", "add", "-A"], wt)
    msg = f"{row['id']}: {row['title']}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
    rc, out = sh(["git", "-c", "user.name=Claude", "-c", "user.email=noreply@anthropic.com", "commit", "-q", "-m", msg], wt)
    if rc:
        set_status(row["id"], "blocked")
        print("commit failed:\n" + out)
        attention(row["id"], "blocked", "commit failed:\n" + out, wt)
        return 1
    # main may have moved while the slice ran (queue or card commits). Rebase first.
    rc, out = sh(["git", "rebase", "-q", "main"], wt)
    if rc:
        sh(["git", "rebase", "--abort"], wt)
    rc, out = sh(["git", "merge", "-q", "--ff-only", branch], REPO)
    if rc:
        set_status(row["id"], "blocked")
        print(f"merge into main failed; branch {branch} kept:\n{out}")
        attention(row["id"], "blocked", f"merge into main failed; branch {branch} kept (salvage retries it):\n{out}", wt)
        return 1
    set_status(row["id"], "done")
    sh(["git", "add", "tools/slices/queue.tsv"], REPO)
    sh(["git", "-c", "user.name=Claude", "-c", "user.email=noreply@anthropic.com", "commit", "-q", "-m",
        f"queue: {row['id']} done\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>", "--", "tools/slices/queue.tsv"], REPO)
    sh(["git", "worktree", "remove", "--force", str(wt)], REPO)
    sh(["git", "branch", "-q", "-D", branch], REPO)
    if row["kind"] == "spec":
        chain_rust(row)
    if push:
        rc, out = sh(["bash", "tools/push_github.sh"], REPO)
        print(out.strip().splitlines()[-1] if out.strip() else f"push rc={rc}")
    attention(row["id"], None)
    print(f"DONE {row['id']} ({tokens:,} tokens, {sum(r['s'] for r in log)} s)")
    return 0


def chain_rust(row):
    """After a spec slice: queue its Rust card (<id>-rust) if the card exists."""
    rid = f"{row['id']}-rust"
    card = REPO / f"Docs/Plan/cards/{rid}.md"
    _, rows = load()
    if not card.is_file() or any(r["id"] == rid for r in rows):
        return
    text = card.read_text()
    crate = re.search(r"cargo (?:test|clippy) -p ([\w-]+)", text)
    files = re.search(r"^## Files\n(.*?)(?=^## |\Z)", text, re.S | re.M)
    paths = [m.split()[0] for m in re.findall(r"^- `([^`]+)`", files.group(1), re.M)] if files else []
    title = re.search(r"^# (.+)$", text, re.M)
    if not crate or not paths:
        print(f"chain: {rid} not queued (no crate or Files list in card)")
        return
    paths.append("Cargo.lock")
    t = (title.group(1).split(":", 1)[-1].strip() if title else rid).replace("\t", " ")
    with QUEUE.open("a") as f:
        f.write("\t".join([rid, "rust", "sonnet", crate.group(1), ",".join(paths), row["id"], "120", "ready", t]) + "\n")
    sh(["git", "add", "tools/slices/queue.tsv"], REPO)
    sh(["git", "-c", "user.name=Claude", "-c", "user.email=noreply@anthropic.com", "commit", "-q", "-m",
        f"queue: add {rid}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>", "--", "tools/slices/queue.tsv"], REPO)
    print(f"chain: queued {rid} ({crate.group(1)})")


def salvage():
    """Merge blocked slices whose gate passed but whose ff merge failed (main moved)."""
    recs = [json.loads(l) for l in LEDGER.read_text().splitlines() if l.strip()] if LEDGER.is_file() else []
    last = {r["slice"]: r.get("gate") for r in recs}
    _, rows = load()
    for row in rows:
        wt, branch = WT_ROOT / row["id"], f"slice/{row['id']}"
        if row["status"] != "blocked" or last.get(row["id"]) != "pass" or not wt.is_dir():
            continue
        _, ahead = sh(["git", "rev-list", "--count", f"main..{branch}"], REPO)
        if ahead.strip() in ("", "0"):
            continue
        rc, _ = sh(["git", "rebase", "-q", "main"], wt)
        if rc:
            sh(["git", "rebase", "--abort"], wt)
            print(f"salvage: {row['id']} rebase conflict, left blocked")
            continue
        rc, out = sh(["git", "merge", "-q", "--ff-only", branch], REPO)
        if rc:
            print(f"salvage: {row['id']} merge failed:\n{out}")
            continue
        set_status(row["id"], "done")
        sh(["git", "-c", "user.name=Claude", "-c", "user.email=noreply@anthropic.com", "commit", "-q", "-m",
            f"queue: {row['id']} done (salvaged)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>", "--", "tools/slices/queue.tsv"], REPO)
        sh(["git", "worktree", "remove", "--force", str(wt)], REPO)
        sh(["git", "branch", "-q", "-D", branch], REPO)
        if row["kind"] == "spec":
            chain_rust(row)
        attention(row["id"], None)
        print(f"salvage: {row['id']} merged")


def loop(push, max_slices):
    """Run ready slices one by one. Skip past blocked ones. Stop on empty queue or no quota."""
    rc = _loop(push, max_slices)
    why = {0: f"done {max_slices} slices (--max)", 2: "no ready slice (queue empty, or the rest wait on blocked cards)",
           75: "no account had quota for 12 h", 3: "stop asked (tools/run_queue.sh stop)"}.get(rc, f"rc {rc}")
    attention("_loop", "stopped", f"The loop stopped: {why}. Start it again: tools/run_queue.sh")
    print(f"LOOP STOPPED: {why}", flush=True)
    return rc


def _loop(push, max_slices):
    attention("_loop", None)
    salvage()
    for _ in range(max_slices):
        if STOP.exists():
            STOP.unlink()
            return 3
        rc = run(None, False, push)
        salvage()
        waited = 0
        while rc == 75 and waited < WAIT_MAX_S:
            # All accounts over the 5h limit: wait for a reset instead of stopping.
            print(f"no quota; wait {WAIT_STEP_S // 60} min ({waited // 60} min so far)", flush=True)
            time.sleep(WAIT_STEP_S)
            waited += WAIT_STEP_S
            rc = run(None, False, push)
            salvage()
        if rc in (2, 75):
            return rc
    return 0


def status():
    _, rows = load()
    for r in rows:
        print(f"{r['status']:8} {r['id']:16} {r['kind']:4} {r['model']:6} {r['title']}")
    if LEDGER.is_file():
        recs = [json.loads(l) for l in LEDGER.read_text().splitlines() if l.strip()]
        tot = sum(r["input"] + r["cache_write"] + r["cache_read"] + r["output"] for r in recs)
        t = lambda r: r["input"] + r["cache_write"] + r["cache_read"] + r["output"]
        failed = sum(t(r) for r in recs if r.get("gate") == "fail")
        repair = sum(t(r) for r in recs if r.get("gate") != "fail" and r.get("role", "repair" if r.get("repair") else "work") == "repair")
        wrap = sum(t(r) for r in recs if r.get("role") == "wrap")
        print(f"ledger: {len(recs)} sessions, {tot:,} tokens")
        print(f"  failed gate (work may be kept by a later run): {failed:,} ({100 * failed / max(tot, 1):.1f} %)")
        print(f"  repairs that passed: {repair:,} ({100 * repair / max(tot, 1):.1f} %)")
        print(f"  stage notes (wrap): {wrap:,} ({100 * wrap / max(tot, 1):.1f} %)")
    return 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=("status", "run", "loop"))
    ap.add_argument("id", nargs="?")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--no-push", action="store_true")
    ap.add_argument("--max", type=int, default=8, help="loop: maximum slices")
    a = ap.parse_args()
    if a.cmd == "loop":
        return loop(not a.no_push, a.max)
    return status() if a.cmd == "status" else run(a.id, a.dry_run, not a.no_push)


if __name__ == "__main__":
    sys.exit(main())
