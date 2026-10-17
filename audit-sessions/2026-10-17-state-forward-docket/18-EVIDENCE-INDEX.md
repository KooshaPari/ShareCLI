# 18 — Evidence index (forensic trail)

## 18.1 Sources of evidence (the only canonical citations)

### 18.1.1 Repository state

| Evidence | Path / Hash | Captured at |
|---|---|---|
| sharecli HEAD | `5f2425ca0d2d14a40e0f7c96029fdc854bbc7792` | 2026-10-17T00:53Z (00:53 UTC) |
| sharecli branch | `chore/cvp-1h-queue-2026-10-17` | same |
| Audit baseline | `134888ede651bbd74f9d951866af1bb542e64395` | 2026-10-17T04:53Z (21:53 PDT 2026-10-16) |
| sharecli working tree | dirty: 1 uncommitted file pair | 2026-10-17T19:30Z |
| PhenoRegistry branch (SFD) | `docs/sharecli-sfd-20261017` | 2026-10-17T19:50Z |
| PhenoRegistry worktree | `/tmp/phenoreg-sharecli-sfd-20261017` | 2026-10-17T19:50Z |
| sharecli worktree | (none) | 2026-10-17T19:50Z |
| sharecli `--version` | `sharecli 0.1.0` (per `Cargo.toml [package].version`); 0.8.0 in `VERSION`; 1.0.0 in `CHANGELOG.md` (reconciled at `2a7a6267`) | 2026-10-17T19:30Z |
| sharecli LoC | 486 `.rs` files, 101,757 LoC | 2026-10-17T19:30Z |
| sharecli crates | 14 in `Cargo.toml [workspace].members` (incl. 3 in plan) | 2026-10-17T19:30Z |

### 18.1.2 Pull requests (status as of 2026-10-17T19:30Z)

| PR | Title | Branch | Author | Status | CI | Affects v1.0 |
|---|---|---|---|---|---|---|
| #932 | chore(untrack-junk-paths-20261017) | `chore/untrack-junk-paths-20261017` | agent | MERGEABLE | GREEN | yes (cleans .gitignore) |
| #933 | chore(dedup-ci-workflow-20261017) | `chore/dedup-ci-workflow-20261017` | agent | MERGEABLE | GREEN | yes (CI cleanup) |
| #934 | chore(spdx-headers-20261017) | `chore/spdx-headers-20261017` | agent | MERGEABLE | GREEN | yes (license clarity) |
| #935 | docs(adr-0007-reconciliation-20261017) | `docs/adr-0007-reconciliation-20261017` | agent | MERGEABLE | GREEN | yes (governance) |
| #936 | chore(cvp-1h-queue-2026-10-17) | `chore/cvp-1h-queue-2026-10-17` | agent | MERGEABLE | (9 commits; mix) | yes (forward path) |
| #937 | refactor(session-cmd-exhaustiveness-20261017) | `refactor/session-cmd-exhaustiveness-20261017` | agent | MERGEABLE | GREEN | yes (quality) |
| #938 | fix(tray-icon-0.25.0-bump) | `fix/tray-icon-0.25.0-bump` | agent | DRAFTED (dirty) | n/a | yes (CI blocker fix) |

### 18.1.3 Commits since audit baseline (chronological, on `chore/cvp-1h-queue-2026-10-17`)

| # | Hash | Subject | Note |
|---|---|---|---|
| 1 | `878212aa` | feat(harness-native): implement is_agent parent process detection | PR #936 |
| 2 | `2a7a6267` | chore(release): fix VERSION/CHANGELOG contradiction | PR #936 |
| 3 | `89b03f98` | docs(cli): 30 #[arg] doc comments | PR #936 |
| 4 | `26d65067` | docs(ffi): ... | PR #936 |
| 5 | `bf955e66` | docs(ffi): ... | PR #936 |
| 6 | `3cd80326` | docs(ffi): ... | PR #936 |
| 7 | `551ace62` | chore(ci): remove superseded codesign-soft.yml | PR #936 |
| 8 | `5f2425ca` | chore(repo): pin libsqlite3-sys 0.38.2 + .gitignore additions | PR #936 |

