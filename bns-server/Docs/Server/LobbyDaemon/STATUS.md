# LobbyDaemon step 1 of the full port (analysis) — report

| Batch | Spec | State |
| --- | --- | --- |
| A to D | `12-job-map.md`, `13` to `19` (45 job classes) | done |
| E | `20-handlers.md` | done |
| F | `21-lobbydb-wrappers.md` | done |
| G | `22-compare-307.md`, `fn-map-307.json` | done |

- Build 307 against build 348: 1813 of 1864 named function pairs have the same strings, callees and constants.
- Open ❓: `22-compare-307.md` §7. Largest item: the caller maps from `dbrtn` to wire codes.
- Next step: Rust port from the specs.

# LobbyDaemon step 11 — report

Step 11 is partial. New work stopped at turn 25. The opt-in local character flow passes.

| Part | State | Test evidence |
| --- | --- | --- |
| Blocking spec | partial | Section 15 defines R15. Checked 307 completion paths. ❓ Full error maps, name rules and SQL time conversion remain open. |
| Store | done | Eight new methods implement the local contract. Tests check typed errors, separate states, repeated restoration, immediate deletion and PCID tombstones. |
| GameDaemon stub | done | Create/delete/restore support fixed failures and delay. Success, rejection and timeout tests pass. No external transport was added. |
| D44/D47/D51/D54/D103 | partial | Fourteen generated tests pass for R15. D103 uses NP, not a guessed DB procedure. ❓ Production parity remains open. |
| Gate lifecycle | done | Encrypted login → empty list → create → list → delete → waiting list → cancel → normal list passes. D54 rejection relay passes. D41 is tested separately; C41 stays blocked. |
| SQL adapter/tests | partial | Eight bound-parameter methods compile. New generated lifecycle test checks PCID/time outputs and state cycles. Cleanup includes CharacterLifeStory. ❓ Live execution remains open. |
| Validation | done | Workspace tests, both requested Clippy checks and lobbyd formatting pass. Four offline SQL unit tests also pass. |

**Totals:** 224 passed, 0 failed, 6 ignored in the default workspace run; step-10 baseline: 208 passed.
Breakdown: 102 proto/server + 67 lobbyd + 55 data/model/schema. One imported gate test runs again.
Both Clippy checks pass with `-D warnings`: workspace/all-targets and lobbyd/mssql/all-targets.
The latter compiles SQL integration targets. It does not execute them.
The added wrong-link test first failed with `Hello required`. The fixture now sends Hello. All final tests pass.

**Spec:** `Docs/Server/LobbyDaemon/11-character-select.md` §15 records R15 and these build-307 sources:
Create `0x140042610`/`0x140042CA0`; delete `0x140046C30`/`0x140046DB0`;
restore `0x1400189B0`, `0x1400191F0`, `0x140019670`, `0x140019B20`;
D54 `0x140065270`; D103 `0x14002EF50`/`0x14002FAB0`. ❓ D103 assembly check remains open.

**Local policies:** Mutation requires explicit slots, worlds, name bounds and wait seconds. Only non-jumping creation is supported.
R15 uses ASCII names, fixture approvals, local wire errors and monotonic deadlines. It does not claim production NP/name parity.
Freshly loaded rows lack lifecycle metadata. They reject mutation. No deletion scheduler was added.
SQL preserves timestamp components; reload still rejects non-NULL character times. Uncertain writes have no automatic retry.
Existing five-second NP/world waits, ten-second SQL wait, opt-in INIT expiry, positive token lifetimes and same-link retention remain.
D103 needs an explicit local initial-name control. Optional production paths stay disabled.

**Operator tests:** ❓ The unset-variable skip command required approval and did not run. Live SQL and cleanup were not run.
With `LOBBYD_MSSQL_URL` unset, run `cargo test --manifest-path bns-server/Cargo.toml -p lobbyd --features mssql --test mssql -- --nocapture`; expect SKIP messages and no SQL connection.
On an approved disposable SQL instance, run the same command with the variable set. Expect state cycles, both outputs and generated-row cleanup to pass.
**Boundaries:** No dependency, proto/server behavior, binary, IDB or `Program/Tools/Emu/` change. No external service call or excluded component analysis. IDA closed without saving.
❓ Final git diff/status checks were not run. No commit was made.
**Next job:** Verify remaining 307 maps and name/config rules. Add persisted lifecycle metadata and SQL time conversion. Then run approved SQL integration tests.

---

# LobbyDaemon step 10 — report

Stopped at the budget limit. Step 10 is partial.

| Part | State | Test evidence |
| --- | --- | --- |
| Character-screen spec | partial | `Docs/Server/LobbyDaemon/11-character-select.md` covers all seven commands and D57/D79. ❓ Some errors, name rules, config bindings and time/state conversions remain open. |
| Gate check | done | No gate change is required for the checked relays. Encrypted D38 success/error checks pass. C41 stays blocked. |
| Rust handlers | partial | D38 uses the local cache. D41 is a no-op. Five new rule tests pass. D44/D47/D51/D54/D103 remain unimplemented. |
| Store / GameDaemon / SQL extensions | not started | No new methods or bindings. No dependency changes. |
| Full character lifecycle test | not started | Create/delete/wait/cancel/recovery flow remains open. The target has no successful D41 name-check reply. |
| Validation | done | Workspace tests and both requested Clippy commands pass with `-D warnings`. |

