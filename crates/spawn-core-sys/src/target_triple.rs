// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Cargo target triple -> Zig target triple mapping.
//!
//! `build.rs` compiles the Zig `spawn-core` static library, and Zig must be told
//! which target to build for. Without `-Dtarget`, `zig build` always targets the
//! **host**, so cross-compiling to `x86_64-unknown-linux-musl` on a glibc host
//! produced a glibc-ABI object (`mmap64`, `openat64`, `pread64`, `getcontext`)
//! that then failed to link against musl's self-contained crt.
//!
//! Zig spells targets without the vendor component: `x86_64-linux-musl`,
//! `x86_64-freebsd`, `x86_64-macos`. This module keeps that translation a small
//! pure function so it is unit-testable (`cargo test -p spawn-core-sys`).

/// Translate a Cargo target triple into Zig's vendor-less spelling.
///
/// Drops the vendor component (`<arch>-<vendor>-<rest>` -> `<arch>-<rest>`),
/// which is what Zig expects:
///
/// | Cargo                    | Zig            |
/// |--------------------------|----------------|
/// | `x86_64-unknown-linux-musl` | `x86_64-linux-musl` |
/// | `x86_64-unknown-linux-gnu`  | `x86_64-linux-gnu`  |
/// | `aarch64-unknown-linux-gnu` | `aarch64-linux-gnu` |
/// | `x86_64-unknown-freebsd`    | `x86_64-freebsd`    |
/// | `x86_64-apple-darwin`       | `x86_64-macos`      |
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
    use super::cargo_target_to_zig;

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
}
