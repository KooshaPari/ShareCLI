# Releasing teamcomm

teamcomm is a side-workspace that lives inside the sharecli git repo but
has its own `[workspace.package]` and its own release cadence. Releases
are INDEPENDENT of sharecli releases.

## Versioning

teamcomm follows semver. The current version is in
`lib/teamcomm/Cargo.toml` under `[workspace.package].version`.

| Bump kind | When                                                   |
|-----------|--------------------------------------------------------|
| `PATCH`   | Bug fix in an existing API or behavior                 |
| `MINOR`   | New crate, new module, new public API, additive change |
| `MAJOR`   | Breaking change to a public API                        |

## Release process

1. Update `lib/teamcomm/Cargo.toml`: bump `[workspace.package].version`.
2. Update `lib/teamcomm/CHANGELOG.md`: add release notes.
3. Run `cd lib/teamcomm && cargo check --workspace && cargo test --workspace`.
4. Commit: `git commit -m "release(teamcomm): v0.X.Y"`
5. Tag: `git tag -a teamcomm-v0.X.Y -m "teamcomm v0.X.Y"`
6. Push: `git push origin main && git push origin teamcomm-v0.X.Y`

Tags use the `teamcomm-v` prefix to disambiguate from sharecli's release
tags. sharecli currently uses `vX.Y.Z`; teamcomm uses `teamcomm-vX.Y.Z`.

## Cross-cutting changes

If a change to teamcomm requires a change to sharecli (or vice versa):

1. Land them in SEPARATE commits on SEPARATE PRs.
2. The PR that breaks compatibility goes first; the PR that follows it
   must reference the first.
3. Never bundle a teamcomm change with a sharecli change in one PR.

This keeps release-cadence separation intact: a sharecli release can
ship without waiting on a teamcomm release, and vice versa.

## When to release

teamcomm should be released when:

- A teamcomm crate's public API changes (MINOR or MAJOR).
- A teamcomm fix lands that downstream consumers need (PATCH).
- A teamcomm milestone is reached (any kind, at maintainer discretion).

teamcomm does NOT need a release for:

- Internal refactors that preserve public APIs.
- Documentation-only changes inside `lib/teamcomm/`.
- Test-only changes.

## Release cadence target

There is no fixed calendar cadence. Releases happen when there is
something worth releasing. Aim for at most one release per week during
active development; longer gaps are fine when nothing has changed.

## First release

The first formal teamcomm release will be cut after this lane lands and
the teamcomm crates have been smoke-tested end-to-end.