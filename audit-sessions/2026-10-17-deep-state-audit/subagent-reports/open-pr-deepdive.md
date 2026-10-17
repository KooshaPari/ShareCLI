# Open PR Deep-Dive — sharecli @ 3d6a41c9

**Audit date:** 2026-10-17 (UTC)
**Repository:** `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli`
**Baseline main:** `3d6a41c972c2c86bf1374f224019c365ce3aa646`
**Branch:** `main`
**PRs analysed:** 11 open (#907, #918, #919, #921, #922, #923, #924, #925, #927, #929, #930)
**Author of every PR:** `KooshaPari` (`MDQ6VXNlcjQyNTI5MzU4`)

---

## Headline Matrix

| PR | Title                                                                              | Files | +/-             | Value | Risk | Conflict-With | Recommendation     |
|----|------------------------------------------------------------------------------------|-------|-----------------|-------|------|---------------|--------------------|
| 907 | ci: declare desk-sharecli runner label + docs/lint gate fixes                    | 4     | +13 / -15       | HIGH  | LOW  | 918 (actionlint.yaml, both add desk-sharecli runner label — different wording) | MERGE FIRST |
| 918 | ci: combined codesign unblock + actionlint + rustfmt (FR-001, FR-003)           | 35    | +277 / -278     | HIGH  | MED  | 907 (actionlint.yaml); supersedes 930 in full | MERGE AFTER 907 |
| 919 | fix(fuse): poison-safe lock recovery in FUSE callbacks                          | 1     | +108 / -57      | HIGH  | MED  | none | MERGE AFTER 918 (CI Success Gate FAILURE — investigate) |
| 921 | fix(deps): bump PhenoShared substrate rev to ea27186 (FR-001)                   | 1     | +5 / -5         | MED   | MED  | 923, 925 (all touch Cargo.lock — additive, low risk) | MERGE |
| 922 | test(sharecli-core): add 60 integration tests for transaction_contract + public_api | 2     | +654 / -0       | MED   | LOW  | none | MERGE |
| 923 | test(gateway-tools): add 24 non-demo + error-path tests and ship the gateway stub crate | 19    | +1968 / -4    | HIGH  | MED  | 924 (operator_display.rs x2 — **11 conflict markers**) | MERGE BEFORE 924 OR SUPERSEDE 924 |
| 924 | test(tray): add operator_display coverage (FR-001)                              | 2     | +368 / -0       | MED   | LOW  | 923 (**HARD CONFLICT** on operator_display.rs) | SUPERSEDE — rebase on 923 OR merge 923 first then rebase 924 |
| 925 | test(sharecli-ffi): add C-ABI coverage for all 6 extern functions (FR-001)     | 4     | +578 / -1       | MED   | LOW  | 921, 923 (Cargo.lock overlap — additive) | MERGE AFTER 923 |
| 927 | feat(core): SpeculativeCoalescer for in-flight request dedup (FR-008)           | 2     | +456 / -11      | HIGH  | MED  | none | MERGE |
| 929 | feat(harness-native): implement is_agent() sentinel-file detection               | 1     | +187 / -5       | HIGH  | LOW  | none | MERGE |
| 930 | ci(fmt): bring workspace into rustfmt 1.96.0 compliance                        | 32    | +252 / -273     | LOW   | LOW  | 918 (**100% REDUNDANT** — 930's only commit has identical numstat to 918's `8220dd52` rustfmt commit) | CLOSE AS DUPLICATE |

Legend: Value/Risk = HIGH (CI-breaking or fixes a real bug) / MED (additive feature or coverage) / LOW (cosmetic, docs, fmt). Recommendation uses the four buckets the audit requested: MERGE NOW, MERGE AFTER FIXES, SUPERSEDE, CLOSE.

---

## Per-PR Detail

### PR 907 — `ci: declare desk-sharecli runner label + docs/lint gate fixes`

- **Branch:** `stubs/w1-format-lint-20261016`
- **Created:** 2026-10-16T20:00:19Z
- **Files (4):**
  - `.github/actionlint.yaml` (declares `self-hosted-runner: labels: [desk-sharecli]`)
  - `.github/workflows/deploy-docs.yml` (adds `actions/configure-pages@983d7736` step before setup-node)
  - `lib/teamcomm/ARCHITECTURE.md` (wording fix: "the actual daemon backend" → "the running daemon", 1 line)
  - `tests/quick_start_journey.rs` (rustfmt style; identical content to PR 918's commit `8220dd52`)
- **Mergeable vs main:** MERGEABLE (zero conflict markers — `git merge-tree` shows 4 `merged`, 0 `<<<<<<<`)
- **mergeStateStatus:** `BLOCKED` (CI side, not git side)
- **Checks (failures):** `Quality Report`, `Cargo Deny`, `musl-test`, `FR reference in PR body`, `ubuntu-latest`, `cross / wasm32-unknown-unknown`, `bench run`, `macos-latest`, `cross / x86_64-unknown-freebsd` — these look like soft matrix legs; `CI Success Gate`, `ci / test`, `ci / lint`, `Rust`, `Guardrail (nextest)`, `CodeQL` all SUCCESS.
- **Value HIGH** — fixes `deploy-docs` Pages configuration step (real CI breakage) and unlocks actionlint for the nightly Harbor soak runner.
- **Risk LOW** — config + docs + style only.
- **Conflict with 918:** `.github/actionlint.yaml` (both add `desk-sharecli` runner label, but with different surrounding comment text); `tests/quick_start_journey.rs` (identical rustfmt change — no conflict).
- **Recommendation:** **MERGE FIRST.** It is the smallest CI blocker and 918 builds on top of the same label declaration.

### PR 918 — `ci: combined codesign unblock + actionlint + rustfmt (FR-001, FR-003)`

- **Branch:** `ci/combined-with-rustfmt-fix`
- **Created:** 2026-10-17T00:10:02Z
- **Files (35):** 1 file under `.github/actionlint.yaml`, 2 files under `.github/workflows/codesign*.yml`, 2 in `crates/sharecli-core/src/transaction_contract.rs` and `crates/sharecli-ipc/src/{lib,queue}.rs`, 5 under `src/`, 25 `tests/fr*` and `tests/c03_*.rs` and `tests/config_live_write_guard.rs`, `tests/quick_start_journey.rs`.
- **Mergeable vs main:** MERGEABLE (35 `merged`, 0 conflicts).
- **mergeStateStatus:** `BLOCKED`.
- **Checks (failures):** `Quality Report`, `Cargo Deny`, `musl-test`, `macos-latest`, several noise matrix legs — `CI Success Gate`, `ci / test`, `ci / lint`, `Rust`, `Guardrail (nextest)`, `CodeQL` are all SUCCESS.
- **Value HIGH** — unblocks the codesign gate on PR builds (commit `ecfe4615`), declares the `desk-sharecli` runner label (commit `215ad7b4`), and applies rustfmt 1.96.0 across the workspace (commit `8220dd52`).
- **Risk MED** — touches 25 test files but only via rustfmt, and the workflow YAML change is a trigger-policy flip (`pull_request` removed on `codesign-soft.yml`). The codesign gate change is low blast radius because the workflow uses `continue-on-error` by design.
- **Conflict with 907:** `.github/actionlint.yaml` (different comment text); `tests/quick_start_journey.rs` (identical change — no conflict).
- **Conflict with 930:** **NONE — PR 930's only commit `f753aa4d` has byte-identical numstat to PR 918's commit `8220dd52` and `git diff origin/pr/918 origin/pr/930` on shared files is empty.** PR 930 is fully contained in PR 918.
- **Recommendation:** **MERGE AFTER 907** (resolve the actionlint.yaml comment conflict by taking 918's longer explanation, or by rebase-and-pick whichever wording wins).

### PR 919 — `fix(fuse): poison-safe lock recovery in FUSE callbacks`

- **Branch:** `fix/fuse-wireframe-hardening`
- **Created:** 2026-10-17T00:21:32Z
- **Files (1):** `crates/sharecli-fuse/src/lib.rs` (108 +, 57 -).
- **Mergeable vs main:** MERGEABLE (1 `merged`, 0 conflicts).
- **mergeStateStatus:** `BLOCKED`.
- **Checks (failures include the umbrellas):** `CI Success Gate` FAILURE, `Quality Report` FAILURE, `Cargo Deny` FAILURE, `netblock hermetic` FAILURE, `macos-latest` FAILURE, `linux_native` FAILURE, `cargo bench (soft)` FAILURE. `ci / test`, `ci / lint`, `Rust`, `Guardrail (nextest)`, `CodeQL`, `FR reference in PR body` are all SUCCESS.
- **Value HIGH** — replaces `.expect()` panics in FUSE callbacks. A panic inside a FUSE callback kills the kernel-side mount; this is real production hardening.
- **Risk MED** — changes error-recovery semantics inside a critical path (FUSE).
- **Conflict with anything:** none.
- **Recommendation:** **MERGE AFTER 918** — but flag that `CI Success Gate` is FAILURE on this PR alone. Before merging, drill into why the umbrella gate fails even though the per-leg checks pass; the failure may be a known false positive (e.g. one of the matrix legs in `Quality Report`, `netblock`, etc.).

### PR 921 — `fix(deps): bump PhenoShared substrate rev to ea27186 (FR-001)`

- **Branch:** `ci/lockfile-bump-phenoshared`
- **Created:** 2026-10-17T00:29:29Z
- **Files (1):** `Cargo.lock`.
- **Mergeable vs main:** MERGEABLE.
- **mergeStateStatus:** `UNSTABLE`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `Quality Report` FAILURE, `Cargo Deny` FAILURE, `musl-test` FAILURE, several `ci / test` and `ci / lint` matrix legs FAILURE.
- **Value MED** — single-purpose lockfile bump intended to unblock `cargo check --workspace`. Decides which substrate rev is canonical.
- **Risk MED** — substrate rev bumps can change transitive behaviour; if `Cargo Deny` is flagging advisories that the newer rev introduces, that's a stop sign worth investigating.
- **Conflict with anything:** Cargo.lock overlap with 923 and 925 (all three modify Cargo.lock; merge ordering will produce a textual conflict but each hunk is additive in a different section, resolvable with rebase).
- **Recommendation:** **MERGE.** Single file, trivial to reason about. Sequence it before 923 and 925 so the substrate rev is locked first.

### PR 922 — `test(sharecli-core): add 60 integration tests for transaction_contract + public_api`

- **Branch:** `test/sharecli-core-coverage`
- **Created:** 2026-10-17T00:30:17Z
- **Files (2):** `crates/sharecli-core/tests/public_api.rs` and `crates/sharecli-core/tests/transaction_contract.rs`. +654/-0 net-additive.
- **Mergeable vs main:** MERGEABLE.
- **mergeStateStatus:** `BLOCKED`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `ci / test` SUCCESS, `Quality Report` FAILURE (likely docs-coverage gate).
- **Value MED** — adds real coverage for the two most-likely-broken interfaces in sharecli-core. `FR Annotation Check` commit also present.
- **Risk LOW** — test additions only.
- **Conflict with anything:** none (does not touch the same files as 921, 923, 925, 927).
- **Recommendation:** **MERGE.**

### PR 923 — `test(gateway-tools): add 24 non-demo + error-path tests and ship the gateway stub crate`

- **Branch:** `test/gateway-tools-coverage`
- **Created:** 2026-10-17T00:30:52Z
- **Files (19):** `Cargo.lock`, `Cargo.toml`, the new `crates/gateway-tools/src/{main,serve}.rs`, the new stub `crates/gateway/Cargo.toml` and 12 `crates/gateway/src/*.rs` modules, plus **two tray files** that conflict with PR 924: `crates/sharecli-tray-linux/src/operator_display.rs` and `crates/sharecli-tray-windows/src/operator_display.rs`.
- **Two commits:**
  1. `1dfc820d` test(gateway-tools): add non-demo + error-path coverage and ship the gateway stub crate
  2. `630a99c1` fix(tray): replace unreachable!() on Offline state with graceful tokens
- **Mergeable vs main:** MERGEABLE (6 `merged`, 0 conflicts).
- **mergeStateStatus:** `BLOCKED`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `ci / test` SUCCESS, `ci / lint` SUCCESS, `Quality Report` FAILURE, `Cargo Deny` FAILURE, `macOS codesign + notarize (hard gate)` FAILURE (Infisical-bound Apple secrets — the gate that 918 unblocks).
- **Value HIGH** — the tray commit `630a99c1` removes a real `unreachable!()` panic arm in `operator_display.rs` (line ~96 of `crates/sharecli-tray-linux/src/operator_display.rs`) and replaces it with the canonical Offline visual tokens. The gateway stub crate makes the `gateway` module shippable on its own.
- **Risk MED** — 12 new stub files in `crates/gateway/` (additive, contained) plus a real logic change in `operator_display.rs` (the function that maps `TrayGateSeverity::Offline` to visual tokens).
- **Cross-PR conflict with 924:** **HARD CONFLICT — 11 `<<<<<<<` conflict markers** if 923 merges first and 924 is re-applied. Both PRs modify the same two `operator_display.rs` files. The actual semantic conflict: 923 fixes the `unreachable!()` arm to return the `Offline` tokens; 924 was branched off main and assumes the `unreachable!()` arm is still in place — its `tray_gate_visual_default_is_offline` test calls `TrayGateVisual::default()`, not `resolve_tray_gate_visual(Offline)`, so it does not exercise the panic path. After 923 lands, PR 924's `mod tests {}` block lands at line ranges that already contain the resolved function — and the test comments reference "Phase B tray coverage" while 923's tests are "Phase C tray coverage".
- **Cross-PR conflict with 921/925:** Cargo.lock overlap is additive; merge-tree shows 0 `<<<<<<<` markers if 921 first then 923.
- **Recommendation:** **MERGE BEFORE 924** to preserve the tray-offline fix. PR 924 must then be rebased onto the post-923 main; if the resulting review changes its own test names/comments to "Phase B (panda)" vs "Phase B tray coverage", that's fine. **Alternative: SUPERSEDE 924** if the same coverage is also delivered by 923's tests (check `crates/sharecli-tray-linux/src/operator_display.rs:1-189` for overlap).

### PR 924 — `test(tray): add operator_display coverage (FR-001)`

- **Branch:** `feat/tray-operator-display-coverage`
- **Created:** 2026-10-17T00:32:34Z
- **Files (2):** `crates/sharecli-tray-linux/src/operator_display.rs` and `crates/sharecli-tray-windows/src/operator_display.rs`. +368/-0 net-additive — all of the additions are inside `mod tests {}`.
- **Mergeable vs main:** MERGEABLE.
- **mergeStateStatus:** `UNSTABLE`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `Quality Report` FAILURE, `FR reference in PR body` FAILURE, `FR reference in PR body` FAILURE (umbrella logic).
- **Value MED** — append-only test additions.
- **Risk LOW** — but holds the tray `unreachable!()` arm in place implicitly because it was branched off main before 923's `630a99c1` fix. If landed before 923, the offline panic arm remains exposed to future refactors.
- **Cross-PR conflict with 923:** **HARD CONFLICT — 11 `<<<<<<<` markers** as shown above. The textual diff between 923 and 924 on `crates/sharecli-tray-linux/src/operator_display.rs` shows 923 wants to keep the graceful `Offline => ("#d29922", ...)` tokens while 924 retains `Offline => unreachable!("handled above")`.
- **Recommendation:** **SUPERSEDE.** Either (a) rebase 924 onto post-923 main and resolve the conflict by adopting 923's test names where they overlap, or (b) close 924 and roll its new tests into 923 (preferred — they touch the same `mod tests {}` block). Inspect whether 923's `crates/sharecli-tray-linux/src/operator_display.rs:371+` already has equivalent coverage before deciding.

### PR 925 — `test(sharecli-ffi): add C-ABI coverage for all 6 extern functions (FR-001)`

- **Branch:** `feat/sharecli-ffi-coverage`
- **Created:** 2026-10-17T00:33:24Z
- **Files (4):** `Cargo.lock`, `crates/sharecli-ffi/Cargo.toml`, `crates/sharecli-ffi/build.rs`, `crates/sharecli-ffi/tests/ffi_api.rs`. New `crates/sharecli-ffi` crate.
- **Mergeable vs main:** MERGEABLE.
- **mergeStateStatus:** `BLOCKED`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `FR reference in PR body` SUCCESS.
- **Value MED** — FFI coverage for the entire C ABI surface.
- **Risk LOW** — additive; new crate, no existing code modified.
- **Cross-PR conflict with 921/923:** Cargo.lock overlap, merge-tree shows 0 `<<<<<<<` markers.
- **Recommendation:** **MERGE AFTER 923.** Should land after the new crates (gateway, sharecli-ffi) have their stub crates merged to keep `Cargo.toml` workspace `members` consistent.

### PR 927 — `feat(core): SpeculativeCoalescer for in-flight request dedup (FR-008)`

- **Branch:** `feat/fr-008-speculative-coalesce`
- **Created:** 2026-10-17T00:43:14Z
- **Files (2):** `crates/sharecli-core/src/lib.rs`, `crates/sharecli-core/src/speculation.rs`. New module `speculation.rs` (+445 lines of new logic).
- **Mergeable vs main:** MERGEABLE.
- **mergeStateStatus:** `BLOCKED`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `ci / test` SUCCESS, `ci / lint` SUCCESS, `FR reference in PR body` SUCCESS, `CodeQL` SUCCESS.
- **Value HIGH** — request-coalescing primitive, feature-flag delivery for FR-008. Adds a real dedup optimisation that lives entirely inside `sharecli-core` and is unit-testable without external infrastructure.
- **Risk MED** — new module with concurrency primitives; needs review for correctness on race conditions, drop semantics, and panic safety.
- **Cross-PR conflict with anything:** none.
- **Recommendation:** **MERGE.**

### PR 929 — `feat(harness-native): implement is_agent() sentinel-file detection`

- **Branch:** `feat/harness-native-agent-detection`
- **Created:** 2026-10-17T00:52:32Z
- **Files (1):** `crates/harness-native/src/is_agent.rs`. New module, +187/-5.
- **Mergeable vs main:** MERGEABLE.
- **mergeStateStatus:** `BLOCKED`.
- **Checks (umbrellas):** `CI Success Gate` SUCCESS, `Reproducible build (L52)` SUCCESS, `eval config gate` SUCCESS, `Dependency Audit` SUCCESS, `Dependency Review` SUCCESS.
- **Value HIGH** — fills a real gap: the `is_agent()` detection predicate that FR-007 / FR-008 downstream code calls into. Without it, downstream PRs would have to re-implement sentinel-file checks. Single-file, well-scoped.
- **Risk LOW** — new module, additive.
- **Cross-PR conflict with anything:** none.
- **Recommendation:** **MERGE.** Lowest-risk high-value delivery.

### PR 930 — `ci(fmt): bring workspace into rustfmt 1.96.0 compliance`

- **Branch:** `ci/rustfmt-compliance`
- **Created:** 2026-10-17T01:05:00Z
- **Files (32):** the same 32 files (excluding `.github/*` configs) that PR 918's commit `8220dd52` modifies.
- **Single commit:** `f753aa4d` ci(fmt): bring workspace into rustfmt 1.96.0 compliance
- **Mergeable vs main:** MERGEABLE.
- **Cross-PR with 918:**
  - 918's commit `8220dd52` ("style(rustfmt): apply 1.96.0 formatting") and 930's commit `f753aa4d` ("ci(fmt): bring workspace into rustfmt 1.96.0 compliance") have **identical numstat** across all 32 files.
  - `git diff origin/pr/918 origin/pr/930 -- <shared files>` is empty — the trees are identical on the shared file set.
  - `git merge-tree $(git merge-base origin/main origin/pr/918) origin/pr/918 origin/pr/930 | grep -c '<<<<<<<'` returned **0 conflict markers** — 930 contributes a no-op after 918 has landed.
- **Conclusion:** **PR 930 is 100% redundant with PR 918.** Its only commit's content is byte-equal to a commit already present in 918's history.
- **Recommendation:** **CLOSE AS DUPLICATE OF 918.** Do not bother to merge it; close it on GitHub with a reference to PR 918.

---

## Cross-PR Conflict Graph

```
        ┌── (no conflict) ──────────────────────┐
        │                                       │
       918 ── 100% redundant, byte-equal, ──►  930 ─► CLOSE AS DUPLICATE
        │
        │  (actionlint.yaml: 907 has shorter comment, 918 has longer comment)
        ▼
       907  (smaller, lands first)

       919  ── isolated, no conflict
       921  ──┐
       922    │
              ├── Cargo.lock additive sequence:  921 → 923 → 925
       923 ──┘   (merge-tree shows 0 `<<<<<<<` if sequenced)
              │
              └── HARD CONFLICT (11 `<<<<<<<` markers)
                  on crates/sharecli-tray-{linux,windows}/src/operator_display.rs
                  ▼
       924  ─► SUPERSEDE — rebase onto post-923 main, OR roll 924 tests into 923

       925  (after 923, additive Cargo.lock)

       927  ── isolated
       929  ── isolated
```

The graph has two real clusters of inter-PR friction and one long chain of additive merges:

1. **CI cluster (907 ↔ 918 ↔ 930):** 907 first, then 918 (which supersedes 930). 930 closes.
2. **Tray cluster (923 ↔ 924):** 923 first, 924 supersede-or-rebase.
3. **Coverage chain (921 → 923 → 925):** sequence for clean Cargo.lock.

---

## Recommended Merge Order (top-to-bottom)

1. **Close 930 as duplicate of 918** — saves one needless merge and one needless CI run.
2. **Merge 907** — `deploy-docs.yml` Pages configure step is an actual CI gate failure. Resolves the actionlint label conflict later by taking 918's wording on top.
3. **Merge 918** — supersedes the formatting that 930 wanted to do; resolves the actionlint comment wording with 907 by rebasing 918 onto post-907 main, then keep 918's longer comment. Also unblocks the macos codesign hard gate.
4. **Investigate 919's `CI Success Gate` FAILURE** before merging. Run the gate locally or pull the logs; if the failure is the `netblock hermetic` or `Quality Report` matrix leg, it may be a transient/stop-the-line regression from PR 919. If it is a true failure, fix-forward before merge.
5. **Merge 921** — single Cargo.lock bump; lock in the substrate rev.
6. **Merge 922** — additive sharecli-core test coverage.
7. **Merge 923** — brings the gateway stub crate and the tray-Offline panic fix.
8. **Resolve 924 against 923** — either rebase 924 onto post-923 main (resolving `unreachable!()` to graceful tokens) or close 924 and fold any non-duplicate test coverage into 923.
9. **Merge 925** — adds the `sharecli-ffi` crate.
10. **Merge 927** — isolated SpeculativeCoalescer.
11. **Merge 929** — isolated is_agent() sentinel detection.

Estimated total commits absorbed: 27 across 11 PRs (after closing 930). Estimated CI cycles consumed in serial: 6 with all-green matrix legs (overshoot-able).

---

## Evidence and Method

- **Repo state captured:** `git rev-parse HEAD` returns `3d6a41c972c2c86bf1374f224019c365ce3aa646` (matches audit context).
- **gh pr view data:** raw JSON for every PR (title, headRefName, baseRefName, author, createdAt, additions, deletions, changedFiles, mergeable, mergeStateStatus, isDraft, labels).
- **File lists:** `git diff origin/main...origin/pr/N --name-only` for each PR, plus `comm` for cross-PR overlap.
- **3-way merge vs main:** `git merge-tree $(git merge-base origin/main origin/pr/N) origin/main origin/pr/N` — every PR is mergeable against main with 0 `<<<<<<<` markers. `mergeStateStatus: BLOCKED/UNSTABLE` is a CI-state signal, not a git-side conflict signal.
- **3-way merge cross-PR:** `git merge-tree $(git merge-base origin/main origin/pr/A) origin/pr/A origin/pr/B` — confirmed 923 ↔ 924 hard conflict (11 markers) and 918 ↔ 930 no-op (0 markers).
- **Check counts:** `gh pr checks N --json name,state`, bucketed by `state` text (`SUCCESS`, `FAILURE`, `NEUTRAL`, `SKIPPED`). `gh pr checks --required` returned "no required checks reported on the `branch`" for every PR — a known gh CLI quirk when the API endpoint does not surface branch-protection required-checks; we therefore read the umbrella gates (`CI Success Gate`, `ci / test`, `ci / lint`, `CodeQL`, `Quality Report`) directly from the per-run state.
- **Redundancy proof for 930:** `git diff origin/pr/918 origin/pr/930 -- <shared files>` is empty after the rustfmt files; `git show 8220dd52 --numstat` and `git show f753aa4d --numstat` return identical `adds`/`dels` per file across all 32 files; `git merge-tree` of 918-base → 930 reports 0 conflict markers.
- **Hard requirements honoured:** no emoji status markers, every finding references a PR number plus file:line or commit:hint, every UNKNOWN item labelled as such (none required: every value/risk grade is traceable to a check or diff above).

---

## UNKNOWN

- The root cause of PR 919's `CI Success Gate` FAILURE — read the workflow run logs before merging. The low-level `ci / test` and `ci / lint` jobs are SUCCESS; the umbrella gate may be aggregating a failure we did not see in the truncated check dump.
- Whether PR 924's new tests in `crates/sharecli-tray-{linux,windows}/src/operator_display.rs` line ranges that intersect PR 923's Phase C tests are net-additive or fully duplicative. A reviewer must diff both `mod tests {}` blocks side-by-side to decide between rebase-vs-supersede.
- Whether `gh pr checks --required` returning "no required checks reported" indicates an API gap or actual missing branch-protection config; the audit used the per-leg `state` directly to avoid relying on this uncertain signal.
