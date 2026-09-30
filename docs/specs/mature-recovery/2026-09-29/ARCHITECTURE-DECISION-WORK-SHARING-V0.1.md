# Architecture decision candidate — ShareCLI work sharing v0.1

Status: **PROVISIONAL DECISION / architecture gate not fully closed**  
Date: 2026-09-30.

## Decision under test

Reject **generic durable replay of arbitrary commands** as ShareCLI's default work-sharing architecture.

Prefer:
1. normal execution when equivalence is unknown;
2. tool-native caches where their semantic model is stronger;
3. explicit ShareCLI equivalence adapters for bounded operation families;
4. in-flight duplicate suppression as a separately authorized mechanism;
5. durable replay only where an adapter establishes immutable/relevant input identity and side-effect policy.

## Evidence

Exact native Hypervisor tests reproduced incorrect reuse for:
- Git mode: changed file bytes under stable HEAD/status shape;
- Args mode: same argv across different workspaces;
- Git mode: relevant environment changed;
- Time mode: external state changed while key dimensions remained stable.

This is not one missing hash field. Each historical mode omits a different semantic dimension, and arbitrary commands can observe dimensions that no generic static key can enumerate safely.

## Rejected alternatives

### “Hash more fields globally”
Rejected as default. Adding cwd/env/content still cannot generally capture network, clock, randomness, implicit config, external services, principal state, kernel/host state or command side effects.

### “Git mode for all repo work”
Rejected as default. Git repository identity does not define all command inputs and current mode is already natively falsified.

### “Args mode for deterministic tools”
Insufficient without an adapter proving determinism and workspace/input independence.

### “Disable caching entirely”
Still a valid fallback/status-quo baseline, but it discards the accepted outcome of avoiding duplicate equivalent work. Retain as safe behavior for unknown domains while adapters are evaluated.

## Adapter contract candidate

An adapter declares:
- adapter/version identity;
- eligible executable/operation family;
- relevant immutable inputs and how they are fingerprinted;
- relevant environment/toolchain/config dimensions;
- principal/scope policy;
- side-effect policy;
- nondeterminism/external-input policy;
- whether in-flight sharing, durable replay, or both are allowed;
- cache/result schema and invalidation/migration semantics;
- independent negative fixtures.

No adapter may infer eligibility solely from a cache hit.

## First prototype choices

Prototype one domain where an existing cache already supplies semantics and one bounded read-only domain where ShareCLI must supply them. Compare against simply delegating to the native cache.

Do **not** implement a universal Merkle filesystem scanner before that comparison; its cost and still-incomplete external-input coverage could reproduce the same conceptual error at greater complexity.

## Remaining gate

This decision becomes accepted only after:
- in-flight vs durable semantics are tested separately;
- one adapter prototype is compared against tool-native behavior;
- foreground/latency benefit is measured enough to justify the added layer;
- user/donor authority confirms duplicate-work reduction is an accepted outcome independent of the historical cache implementation.