**Totals:** 208 passed, 0 failed, 6 ignored. Baseline: 203 passed, 6 ignored.
The total is 102 proto/server + 51 lobbyd + 55 data/model/schema tests.
The five added tests use generated data. The existing encrypted gate test now checks D38.
One new assertion first failed on trailing-byte tolerance. The corrected local-policy test passes.
Both Clippy checks pass: workspace/all-targets and lobbyd/mssql/all-targets.
The mssql Clippy check compiles the integration targets. It does not execute SQL tests.

**307 sources:** D38 `0x1400C0DD0`; D41 `0x1400C1270`; D44 `0x1400C12A0`; D47 `0x1400C1920`.
D51 `0x1400C1F60`; D54 `0x1400C25A0`; D103 `0x1400C6790`; name rules `0x14006AC30`.
D51 uses TryRestoration → GameDaemon restore → FinishRestoration, not CancelCharacterDeletion.

**Retained policies:** Five-second local NP/world waits, opt-in INIT expiry, positive token lifetimes and same-link retention stay in place.
SQL keeps its ten-second local timeout, timestamp refusal and no-retry policy. Optional paths stay disabled.
Proto/server behavior, existing tests and `Program/Tools/Emu/` were not changed. IDA closed without saving.
No external service call or excluded licence/anti-cheat analysis was made. The spec records the boundaries.

**Operator tests:** ❓ Live SQL, cleanup and the unset-variable skip run remain unverified. The safe-unset command required approval and did not run.
❓ Final git diff/status checks also required approval and did not run.
**Next job:** Finish the remaining maps and cache model. Then implement D44/D47/D51/D54/D103, SQL bindings and the lifecycle test.

---

# LobbyDaemon step 9 — report

| Part | State | Test evidence |
| --- | --- | --- |
| SQL Server Store | partial | All 12 Store methods cover 16 procedures. Four offline SQL unit tests pass. ❓ Live SQL behavior and timestamp/cache gaps remain. |
| Environment settings | done | Runtime LOBBYD_MSSQL_URL selects the feature-enabled adapter. No connection value or dependency was added. Build and Clippy pass. |
| SQL integration tests | partial | Three generated-row tests compile. They cover load/list, score updates, retained cache, restoration, encrypted gate flow and four blob boundaries. ❓ SQL execution and cleanup remain unverified. |
| Validation | done | Default workspace: 148 passed, 0 failed, 0 ignored. Workspace and mssql Clippy pass with -D warnings. Formatting and binding JSON checks pass. |

**Test totals:** 102 proto/server + 46 lobbyd tests pass with default features.
Four additional mssql unit tests pass without connections. The SQL integration target builds with `--no-run`.
The safe-unset integration command required approval and did not run. ❓ Skip-path execution remains unverified.
Initial checks found Chrono-format and test-helper visibility errors. Both were fixed. Final checks pass.

**Spec changes:** `08-db-binding.md`, `db-binding.json` and `09-store-check.md` record adapter rules and limits.
Existing build-307 sources: account `0x14009C710`; character list `0x14009E100`; restoration `0x1400A1470`.
The four 512-byte limits also use `0x1400A59E0`, `0x1400A70C0`, `0x1400A88D0`. No new addresses were recovered.
Fixed call text uses bound sp_executesql RPC parameters. This is not a direct named-procedure TDS RPC.
Nonzero procedure returns stay typed inside io::Error. Existing D11/16 and D26/2 maps stay in place.

**Local limits:** Non-NULL character timestamps reject. ❓ Their wire units remain unknown.
Representative-world stays zero. BotScoreIgnore, blocked rows and rights other than 1 reject.
Store calls remain synchronous. SQL latency can block lanes through the shared lock.
The SQL worker uses a ten-second local timeout. It does not retry failed or uncertain writes.
NP/world waits stay five seconds. INIT expiry stays opt-in. Positive token lifetimes and same-link retention stay required.
Optional NP refresh, optional login data, detail-dependent recovery and real world transport stay disabled.
Blobs above 512 bytes reject. The other 72 procedure templates remain outside Store.

**Operator:** Set LOBBYD_MSSQL_URL in the environment for a disposable recreated LobbyDB.
Run `cargo test --manifest-path bns-server/Cargo.toml -p lobbyd --features mssql --test mssql -- --nocapture --test-threads=1`.
Expect three SQL tests to pass and remove their generated rows. The helper import also repeats the memory gate test.
Expect 0/511/512-byte blobs to pass and 513/4096-byte blobs to reject. Retained login must reuse cache; restoration must set state 2; kick must persist score 29.
**Next job:** Run these SQL tests. Check cleanup. Resolve timestamp units and representative-world derivation before broader SQL use.
Then address async Store scheduling, remaining caller maps and retained NP refresh. Do not replace unknown checks with success.

Only `bns-server/` was written. Proto/server behavior/tests, tools/emu and both parallel spec files stayed unchanged.
No SQL Server or external service was called. No dependency changed. No IDA database was opened or saved.
Licence, BNSHelper and anti-cheat work stayed excluded. TSS boundaries remain `0x140175FC8` / `0x140175FD8`.
GameGuard sources remain `ggauth64_27.dll` / `csauth3.cfg` directory entries. ❓ Their EXE addresses remain unknown.

