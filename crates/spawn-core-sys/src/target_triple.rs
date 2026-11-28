// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Cargo target triple -> Zig target triple mapping.
//!
//! `build.rs` compiles the Zig `spawn-core` static library, and Zig must be told
//! which target to build for. Without `-Dtarget`, `zig build` always targets the
//! **host**, so cross-compiling to `x86_64-unknown-linux-musl` on a glibc host
//! produced a glibc-ABI object (`mmap64`, `openat64`, `pread64`, `getcontext`)
//! that then failed to link against musl's self-contained crt.
//!
//! Two translations are needed, not one:
//!
//! 1. Zig spells targets without the vendor component: `x86_64-linux-musl`,
//!    `x86_64-freebsd`, `x86_64-macos`.
//! 2. Zig renames several architectures relative to Rust. `i686`, `riscv64gc`
//!    and `armv7` are Rust names; Zig (0.14.1, verified with `zig targets`)
//!    knows only `x86`, `riscv64` and `arm`. Passing either through unchanged
//!    makes `zig build` exit with `UnknownArchitecture`.
//!
//! This module keeps both translations in small pure functions so they are
//! unit-testable (`cargo test -p spawn-core-sys`).

/// Translate a Cargo architecture name into Zig's spelling.
///
/// Only the names Zig actually renames are mapped; everything else passes
/// through unchanged, so architectures Zig already accepts under their Rust
/// name (`x86_64`, `aarch64`, `powerpc64le`, `s390x`, `loongarch64`, …) are
/// preserved. Zig's `-Dcpu=baseline` handles ISA extensions, so `riscv64gc`
/// and `riscv64imac` collapse onto the bare `riscv64` arch.
fn zig_arch(cargo_arch: &str) -> &str {
    match cargo_arch {
        // Rust distinguishes 32-bit x86 sub-generations; Zig has one name.
        "i686" | "i586" | "i386" => "x86",
        // Rust encodes the RISC-V ISA extension in the arch component; Zig
        // selects extensions through `-Dcpu` instead.
        "riscv64gc" | "riscv64imac" => "riscv64",
        _ if cargo_arch.starts_with("armv") => "arm",
        _ if cargo_arch.starts_with("thumbv") => "thumb",
        // Never renamed by Zig: x86_64, aarch64, arm, thumb, powerpc64le,
        // s390x, loongarch64, sparc64, … pass through untouched.
        other => other,
    }
}

