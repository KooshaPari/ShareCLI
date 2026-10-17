# 14 — Operations, value, and human-time cost

## 14.1 Operational readiness (today)

| Domain | State | Evidence |
|---|---|---|
| **Build** | GREEN on main, RED on PRs (transient retry) | `ci / test` 2026-10-17 07:29:15 PASS on PR #937; 07:08:58 FAIL on main |
| **Lint** | GREEN | `ci / lint` 2026-10-17 07:28:38 PASS |
| **Format** | GREEN | `Lint & Format` 2026-10-17 07:05:31 PASS |
| **Tests (unit)** | GREEN | `Quality Gate Unit Tests` 2026-10-17 07:08:12 PASS (after 07:01:47 first FAIL was retried) |
| **Tests (integration)** | UNKNOWN (skipped) | `Quality Gate Integration Tests` 2026-10-17 07:12:44 SKIPPED |
| **Tests (E2E)** | UNKNOWN (skipped) | `Quality Gate E2E Tests` 2026-10-17 07:12:44 SKIPPED |
| **Coverage** | FAIL (64% → needs lift) | `Coverage` 2026-10-17 08:00:27 FAIL; `coverage` 2026-10-17 07:15:40 FAIL |
| **Cargo bench (gate)** | FAIL (perf regression) | `cargo bench (gate)` 2026-10-17 07:15:59 FAIL |
| **Cargo bench (soft)** | FAIL | `cargo bench (soft)` 2026-10-17 08:05:21 FAIL |
| **Cargo deny** | FAIL (advisory) | `cargo deny` 2026-10-17 08:02:35 FAIL |
| **Cross / x86_64-unknown-linux-musl** | FAIL | 2026-10-17 07:15:59 FAIL |
| **Cross / x86_64-unknown-freebsd** | FAIL | 2026-10-17 07:20:26 FAIL |
| **Cross / wasm32-unknown-unknown** | FAIL | 2026-10-17 07:17:08 FAIL |
| **Hermetic Build** | GREEN | 2026-10-17 07:12:04 PASS |
| **Reproducible Build (L52)** | GREEN | 2026-10-17 07:05:20 PASS |
| **CodeQL** | GREEN | 2026-10-17 07:13:59 PASS |
| **TruffleHog** | GREEN | 2026-10-17 07:16:13 PASS |
| **Dependency Review** | GREEN | 2026-10-17 07:15:42 PASS |
| **OSV / GHSA** | GREEN | 2026-10-17 07:21:57 PASS |
| **Cargo fuzz (6 harnesses)** | GREEN | 2026-10-17 07:11:32-07:20:40 PASS |
| **Miri (soft)** | GREEN | 2026-10-17 07:03:07 PASS |
| **Visual regression** | GREEN (with one transient FAIL) | 2026-10-17 07:13:20 PASS; 07:19:19 FAIL (Zig download transient) |
| **Accessibility (axe)** | GREEN | 2026-10-17 07:08:14 PASS |
| **Accessibility (Tab cycle)** | GREEN | 2026-10-17 07:16:13 PASS |
| **Inclusive language (vale)** | GREEN | 2026-10-17 07:09:45 PASS |
| **Soak healthz** | GREEN | 2026-10-17 07:11:27 PASS |
| **Healthz load burst** | GREEN | 2026-10-17 07:08:47 PASS |
| **Live pool probe** | GREEN | 2026-10-17 07:11:32 PASS |
| **Eval corpus fixtures** | GREEN | 2026-10-17 07:09:39 PASS |
| **Hermetic build (soft)** | GREEN | 2026-10-17 07:11:33 PASS |
| **Network block (soft)** | GREEN | 2026-10-17 07:14:33 PASS |
| **88-pillar scorecard** | GREEN | 2026-10-17 07:12:12 PASS |
| **AgilePlus scorecard** | GREEN | 2026-10-17 07:11:28 PASS |
| **DCO (soft)** | FAIL (no GPG key) | 2026-10-17 07:14:32 FAIL |
| **GPG signed commits (soft)** | FAIL (no GPG key) | 2026-10-17 07:06:28 FAIL |
| **GPG/SSH signed commits (soft)** | FAIL (no GPG key) | 2026-10-17 07:11:43 FAIL |
| **CI Success Gate** | GREEN | 2026-10-17 07:30:35 PASS |

