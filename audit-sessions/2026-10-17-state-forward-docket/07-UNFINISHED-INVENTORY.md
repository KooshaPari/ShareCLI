# 07 — Unfinished work and outstanding debts

## 7.1 Open, unfinished, in-flight items

| Item | Source / authority | Type | Why open | Cost-to-finish | Notes |
|---|---|---|---|---|---|
| **Merge train #932 → #937 + #921 (first)** | this SFD §01, §12 | merge | all PRs MERGEABLE; `#921` first because it carries the Cargo.lock cluster; then smallest-conflict | ~30 minutes (8 squash merges) | the `cargo-deny` advisory is on PR #921, so its failure may carry forward |
| **PR #938 (tray-icon fix)** | this SFD §15, §12 | merge | `crates/sharecli-tray-windows` tray-icon 0.25.0 backend broken; one-line fix; can fold into #936 or stand as own | 10 min for own PR; 5 min for fold | alternative: bump tray-icon to 0.27.x; needs test of libappindicator + ksni backends |
| **Real-user-validation session** | this SFD §02.5, §04.4, §06.3 G-real-user-validation | missing evidence | no recorded beneficiary session exists; required for MVP gate | 1 human-hour + session write-up | operator unblocked, no automation path |
| **`cargo-deny` advisory allowlist** | this SFD §06.3 G-cargo-deny | follow-up | the deny.toml is stale; need to add advisory identifier or upgrade `tray-icon` to a patched version | 30 min | PRs #896, #909 in flight; audit §6.4 may have detail |
| **AgilePlus sharecli backlog** | this SFD §12.5, §15 | missing artifact | `WORK_DAG.md` exists but is not linked to AgilePlus | 1 hour to seed | idempotent; rerunnable |
| **Tracera sharecli product card** | this SFD §18, §15 | missing artifact | sharecli not yet in Tracera; requires human approval per `KooshaPari/Tracera` policy | depends on operator | not blocking any SFD gate |
| **`crane.yml` operator-decisions** | (not in this SFD scope) | review needed | cross-harness config; operator owns | operator-side | out of agent scope |
| **Cross-publish to sharecli `audit-sessions/2026-10-17-state-forward-docket/`** | this SFD §00 §16 | follow-up | PhenoRegistry gets the canonical; sharecli gets the cross-published copy | 15 min | after PhenoRegistry commit |
| **Update PhenoRegistry `audits/sharecli/2026-10-17-sfd-001/`** | this SFD §12 | follow-up | pointer to this SFD; one-file addition | 5 min | after PhenoRegistry commit |
| **Update `docs/intent/`, `docs/adr/` in sharecli** | this SFD §13 | follow-up | add pointer to this SFD from each artifact's "see also" | 15 min | after sharecli PR #938 merges |
| **SpeculativeCoalescer (PR #927) merge** | this SFD §01 §12 | merge | lands FR-008 leaf; the PR is MERGEABLE but not yet merged | 5 min squash | independent of tray-icon fix |
| **PhenoShared substrate rev bump (PR #921) merge** | this SFD §01 | merge | Cargo.lock cluster; needs to land first to avoid merge conflict in subsequent PRs | 5 min squash | cargo-deny advisory still FAIL on this PR |
| **`monitoring.rs` / `pyroscope_stub.rs` extraction** | this SFD §02.6 | deferred | honest stubs accepted; not blocking | 1 hour | not for v1.0 |
| **`sharecli-runtime` extraction** | this SFD §02.6 | deferred | `lib-sprawl-plan.md` Phase 4; v1.1 | 2-3 hours | not for v1.0 |
| **`crates/spawn-core` (Zig) build wiring** | this SFD §02.6 | deferred | on disk, not in workspace; ADR-0006 placement | 1 day (operator: port to portage-temp) | not for v1.0 |
| **`crates/gateway-tools` workspace membership** | this SFD §01 §12 | in flight | PR #923 MERGEABLE; needs merge | 5 min | independent |
| **Rustdoc coverage lift to 80% on hot crates** | this SFD §15 | slow | 64% today; rustdoc CI runs on every PR | passive | via incremental PRs |
| **FR.md Phase 3 → Phase 4 transition** | this SFD §02 §13 | follow-up | Phase 3 covers FR-001..FR-005; Phase 4 introduces FR-006..FR-012 (planned but not numbered) | 1 hour doc | independent |
| **`cliff.toml` template fix for `commit.link`** | this SFD §12.4 | follow-up | `commit.link` is not a standard git-cliff var; needs custom template | 30 min | independent |
| **Operator review of PR #935 (ADR-0007 reconciliation)** | this SFD §02.2, §13 | merge | the ADR-0007 contradiction is resolved in code; the doc is updated; operator's call to merge | depends on operator | not blocking |
| **Operator review of PR #937 (session exhaustive)** | this SFD §01, §12 | merge | smallest PR; should land first | 5 min squash | not blocking |
| **Operator review of PR #936 (cvp 1h queue)** | this SFD §01, §12 | merge | 9 commits; carries is_agent + FFI docs + CLI docs + gitignore + codesign-soft removal + libsqlite pin | 10 min squash (with is_agent) | the 9-commit batch may want a split |
| **Operator review of `thegent-sharecli` absorption** | this SFD §02.6 | cross-repo | (not in this SFD scope) | depends on operator | out of agent scope |
| **SignPath Foundation codesign release** | this SFD §05.6 | not for v1.0 dry-run | PR #918 establishes codesign.yml; v1.0 release would invoke | depends on operator | out of agent scope |
| **Homebrew formula `homebrew-sharecli`** | this SFD §05.6 | not for v1.0 | no Homebrew tap exists | 1 hour fork + 1 day review | not for v1.0 |
| **`cargo install` instructions polish** | this SFD §12.4 | follow-up | README says "cargo install" but no prebuilt binary; install path is from source | 15 min doc | not for v1.0 |

## 7.2 Known bugs, known unknowns, and risks

| Item | Severity | Why | Mitigation | Owner |
|---|---|---|---|---|
| **tray-icon 0.25.0 backend broken on Linux** | BLOCKER | `pub(crate) use self::platform::*;` doesn't re-export | PR #938 (or fold into #936); bump to 0.27.x as alt | sharecli engineer |
| **ADR-0007 contradicts `COMPREHENSIVE_AUDIT_SCORECARD.md` on sharecli-core deps** | MEDIUM | architectural intent vs measured behavior | PR #935 MERGEABLE; merge before any new ADR | sharecli architect |
| **14 files > 1000 LoC** | LOW (acknowledged) | AGENTS.md 500L cap acknowledged; `BOUNDARY.md` provides rationale | not for v1.0; not silently re-scoped | sharecli engineer |
| **`monitoring.rs` is a stub** | LOW (acknowledged) | dead code is documented as such | not for v1.0 | sharecli engineer |
| **`crates/spawn-core` ownership undecided** | LOW (acknowledged) | on disk in sharecli repo but ADR-0006 says portage-temp | decide before v1.1; freeze v1.0 with current state | sharecli architect |
| **`crates/gateway-tools` not in workspace** | LOW | on disk; PR #923 fixes | merge #923 | sharecli engineer |
| **cargo-deny advisory not allowlisted** | MEDIUM | blocking `cargo-deny` gate | update deny.toml or upgrade dependency | sharecli engineer |
| **musl/freebsd/wasm cross builds FAIL** | MEDIUM | cross platform gate | likely tray-icon upstream; PR #938 + maybe 0.27.x bump | sharecli engineer |
| **real-user validation gap** | HIGH (for MVP) | MVP gate requires recorded beneficiary session | operator runs a session; one human-hour | operator |
| **No prebuilt binary** | LOW (acknowledged) | source build only | not for v1.0; homebrew formula in plan | sharecli engineer |
| **Visual regression `visual` workflow FAIL** | LOW (acknowledged) | transient Zig download failure | re-run; not blocking | sharecli CI |
| **Cargo bench (gate) FAIL** | MEDIUM | perf regression on `healthz` path | investigate; gate is BLOCKING | sharecli engineer |
| **`phenoAI` Hermes imports** | MEDIUM | carried over from PhenoRegistry `hermes-imports` skill; some skill metadata may drift | re-run importer; track via `hermes-imports` skill | PhenoRegistry ops |
| **Tracera org chart stale for sharecli** | LOW | sharecli not added; affects visual org model | add Tracera entry on operator's call | operator |
| **cliff.toml `commit.link`** | LOW | non-standard template var | patch to use `{{ commit.id \| truncate(length=7, end="") }}` + manual URL | sharecli engineer |
| **Cross-harness `crane.yml` operator-decisions** | LOW | not audited; cross-harness | operator reviews; not in agent scope | operator |
| **C7 `crates/sharecli-core/src/hypervisor.rs` single-syscall optimisation** | LOW | 1.6% perf headroom | v1.1 | sharecli engineer |
| **`fs_ext.rs` openat vs openat2 syscalls** | LOW | new kernel feature; sharecli uses openat | v1.1 | sharecli engineer |

## 7.3 Cancelled or descoped (per ADR / explicit operator decision)

| Item | Authority | Reason |
|---|---|---|
| Mobile companion | ADR-0001 | not product |
| MCP server | ADR-0004 | not product |
| Harbor / SWE-bench integration | `PRD.md` §Out of product scope | out of scope; lives in `phenotype-tooling/crates/benchora` |
| Agent-eval harness | ADR-0005 | eval surface is separate (benchora) |
| Per-vendor agent wrapping | `PRD.md` §Origin thesis | detection not wrapping |
| File-size decomposition of all 14 over-1000L files | `BOUNDARY.md` | accepted as "post-MVP" with explicit rationale |
| `monitoring.rs` and `pyroscope_stub.rs` extraction | audit §3 Theme B | honest stubs with module-level docs |
| `crates/spawn-core` Zig build wiring | audit §4.4 F2 | on disk but not in workspace |
| `crates/gateway-tools` workspace membership (before #923) | audit §4.4 F3 | pre-#923; now MERGEABLE |

**Do not silently re-add any of these as scope.**
