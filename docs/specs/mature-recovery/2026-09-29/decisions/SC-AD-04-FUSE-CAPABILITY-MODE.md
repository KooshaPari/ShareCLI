# Architecture decision candidate SC-AD-04 — FUSE as optional capability adapter with explicit required mode

Status: **PROVISIONAL ACCEPT / native required-mode experiment still open**  
Date: 2026-09-30.

## Evidence

The frozen `sharecli-core` implementation documents and implements FUSE as unconditional best-effort:

- `FuseGuard::try_mount` never returns an error to its caller;
- mount/readiness failure produces a loud message and a no-op guard;
- Hypervisor then executes from the original cwd without interception.

The exact SC-F01 native run reproduced this path in CI: FUSE mount failed because `allow_other` was unavailable, and the Hypervisor continued without FUSE. This did not cause the cache false hit, proving filesystem interception and command-result sharing are independent capabilities.

No experiment in this recovery program has established that every ShareCLI journey requires FUSE for correctness.

## Decision

FUSE is not a universal prerequisite for ShareCLI.

Model it as a capability adapter with explicit policy:

- `off` — no filesystem interception requested;
- `optional` — attempt interception; on failure continue, but capability/evidence must truthfully report degraded/unavailable;
- `required` — the accepted journey depends on filesystem interception; failure to establish a verified mount aborts the operation before the dependent behavior executes.

Default mature behavior should be `optional` or `off` unless a specific accepted adapter/journey requires interception.

## Why this is safer

Treating FUSE as globally required would:
- impose privilege/platform burden on monitor/supervisor-only journeys;
- make unsupported platforms nonfunctional even when no filesystem semantics are needed;
- couple observation/admission/work-sharing to an independent mechanism.

Treating FUSE as always best-effort is also insufficient because a future CoW/isolation journey could silently execute against the real backing tree when interception is actually required.

## Capability truth

The product state must distinguish:

`fuse_requested, fuse_mode, fuse_available, fuse_ready, fuse_session_id, degraded_reason`

A command/result receipt must not claim FUSE-derived provenance when the run fell back to the original cwd.

## Negative controls

1. optional mode + unavailable FUSE → execution may continue, result explicitly says interception unavailable;
2. required mode + unavailable FUSE → command dependent on interception does not execute;
3. off mode → no mount attempt;
4. mount becomes unready after start → required-mode operation does not silently continue;
5. cached result produced without FUSE cannot later be represented as FUSE-observed evidence;
6. FUSE-active command mediation remains independent from `helios-shield`/Hypervisor interception state.

## Falsification

Revise if direct product intent establishes that all ShareCLI-managed execution must be filesystem-intercepted, or if experiments show an alternative filesystem mechanism should replace FUSE entirely.

This decision does not preserve FUSE implementation for its own sake; it preserves the capability contract.