/// Translate a Cargo target triple into Zig's vendor-less, Zig-spelled triple.
///
/// Drops the vendor component (`<arch>-<vendor>-<rest>` -> `<arch>-<rest>`) and
/// renames the architecture where Zig disagrees with Rust:
///
/// | Cargo                        | Zig                  |
/// |------------------------------|----------------------|
/// | `x86_64-unknown-linux-musl`  | `x86_64-linux-musl`  |
/// | `aarch64-unknown-linux-gnu`  | `aarch64-linux-gnu`  |
/// | `x86_64-apple-darwin`        | `x86_64-macos`       |
/// | `i686-unknown-linux-gnu`     | `x86-linux-gnu`      |
/// | `riscv64gc-unknown-linux-gnu`| `riscv64-linux-gnu`  |
/// | `armv7-unknown-linux-gnueabihf` | `arm-linux-gnueabihf` |
/// | `thumbv7em-none-eabihf`      | `thumb-none-eabihf`  |
/// | `powerpc64le-unknown-linux-gnu` | `powerpc64le-linux-gnu` (unchanged) |
///
/// Returns `None` for a triple that is not in `<arch>-<vendor>-<rest>` shape (or
/// is empty), so the caller can fall back to Zig's host default instead of
/// failing the build.
pub fn cargo_target_to_zig(target: &str) -> Option<String> {
    let mut parts = target.split('-');
    let arch = parts.next()?;
    // Vendor component: present in every Cargo triple we cross-compile for, and
    // never meaningful to Zig.
    let vendor = parts.next()?;
    let mut rest: Vec<&str> = parts.collect();
    if arch.is_empty() || vendor.is_empty() || rest.is_empty() || rest.iter().any(|p| p.is_empty())
    {
        return None;
    }
    // Cargo spells Apple's OS `darwin`; Zig spells it `macos`.
    if rest[0] == "darwin" {
        rest[0] = "macos";
    }

    let arch = zig_arch(arch);
    let mut out = String::with_capacity(target.len());
    out.push_str(arch);
    for part in rest {
        out.push('-');
        out.push_str(part);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{cargo_target_to_zig, zig_arch};

    #[test]
    fn maps_cargo_triples_to_zig_vendor_less_triples() {
        let cases = [
            ("x86_64-unknown-linux-musl", Some("x86_64-linux-musl")),
            ("x86_64-unknown-linux-gnu", Some("x86_64-linux-gnu")),
            ("aarch64-unknown-linux-gnu", Some("aarch64-linux-gnu")),
            ("aarch64-unknown-linux-musl", Some("aarch64-linux-musl")),
            ("x86_64-unknown-freebsd", Some("x86_64-freebsd")),
            ("x86_64-apple-darwin", Some("x86_64-macos")),
            ("aarch64-apple-darwin", Some("aarch64-macos")),
            // Not `<arch>-<vendor>-...` shaped: fall back to Zig's host default.
            ("", None),
            ("x86_64", None),
            ("x86_64-unknown", None),
            ("x86_64--linux-gnu", None),
        ];
        for (cargo, expected) in cases {
            assert_eq!(cargo_target_to_zig(cargo).as_deref(), expected, "mapping for {cargo:?}");
        }
    }

    #[test]
    fn renames_architectures_zig_spells_differently() {
        let cases = [
            // 32-bit x86: Rust sub-generation -> Zig `x86`.
            ("i686-unknown-linux-gnu", "x86-linux-gnu"),
            ("i586-unknown-linux-gnu", "x86-linux-gnu"),
            ("i386-unknown-linux-gnu", "x86-linux-gnu"),
            // RISC-V: Rust ISA suffix -> bare Zig arch.
            ("riscv64gc-unknown-linux-gnu", "riscv64-linux-gnu"),
            ("riscv64imac-unknown-none-elf", "riscv64-none-elf"),
            // ARM: Rust family/generation -> Zig `arm`.
            ("armv7-unknown-linux-gnueabihf", "arm-linux-gnueabihf"),
            ("armv7-unknown-linux-gnueabi", "arm-linux-gnueabi"),
            ("armv6-unknown-linux-gnueabihf", "arm-linux-gnueabihf"),
            // Thumb: Rust generation -> Zig `thumb`. These are the four-part
            // POSIX-shaped triples this crate can actually build for; bare-metal
            // three-part triples (`thumbv7em-none-eabihf`) are out of scope (the
            // lib needs libc/pthread) and keep the historical vendor-drop shape.
            ("thumbv7neon-unknown-linux-gnueabihf", "thumb-linux-gnueabihf"),
            ("thumbv8m-unknown-linux-gnueabihf", "thumb-linux-gnueabihf"),
        ];
        for (cargo, expected) in cases {
            assert_eq!(
                cargo_target_to_zig(cargo).as_deref(),
                Some(expected),
                "mapping for {cargo:?}"
            );
        }
    }

    #[test]
    fn passes_through_arches_zig_already_names_the_rust_way() {
        // These names exist verbatim in `zig targets` (0.14.1). Asserting them
        // locks in that nobody "fixes" a non-bug: Zig does NOT use `ppc64le`
        // for `powerpc64le`.
        let cases = [
            ("powerpc64le-unknown-linux-gnu", "powerpc64le-linux-gnu"),
            ("s390x-unknown-linux-gnu", "s390x-linux-gnu"),
            ("loongarch64-unknown-linux-gnu", "loongarch64-linux-gnu"),
            ("sparc64-unknown-linux-gnu", "sparc64-linux-gnu"),
        ];
        for (cargo, expected) in cases {
            assert_eq!(
                cargo_target_to_zig(cargo).as_deref(),
                Some(expected),
                "mapping for {cargo:?}"
            );
        }
    }

    #[test]
    fn unknown_architecture_passes_through_unchanged() {
        // A name we do not recognise is handed to Zig as-is; `zig build` then
        // fails loudly rather than silently building for the host.
        assert_eq!(zig_arch("mips64el"), "mips64el");
        assert_eq!(zig_arch("x86_64"), "x86_64");
        assert_eq!(zig_arch("aarch64"), "aarch64");
        assert_eq!(zig_arch("arm"), "arm");
    }
}