---

# LobbyDaemon step 9 — progress

| Part | State | Evidence |
| --- | --- | --- |
| SQL Store | partial | All 12 Store methods / 16 procedures compile. Fixed batches use bound sp_executesql RPC parameters. Non-NULL character timestamps reject until units are specified. |
| Environment settings | done | from_env reads LOBBYD_MSSQL_URL only. Feature-enabled CLI selects it when set. No SQL connection was attempted. |
| SQL integration tests | partial | Three operator SQL tests added. They cover load/list, optional loaders, four blob fields, score, retained cache, restoration and encrypted gate flow. Generated-row cleanup is included. ❓ Live execution remains for the operator. |
| Validation | partial | Default workspace: 148 tests pass. Workspace and mssql Clippy pass with -D warnings. Four SQL unit tests pass without connections. A safe-unset integration test command required approval and did not run. ❓ Three live SQL tests remain for the operator. |

---

# LobbyDaemon step 8 — report

| Part | State | Test evidence |
| --- | --- | --- |
| DB caller error maps | partial | 16 scoped call paths recorded. JSON validation passes: 88 contracts and 13 fresh-login failure maps. ❓ Other callers and driver-specific NULL/conversion behavior remain open. |
| Store growth, local scope | done | 12 methods cover 16 templates. Four new tests failed to compile first, then passed. All four blob fields cover lengths 0, 511, 512, 513 and 4096. |
| NP/INIT timers and retained refresh | partial | Registration/index and callback paths checked. Existing retained/world tests pass. ❓ Timer registration, descriptors and complete grade fields remain open. Refresh stays disabled. |

**Validation:** `cargo test --manifest-path bns-server/Cargo.toml`: **148 passed, 0 failed, 0 ignored**.
Total: **102 proto/server tests + 46 lobbyd tests**.
`cargo clippy --manifest-path bns-server/Cargo.toml --workspace --all-targets -- -D warnings`: **PASS**.
Lobbyd formatting check: **PASS**. No dependency was added.

**Spec changes, build 0.0.307:** `08-db-binding.md` and `db-binding.json` add caller and fetch rules.
Fresh-login cache DB failure sends D11/16 through `0x1400336F0` → `0x1400D7190`. Rust now uses this map.
Restoration DB failure becomes result 6, then D26/2: `0x140031310` → `0x14007BB40`.
Fetch warning handling: `0x1400E3CB0`. Account NULL defaults: `0x14009C710`.
Showcase length path: `0x14009E100`; appearance output limits: `0x1400A59E0`, `0x1400A70C0`, `0x1400A88D0`.
`09-store-check.md` records explicit optional/detail fixtures and one-time local restoration.
`10-timers-refresh.md` adds NP context `0x140001FE0`, callback `0x1400CDA20`, and INIT index `0x14003CE40`.
❓ These paths do not establish numeric request/INIT lifetimes or XML descriptor bindings.

**Local policies:** NP/world waits stay five seconds. INIT expiry stays opt-in. Scheduler resolution stays 10 ms.
Positive token lifetimes, same-link retention, IPv4 checks and unknown-check rejection stay in place.
Kind-6 recovery stays supported. Other service kinds stay blocked. New detail methods do not enable recovery.
Optional login data and retained refresh stay disabled. Real world transport stays deferred.
Rust rejects blobs above 512 bytes. ❓ This does not prove original ODBC truncation behavior.

**Next job:** finish the other caller maps and service setup/output checks. Test ODBC boundaries under operator control.
Resolve NP/INIT registration and config descriptors. Complete grade fields and D16 selection before retained refresh.
Then use the parallel GameDaemon spec for later world entry. Do not replace unknown checks with success.

Only `bns-server/` was written. Proto/server behavior/tests, `tools/emu`, and both parallel spec files stayed unchanged.
No SQL Server or external service was called. Tests use generated data. Target IDA closed without a save request; guide not opened.
Python helper execution required approval and did not run; jq validated JSON. The retained helper only validates files.
TSS boundaries stay `0x140175FC8` / `0x140175FD8`. GameGuard sources stay the `ggauth64_27.dll` / `csauth3.cfg` directory entries.
❓ GameGuard EXE addresses stay unknown. Licence, BNSHelper and anti-cheat work stayed excluded.

---

# LobbyDaemon step 8 — progress

| Part | State | Evidence |
| --- | --- | --- |
| DB caller error maps | partial | Login/restore paths checked. D11/16 replaces the local required-loader D11/2 map. Fetch warning and NULL rules recorded in 08. ❓ Other callers remain open at the eight-turn research limit. |
| Store trait growth | done | 12 methods cover 16 templates. Four new generated tests failed to compile first, then passed. Focused tests: 27 rules + 4 growth + 8 world pass. All four blob fields cover 0/511/512/513/4096. Optional features and detail recovery stay disabled. Workspace: 148 tests pass (102 proto/server + 46 lobbyd); workspace Clippy passes with -D warnings. Lobbyd formatting passes. |
| NP/INIT and retained refresh | not started | Existing local policies stay in place. |

---

# LobbyDaemon step 7 — report

