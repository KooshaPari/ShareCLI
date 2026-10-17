# 03 — Source coverage, archaeology and authority

## 3.1 Source surface inventory

| Source ID | Surface / exact locator | Revision / date | Authority class | Read scope | Limits / conflicts |
|---|---|---|---|---|---|
| S01 | `KooshaPari/ShareCLI` (origin) | origin/main @ `ec7bcbd9c630a1e393f5cd5ee044f0a9d92b7548`, 2026-10-17 00:10 UTC | canonical source | complete (clone) | none |
| S02 | `KooshaPari/ShareCLI` (local clone) | `chore/cvp-1h-queue-2026-10-17` @ `5f2425ca0d2d14a40e0f7c96029fdc854bbc7792`, 2026-10-17 00:53 UTC; working tree dirty (Cargo.lock, sharecli-tray-windows/Cargo.toml) | local working state | complete | dirty; uncommitted tray-icon fix |
| S03 | Audit baseline | `134888ede651bbd74f9d951866af1bb542e64395`, 2026-10-16 21:53 PDT | pre-state for delta | sampled (8 subagent reports) | audit subagent reports cited file:line evidence |
| S04 | Audit deliverable | `audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md` (485 lines, 34,883 bytes) | synthesized findings | full | none |
| S05 | Audit subagent reports | 8 reports, 1,982 lines, 114,446 bytes | lane-by-lane evidence | full | none |
| S06 | Audit PDF | `audit-sessions/2026-10-17-deep-state-audit/deliverables/MASTER-STATE-AUDIT.pdf` (575 KB, 8 pages) | rendered projection | rendered | pagination varies; content unchanged |
| S07 | PhenoRegistry | `KooshaPari/PhenoRegistry` @ main `a4f4a409f` (2026-10-14) | org-level portfolio + intent | complete (clone) | sharecli-specific record absent; this SFD creates it |
| S08 | State & Forward Docket v1.0 spec | `KooshaPari/PhenoRegistry` @ `docs/high-velocity-state-forward-docket-20261016` (2026-10-17 08:01 UTC) | this docket's template source | full | contract, README, ADOPTION, 19 templates, 2 examples |
| S09 | 9 commits since audit | `5f2425ca`, `551ace62`, `89b03f98`, `26d65067`, `bf955e66`, `3cd80326`, `2a7a6267`, `878212aa`, `f1479c05` (most recent first) | direct source changes | full | none |
| S10 | 7 PRs from this session | #932, #933, #934, #935, #936, #937, plus #938 (later) | forward-path work | full | all MERGEABLE; #938 new since 11:11 UTC |
| S11 | `thegent-sharecli` fork-line | (now defunct; pre-2026-04) | historical only | partial | defunct, no recovery |
| S12 | `phenotype-tooling/crates/benchora` | out of sharecli repo; lives in sibling | related but separate | referenced | not in scope |
| S13 | `crates/gateway-tools` (in-repo) | 1,025 + 557 LoC main+serve | on disk but not in workspace | complete | not part of `cargo build` until PR #923 lands |
| S14 | `crates/spawn-core` (Zig) | on disk but not in workspace | referenced in ADR-0006 | sampled | not built today; `zig build` not exercised |
| S15 | `lib/teamcomm/` (vendored) | untracked after PR #921 / e182151d | separate product (teamcomm) | header only | not part of sharecli |
| S16 | `sharecli/preserved/migrated-from-10-override-batch/` | still tracked | off-topic archive | none (junk) | PR #932 untracks it; archive repo TBD |
| S17 | `recovery/feb-2026-agent-harness/` | still tracked | recovery artifact | none (junk) | PR #932 untracks; 2.1 MB binary |
| S18 | Tracera at `http://localhost:3180` | live (2026-10-17) | org chart / paperclip | not invoked | sharecli not yet in Tracera |
| S19 | AgilePlus | (not yet created) | task tracking | not invoked | sharecli backlog in `WORK_DAG.md` not linked to AgilePlus |
| S20 | Repo conversations / Linear / Slack | operator-only | out of agent reach | none | not searched |

**Inaccessible / unsearched surfaces:** S11, S18 (Tracera not queried), S19 (AgilePlus not invoked), S20 (operator's private channels). These are not part of this docket's evidence base.

## 3.2 Authority precedence (where contradictions exist)

| Contradiction | Source A | Source B | Resolution |
|---|---|---|---|
| `sharecli-core` dep direction | ADR-0007 §1: Tier A pure, no infra deps | `COMPREHENSIVE_AUDIT_SCORECARD.md` C00 row 3: depends on ipc/fleet/fuse, scored 3/3 | **Code wins**; ADR-0007 §1 is updated by PR #935 (MERGEABLE) |
| Version label | `VERSION:1` says 0.1.0; `CHANGELOG.md:27` says 1.0.0 | (no third source) | commit `2a7a6267` reconciles: `VERSION=0.8.0`, CHANGELOG kept on 1.0.0 for release tag; **PR #936 carries** |
| `crates/spawn-core` ownership | on disk in sharecli repo | ADR-0006 says it lives in `portage-temp` | (not resolved) — documented but no `move` PR exists; **leave as known unknown** |
| `crates/gateway-tools` membership | on disk in sharecli repo | `Cargo.toml` `[workspace]` doesn't include | PR #923 in flight to add to workspace; **PR #923 carries** |
| Boundary lock | `BOUNDARY.md` says "shared CLI process manager" | `PRD.md` says "agent runtime / hypervisor" | Both are correct at different abstraction levels; `PRD.md` is the mature-contract authority; `BOUNDARY.md` describes the cross-project boundary. PR #913 framing harmonizes them. |
| `monitoring.rs` stub-ness | audit §3 Theme B: "honest about stub-ness" | operator intuition: should be deleted | audit wins; honest stub is acceptable for v1.0 |
| `sharecli-runtime/serve/config` extraction | `lib-sprawl-plan.md` Phase 4: extract to crates | audit §4.8 MEDIUM: not yet extracted | operator decision: defer to v1.1 (post-MVP) |
| `sharecli-tray-windows` tray-icon version | `default-features = false` (broken) | Linux requires a feature | fix is `features = ["libappindicator"]`; PR #938 in flight |

## 3.3 Renamed / archived / aliased surfaces

| Alias | Canonical | Note |
|---|---|---|
| `thegent-sharecli` | `sharecli` (this repo) | fork-line duplicate, defunct, absorb into `thegent` later per `BOUNDARY.md` |
| `portage` / `harbor` | NOT sharecli | portage-temp lives at `KooshaPari/portage-temp`; out of scope per `PRD.md` |
| `benchora` | `phenotype-tooling/crates/benchora` | out of sharecli; sibling |
| `teamcomm` | `lib/teamcomm/` (separated) | not sharecli; untracked in HEAD via PR #921 / e182151d |
| `thegent` | `KooshaPari/thegent` | separate CLI; cross-link in `BOUNDARY.md` |
| `phenoAI` | `phenoAI` (PhenoRegistry product) | not sharecli |
| `portage` | `portage` (PhenoRegistry product) | not sharecli |

A search returning nothing for any of these within `sharecli/` is correct and expected.
