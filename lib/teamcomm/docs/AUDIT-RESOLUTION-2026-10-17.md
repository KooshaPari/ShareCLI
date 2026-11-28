# teamcomm audit resolution — 2026-10-17

This note records the resolution of the sharecli reaudit finding
`sharecli-reaudit-2026-10-17.md:88` which flagged teamcomm as a
"side-workspace orphan in `lib/teamcomm/`".

## Finding (verbatim)

> teamcomm side-workspace orphan in `lib/teamcomm/`. teamcomm is a
> side-workspace (likely a separate MCP tool-call surface) that currently
> lives as an orphan inside sharecli's `lib/teamcomm/` directory. It has
> no `[workspace.package]`, no version, no independent release cadence.

## Resolution decision (HYBRID)

- **Keep teamcomm in the same git repo as sharecli** (shared CI, shared
  PR conventions, no fork tax).
- **Give it its own `[workspace.package]`** in a separate `Cargo.toml`
  at `lib/teamcomm/Cargo.toml`.
- **Independent versioning**: `teamcomm = "0.1.0"` (own semver).
- **Independent release cadence**: sharecli and teamcomm release
  separately.
- **Document the policy** in `lib/teamcomm/MAINTAINERS.md`.

## State at 2026-10-17 (this lane start)

| Concern                          | State before this lane            |
|----------------------------------|-----------------------------------|
| `[workspace]` block              | Already present (resolver = "2")  |
| `[workspace.package]`            | Already present at v0.1.0         |
| `[workspace.package].version`    | Already present at "0.1.0"        |
| `[workspace.package].description` | **Missing** — added by this lane |
| `lib/teamcomm/Cargo.toml`        | Already declares 6 member crates  |
| `lib/teamcomm/MAINTAINERS.md`    | **Missing** — created by this lane |
| `lib/teamcomm/docs/RELEASING.md` | **Missing** — created by this lane |
| `cargo check` (teamcomm ws)      | PASS (warnings only, 1m 28s)      |
| `cargo check` (sharecli ws)      | PASS (warnings only, 11m 51s)     |

The audit's "no `[workspace.package]`, no version" claim was already
obsolete when this lane started; the workspace had been set up correctly
in a prior commit. What this lane actually adds:

1. `description = "teamcomm — multi-agent coordination protocol"` to
   `[workspace.package]`.
2. `lib/teamcomm/MAINTAINERS.md` documenting the HYBRID decision.
3. `lib/teamcomm/docs/RELEASING.md` documenting the independent release
   cadence.
4. `lib/teamcomm/docs/BUILD-VERIFICATION-2026-10-17.md` recording the
   two-workspace `cargo check` results.

## Lane boundary

This lane did NOT:
- Create a separate git repo for teamcomm.
- Move `lib/teamcomm/` out of sharecli.
- Add teamcomm as a member of sharecli's parent `[workspace]`.
- Touch any file outside `lib/teamcomm/`.

These are out of scope per the HYBRID decision. If teamcomm grows large
enough to warrant a separate repo, the migration path is documented in
`lib/teamcomm/MAINTAINERS.md`.