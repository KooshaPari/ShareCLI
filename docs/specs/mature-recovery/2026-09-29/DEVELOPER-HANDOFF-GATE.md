# ShareCLI developer-agent handoff gate — v0.2

Status: **PARTIAL / TWO-TIER HANDOFF**  
Date: 2026-09-30.

Developer agents may work only inside the tiers below. Specification/design completion remains false.

## Tier A — structurally authorized now

### SC-DEV-A1 — equivalence adapter contract
Implement a minimal versioned adapter interface and decision enum:
`BYPASS | IN_FLIGHT | DURABLE`.

Hard constraints:
- unknown/unregistered adapter = BYPASS;
- no generic Time/Args/Git mode may manufacture DURABLE eligibility;
- receipt records adapter/version/input identity/decision;
- one deterministic fixture adapter only initially.

Authority: SC-AD-02 + native SC-F01/F06/F07/F08.

### SC-DEV-A2 — capability-truth model
Represent observed/attributed/owned/supervised/mediated/filesystem-intercepted/optimization-eligible/result-shareable/recovery-managed independently.

No UI may infer one state from another.

### SC-DEV-A3 — FUSE policy plumbing
Introduce `off | optional | required` configuration/result types and truthful degraded evidence.

Production behavior change for `required` must remain behind explicit opt-in until its fail-closed native oracle exists.

### SC-DEV-A4 — trace/evidence enforcement
Implement machine validation that:
- relation-less FR tags are not semantic trace edges;
- old scorecards/catalogs are excluded unless explicitly qualified;
- exact candidate/configuration/verifier identity is required for green.

This is control-plane work, not product behavior.

## Tier B — experiment-dependent / do not merge production remediation yet

### SC-DEV-B1 — split in-flight from durable storage
Design/prototype is authorized. Production merge waits for the exact `recovery_fr008_inflight_vs_durable` receipt and independent review of owner-failure semantics.

### SC-DEV-B2 — queue ownership/fairness replacement
Prototype abstractions are authorized. Production remediation waits for exact queue aging/FIFO/PID-reuse receipts and native-jobserver comparison.

Do not patch only u8 wrap/string ordering and call queue architecture solved.

### SC-DEV-B3 — platform ProcessGeneration provider
Prototype Linux pidfd / Windows / macOS candidates. No cross-platform public contract is frozen yet.

### SC-DEV-B4 — real tool durable adapter
Not authorized until one real tool family demonstrates value over its native cache.

## Explicitly prohibited

- universal durable command cache rewrite;
- hashing additional generic fields and declaring semantic equivalence solved;
- universal FUSE requirement;
- mesh/distributed expansion;
- dashboard growth unrelated to capability truth;
- requirement-count padding;
- broad implementation-complete claim.

## Merge receipt

Architecture-sensitive work must identify ontology/obligation/decision IDs, exact candidate/dependencies, positive and negative oracle commands, exact run/job IDs, executed-case count, and unresolved risks.

## Verdict

**Tier A developer handoff: READY.**  
**Tier B prototype handoff: READY; production merge gate CLOSED.**  
**Whole mature ShareCLI implementation: NOT READY.**
