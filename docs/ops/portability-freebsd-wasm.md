# FreeBSD / WASM portability plan (soft)

## Current CI signal (measured 2026-11-24)

`cross-platform.yml` carries **no `wasm32` job**. wasm32 is unsupported, not
merely unbuilt: `spawn_core` needs `fork`/`waitpid`/pthread with no WASI
equivalent, and no workspace crate is `cfg(target_arch = "wasm32")`-gated. The
wasm32 job was removed rather than left permanently red; the requirement is
recorded here so it cannot be silently claimed as supported. Do not cite the
Cross-Platform workflow as proving wasm support until the promotion checklist
below is complete and a real wasm job exists.

`x86_64-unknown-freebsd` and `x86_64-unknown-linux-musl` are **build-only**
signal targets: they prove the tree links on those targets, exercise no tray and
produce no release artifacts. All Unix jobs require Zig 0.14.1
(`spawn-core-sys/build.rs`); FreeBSD gets it inside the cross-rs container via
`.github/cross-freebsd.toml`.

Audit-v38 **C07 L69** (cross-platform CI) and **L70** (reproducible local dev).
PR CI and release already cover linux + macOS + Windows (L69 score **2**).
This doc records the gap to score **3** (`+freebsd+wasm+musl`) and sketches
local/cross experiments without widening the required matrix yet.

## Current CI matrix (evidence)

| Surface | Targets / runners | L69 role |
|---------|-------------------|----------|
| `.github/workflows/ci.yml` | `ubuntu-24.04`, `macos-latest`, `windows-latest` — clippy, nextest, build | Score-2 bar met |
| `.github/workflows/release.yml` | `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` | Release parity only |
| `crates/spawn-core-sys/build.rs` | Zig static lib on Unix; Rust stub on Windows | Blocks naive wasm/freebsd without work |
| `just dev` | Rust + Zig 0.14.1 + `--help` smoke | L70 score **2** (`task dev` present; seed-data deferred) |

**Gaps for L69 score 3**

| Target | Blocker | Effort |
|--------|---------|--------|
| `x86_64-unknown-freebsd` | No GHA runner; cross from Linux needs Zig libc + spawn-core POSIX | L |
| `wasm32-wasip1` / `wasm32-wasi` | `spawn_core` uses `fork`/`waitpid`/pthread — no WASI equivalent | L |
| `*-unknown-linux-musl` | glibc vs musl link of `libspawn_core.a`; untested in PR matrix | M |

## Zig / wasm32-wasi sketch

`spawn-core` is built with `zig build` (Linux) or `zig build-obj` + `ar` (macOS).
WASI has no `posix_spawn` / `fork` — the hot path cannot link as-is.

**Soft experiment (non-CI, document-only today):**

```bash
# 1. Add target (rustup)
rustup target add wasm32-wasip1

# 2. Cross-compile sharecli without spawn-core (stub path TBD)
#    Today: build.rs always links spawn_core on non-Windows Unix.
#    Promotion: cfg(wasm) branch mirroring windows stub in spawn-core-sys.
export CARGO_CFG_TARGET_OS=wasip1   # sketch — needs explicit build.rs branch
cargo zigbuild --target wasm32-wasip1 -p sharecli --no-default-features 2>&1 | tee /tmp/wasm-sketch.log
```

**Promotion path**

1. `spawn-core-sys`: `#[cfg(any(windows, target_os = "wasi"))]` → Rust stub (same as Windows).
2. `build.rs`: skip Zig when `CARGO_CFG_TARGET_OS` is `wasi` (mirror Windows early-return).
3. Soft workflow `portability-wasm-soft.yml`: weekly `cargo check --target wasm32-wasip1` on Ubuntu, `continue-on-error: true`.
4. Pure-Rust modules (`wasm_opcode`, config validators) compile first; CLI/process features stay `cfg(not(target_arch = "wasm32"))`.

`src/wasm_opcode.rs` is a host-side disassembly helper — not a WASI runtime artifact — but
validates that part of the tree is wasm-friendly without libc spawn.

## FreeBSD cross notes

FreeBSD is closer to Linux for `spawn_core` (POSIX), but GHA has no `freebsd-latest`.

**Cross from Ubuntu (sketch):**

```bash
rustup target add x86_64-unknown-freebsd

# Zig 0.14.1 (already pinned in CI / devcontainer)
export ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-$HOME/.cache/zig}"

# Build spawn-core for FreeBSD via Zig target triple
cd crates/spawn-core
zig build -Dtarget=x86_64-freebsd-none -Doptimize=ReleaseSafe
cd ../..

# Link Rust crate against zig-out/lib/libspawn_core.a
export CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER=zig
export RUSTFLAGS="-C linker=zig -C link-arg=-target -C link-arg=x86_64-freebsd"
cargo build --target x86_64-unknown-freebsd -p sharecli --locked
```

**Gotchas**

- `build.rs` today branches `macos` vs `linux` only — add `freebsd` → same path as Linux (`zig build` + `libc`/`pthread`).
- Validate `setpriority`, `posix_spawn`, and semaphore paths on real FreeBSD before hard CI.
- Soft CI option: `cross` or `cargo-zigbuild` job on `ubuntu-24.04` with `continue-on-error: true` (no VM).

## musl (adjacent)

Rubric L69 score 3 lists musl with freebsd/wasm. Linux musl is a smaller lift than WASI:

```bash
rustup target add x86_64-unknown-linux-musl
# cargo-zigbuild or zig cc -target x86_64-linux-musl when linking spawn_core.a
cargo zigbuild --target x86_64-unknown-linux-musl -p sharecli
```

Add to PR matrix only after spawn-core musl link is green locally for one week.

## L70 tie-in

`just dev` bootstraps Rust + Zig + build smoke but does not seed fixtures or verify
cross targets. Hard L70 (`one command+verify+seed-data`) is unchanged; this doc does
not claim L70 lift — portability experiments are opt-in behind documented commands.

## Hard promotion checklist

- [ ] `spawn-core-sys` wasm/wasi stub + `build.rs` early-return
- [ ] `spawn-core-sys` freebsd branch in `build.rs` (linux-like link)
- [ ] Soft workflows: `portability-wasm-soft.yml`, `portability-freebsd-soft.yml` (weekly, `continue-on-error`)
- [ ] One-week green soft runs → optional PR matrix row or release archive entry
