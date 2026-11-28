# teamcomm maintainers

teamcomm is a side-workspace maintained alongside sharecli. Ownership is
shared between the sharecli maintainers and any teamcomm-specific
contributors.

## Decision: independent workspace, shared git repo

teamcomm lives at `lib/teamcomm/` inside the sharecli git repo but has
its own `[workspace.package]` and its own release cadence. This was
decided 2026-10-17 to resolve the "side-workspace orphan" audit finding
(`sharecli-reaudit-2026-10-17.md:88`).

### Why not a separate git repo?

A separate repo would create a fork tax (separate CI, separate issues,
separate permissions) for a small surface (6 crates, ~3 KLOC). The
hybrid keeps teamcomm close to sharecli (shared CI/PR conventions) while
giving it independent versioning.

### Why not a `[workspace.dependencies]` entry in the parent workspace?

A shared dependency declaration would couple teamcomm's release cadence
to sharecli's. teamcomm's own `[workspace]` decouples them.

### Why not a member of sharecli's parent `[workspace]`?

Adding `lib/teamcomm` to sharecli's `[workspace.members]` would force
teamcomm to participate in sharecli's release pipeline. By being a
separate `[workspace]` (a "side-workspace"), teamcomm's `cargo check`
and `cargo test` runs are completely independent of sharecli's.

## Decision authority

- **Backwards-incompatible changes** to a teamcomm public API require
  one maintainer approval (currently: sharecli maintainers, who are also
  teamcomm maintainers by default).
- **Additive changes** to teamcomm (new crate, new module, new public
  API) require no approval beyond a passing CI run.
- **Cross-cutting changes** (teamcomm + sharecli in one PR) are NOT
  permitted. See `docs/RELEASING.md`.

## Ownership matrix

| Path                              | Default owner       |
|-----------------------------------|---------------------|
| `lib/teamcomm/**`                 | sharecli maintainers |
| `lib/teamcomm/MAINTAINERS.md`     | sharecli maintainers |
| `lib/teamcomm/docs/RELEASING.md`  | sharecli maintainers |
| `lib/teamcomm/Cargo.toml`         | sharecli maintainers |
| `lib/teamcomm/crates/*/Cargo.toml` | sharecli maintainers |

## Migration path: separate git repo (if needed later)

If teamcomm grows large enough to warrant a separate repo, the
migration is:

1. Create `KooshaPari/teamcomm` repo.
2. `git filter-repo --subdirectory-filter lib/teamcomm` on a clone.
3. Push to new repo.
4. Update sharecli to depend on teamcomm via git tag (e.g.,
   `teamcomm = { git = "https://github.com/KooshaPari/teamcomm.git", tag = "teamcomm-v0.X.Y" }`).

This is a 1d migration when needed; not premature now.

## Out-of-band tool

The teamcomm justfile (at `lib/teamcomm/justfile`) is the canonical
local-task surface for teamcomm contributors. It exposes `build`,
`test`, `lint`, `audit`, `deny`, `typecheck`, `docs`, `clean`,
`test-crate <crate>`, `crates`, and a `ci` recipe that runs the full
local CI sweep.

sharecli's justfile does NOT run teamcomm recipes. To work on teamcomm,
`cd lib/teamcomm && just ci`.