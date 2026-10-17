# 13 — Decisions, blockers and governance

## 13.1 Decisions required (this snapshot)

### D1: ADR-0007 reconciliation

| Aspect | Value |
|---|---|
| Question | Is the contradiction between ADR-0007 §1 ("sharecli-core Tier A pure, no infra deps") and `COMPREHENSIVE_AUDIT_SCORECARD.md` C00 row 3 ("sharecli-core depends on ipc/fleet/fuse, scored 3/3") resolved by the code, the ADR, or both? |
| Status | **DRAFTED (PR #935)**; awaiting operator merge |
| Options | A. code wins (PR #935) | B. ADR wins (refactor) |
| Evidence | the code follows the scorecard; the ADR's intent is preserved as a guideline not a hard rule |
| Recommendation | A. code wins; merge #935 |
| Operator action | merge #935 |

### D2: tray-icon 0.25.0 backend broken on Linux

| Aspect | Value |
|---|---|
| Question | How do we fix `sharecli-tray-windows` (and `sharecli-tray-linux` indirectly via Cargo.lock)? |
| Status | **DRAFTED (PR #938)**; uncommitted in dirty worktree |
| Options | A. add `features = ["libappindicator"]` to `tray-icon = "0.25"` (1 line) | B. bump `tray-icon` to `0.27.x` (multi-line) |
| Evidence | upstream `pub(crate) use self::platform::*;` doesn't re-export `pub(crate)` items via glob in 0.25.0/0.25.1; 0.26.1 has same bug; 0.27.x is unknown |
| Recommendation | A. add feature (smallest diff, working); future cleanup is 0.27.x |
| Operator action | approve PR #938 or fold into #936 |

### D3: `chore/cvp-1h-queue-2026-10-17` squash vs split

| Aspect | Value |
|---|---|
| Question | Merge as one squash PR (9 commits) or split into is_agent / FFI docs / CLI docs / gitignore / codesign-soft / libsqlite? |
| Status | **OPEN**; PR #936 OPEN with 9 commits |
| Options | A. one squash | B. split (~5 min `git reset` work) |
| Evidence | one squash keeps the 1h queue atomic; split is cleaner per-commit audit |
| Recommendation | B. split; cleaner history |
| Operator action | decide before merge |

### D4: `cargo-deny` advisory allowlist

| Aspect | Value |
|---|---|
| Question | How do we pass `cargo-deny` (currently FAIL)? |
| Status | **OPEN**; PRs #896, #909 in flight |
| Options | A. allowlist the advisory (immediate) | B. upgrade dependency (longer-term) |
| Evidence | tray-icon 0.25.0 has a known advisory; PR #921 carries the dep upgrade |
| Recommendation | A. allowlist now; B. follow up in v1.1 |
| Operator action | approve allowlist PR |

### D5: 14 files > 1000 LoC

| Aspect | Value |
|---|---|
| Question | Should we decompose all 14 over-1000L files? |
| Status | **ACCEPTED (BOUNDARY.md)**; audit §4.4 F1 |
| Options | A. accept (current) | B. decompose (audit §6.1 lower risk if we can find decomposition seams) |
| Evidence | the files have a single cohesive responsibility (e.g., `sharecli-core`); decomposition would create coupling without clarity |
| Recommendation | A. accept |
| Operator action | confirm BOUNDARY.md rationale is sufficient |

### D6: `monitoring.rs` and `pyroscope_stub.rs` extraction

| Aspect | Value |
|---|---|
| Question | Are these honest stubs acceptable? |
| Status | **ACCEPTED (audit §3 Theme B)** |
| Options | A. accept as-is | B. delete | C. wire up (perf integration) |
| Evidence | `#![allow(dead_code)]` + module-level docs; not silently broken |
| Recommendation | A. accept; C. is v1.1 |
| Operator action | confirm |

### D7: `crates/spawn-core` (Zig) ownership

| Aspect | Value |
|---|---|
| Question | Where does `crates/spawn-core` (Zig) live? |
| Status | **DEFERRED**; on disk in sharecli repo but ADR-0006 says `portage-temp` |
| Options | A. move to `portage-temp` (operator-side effort) | B. leave in sharecli (no PR exists) | C. document as frozen |
| Evidence | not in workspace; `zig build` not exercised |
| Recommendation | C. document as frozen; A. is v1.1 |
| Operator action | confirm ADR-0006 is the authority |

### D8: `crates/gateway-tools` workspace membership

| Aspect | Value |
|---|---|
| Question | Add to workspace? |
| Status | **MERGEABLE (PR #923)** |
| Options | A. add (current) | B. keep off-workspace (no PR) |
| Evidence | unblocks `cargo build` from seeing it |
| Recommendation | A. add |
| Operator action | merge #923 |

### D9: `sharecli-runtime` extraction

| Aspect | Value |
|---|---|
| Question | Extract `src/runtime.rs` to `crates/sharecli-runtime`? |
| Status | **DEFERRED to v1.1**; `lib-sprawl-plan.md` Phase 4 |
| Options | A. extract now | B. defer to v1.1 |
| Evidence | 1,358 LoC; would reduce `src/` size; not blocking v1.0 |
| Recommendation | B. defer |
| Operator action | confirm |

### D10: Real-user validation

| Aspect | Value |
|---|---|
| Question | When is the first beneficiary session recorded? |
| Status | **OPEN**; no `docs/eval/<date>-<human>-<journey>/` session today |
| Options | A. operator records within 30 days | B. defer to v1.1 | C. accept synthetic eval as substitute (REJECTED — synthetic is not beneficiary) |
| Evidence | MVP gate is BLOCKED on this |
| Recommendation | A. operator records within 30 days |
| Operator action | schedule 1 hour |

## 13.2 Blockers (must-clear for v1.0)

| ID | Blocker | Why | What unblocks |
|---|---|---|---|
| B1 | ci / test on main (tray-icon) | PR #938 lands, or fold into #936 | merge #938 (or fold) |
| B2 | cargo-deny on PR #921 | allowlist tray-icon advisory; or upgrade dep | PR #896 / #909 / a follow-up |
| B3 | cross / musl / freebsd / wasm | upstream `tray-icon` may be the cause | investigate post-#938; bump to 0.27.x if needed |
| B4 | R-eval-session (real-user validation) | no recorded beneficiary session | operator records one |
| B5 | agileplus / Tracera sharecli entries | not yet created | seed from `WORK_DAG.md`; operator adds to Tracera |
| B6 | cross-publish to sharecli `audit-sessions/2026-10-17-state-forward-docket/` | SFD not yet cross-published | after PhenoRegistry PR opens |
| B7 | operator-decisions on D1-D10 | the 10 decisions above | operator signals each |

## 13.3 Authority precedence (governance)

When two sources disagree, the precedence is:

1. **Code** (what is in the repo today) — top authority for behavior
2. **Audit** (`COMPREHENSIVE_AUDIT_SCORECARD.md`, audit-sessions) — second authority for measured vs intent
3. **PRD** (mature contract) — third authority for product scope
4. **ADRs** (architectural decisions) — fourth authority for design intent
5. **SPEC, FR** (functional requirements) — fifth authority for functional surface
6. **CHANGELOG, commit messages** — sixth authority for historical record
7. **Operator's typed signals** — final override (operator always wins on a tie)

When an ADR contradicts the code, **code wins** (the operator must update the ADR via a documented PR — see PR #935 for the canonical pattern). When an ADR contradicts the PRD, **PRD wins** (the ADR must be retired or revised).

## 13.4 Governance gaps

| Gap | Surface | Why it's a gap | Proposed path |
|---|---|---|---|
| `crane.yml` operator-decisions | cross-harness | not audited; cross-harness | operator reviews; not in agent scope |
| AgilePlus sharecli backlog | web | not seeded from `WORK_DAG.md` | agent seeds (1 hour) |
| Tracera sharecli product card | web | not present | operator adds |
| Recorded beneficiary session | evidence | none yet | operator records one |
| `phenoAI` Hermes imports in PhenoRegistry | skill | carried over; sharecli has no hermes workflows | n/a today; re-run if sharecli adds a hermes workflow |
| `KooshaPari/homebrew-sharecli` | distribution | not present | operator-side; not for v1.0 |
| `signpath` codesign release | distribution | dry-run via PR #918; v1.0 not yet cut | operator-side; v1.0 |
| `cliff.toml` `commit.link` template | docs | non-standard | sharecli engineer; 30 min |

## 13.5 What changes to the mature contract require

- A new ADR (with operator approval)
- A new PRD section
- A new SFD (this pattern)

What does **not** require an ADR:

- A bug fix
- A test lift
- A doc comment addition
- A SPDX header
- A untrack-junk PR
- A dedup PR

## 13.6 What is explicitly out of governance

- Operator's private channels (S20): not in agent scope
- Tracera (S18): not invoked; sharecli not yet added
- AgilePlus (S19): not invoked; backlog not yet seeded
- `crane.yml`: cross-harness; operator owns

These are listed as `not invoked` and **do not** affect the SFD's evidence base.
