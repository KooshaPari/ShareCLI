# teamcomm build verification — 2026-10-17

This note records the two-workspace `cargo check` results that confirm
the HYBRID resolution does not break either workspace.

## Build environment

- **Date**: 2026-10-17
- **Lane**: sharecli `refactor/teamcomm-workspace-2026-10-17`
- **Worktree**: `~/CodeProjects/Phenotype/repos/sharecli-worktrees/teamcomm-workspace-2026-10-17`
- **Base**: `origin/main` at `b4644757` (`fix(tray-windows): enable libappindicator feature for Linux tray-icon backend`)
- **Toolchain**: rustc/cargo from the rust-toolchain.toml at repo root

## Workspace 1: teamcomm (`cd lib/teamcomm && cargo check --workspace`)

```
Checking teamcomm-protocol v0.1.0
Checking teamcomm-client v0.1.0
Checking teamcomm-daemon v0.1.0
Checking teamcomm-cli v0.1.0
Checking teamcomm-mcp v0.1.0
Checking teamcomm-smoke v0.1.0
warning: `teamcomm-smoke` (bin "teamcomm-smoke") generated 17 warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 28s
exit code: 0
```

- All 6 teamcomm member crates compile.
- 17 warnings, all pre-existing in `teamcomm-smoke` (`value assigned to
  \`failed\` is never read` in the smoke harness). Not introduced by
  this lane.
- No errors.

**Result: PASS**

## Workspace 2: sharecli parent (`cd <repo-root> && cargo check --workspace`)

```
Checking sharecli v0.8.0
Checking sharecli-ipc v0.8.0
Checking sharecli-ffi v0.8.0
Checking sharecli-core v0.8.0
Checking harness-native v0.8.0
...
warning: unused import: \`DEFAULT_NOCACHE_ARGS\` in crates/harness-native/src/strategies/hypervisor_lane.rs:6
warning: \`harness-native\` (lib) generated 1 warning
Finished `dev` profile [unoptimized + debuginfo] target(s) in 11m 51s
exit code: 0
```

- All 14 sharecli parent workspace crates compile.
- 1 warning, pre-existing in `harness-native` (unused import). Not
  introduced by this lane.
- No errors.

**Result: PASS**

## Workspace independence

The two `cargo check` invocations ran from different working directories
(`lib/teamcomm/` and `<repo-root>/`) and did NOT see each other's
crates. This is the expected HYBRID behavior: teamcomm is a
side-workspace, not a member of sharecli's parent workspace.

## What was NOT tested

This lane ran `cargo check` only. Not run:

- `cargo test --workspace` for either workspace.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- `cargo fmt --all -- --check`.
- `cargo audit`, `cargo deny check`.

These are out of scope for the workspace-migration lane. They are
covered by the existing sharecli CI on `main` and by the teamcomm
justfile's `ci` recipe (which can be run by maintainers: `cd
lib/teamcomm && just ci`).

## Lane impact

This lane touched only `lib/teamcomm/`:

- `lib/teamcomm/Cargo.toml` (added `description` field, 1 line).
- `lib/teamcomm/docs/AUDIT-RESOLUTION-2026-10-17.md` (new file).
- `lib/teamcomm/docs/RELEASING.md` (new file).
- `lib/teamcomm/docs/BUILD-VERIFICATION-2026-10-17.md` (this file).
- `lib/teamcomm/MAINTAINERS.md` (new file).

No file outside `lib/teamcomm/` was modified.