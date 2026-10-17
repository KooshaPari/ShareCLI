# 10 — Research, prior art and architectural decisions

## 10.1 Prior art and inspiration (recovered from archaeology)

| Source | What we learned | What we rejected | Evidence |
|---|---|---|---|
| `thegent-sharecli` (defunct, pre-2026-04) | "shared CLI process manager" framing; absorb into `thegent` | per-vendor agent wrapping | `BOUNDARY.md`, `PRD.md` §Origin |
| `claude` CLI (Anthropic, vendor) | benchmark, agent UX patterns | n/a | PRD §Why-not |
| `codex` CLI (OpenAI) | benchmark, agent UX patterns | n/a | PRD §Why-not |
| `thegent` (sibling CLI) | shareable process model, IPC, signal-safe control | n/a | `BOUNDARY.md` |
| `tmux` (process multiplexer) | session model, persistence | tmux's text-only UX; sharecli supervises agents, not text | sharecli design §2.1 |
| `systemd --user` | unit lifecycle, cgroup, dependency | sharecli does not manage OS services; agents are user-level | sharecli design §2.2 |
| `cgroups v2` | resource cap enforcement | sharecli uses `/proc` + rlimit, not cgroups (v1.1 may add) | audit §2 |
| `FUSE` | intercept at the file system boundary | WinFSP for Windows; works | FUSE + WinFSP |
| `Kubernetes` (fleet mgmt) | declarative intent, controllers, reconciliation | sharecli is single-host; cross-host not in scope | sharecli design §2.3 |
| `Hypervisor` terminology | supervisor pattern | sharecli supervises processes, not VMs | PRD §Origin thesis |
| `libfuse` examples (passthrough, intercept) | mount + intercept pattern | direct passthrough (sharecli-fuse adds meter + forward) | sharecli-fuse/src/backend.rs |
| `dtach` / `abduco` | detach/reattach | sharecli's session model is different (multi-agent, mesh) | sharecli design §2.4 |
| `HyperDX` / `Phoenix LiveView` | operator dashboard | sharecli dashboard is local-only; no remote | sharecli-dashboard |
| `coz` (causal profiler) | performance instrumentation | sharecli's `monitoring.rs` stub; future integration | `crates/sharecli-core/src/monitoring.rs` |
| `pyroscope` (continuous profiler) | continuous flamegraph | `pyroscope_stub.rs` stub; future integration | `crates/sharecli-core/src/pyroscope_stub.rs` |
| `Harbor` (PhenoRegistry product) | eval orchestration | out of scope; lives in `phenotype-tooling/crates/benchora` | `PRD.md` |
| `SWE-bench` / `Terminal-Bench` | evaluation harnesses | out of scope | `PRD.md` |
| `lock-wait-cache` (research paper, decade-old) | coalesce redundant work | sharecli implements the pattern with a Rust `CoalesceCache` | `crates/sharecli-ipc/src/cache_key.rs` |
| `SpeculativeCoalescer` (PhenoShared substrate) | optimistic coalesce | implements FR-008 | PR #927 |
| ADR-0007 (ports-adapters-hexagonal) | architecture intent | contradicts measured behavior; PR #935 reconciles | `docs/adr/0007-*.md` + `COMPREHENSIVE_AUDIT_SCORECARD.md` |
| ADR-0008 (session ledger) | atomic txn admission across global nested ledger | implemented | `docs/adr/0008-*.md`, `crates/sharecli-session/src/ledger.rs` |
| ADR-0009 (airlock-convergence-contract-capture) | FFI C-ABI stable contract | implemented | `docs/adr/0009-*.md` |
| `KooshaPari/PhenoShared` substrate | shared types, evtoolkit, etc. | PR #921 bumps substrate rev | PR #921 |
| `KooshaPari/PhenoRegistry` | org-level portfolio + intent | this SFD writes into it | `KooshaPari/PhenoRegistry` `main` |
| `KooshaPari/Tracera` (Paperclip) | org chart | sharecli not yet added; operator adds | http://localhost:3180 |
| `hermes-imports` skill (PhenoRegistry) | hermes workflows → sanitized skills | sharecli has no hermes workflows yet | `KooshaPari/PhenoRegistry` `handbook/skills/hermes-imports/SKILL.md` |