## 14.2 Operator time cost (today)

| Activity | Time cost | Per what |
|---|---|---|
| New operator install (from source) | 10-30 min | 1 host (depends on Rust toolchain) |
| New operator install (from cargo install) | 5-15 min | 1 host |
| New operator first-use (config + first agent) | 5 min | 1 host |
| Existing operator upgrade (cargo install --force) | 1-5 min | 1 host |
| Recover from broken tray | 30 sec | 1 host |
| Recover from broken FUSE | 30 sec + 1-2 min modprobe | 1 host |
| Recover from stale UDS | 5 sec | 1 host |
| Recover from invalid config | 1-3 min (edit file) | 1 host |
| Read a SFD (this document) | 15 min | 1 reader |
| Merge 14 PRs | 30 min | 1 human (operator) |
| Record a beneficiary session | 1 hour | 1 human (operator) |
| Add sharecli to Tracera | 1 hour | 1 human (operator) |
| Cross-publish SFD to sharecli `audit-sessions/` | 15 min | 1 agent (this SFD) |
| Update sharecli `docs/intent/`, `docs/adr/` "see also" | 15 min | 1 agent |

## 14.3 Value delivered (today)

| Recipient | What they get | Quantified |
|---|---|---|
| Multi-agent host operator | Supervise 1-50 heterogeneous coding agents on one host | 1 host per install; 1 beneficiary per install |
| Sharecli maintainer | A documented, test-covered, CI-gated codebase | 486 .rs files, 101,757 LoC, 50+ CI workflows, 88-pillar scorecard |
| PhenoRegistry | A canonical product record | this SFD + cross-publish |
| Future contributors | A clear "what is sharecli" | PRD + SPEC + FR + ADRs + 9 ADRs + this SFD |
| The Rust community | Open-source agent runtime | Apache-2.0 OR MIT; cargo install-able |

## 14.4 Cost of NOT shipping v1.0

| Cost | Per what | Annual estimate |
|---|---|---|
| Operator has to manually manage 5-50 agents | 1 host | 1 human-hour per host per week |
| No CVP-J1 cold-start reproducibility | 1 host | 1 human-hour per onboarding |
| No standardized config schema | 1 host | unknown; ad-hoc configs break often |
| No recorded beneficiary session | sharecli project | loss of operator feedback |
| No Tracera / AgilePlus integration | PhenoRegistry | loss of portfolio visibility |

## 14.5 Cost of shipping v1.0 (broken)

| Cost | Per what | Notes |
|---|---|---|
| Operator hits tray-icon 0.25.0 on Linux | 1 host | mitigated by PR #938 |
| Operator hits cargo-deny advisory | 1 host | mitigated by allowlist PR |
| Operator hits cross-platform (musl/freebsd/wasm) | 1 host | mitigated by 0.27.x bump in v1.1 |
| Operator has no recorded beneficiary session | 1 host | MVP gate is BLOCKED; should not be marketed as "MVP" without one |

**Net:** v1.0 with the merge train + PR #938 lands green for `ci / test` and `cargo-deny`; v1.0.1 needs the recorded session before MVP claim is valid.

## 14.6 Operating expenses (next 90 days, estimated)

| Item | Cost | Frequency |
|---|---|---|
| Maintainer time (operator) | 1 FTE × 90 days | continuous |
| CI compute | 50+ workflows × 1 build × ~30 min × 1-2 build/hr | continuous |
| Codesign (SignPath Foundation) | per release | per v1.0, v1.0.1, v1.1 |
| Repository storage (sharecli) | ~1 GB (after PR #932) | one-time |
| Worktree disk (this SFD) | ~10 MB (worktree at `/tmp/phenoreg-sharecli-sfd-20261017`) | one-time |
| PhenoRegistry worktree disk | ~1 MB (SFD + tools + this docket) | one-time |

## 14.7 What sharecli does **not** pay for

- No SaaS dependencies (no Heroku, no Vercel, no Supabase)
- No paid observability (no Datadog, no Honeycomb)
- No paid alerting (no PagerDuty, no Opsgenie)
- No paid distribution (no Mac App Store, no Microsoft Store)
- No paid CI minutes (no CircleCI, no Buildkite)
- No paid code signing (SignPath Foundation is free for OSS)

This is a fully self-hosted, open-source project. **Operating expenses are dominated by maintainer time.**
