# ADR-0010: State & Forward Docket v1.0 — 2026-10-17

**Date:** 2026-10-17
**Status:** Accepted (operator review pending)
**Supersedes:** none
**Superseded-by:** none
**Author:** jcode (M3 via OpenAI-compatible)

## Context

sharecli had no canonical "where are we right now, what is unfinished,
what is forward" record. The audit at `134888ede` (2026-10-16) cataloged
the unfinished state in prose and the deep-state-audit session ranked
open PRs and the unfinished inventory, but neither document was a
self-contained, validator-checked, renderable document.

The PhenoRegistry project introduced the State & Forward Docket v1.0
spec at `handbook/specs/assessment/state-forward-docket-v1/`. The spec
defines 19 templates, one JSON snapshot shape, a validator, and a
renderer. The spec was designed so that:

- A snapshot is bounded to a specific git revision.
- The validator checks schema, evidence-ids, and a deterministic
  "all critical operational leaves PASS" gate.
- The renderer produces offline HTML + PDF without network fetches.
- Snapshots are immutable (frozen; new instance dir for next one).

## Decision

We adopt the State & Forward Docket v1.0 spec for sharecli and publish
the first instance at:

- **Authoritative copy:** PhenoRegistry
  `projects/sharecli/state/2026-10-17-sfd-001/`
  (PhenoRegistry/PhenoRegistry#666)
- **Mirror:** sharecli `audit-sessions/2026-10-17-state-forward-docket/`
  (this commit)
- **Audit-pointer:** sharecli `audit/2026-10-17-state-forward-docket.json`

Future instances will live at `projects/sharecli/state/<date>-sfd-NNN/`
with a sibling mirror in `audit-sessions/`. We do NOT edit a published
snapshot in place; the next instance is a sibling.

## Consequences

Positive:

- A self-contained, validator-checked, renderable record of
  "where are we right now" bound to a specific git revision.
- The validator's "BLOCKED on J-BENEFICIARY NOT_RUN" is honest
  signal, not a fabricated PASS.
- Cross-publish to PhenoRegistry means the audit-trail is durable
  even if sharecli is force-pushed or rewritten.

Negative:

- SFDs are immutable; we need a new instance every time we want
  a fresh "where are we right now". The instance-dir + cross-publish
  pattern costs ~30 minutes of authoring + ~5 minutes of rendering
  per snapshot.
- The validator's evidence requirements are strict: for every
  ACCEPTED operational PASS/FAIL leaf, every evidence ref must be
  EXECUTED_PRODUCT with a real sha256. SOURCE_REPORT evidence is
  fine for context but cannot be the only evidence for a passing
  leaf. This catches bad-faith "PASS" claims early.

## What this is NOT

- This is NOT a product-acceptance document. The validator returns
  `not_product_acceptance: true` and the record class is
  `OPERATIONAL`. It describes the *current state* of a project; it
  does not certify that the product is ready for end users.
- This is NOT a substitute for the deep-state-audit or the audit
  deliverable. It is a *bound snapshot* of the project, anchored to
  a specific audit baseline (`134888ede`).
- This is NOT a code review. The architecture and unfinished-inventory
  sections *describe* what exists, not *what should exist*.

## Rollout

1. PhenoRegistry PR #666 (this commit's author = M3 via OpenAI-compatible).
2. sharecli PR on `docs/sharecli-sfd-20261017` from the cross-publish.
3. PhenoRegistry audits index update at `audits/sharecli/2026-10-17-sfd-001/README.md`.
4. sharecli docs/intent and docs/adr linked from this ADR.
5. Re-snapshot at the next major checkpoint (e.g. after PR #938 lands
   or after the cvp-1h queue flushes).

## References

- PhenoRegistry spec: `handbook/specs/assessment/state-forward-docket-v1/`
- Audit deliverable: `audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md`
- Deep state audit brief: `audit-sessions/2026-10-17-deep-state-audit/00_BRIEF.md`
- Forward docket JSON: `audit-sessions/2026-10-17-state-forward-docket/snapshots/2026-10-17T19-55-00Z.json`
- Forward docket HTML: `audit-sessions/2026-10-17-state-forward-docket/reports/SFD-sharecli-2026-10-17.html`
- Forward docket PDF: `audit-sessions/2026-10-17-state-forward-docket/reports/SFD-sharecli-2026-10-17.pdf`