## 10.2 Decisions logged (this snapshot)

| Decision | Status | Authority | Rationale | Evidence |
|---|---|---|---|---|
| ADR-0007 §1 updated to match current code | DRAFTED (PR #935) | sharecli architect | code wins | PR #935 |
| `crates/gateway-tools` added to workspace | MERGEABLE (PR #923) | sharecli engineer | unblocks workspace build | PR #923 |
| `crates/spawn-core` placement | DEFERRED | sharecli architect | on disk, not in workspace; ADR-0006 portage-temp | audit §4.4 F2 |
| `monitoring.rs` is honest stub | ACCEPTED | sharecli architect | dead code documented | audit §3 Theme B |
| 14 files > 1000 LoC | ACCEPTED | sharecli architect | `BOUNDARY.md` rationale | `BOUNDARY.md` |
| `sharecli-runtime` extraction | DEFERRED | sharecli architect | v1.1; `lib-sprawl-plan.md` Phase 4 | `lib-sprawl-plan.md` |
| tray-icon 0.25.0 backend broken | FIX (PR #938) | sharecli engineer | one-line `features = ["libappindicator"]` | PR #938 |
| is_agent implementation | DONE (commit `878212aa` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | replaces silent exit | `878212aa` |
| FFI docs lift | DONE (commits `26d65067` `bf955e66` `3cd80326` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | 6 pub extern C fn have `///` | PR #936 |
| 30 `#[arg]` doc comments | DONE (commit `89b03f98` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | better `--help` output | PR #936 |
| CHANGELOG/VERSION reconcile | DONE (commit `2a7a6267` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | was contradicted | PR #936 |
| codesign-soft.yml removal | DONE (commit `551ace62` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | superseded | PR #936 |
| libsqlite3-sys pin | DONE (commit `5f2425ca` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | reproducibility | PR #936 |
| README hygiene | DONE (commit `f1479c05` on `chore/cvp-1h-queue-2026-10-17`) | sharecli engineer | 3 dead URLs | PR #936 |
| 5 `unreachable!()` arms consolidated to 1 | DONE (PR #937) | sharecli engineer | defensive; or-pattern is clearer | PR #937 |
| SPDX headers on all .rs | MERGEABLE (PR #934) | sharecli engineer | Apache-2.0 OR MIT clarity | PR #934 |
| Untrack 13+ junk paths | MERGEABLE (PR #932) | sharecli engineer | 16k lines of artifacts | PR #932 |
| ci.yml vs ci-gate.yml dedup | MERGEABLE (PR #933) | sharecli engineer | 388 LoC legacy | PR #933 |
| 60 sharecli-core integration tests | MERGEABLE (PR #922) | sharecli engineer | coverage lift | PR #922 |
| FFI C-ABI coverage | MERGEABLE (PR #925) | sharecli engineer | FFI-AC lift | PR #925 |
| Tray coverage | MERGEABLE (PR #924) | sharecli engineer | libappindicator/ksni | PR #924 |
| SpeculativeCoalescer | MERGEABLE (PR #927) | sharecli engineer | FR-008 | PR #927 |
| PhenoShared substrate rev bump | MERGEABLE (PR #921) | sharecli engineer | ea27186 | PR #921 |
| FUSE poison-safe lock recovery | MERGEABLE (PR #919) | sharecli engineer | safer FUSE | PR #919 |
| fail-closed when pooled execution unavailable | MERGEABLE (PR #938) | sharecli engineer | no silent spin | PR #938 |
| This SFD exists | WRITTEN | this SFD | audit + 9 commits forward path need canonical record | this SFD |

## 10.3 Open / pending decisions (operator-owned)

| Decision | Options | Recommendation | Operator action |
|---|---|---|---|
| Push `chore/cvp-1h-queue-2026-10-17` as one squash PR or split? | one squash (9 commits) / split (is_agent, FFI docs, CLI docs, gitignore, codesign-soft, libsqlite) | split is cleaner; cost is 5 min of `git reset` work | operator decides |
| Stand PR #938 alone or fold into #936? | alone (10 min) / fold (5 min) | fold into #936 keeps the 1h-queue branch atomic | operator decides |
| Merge PRs in which order? | by size (smallest first) / by Cargo.lock cluster (PR #921 first) / by feature | `#921` (cargo lock) → `#937` (smallest) → `#938` (or fold) → `#932/933/934/935/936/922/924/925/927/919` | operator decides |
| Use Tracera for sharecli? | yes / later / never | yes; sharecli is in CVP-1.0 | operator adds |
| Use AgilePlus for sharecli? | yes / later / never | yes; `WORK_DAG.md` is rich | seed from `WORK_DAG.md` |
| Cross-publish this SFD to sharecli `audit-sessions/2026-10-17-state-forward-docket/`? | yes / not yet | yes; MDSET policy | agent does after PhenoRegistry PR opens |
| Update `docs/intent/`, `docs/adr/` in sharecli to reference this SFD? | yes / later | yes; small | agent does after PR #938 merges |
| Bump tray-icon to 0.27.x instead of adding `libappindicator` feature? | feature fix (current) / version bump | feature fix is the smallest diff; 0.27.x is a future cleanup | operator decides |
| When to start v1.1 work (deferred items)? | after v1.0 ships / parallel / never | after v1.0 ships; sequence matters | operator decides |
| Open the GitHub Release for v1.0? | yes / no / later | once all gates green | operator decides |

## 10.4 Key references (re-read in this SFD)

| Doc | Path | Status | When re-read |
|---|---|---|---|
| `PRD.md` | sharecli/PRD.md | canonical | 2026-10-17 |
| `SPEC.md` | sharecli/SPEC.md | canonical | 2026-10-16 |
| `FR.md` | sharecli/docs/specs/FR.md | canonical | 2026-10-16 |
| `BOUNDARY.md` | sharecli/BOUNDARY.md | canonical | 2026-10-16 |
| `COMPREHENSIVE_AUDIT_SCORECARD.md` | sharecli/COMPREHENSIVE_AUDIT_SCORECARD.md | canonical | 2026-10-16 |
| ADR-0001..ADR-0009 | sharecli/docs/adr/ | canonical | 2026-10-16 |
| `WORK_DAG.md` | sharecli/WORK_DAG.md | rich; not yet in AgilePlus | 2026-10-16 |
| `lib-sprawl-plan.md` | sharecli/lib-sprawl-plan.md | plan; Phase 4 deferred | 2026-10-16 |
| `VERSION` | sharecli/VERSION:1 | reconciled at 0.8.0 | 2026-10-17 |
| `CHANGELOG.md` | sharecli/CHANGELOG.md | 1.0.0 release tag | 2026-10-17 |
| audit deliverable | sharecli/audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md | canonical for this SFD's evidence | 2026-10-17 |
| SFD spec | PhenoRegistry/handbook/specs/assessment/state-forward-docket-v1/ | canonical | 2026-10-17 |
| SFD ADOPTION | PhenoRegistry/handbook/specs/assessment/state-forward-docket-v1/ADOPTION.md | canonical | 2026-10-17 |
| SFD CONTRACT | PhenoRegistry/handbook/specs/assessment/state-forward-docket-v1/CONTRACT.md | canonical | 2026-10-17 |
| SFD examples | PhenoRegistry/handbook/specs/assessment/state-forward-docket-v1/examples/ | illustrative | 2026-10-17 |
| SFD tools | PhenoRegistry/handbook/specs/assessment/state-forward-docket-v1/tools/ | canonical | 2026-10-17 |
| SFD schema | PhenoRegistry/handbook/specs/assessment/state-forward-docket-v1/schemas/snapshot.schema.json | canonical | 2026-10-17 |
| Operator panel screenshots | sharecli/screenshots/, sharecli/audit-sessions/2026-10-17-deep-state-audit/figures/ | illustrative | 2026-10-16 |