| Part | State | Test evidence |
| --- | --- | --- |
| DB binding spec | partial | 88 templates, 87 wrappers, 94 result columns. Artifact checks pass. 82 scoped agreements, four capacity conflicts, two missing procedures. ❓ Full caller error maps and NULL rules remain. |
| Store trait check | done | Seven methods cover seven templates; 81 operations are absent. Two new tests failed first, then passed after the MemoryStore fixes. All 42 lobbyd tests pass. |
| Timer/config and retained refresh | partial | World-channel lifetime binding and grade-cache writers checked. Existing retained tests pass. ❓ NP/INIT timer registration and refresh runtime remain open. |

**Validation:** `cargo test --manifest-path bns-server/Cargo.toml`: **144 passed, 0 failed, 0 ignored**.
Total: **102 existing proto/server tests + 42 lobbyd tests**.
`cargo clippy --manifest-path bns-server/Cargo.toml --workspace --all-targets -- -D warnings`: **PASS**.
Lobbyd formatting check: **PASS**. No dependency was added.

**Spec changes, build 0.0.307:** `08-db-binding.md` and `db-binding.json` cover all 88 templates.
Bind helpers: `0x1400E3AF0` / `0x1400E3970`; mileage loop: `0x1400B45C0`.
Four result buffers bind 512 bytes against recreated varbinary(4096): `0x14009E100`, `0x1400A59E0`, `0x1400A70C0`, `0x1400A88D0`.
Missing procedures: BindCharacterSlot and UpdateJumpingCharacterStateComplete. ❓ Larger-blob runtime behavior remains untested.
`09-store-check.md` lists coverage and gaps. MemoryStore now persists logout score and rejects missing-account updates.
`10-timers-refresh.md` binds config +248 through `0x140086D50` → `0x14007A110` → channel +40.
`04-login.md` corrects command 1722: grade-level refresh, not charge. Cache writers: `0x140051C30` / `0x140051CE0`.
❓ XML descriptor bindings, numeric deadlines, complete grade meanings and remaining DB error maps stay open.

**Local policies:** NP/world deadlines stay five seconds. INIT expiry stays opt-in. Scheduler resolution stays 10 ms.
Explicit positive token lifetimes, same-link retention, IPv4 checks and unknown-check rejection remain.
Optional retained refresh stays disabled. Kind-6 recovery and world-entry fixture limits remain. Real world transport stays deferred.

**Next job:** finish DB caller error maps and NULL/conversion rules. Test the four blob boundaries with generated data.
Then resolve NP/INIT timer registration and config descriptors. Specify grade fields before adding retained refresh.
Use the parallel GameDaemon spec for later world entry. Do not replace unknown checks with success.

Only `bns-server/` was written. Proto/server behavior and tests, `tools/emu`, and both parallel-job files stayed unchanged.
No SQL Server or external NP service was called. Tests use generated data. Target IDA closed without a save request; guide not opened.
TSS boundaries remain `0x140175FC8` / `0x140175FD8`. GameGuard boundaries remain `ggauth64_27.dll` / `csauth3.cfg` directory entries.
❓ GameGuard EXE addresses remain unknown. Licence, BNSHelper and anti-cheat work stayed excluded.

---

# LobbyDaemon step 7 — progress

| Part | State | Evidence |
| --- | --- | --- |
| DB binding spec | partial | 88 templates / 87 wrappers; 86 schema signatures agree. Four 512/4096-byte result conflicts; two missing procedures. 94 result columns. ❓ Full caller error maps, NULL rules and SQL effects remain. |
| Store trait check | done | Seven methods cover seven templates; 81 operations remain absent. Two generated tests failed first, then passed after the MemoryStore fix. All 42 lobbyd tests pass. Logout score persists; missing-account updates reject. |
| Timer/config and retained refresh | partial | Config +248 reaches channel +40 through 0x140086D50 / 0x14007A110. Command 1722 refreshes grade level, not charge data. ❓ NP/INIT registration and refresh runtime remain open. Workspace: 144 tests pass; Clippy and formatting pass. |

DB research stopped at its ten-turn budget. No SQL connection or IDB save request.

---

# LobbyDaemon step 6 — report

| Part | State | Test evidence |
| --- | --- | --- |
| World-entry spec | partial | Build 307 checks cover NP payload fields/error rules, nested restoration DB work, admission ownership and token consumption. ❓ Production field meanings and later enter effects remain open. |
| D24 local Rust milestone | done | Eight new world tests pass. They cover success, rejection, restoration, timeout, cancellation, queueing, expiry and cleanup. Encrypted C13 → D24 → D25 → C14 passes through the unchanged gate. |
| Timer/config bindings and retained refresh | not started | Existing login and retained-session tests still pass. No new binding or refresh behavior is claimed. |

**Validation:** `cargo test --manifest-path bns-server/Cargo.toml`: **142 passed, 0 failed, 0 ignored**.
Total: **102 existing proto/server tests + 40 lobbyd tests**.
`cargo clippy --manifest-path bns-server/Cargo.toml --workspace --all-targets -- -D warnings`: **PASS**.
Lobbyd formatting check: **PASS**.

**Spec changes, build 0.0.307:** `06-world-entry.md` adds NP commands 1711/2711/1502/2901 and their checked decisions at `0x14007C080`–`0x14007DF70`.
Restoration reaches `usp_FinishCharacterRestoration` through `0x14007BB40` → `0x140031310` → `0x1400A1470`.
Admission ownership uses `0x140034710` / `0x140034960`. Issuance uses `0x14007EF80`; consumption uses `0x140087D40` / `0x140093EB0`.
`04-login.md` and local rule R14 now describe the implemented boundary.
❓ D25 trailing short, complete D27 meanings, production eligibility fields, restore DB bindings and token failure-hook effects remain open.

