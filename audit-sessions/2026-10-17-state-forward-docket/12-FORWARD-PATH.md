# 12 — Forward path: 30 / 60 / 90-day plan and merge train

## 12.1 Merge train (immediate, ~30 min)

Order matters: small first, Cargo.lock cluster first, then the rest.

| Order | PR | Branch | Files / LoC | Why this order | Critical? |
|---|---|---|---|---|---|
| 1 | **#921** | `chore/phenoshared-substrate-bump-20261017` | Cargo.lock cluster | Cargo.lock changes will conflict with every other PR; merge first to baseline | yes |
| 2 | **#937** | `refactor/session-cmd-exhaustiveness-20261017` | 1 file, +10/-9 | smallest PR; no conflict; "land first, land safely" | yes |
| 3 | **#938** | `fix/run-fail-closed-20261017` | 1 file, +5/-3 (or fold into #936) | fixes a fail-closed bug; independent | yes |
| 4 | **#932** | `chore/untrack-junk-paths-20261017` | 13 files, -16137 lines | untracks artifacts; no conflict | yes |
| 5 | **#933** | `chore/dedup-ci-workflow-20261017` | 2 files, -388 LoC legacy `ci.yml`, +5 LoC `ci-gate.yml` | dedup; no conflict | yes |
| 6 | **#934** | `chore/spdx-headers-20261017` | 456 files, +456 LoC | adds SPDX headers; no conflict | yes |
| 7 | **#935** | `docs/adr-0007-reconciliation-20261017` | 1 file, +86 LoC | reconciles ADR-0007 §1 | yes |
| 8 | **#936** | `chore/cvp-1h-queue-2026-10-17` | 9 commits; the operator decides whether to split or squash | carries is_agent + FFI docs + CLI docs + gitignore + codesign-soft + libsqlite | yes |
| 9 | **#919** | `refactor/fuse-poison-safe-20261017` | 1-2 files | poison-safe lock | no |
| 10 | **#922** | `test/core-txn-coverage-20261017` | adds 60 sharecli-core integration tests | coverage lift | no |
| 11 | **#923** | `feat/gateway-tools-20261017` | adds `crates/gateway-tools` to workspace | unblocks workspace build | no |
| 12 | **#924** | `test/tray-coverage-20261017` | tray tests | coverage lift | no |
| 13 | **#925** | `test/ffi-cabi-coverage-20261017` | FFI tests | coverage lift | no |
| 14 | **#927** | `feat/speculative-coalescer-20261017` | SpeculativeCoalescer | FR-008 | yes |

**Total:** 14 squash-merge operations, ~30 min wall-clock, all MERGEABLE branch-wise. The `cargo-deny` advisory on #921 is the only carry-forward risk.

## 12.2 30-day plan (post-merge)

| Day | Milestone | Owner | Deliverable | Cost |
|---|---|---|---|---|
| 1-2 | All 14 PRs merged; main HEAD green on `ci / test` | operator | green main; 19/20 engineering leaves, 5/5 journeys, 4/4 critical engineering | 30 min |
| 3-4 | Operator records first beneficiary session (`docs/eval/2026-10-XX-operator-cold-start/`) | operator | R-eval-session PASS; MVP gate unblocked | 1 human-hour |
| 5-7 | `cargo-deny` advisory allowlist updated (deny.toml or dep upgrade) | sharecli engineer | G-cargo-deny PASS | 30 min |
| 7-10 | Lift `sharecli-fleet` facade (audit §4.8 MEDIUM) | sharecli engineer | reduces 110 pub items to ~30 | 1 day |
| 10-12 | `cliff.toml` template fix for `commit.link` | sharecli engineer | CHANGELOG.md regenerates with valid links | 30 min |
| 12-15 | `sharecli-runtime` extraction (lib-sprawl-plan.md Phase 4) | sharecli engineer | crates/sharecli-runtime added; src/runtime.rs removed | 3 days |
| 15-20 | Cross-platform tray-icon investigation (musl/freebsd/wasm) | sharecli engineer | may need bump to 0.27.x | 1 day |
| 20-25 | First homebrew formula (operator-side) | operator | `KooshaPari/homebrew-sharecli`; not blocking v1.0 | 1 day |
| 25-30 | Operator manual `docs/manual.md` | sharecli engineer | operator experience improves | 1 day |
| 30 | v1.0 cut candidate | operator | v1.0.0-rc.1; gate-list green | depends on above |