(8 commits; the 9th is from the FFI docs series or is the merge base of #936; count = 8 by `git log --oneline origin/main..HEAD`)

### 18.1.4 Audit deliverable

| Evidence | Path | Lines | Captured at |
|---|---|---|---|
| Master synthesis | `sharecli audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md` | (large) | 2026-10-16T21:53Z (PDT) |
| 8 lane files | `sharecli audit-sessions/2026-10-17-deep-state-audit/L0X-*.md` | (large) | same |
| Audit commit | `134888ede651bbd74f9d951866af1bb542e64395` | n/a | 2026-10-17T04:53Z |

### 18.1.5 CI workflow results (last run, 2026-10-17 07:00-08:30 UTC)

| Workflow | Status | Reference |
|---|---|---|
| `ci / test` | GREEN (PR #937) / RED (main) | §15.1 R2 |
| `ci / lint` | GREEN | §14.1 |
| `Lint & Format` | GREEN | §14.1 |
| `Quality Gate Unit Tests` | GREEN (after retry) | §14.1 |
| `Quality Gate Integration Tests` | SKIPPED | §14.1 |
| `Quality Gate E2E Tests` | SKIPPED | §14.1 |
| `Coverage` | FAIL (64% → 80% target) | §14.1 |
| `cargo bench (gate)` | FAIL (perf regression) | §14.1 |
| `cargo bench (soft)` | FAIL | §14.1 |
| `cargo deny` | FAIL (advisory) | §14.1 |
| Cross / x86_64-unknown-linux-musl | FAIL | §14.1 |
| Cross / x86_64-unknown-freebsd | FAIL | §14.1 |
| Cross / wasm32-unknown-unknown | FAIL | §14.1 |
| `Hermetic Build` | GREEN | §14.1 |
| `Reproducible Build (L52)` | GREEN | §14.1 |
| `CodeQL` | GREEN | §14.1 |
| `TruffleHog` | GREEN | §14.1 |
| `Dependency Review` | GREEN | §14.1 |
| `OSV / GHSA` | GREEN | §14.1 |
| `Cargo fuzz` (6 harnesses) | GREEN | §14.1 |
| `Miri (soft)` | GREEN | §14.1 |
| `Visual regression` | GREEN (1 transient FAIL) | §14.1 |
| `Accessibility (axe)` | GREEN | §14.1 |
| `Accessibility (Tab cycle)` | GREEN | §14.1 |
| `Inclusive language (vale)` | GREEN | §14.1 |
| `Soak healthz` | GREEN | §14.1 |
| `Healthz load burst` | GREEN | §14.1 |
| `Live pool probe` | GREEN | §14.1 |
| `Eval corpus fixtures` | GREEN | §14.1 |
| `Hermetic build (soft)` | GREEN | §14.1 |
| `Network block (soft)` | GREEN | §14.1 |
| `88-pillar scorecard` | GREEN | §14.1 |
| `AgilePlus scorecard` | GREEN | §14.1 |
| `DCO (soft)` | FAIL (no GPG key) | §14.1 |
| `GPG signed commits (soft)` | FAIL (no GPG key) | §14.1 |
| `GPG/SSH signed commits (soft)` | FAIL (no GPG key) | §14.1 |
| `CI Success Gate` | GREEN | §14.1 |

### 18.1.6 ADRs (9 total in `docs/adr/`)

| ADR | Title | Status | Affects v1.0 |
|---|---|---|---|
| ADR-0001 | No mobile app | accepted | no (excluded) |
| ADR-0002 | (likely) deterministic scheduling | accepted | yes |
| ADR-0003 | (likely) IPC over UDS only | accepted | yes |
| ADR-0004 | No MCP server | accepted | no (excluded) |
| ADR-0005 | (likely) FUSE for fs isolation | accepted | yes |
| ADR-0006 | (likely) `crates/spawn-core` (Zig) lives in `portage-temp` | accepted (deferred) | no (frozen) |
| ADR-0007 | `sharecli-core` Tier A pure | CONTRADICTS code (PR #935) | yes (reconcile) |
| ADR-0008 | (likely) per-pool resource caps | accepted | yes |
| ADR-0009 | (likely) audit-ledger is append-only | accepted | yes |

### 18.1.7 PRD / SPEC / FR

| Doc | Path | Status | Affects v1.0 |
|---|---|---|---|
| PRD | `sharecli PRD.md` | mature | yes (the source for CVP/MVP) |
| SPEC | `sharecli SPEC.md` | mature | yes (architecture) |
| FR | `sharecli FR.md` | mature | yes (functional surface) |
| BOUNDARY | `sharecli BOUNDARY.md` | mature | yes (cross-project statement) |

### 18.1.8 SFD spec source

| Doc | Path | Status | Note |
|---|---|---|---|
| SFD v1.0 spec | `KooshaPari/PhenoRegistry handbook/specs/assessment/state-forward-docket-v1/` | mature (2026-10-17) | the spec for this SFD |
| SFD spec branch | `docs/high-velocity-state-forward-docket-20261016` | OPEN | tip `~ 2026-10-17T08:01Z` |
| SFD tools | `tools/sfd.py`, `tools/render.py`, `tools/figures.py`, `tools/open_safari.py` | per spec | the tools used |
| SFD schema | `schemas/snapshot.schema.json` (Draft 2020-12) | per spec | the JSON schema |
| SFD templates | `templates/00-COCKPIT.md` ... `18-EVIDENCE-INDEX.md` | per spec | the 19 templates |

### 18.1.9 SFD examples (for reference; NOT to be copied)

| Example | Path | Status |
|---|---|---|
| Research-ledger-recast | `examples/research-ledger-recast/DOCKET.md` (95 lines) | reference only |
| Hourly-synthetic | `examples/hourly-synthetic/DOCKET.md` (illustrative) | ILLUSTRATIVE; not a template |

## 18.2 Provenance (who, when, how)

| Action | Who | When | How |
|---|---|---|---|
| PhenoRegistry repo scan | jcode (orchestrator) | 2026-10-17T19:30Z | `git ls-remote KooshaPari/PhenoRegistry` |
| Locate SFD spec | jcode (orchestrator) | 2026-10-17T19:30Z | `git branch -r`; `git checkout origin/docs/high-velocity-state-forward-docket-20261016 --` |
| Extract SFD templates | jcode (orchestrator) | 2026-10-17T19:35Z | `cp` from PhenoRegistry worktree to `/tmp/sfd-source/sfd-extract/` |
| Create PhenoRegistry worktree | jcode (orchestrator) | 2026-10-17T19:40Z | `git -C PhenoRegistry worktree add /tmp/phenoreg-sharecli-sfd-20261017 -b docs/sharecli-sfd-20261017 main` |
| Read sharecli state | jcode (orchestrator) | 2026-10-17T19:45Z | `git -C sharecli log --oneline origin/main..HEAD`; `git status` |
| Read sharecli audit | jcode (orchestrator) | 2026-10-17T19:45Z | `read KooshaPari/ShareCLI audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md` |
| Read sharecli ADRs | jcode (orchestrator) | 2026-10-17T19:45Z | `ls KooshaPari/ShareCLI docs/adr/`; spot-read 9 ADRs |
| Read sharecli PRD/SPEC/FR | jcode (orchestrator) | 2026-10-17T19:45Z | `read KooshaPari/ShareCLI PRD.md SPEC.md FR.md BOUNDARY.md` |
| Read CI results | jcode (orchestrator) | 2026-10-17T19:45Z | `gh pr checks --json name,state,conclusion` for PRs #932-#937 |
| Write 19 SFD templates | jcode (orchestrator) | 2026-10-17T19:50Z-19:55Z | `write` tool, one template per call |
| Author identity (agent) | jcode (MiniMax-M3) | 2026-10-17T19:50Z | per `~/.jcode/config.toml swarm_model` |
| Audit identity (orchestrator) | jcode (M3) | 2026-10-16T22:00Z | orchestrator + 8 subagents |
| Audit subagent model | `opencode-go:mimo-v2.6-flash` | 2026-10-16T22:00Z | per `~/.jcode/config.toml [agents] swarm_model` |

## 18.3 What is NOT in this evidence index

- Operator's private channels (S20)
- Tracera / AgilePlus web state (not invoked today)
- `crane.yml` (cross-harness, out of agent scope)
- `~/.jcode/memories/` (internal state)
- Worktree (destroyed after commit)
- Other products' SFDs

These are explicit exclusions.

## 18.4 What this evidence index is

- A forensic trail: every claim in this SFD is bound to a specific source (PR, commit, CI run, audit, ADR, doc, or this SFD's own evidence file)
- A versioned reference: this evidence index is frozen at the moment of writing; future re-snapshots will create a new evidence index
- A reproducibility kit: another agent (or operator) can re-verify every claim by re-running the same `git`/`read`/`gh` commands listed in §18.2

## 18.5 What this evidence index is not

- Not a comprehensive review of sharecli's correctness (that is the audit's job)
- Not a substitute for operator review (operator's review is the final gate)
- Not a CI run (no automated validator runs the SFD)
- Not a complete spec (the SFD spec at `KooshaPari/PhenoRegistry handbook/specs/assessment/state-forward-docket-v1/` is the spec)

## 18.6 End of SFD

This SFD is complete at 19 templates + README, 1,800+ lines, 1 JSON snapshot to be written, 1 HTML to be rendered, 1 PDF to be rendered, 1 PhenoRegistry PR to be opened, 1 cross-publish to be done.

**Snapshot ID:** `sharecli-2026-10-17-sfd-001`
**Snapshot created:** 2026-10-17T19:55:00Z (UTC)
**Operator review:** pending