**Local policies:** NP and GameDaemon waits use five-second local deadlines. INIT expiry stays opt-in. Scheduler resolution stays 10 ms.
World entry needs an explicit positive token lifetime, eligibility fixture and NP decision. Unknown checks reject. Pending services block entry.
Admission guards release reservations on cleanup. Queue promotion and token consumption are local controls. D25 uses a local zero short.
Wrong-world consumption restores Lobby under a local policy. Real world transport, world kick and production NP checks remain deferred.
Existing same-link, IPv4, capacity, kind-6 recovery and empty optional-data policies remain in place.

**Next job:** resolve timer/config bindings and retained NP refresh. Use the parallel GameDaemon spec to extend post-consumption entry and queue promotion. Do not replace unknown checks with success.
An unused `tools/step6_update.py` remains. It was not executed. Execution and removal required approval.
Only `bns-server/` was written. Proto/server behavior and tests, `tools/emu`, and the two parallel-job files were not changed by this job.
Tests use generated data. No external service was called. Target IDA closed without a save request. Guide not opened.
TSS boundaries remain `0x140175FC8` / `0x140175FD8`. GameGuard boundaries remain the `ggauth64_27.dll` / `csauth3.cfg` directory entries.
❓ GameGuard EXE addresses remain unknown. Licence, BNSHelper and anti-cheat work stayed excluded.

---

# LobbyDaemon step 6 — progress

| Part | State | Evidence |
| --- | --- | --- |
| World-entry spec | partial | 307 NP commands 1711/2711/1502/2901 and error maps checked. Restore nested DB call 0x140031310 → 0x1400A1470 checked. Admission ownership and consume-before-world-check verified. ❓ Exact platform names, D25 short, D27 meanings and later enter effects remain open. |
| D24 Rust | done | Eight new world tests pass. Encrypted C13 → D24 → D25 → C14 passes through the unchanged gate, followed by local single-use consumption. Workspace tests: 142 pass before the final repeat. Clippy passes. No real world transport is claimed. |
| Timer/config and retained refresh | not started | Existing local policies stay in place. |

Target closed without a save request. Guide not opened. Boundary addresses stay unchanged.

---

# LobbyDaemon step 5 — report

Feature work stopped at the turn-20 limit. Step 5 is partial.

| Part | State | Test evidence |
| --- | --- | --- |
| 1. Login leftovers | partial | Kind-6 recovery and D22 conversion pass. Recovery keeps pending rows and does not complete services. ❓ Timer registration and remaining config bindings are open. |
| 2. Retained-session login | partial | D12/D15 pass for same-link sessions. Tests cover replay, wrong fields, cache reuse, deadlines, kick and disconnect cleanup. One encrypted reconnect passes through the unchanged gate. ❓ Optional NP refresh and cross-link transfer are not implemented. |
| 3. World entry | partial | Spec records guards, states, failures and GameDaemon messages. No D24 implementation, GameDaemon trait or deterministic world stub was added. No D24 test is claimed. |

**Validation:** **134 passed, 0 failed, 0 ignored**: 102 existing proto/server tests + 32 lobbyd tests.
`cargo test --manifest-path bns-server/Cargo.toml`: PASS.
`cargo clippy --manifest-path bns-server/Cargo.toml --workspace --all-targets -- -D warnings`: PASS.
Lobbyd formatting check: PASS. Seven rule tests are new. The existing encrypted test now also checks reconnect and cache retention.
The first crate-wide run found one stale D22 socket expectation. The corrected fixture and final workspace run pass.

**Spec evidence, build 0.0.307:**
- `bns-server/Docs/Server/LobbyDaemon/04-login.md:86`: D22 map `0x1400933D0`; service recovery `0x1400336F0`, `0x14006DC10`, `0x140035D20`.
- Retained init `0x1400535C0` / `0x140053720`; start `0x140075440`–`0x1400768C0`; return `0x140086980`; token verify/expiry `0x140093EB0` / `0x140094140`.
- Startup `0x140097EB0` passes config +252 to token-desk constructor `0x140093B80`. ❓ XML name and numeric default remain open.
- `bns-server/Docs/Server/LobbyDaemon/06-world-entry.md:1`: entry `0x14007AC90`, admission `0x14007EB90`, token `0x14007EF80`; D25/D26/D27 senders `0x1400B8C60` / `0x1400B8B90` / `0x1400B8D40`.

**Local policies:** NP deadline is five seconds. INIT expiry is opt-in, disabled by default. Scheduler resolution is 10 ms.
Retained tokens require an explicit positive lifetime. IPv4 checks default to enabled. Sessions stay on their gate link. D15 uses cached data with empty optional charge/EXP fields and no optional NP refresh.
Only kind-6 service recovery is supported. Other nonempty kinds fail explicitly. Existing ownership, capacity, empty optional-data and synchronous Store policies remain.
❓ NP/INIT timer registration, world-kick config binding, original numeric lifetimes, D14 reason 21 and service-detail loaders remain open.

