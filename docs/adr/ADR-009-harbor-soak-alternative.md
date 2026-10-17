# ADR-009: Harbor Soak Alternative Path

**Status**: Proposed  
**Date**: 2026-08-28  
**Deciders**: ShareCLI maintainers  
**Addresses**: L75 Harbor 7-day Soak (score 0/3), L76 Harbor Soft Gate (score 2/3), C08 cluster, audit-v38-ext gap

---

## Context

The Harbor 7-day soak evaluation harness (L75) is **BLOCKED** by external artifacts:

- **ADR-0002**: Harbor soak depends on benchora/harbor-soft external infrastructure
- **ADR-0005**: Harbor eval requires stable HTTP endpoint + persistent state store
- The `tests/c08_harbor_soft_stub.rs` exists as a stub but is not functional

Current state:

| Pillar | Score | Status |
|--------|-------|--------|
| L75: Harbor 7-day Soak | 0/3 | BLOCKED -- external artifact |
| L76: Harbor Soft Gate | 2/3 | Stub only |

The core issue: ShareCLI needs a local eval harness that can:
1. Run the CLI repeatedly over 7 days
2. Track performance regressions (latency, memory, CPU)
3. Detect stability issues (crashes, hangs, resource leaks)
4. Generate a pass/fail report

## Decision

Implement a **local eval harness** that replaces the external Harbor dependency with self-contained soak testing.

### Design: `sharecli-soak` (local harness)

```
sharecli-soak/
  soak.yaml          # Configuration (duration, intervals, scenarios)
  soak-report.json   # Output report (auto-generated)
  soak-history/      # Historical run data
```

### Configuration

```yaml
# soak.yaml
duration: 7d
interval: 5m          # Run every 5 minutes
scenarios:
  - name: healthz
    command: ["sharecli", "health", "--json"]
    timeout: 10s
    expect:
      exit_code: 0
      contains: '"status":"ok"'
  - name: status
    command: ["sharecli", "status", "--json"]
    timeout: 15s
    expect:
      exit_code: 0
  - name: list
    command: ["sharecli", "list", "--json"]
    timeout: 10s
    expect:
      exit_code: 0
  - name: pool-status
    command: ["sharecli", "pool", "status", "--json"]
    timeout: 10s
    expect:
      exit_code: 0
  - name: config-roundtrip
    command: ["sharecli", "config", "show", "--json"]
    timeout: 10s
    expect:
      exit_code: 0
  - name: process-scan
    command: ["sharecli", "process", "scan", "--json"]
    timeout: 30s
    expect:
      exit_code: 0

thresholds:
  max_crash_rate: 0.01       # <1% crash rate
  max_p99_latency_ms: 500    # p99 <500ms
  max_memory_mb: 32          # <32MB RSS
  max_regression_pct: 5      # <5% regression from baseline
```

### Implementation

1. **Phase 1**: `sharecli soak` command that runs scenarios in a loop
2. **Phase 2**: `sharecli soak report` that generates JSON summary
3. **Phase 3**: CI integration via `workflows/soak.yml` (nightly, 1-hour soak)
4. **Phase 4**: `tests/c08_harbor_soak_gate.rs` that runs a 10-minute soak and asserts thresholds

### Soak gate test

```rust
// tests/c08_harbor_soak_gate.rs
#[test]
fn soak_gate_10min() {
    let harness = SoakHarness::new(SoakConfig {
        duration: Duration::from_secs(600),
        interval: Duration::from_secs(10),
        scenarios: vec![
            Scenario::healthz(),
            Scenario::status(),
            Scenario::list(),
        ],
        thresholds: Thresholds::default(),
    });
    
    let report = harness.run();
    assert!(report.crash_rate < 0.01, "crash rate too high: {}", report.crash_rate);
    assert!(report.p99_latency_ms < 500, "p99 too high: {}ms", report.p99_latency_ms);
    assert!(report.max_memory_mb < 32.0, "memory too high: {}MB", report.max_memory_mb);
}
```

### CI integration

```yaml
# workflows/soak.yml
name: Soak Test (Nightly)
on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM UTC daily
  workflow_dispatch:

jobs:
  soak:
    runs-on: ubuntu-latest
    timeout-minutes: 120
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - name: Run 1-hour soak
        run: |
          ./target/release/sharecli soak \
            --duration 1h \
            --interval 30s \
            --output soak-report.json
      - name: Assert thresholds
        run: |
          jq -e '.crash_rate < 0.01' soak-report.json
          jq -e '.p99_latency_ms < 500' soak-report.json
          jq -e '.max_memory_mb < 32' soak-report.json
      - uses: actions/upload-artifact@v4
        with:
          name: soak-report
          path: soak-report.json
```

## Consequences

- **Positive**: L75 score 0/3 -> 3/3, L76 score 2/3 -> 3/3
- **Positive**: No external dependency on benchora/harbor-soft
- **Positive**: Nightly soak catches regressions early
- **Negative**: ~300 lines of soak harness code
- **Negative**: CI job takes 1-2 hours nightly
- **Risk**: Soak test may be flaky on shared runners

## Verification

- `sharecli soak --duration 10m --interval 30s` runs without crashes
- `sharecli soak report` generates valid JSON
- `tests/c08_harbor_soak_gate.rs` passes
- Nightly CI soak job runs and reports artifacts
- Coverage snapshot includes soak gate test
