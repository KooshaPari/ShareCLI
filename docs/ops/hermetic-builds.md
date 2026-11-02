# Hermetic builds (C06 L54)

Audit-v38 **C06 L54**. Goal: reproducible offline builds after dependency fetch.

## Contract (hard + soft)

1. **`Cargo.lock` + `--locked`** on CI builds (already required).
2. **Fetch then offline:** `cargo fetch --locked` then `cargo build --locked --offline`.
3. **Required CI gate:** `.github/workflows/ci-gate.yml` `ci-netblock` job runs
   [`scripts/ci/netblock_check.sh`](../../scripts/ci/netblock_check.sh) on every PR
   (no `continue-on-error`). The legacy `.github/workflows/ci.yml` `netblock` job and its
   `ci-success` aggregator were deleted by the CI dedup; `ci-netblock` is the surviving
   single definition of this hard gate.
4. **Soft advisory:** `.github/workflows/hermetic-soft.yml` poisoned-proxy smoke
   (`HTTP_PROXY=http://127.0.0.1:9`) and `.github/workflows/netblock-soft.yml` mirror the
   same probe with `continue-on-error: true` for drift signal.
5. **Local:** `just hermetic` (fetch → offline build) and `bash scripts/ci/netblock_check.sh`.

This is **not** a fully network-blocked Actions runner or committed `vendor/` tree yet.
The hard gate proves lockfile completeness for offline compile on every PR.

## Hard follow-up

| Control | Status |
|---------|--------|
| `cargo vendor` + committed or artifact vendor dir | Not required yet |
| Network-blocked Actions step / hermetic builder | Partial — `CARGO_NET_OFFLINE` enforced in CI |
| Bit-identical SOURCE_DATE_EPOCH (see `docs/slsa.md`) | Seeded on release |

## Commands

```bash
cargo fetch --locked
cargo build --locked --offline -p sharecli
# or:
just hermetic
bash scripts/ci/netblock_check.sh
```