**Next job:** finish the NP payload/error rules, nested DB effects, admission ownership and world-token consumption in 06-world-entry.md.
Then implement D24 with a GameDaemon trait and deterministic fixtures. Test rejection, restoration, timeout, queueing and token cleanup. Keep the real GameDaemon transport deferred.
Resolve the remaining timer/config bindings and retained refresh rules. Do not replace unknown platform checks with unconditional success.

Only `bns-server/` was written. Proto/server behavior and tests, and `tools/emu`, stay unchanged. Tests use generated data. No external service was called.
Target IDA closed without a save request. Guide not opened. Licence and anti-cheat work stayed excluded.
TSS boundaries: `0x140175FC8` / `0x140175FD8`. GameGuard boundaries: directory entries `ggauth64_27.dll` / `csauth3.cfg`. ❓ GameGuard EXE source addresses remain unknown.

---

# LobbyDaemon step 5 — progress

| Part | State | Evidence |
| --- | --- | --- |
| 1. Login leftovers | partial | D22 map checked at 0x1400933D0 via 0x140039380. Kind-6 cache recovery implemented from 0x1400336F0 / 0x14006DC10 / 0x140035D20. All 22 rule tests pass, including two new tests. ❓ Timer registration/config bindings remain open at cutoff. |
| 2. Retained-session login | partial | D12/D15 workers and token consumption/expiry checked. Spec adds 0x140075440–0x1400768C0 and 0x140086980. Local same-link controls implemented. All 25 rule tests and the encrypted gate reconnect pass. The D22 socket expectation is corrected. All 132 workspace tests pass. Workspace Clippy passes with -D warnings. Token desk receives config +252 at 0x140097EB0. ❓ XML name/default and NP optional refresh remain open. |
| 3. World entry | partial | New 06-world-entry.md records 0x14007AC90 / 0x14007EB90 / 0x14007EF80, guards, queue/failure rules and required GameDaemon messages. ❓ NP payloads, nested DB effects and Rust GameDaemon trait remain unfinished. No D24 implementation claimed. |

---

# LobbyDaemon step 4 — report

| Part | State | Test evidence |
| --- | --- | --- |
| Original deadlines and kick | partial | INIT kick now sends D20, not D22. New rule test passes. ❓ Original NP/init/kick durations and config bindings remain open. |
| Per-link scheduling | done | Blocked NP on link A does not block login or kick on link B. Tests check reply order, unique keys and shared capacity. |
| Session and token expiry | partial | Paused-time rules check INIT and generated NP-token deadlines. Socket test checks expiry during a partial read. ❓ Original lifetimes and lobby-token expiry remain open. |
| Nonempty-service recovery | not started | The existing explicit-rejection and presence-cleanup test still passes. No recovery is claimed. |
| Gate integration | done | Generated encrypted clients complete login and fragmented character load, then kick and logout through the unchanged gate. |
| Clippy fix | done | Both proto test loops use `as_chunks`. Complete-block checks stay the same. Workspace Clippy passes with `-D warnings`. |

**Validation:** `cargo test --manifest-path bns-server/Cargo.toml`: **127 passed, 0 failed, 0 ignored**.
Total: **102 existing proto/server tests + 25 lobbyd tests**. Five lobbyd tests are new.
`cargo clippy --manifest-path bns-server/Cargo.toml --workspace --all-targets -- -D warnings`: **PASS**.
The first extended gate run failed with `Packet(InvalidSize)`: its helper treated C10 as fragmented. The corrected helper passes.

**Spec changes, build 0.0.307:** `04-login.md` records kick branches at `0x140084F50` / `0x1400850D0`, world-kick outcomes at `0x140085B00`–`0x140086070`, anonymous D20 at `0x140038DB0`, and close cleanup at `0x140032A10`.
NP setup/keepalive at `0x1400025D0` / `0x140001DE0` proves that 30000 is not a request deadline. Transaction and token expiry field use is recorded at `0x14007A250` / `0x140093D20`. ❓ Numeric lifetimes remain unverified.
`05-rust-milestone.md` updates R8/R9 and adds local expiry rules R10/R11.

**Local policies:** NP deadline stays five seconds. INIT expiry is opt-in and disabled by default. Scheduler resolution is 10 ms. Generated NP tokens can have explicit fixture deadlines. No Lobby idle expiry is assumed.
Ownership checks, capacity/queue bounds, empty optional login data and synchronous shared Store calls remain local rules. Nonempty services still fail explicitly. ❓ D22 reason conversion remains open.

**Next job:** find NP request and INIT timer registration. Bind world-kick timeout and token-desk lifetime to config fields. Verify D22 reason conversion. Specify and implement nonempty-service recovery. Keep D12/D15 and D24 out of scope.

Only `bns-server/` was written. Server behavior and `tools/emu` stay unchanged. No sample secret or external service was used.
Target IDA closed without a save request. Guide not opened. Excluded work stayed excluded.
TSS boundaries: `0x140175FC8` / `0x140175FD8`. GameGuard: directory entries `ggauth64_27.dll` / `csauth3.cfg`. ❓ GameGuard EXE source addresses remain unknown.

---

# LobbyDaemon step 4 — progress

