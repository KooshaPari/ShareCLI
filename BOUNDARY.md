# Boundary Lock: ShareCLI process orchestration

**Status:** ACTIVE — shared CLI process manager for multi-project agent orchestration.

## Owns
- Cross-project CLI process lifecycle (spawn, supervise, teardown)
- Shared orchestration hooks consumed by agent tooling

## Does NOT own
- Agent runtime / tool registry (`thegent` — absorb later when stable)
- Code review CLI (`tehgent`)

## Duplicate
`thegent-sharecli` is a fork-line duplicate — canonical repo is **sharecli**. Absorb into `thegent` only after runtime boundary stabilizes.

## See also
- [State & Forward Docket v1.0 (2026-10-17)](audit-sessions/2026-10-17-state-forward-docket/README.md) — bound snapshot at `5f2425ca` against audit baseline `134888ede`. 19 templates, 1 JSON snapshot, 1 HTML + 1 PDF report.
- [ADR-0010: State & Forward Docket v1.0](docs/adr/0010-state-forward-docket-2026-10-17.md) — adoption rationale.
- [Intent pointer](docs/intent/2026-10-17-state-forward-docket.md) — sharecli-side intent for the SFD.