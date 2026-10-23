# ADR-007: Coverage Ratchet Recovery Plan

**Status**: Proposed  
**Date**: 2026-08-28  
**Deciders**: ShareCLI maintainers  
**Addresses**: L19 Coverage Target (score 2/3), C01 cluster, audit-v38-ext gap

---

## Context

The ShareCLI coverage gate in `quality-gate.yml` requires **85% line coverage** (`--lib` scope). Current state:

| Scope | Lines | Covered | Percentage | Status |
|-------|-------|---------|------------|--------|
| `--lib` (current pin) | 27,947 | 21,615 | **77.34%** | BELOW 85% gate |
| `--workspace` (retained) | 40,077 | 32,266 | **80.51%** | Below 85% gate |

**Gap**: ~2,070 additional lines need coverage to hit 85% on `--lib` scope. The primary uncovered areas are:

1. **FR-008 (coalesce mesh)**: Integration test compatibility regressions on Windows (operator-env critical-timeout hang)
2. **FR-009 (FUSE intercept)**: FUSE-specific Linux/macOS code paths untestable on Windows
3. **serve layer**: Partial coverage of `serve_rate_limit.rs`, `serve_auth.rs` edge cases
4. **config watcher**: File-system event debounce paths
5. **thermal gate**: Platform-specific thermal reading paths

## Decision

Implement a **3-phase coverage ratchet** that progressively raises coverage while maintaining CI stability.

### Phase 1: Unblock workspace measurement (Week 1)

1. Add `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "macos")]` gate attributes to FUSE-specific test paths so they compile but skip on Windows
2. Fix `tests/fr008_coalesce_mesh` timeout by adding `tokio::time::timeout` wrapper
3. Add `--workspace` scope measurement to CI as supplementary (not blocking)
4. **Target**: Workspace measurement unblocked, 80%+ verified

### Phase 2: Lift --lib to 80% (Week 2-3)

Add targeted tests for uncovered modules:

| Module | Lines to cover | Test approach |
|--------|---------------|---------------|
| `serve_rate_limit.rs` | ~40 lines | Unit tests for rate limit exhaustion, token bucket refill |
| `serve_auth.rs` | ~30 lines | JWT validation edge cases, expired token, malformed header |
| `config_watcher.rs` | ~50 lines | Mock filesystem events, debounce timer test |
| `thermal.rs` | ~25 lines | Platform-conditional mock thermal readings |
| `dashboard_assets.rs` | ~20 lines | Asset MIME type mapping tests |
| `alloc.rs` | ~15 lines | jemalloc/dhat feature gate compilation tests |
| **Total** | **~180 lines** | |

### Phase 3: Lift to 85%+ (Week 4)

1. Add integration tests for `serve` endpoints with mock state
2. Add `fr012_serve_jwt_auth` edge case tests (token rotation, audience mismatch)
3. Add `fr011_thermal_gate` platform mock tests
4. Add `fr008_coalesce_mesh` mesh convergence tests with mocked network
5. Pin final 85%+ snapshot

### Ratchet Mechanism

Once 85% is achieved, the ratchet locks:

```yaml
# quality-gate.yml
COVERAGE_THRESHOLD: 85  # Hard gate -- never decreases
COVERAGE_RATCHET: true  # Prevents lowering the threshold
```

The `COVERAGE_RATCHET` flag ensures that even if code is removed, the percentage never drops below the last-pinned value. Each coverage-lift PR must include an `audit/coverage-snapshots/<sha>.coverage-snapshot.json` artifact.

## Consequences

- **Positive**: CI gate passes, governance score for L19 goes from 2/3 to 3/3, overall scorecard lifts ~0.5%
- **Positive**: FUSE/thermal tests become cross-platform compilable
- **Negative**: ~180 lines of test code to maintain, 4 weeks of effort
- **Risk**: Windows FUSE test compilation may need ongoing `cfg` maintenance

## Verification

- `just test` passes with all new tests
- `cargo llvm-cov --lib --all-features --locked` reports >=85%
- `audit/coverage-snapshots/` contains pinned snapshot with `meets_lines_target: true`
- PR lint confirms FR-NNN reference in coverage-lift PRs