| Part | State | Evidence |
| --- | --- | --- |
| 1. Original deadlines and kick | partial | Target kick branches and INIT D20 checked. NP 30000 is keepalive, not request expiry. ❓ Numeric request/init/kick deadlines remain open. |
| 2. Scheduling and recovery | partial | 25 lobbyd tests pass, including five new kick/expiry/lane rules. First extended gate test failed: Packet(InvalidSize). The helper treated nonfragmented C10 as fragmented. Helper corrected; validation pending. Nonempty recovery not implemented. |
| 3. Workspace Clippy | done | Both proto loops use as_chunks, with the same complete-block checks. Final workspace tests: 127 pass. Workspace Clippy passes with -D warnings. |

Target IDA closed without a save request. Guide not opened.
Boundaries unchanged: TSS `0x140175FC8` / `0x140175FD8`; GameGuard directory entries `ggauth64_27.dll` / `csauth3.cfg`. ❓ GameGuard EXE source addresses remain unknown. No excluded-component analysis.

---

# LobbyDaemon step 3 — report

| Part | State | Test evidence |
| --- | --- | --- |
| Login spec gaps | partial | ✅ Target checks cover NP init, lobby-token init, kick dispatch and both required DB loader bindings. ❓ Original timer durations and full kick branches remain. |
| Rust milestone | done | ✅ New `Program/Server/LobbyDaemon` binary and library. Protocol, jobs, Store and NP modules. 17 rule tests and two socket tests pass. |
| Real LobbyGate integration | done | ✅ One generated encrypted client logs in through the existing gate and receives ten characters across fragments. |
| Original timer and full kick scheduler | not started | ❓ Only explicit local notification controls and local NP deadlines are implemented. |

**Validation:** `cargo test --manifest-path bns-server/Cargo.toml`: **122 passed, 0 failed, 0 ignored**.
The total includes **102 unchanged tests + 20 new tests**.
Clippy passes: `cargo clippy --manifest-path bns-server/Cargo.toml -p lobbyd --all-targets -- -D warnings`.
The new-crate formatting check passes.

**Spec changes — build 0.0.307:**
- ✅ `04-login.md`: NP init dispatcher/worker `0x140094D30` / `0x140094D50`; event 7, registration, D7/D8 and INIT state.
- ✅ `04-login.md`: lobby-token init dispatcher `0x140053580`; workers `0x1400535C0` / `0x140053720`; retained states and D13/D14.
- ✅ `04-login.md`: kick dispatcher `0x140084E70`; constructor `0x140084D70`; D23 flag and lobby logout at `0x1400BFD80` / `0x1400948C0`.
- ✅ `03-db.md`: ordered parameter bindings, 14 character columns and six service columns at `0x14009E100` / `0x1400A5090`.
- ✅ `05-rust-milestone.md`: local rules R1–R9. Tests cover success, rejection, timeout, cancellation, cache, cleanup and D20/D21/D22.

❓ Original NP/init/kick durations, complete kick effects, DB NULL rules and some error mappings remain open.
The five-second NP deadline is a local policy. Nonempty incomplete services fail explicitly.
The binary binds loopback. Its empty stub rejects all tokens. Tests install generated fixtures.
No format mismatch appeared on the tested gate path. ❓ Other paths remain unverified.

**Next job:** resolve the original deadlines and kick workers first. Then add expiry scheduling, nonempty-service recovery and per-link job scheduling. Keep retained-session login and world entry deferred until their remaining rules are checked.

✅ Existing proto/server behavior, tests and `tools/emu` were not changed. No sample secret was copied. No external service was called.
✅ Target IDA closed without a save request. The guide was not opened. BloodCert, BNSHelper and anti-cheat work stayed excluded.
✅ TSS boundaries: `0x140175FC8` / `0x140175FD8`. GameGuard boundaries: directory entries `ggauth64_27.dll` / `csauth3.cfg`. ❓ Their EXE source addresses remain unknown.

---

# LobbyDaemon step 3 — progress log

| Part | State | Evidence |
| --- | --- | --- |
| Spec gaps | partial | ✅ NP init 0x140094D30/0x140094D50; lobby init 0x140053580; kick dispatcher 0x140084E70; both required DB loader bindings checked. ❓ Timer durations and full kick branches remain. |
| Rust milestone | partial | ✅ 16 new tests pass, including the real-gate encrypted login and fragmented character list. Full workspace: 118 pass (102 unchanged + 16 new). Clippy passes with -D warnings on all new-crate targets. D20 has a fixture only; full timer/kick behavior remains open. The initial codec-type failure was fixed. |

Spec research stopped at the seven-turn limit. No guide DB was needed. No IDA save was requested.
✅ Excluded TSS sources remain 0x140175FC8 and 0x140175FD8. GameGuard boundaries remain the directory entries ggauth64_27.dll and csauth3.cfg. ❓ No EXE address is established for those files. No anti-cheat, BloodCert or BNSHelper analysis was done.

---

# LobbyDaemon step 2 — report

| Part | State | Evidence |
| --- | --- | --- |
| 1. LobbyGate link | partial | ✅ 35 identities and receive formats; 66 channel sender formats. Gate 04/05 comparisons in 02-links.md. ❓ Reply reachability, D4/D58 and fragment rules remain. |
| 2. Login path | partial | ✅ NP start states at 0x1400CDD00; enter-world states at 0x14007AB10; logout at 0x140094740. ❓ Init/lobby-token jobs, full error maps, timer durations and kick job remain. |
| 3. Login DB | partial | ✅ Four wrappers have ordered bindings, result columns and caller checks. ❓ Character, EXP, reward and incomplete-service bindings remain. |
| 4. Config schema | partial | ✅ 161 distinct XML option names with sample sources. ❓ XML-to-code matches, code defaults and listener-list schemas remain. |