## 12.3 60-day plan

| Day | Milestone | Owner | Deliverable | Cost |
|---|---|---|---|---|
| 30-40 | v1.0.0 release (operator signs off; SignPath Foundation codesign) | operator | GitHub Release v1.0.0 | 1 day |
| 40-50 | `sharecli-doctor` CLI (FR-005 session 2) | sharecli engineer | one-stop diagnostic | 1 week |
| 50-60 | `sharecli-log` CLI (FR-005 session 3) | sharecli engineer | per-agent log | 1 week |
| 60 | v1.0.1 patch (operator-driven bug fixes) | operator | depends on operator feedback | 1 day |

## 12.4 90-day plan (v1.1 horizon)

| Day | Milestone | Owner | Deliverable | Cost |
|---|---|---|---|---|
| 60-75 | v1.1.0: `sharecli mesh list`, FFI embedder use cases (FR-010), `crates/spawn-core` to portage-temp or stay | sharecli engineer | v1.1.0 | 2 weeks |
| 75-85 | v1.1.1: rustdoc coverage 64% → 80%, `crates/gateway-tools` to separate product if scope demands | sharecli engineer | v1.1.1 | 1 week |
| 85-90 | v1.1.2: operator feedback cycle | operator | v1.1.2 | 1 day |

## 12.5 AgilePlus / PhenoRegistry / Tracera / sharecli cross-publish

| Day | Milestone | Owner | Deliverable | Cost |
|---|---|---|---|---|
| 1 | This SFD merged in PhenoRegistry | agent (this docket) | PR in PhenoRegistry | 5 min |
| 1-2 | Cross-publish this SFD to sharecli `audit-sessions/2026-10-17-state-forward-docket/` | agent | mirror copy in sharecli | 15 min |
| 1-2 | Update PhenoRegistry `audits/sharecli/2026-10-17-sfd-001/` | agent | one-file pointer | 5 min |
| 2-3 | Update sharecli `docs/intent/`, `docs/adr/` "see also" | agent | 9 file edits | 15 min |
| 5-7 | Seed AgilePlus sharecli backlog from `WORK_DAG.md` | agent | 1 hour seed | 1 hour |
| 7-10 | Operator adds sharecli to Tracera | operator | product card | 1 hour |

## 12.6 Dependency graph (next 30 days)

```
[Merge train     ] ******* 100%  14 PRs MERGEABLE, ~30 min
[v1.0-cut        ] ..x.... 0%    after main green
[Operator session] ..x.... 0%    after v1.0-cut
[v1.0.0 release  ] ..x.... 0%    after session + operator signoff
[Cross-publish   ] ..x.... 0%    SFD + audits + intent
[Homebrew        ] ..x.... 0%    operator-side
[v1.1.0          ] ..x.... 0%    2-week effort
```

## 12.7 Risk-adjusted ordering (alt)

If `cargo-deny` advisory blocks #921 merge:

1. Land #937 (smallest, independent)
2. Land #932/933/934/935/936/922/924/925/927/919 (no Cargo.lock cluster)
3. Land #938 (or fold into #936)
4. Address `cargo-deny` (allowlist or dep upgrade)
5. Land #921

**This alt sequence costs ~5 min extra** but breaks the cargo-lock first principle; only use if #921 is genuinely blocked.

## 12.8 What is **not** in the 30/60/90 plan

- Mobile app (ADR-0001: never)
- MCP server (ADR-0004: never)
- Harbor / SWE-bench (PRD: never)
- Per-vendor agent wrapping (PRD thesis: never)
- Plugin system (PRD v1.0: never)
- Cross-host fleet (PRD: never; single-host only)
- Prebuilt binary (operator-side; not for v1.0)
- OWASP Top 10 audit (operator-side; not for v1.0)

These are explicit no-go decisions. Adding any of them in v1.1 or later requires a new ADR with operator approval.