Totals: **LobbyGate handlers: 35 exact / 0 likely / 0 rejected. DB calls fully bound: 4.**
✅ Exact identity does not prove full behavior. The four DB wrappers include numeric SQL/C types and caller handling.

Minimal first Rust milestone — ❓ proposal, not implementation:
- Commands: D1 Hello; D6/D9 NP Init/Start; D35 character list; D18/D19/D23 close/logout. Test D20/D21/D22 output fixtures.
- DB trait: LoadGameAccount, RegisterGameAccount, UpdateLastLogin, UpdateLastLogout, LoadCharacterList and LoadIncompleteLobbyService. ❓ The last two bindings still block a live DB adapter.
- Stubs: deterministic local NP token/presence/profile/slot/grade answers; in-memory DB; empty incomplete services. Add rejection and timeout fixtures. No external platform calls.
- ❓ Defer retained-session D12/D15 and D24 world entry until their remaining state branches and token lifetimes are checked.

Next job: finish init/lobby-token and kick jobs; bind required login DB calls; check timer durations and config defaults; close D4/D58 and reply-reachability gaps.
✅ No Rust code or network request. No binary patch or IDA save request. Only spec/lobbyd files were written.
✅ TSS remains excluded: tss_get_sdk 0x140175FC8; tss_sdk 0x140175FD8. Presence only.
✅ GameGuard files ggauth64_27.dll and csauth3.cfg remain directory-only boundaries. ❓ EXE source addresses are not established.
✅ BloodCert and BNSHelper remain excluded. No licence-check or anti-cheat analysis was done.

---

# LobbyDaemon step 1 — report

| Part | State | Evidence |
| --- | --- | --- |
| 1. Overview | partial | ✅ 307 startup 0x14009A0B0 / 0x14009A830; all eight link roles listed. ❓ Service branches remain. |
| 2. Name map | partial | ✅ Target tables and SQL anchors. ❓ Most identities need body checks. |
| 3. Command tables | partial | ✅ Five arrays; 118 inbound handlers. ❓ PostOffice, NP and reply formats remain. |
| 4. Config and DB | partial | ✅ 88 DB call templates and code field loads. ❓ XML names/defaults and most DB types remain. |

Totals: **118 handlers mapped; 88 DB calls found; 273 name-map entries (35 exact / 238 likely).**
Handler split: ✅ Manage 6; LobbyGate 35; InfoGate 3; GameDaemon 51; AccountInventory 23.
❓ Exact identity does not prove full behavioral equivalence. Likely entries remain unverified.

Out-of-scope boundaries:
- ✅ Tencent TSS: tss_get_sdk 0x140175FC8; tss_sdk 0x140175FD8. Presence only.
- ✅ GameGuard: ggauth64_27.dll and csauth3.cfg directory entries. ❓ EXE references not found in this pass.
- ✅ BNSHelper.dll directory entry. ❓ BloodCert excluded without mapping. No licence-check work.

Proposed Rust phases — ❓ design proposals, not implementation facts:
1. Keep the existing server crate unchanged. Add lobbyd, lobbyd-protocol, lobbyd-core and lobbyd-store crates.
2. Use per-link ordered tasks and typed job state machines. Add offline packet fixtures first.
3. Start with a DB trait and in-memory tests. Use an ODBC SQL Server adapter for compatibility; ❓ defer a native DB choice until procedure semantics are known.
4. Add an NP trait with a deterministic offline stub. Return explicit unsupported results for unimplemented platform features.
5. Add startup, listeners, login and character flows. Add inventory and PostOffice after their specs pass review.

Next job: verify likely names; finish XML schema/defaults; separate receive/reply formats; map PostOffice and NP; check DB bindings and return codes.
No Rust code was written. No network request was made. No IDA save was requested.

## Work log

- Part 1: guide startup survey, then target listener and outbound-link checks.
- Part 2: string anchors, table-position identities and SQL-name identities.
- Part 3: checked target dispatch bounds; corrected table layout and AccountInventory base.
- Part 4: target SQL templates, descriptor labels and configuration field loads. XML extraction required approval. A tag-only search did not establish option names.
- Final cleanup: excluded component mappings removed; no new reverse engineering.
- Both databases closed without a save request.

## Step 2 progress

- Part 1 partial: 35/35 LobbyGate identities exact. Receive literals checked. Channel sender inventory added. Gate-side comparisons added. ❓ Reply reachability, command 58 and command 4 remain.
- Part 2 partial: login entry points, NP dispatcher states, platform decision points, enter-world states and logout boundary recorded in 04-login.md. ❓ Init/lobby-token job bodies, full failures, timer durations and kick job remain.
- Part 3 partial: four login wrappers have ordered binding and result evidence with caller handling. Ten conditional or required loader wrappers remain unbound. ❓ Timeout helper semantics remain.
- Part 4 partial: 161 distinct XML option names recorded with sample sources. Existing field loads are linked as proposed associations. ❓ Code descriptor matching, defaults and listener-list schemas remain.
