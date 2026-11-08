// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `Stage` — the public enumeration of execution-environment kinds that
//! KDesktopVirt (a.k.a. `kvdesktop`) can drive a virtual desktop on.
//!
//! This module is intentionally small and side-effect-free: it is the
//! minimal, Hash-derivable vocabulary the rest of the crate uses to
//! describe *what kind of surface* a workflow is targeting. Modelling it
//! as a flat C-style enum (no `Arc<dyn …>`, no `String`, no `f64`) means
//! `#[derive(Hash)]` is sound and the type can be used directly as a
//! `HashMap` key, a `HashSet` member, or a `BTreeMap` key without
//! hand-rolled hashing.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// The kind of virtual stage a KDesktopVirt workflow can target.
///
/// Every variant is a unit variant — there is no associated data — so
/// `Hash`, `Eq`, and `Copy` are all trivially derivable. Keep it that
/// way: as soon as a variant gains a non-`Hash` payload (a `String`, a
/// `f64`, a `dyn Trait`) the derive will break and the type will lose
/// its use as a map key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// A container-based stage (Podman / Docker / OCI runtime).
    Container,
    /// A hardware-virtualised stage (KVM / QEMU / hypervisor).
    VirtualMachine,
    /// A stage with no display surface attached.
    Headless,
    /// A stage fronted by an X11 display server.
    X11,
    /// A stage fronted by a Wayland compositor.
    Wayland,
    /// A stage running directly on host hardware.
    BareMetal,
}

impl Stage {
    /// Stable, machine-friendly identifier for the stage kind.
    ///
    /// Useful as a directory name, a config-file key, or a label
    /// embedded in logs and metrics. The values are guaranteed not to
    /// change without a major-version bump.
    pub const fn as_str(self) -> &'static str {
        match self {
            Stage::Container => "container",
            Stage::VirtualMachine => "virtual_machine",
            Stage::Headless => "headless",
            Stage::X11 => "x11",
            Stage::Wayland => "wayland",
            Stage::BareMetal => "bare_metal",
        }
    }

    /// Iterator over every defined [`Stage`] variant, in declaration
    /// order. The order is part of the public contract: callers that
    /// build stable, sorted registries should sort the result themselves.
    pub const fn all() -> [Stage; 6] {
        [
            Stage::Container,
            Stage::VirtualMachine,
            Stage::Headless,
            Stage::X11,
            Stage::Wayland,
            Stage::BareMetal,
        ]
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// KDesktopError — the crate-level error type for the `kvdesktop` Stage API.
//
// Any I/O-producing helper that operates on a [`Stage`] (reading stage
// configuration, probing a display, writing a stage descriptor to disk,
// etc.) is expected to return `Result<_, KDesktopError>` so that callers
// only need to learn a single error vocabulary. The blanket
// `From<std::io::Error>` impl below lets those helpers use `?` directly
// against `std::io::Error` without each one having to write its own
// conversion glue.
// ============================================================================

/// Errors returned by the `kvdesktop` Stage API.
#[derive(Debug, thiserror::Error)]
pub enum KDesktopError {
    /// An underlying I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[source] std::io::Error),
    /// The requested stage is not one of the supported [`Stage`] variants.
    #[error("invalid stage: {0:?}")]
    InvalidStage(String),
}

impl From<std::io::Error> for KDesktopError {
    fn from(err: std::io::Error) -> Self {
        KDesktopError::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_display_impl_matches_as_str() {
        // The `Display` impl on `Stage` is a thin wrapper over `as_str`,
        // and it is the *only* public way to format a `Stage` for logs,
        // CLI banners, and metric labels. Pin the formatting to the
        // exact same `&'static str` values the rest of the crate
        // (directory names, config keys, etc.) already rely on so that
        // a future refactor of one cannot silently desync from the
        // other.
        let cases: &[(Stage, &str)] = &[
            (Stage::Container, "container"),
            (Stage::VirtualMachine, "virtual_machine"),
            (Stage::Headless, "headless"),
            (Stage::X11, "x11"),
            (Stage::Wayland, "wayland"),
            (Stage::BareMetal, "bare_metal"),
        ];
        for (stage, expected) in cases {
            assert_eq!(
                format!("{stage}"),
                *expected,
                "Display output for {stage:?} drifted from as_str()",
            );
            // `Display` and `as_str` must stay in lockstep — if either
            // changes, the other must change with it.
            assert_eq!(
                stage.to_string(),
                stage.as_str(),
                "Display and as_str disagreed for {stage:?}",
            );
        }
    }

    #[test]
    fn stage_works_as_hashmap_key() {
        // Build a registry mapping each Stage kind to the default width
        // we provision for it. This exercises the `Hash` derive: the
        // map has to be able to hash every variant, and `get` has to be
        // able to look each one back up.
        let mut defaults: HashMap<Stage, u32> = HashMap::new();
        defaults.insert(Stage::Container, 1280);
        defaults.insert(Stage::VirtualMachine, 1920);
        defaults.insert(Stage::Headless, 0);
        defaults.insert(Stage::X11, 1920);
        defaults.insert(Stage::Wayland, 1920);
        defaults.insert(Stage::BareMetal, 2560);

        // Round-trip every variant.
        for stage in Stage::all() {
            let expected = match stage {
                Stage::Container => 1280,
                Stage::VirtualMachine => 1920,
                Stage::Headless => 0,
                Stage::X11 => 1920,
                Stage::Wayland => 1920,
                Stage::BareMetal => 2560,
            };
            assert_eq!(
                defaults.get(&stage).copied(),
                Some(expected),
                "missing or wrong default for {stage}",
            );
        }

        // A known entry resolves; an inserted-then-removed entry does not.
        assert_eq!(defaults.remove(&Stage::Container), Some(1280));
        assert_eq!(defaults.get(&Stage::Container), None);
        assert_eq!(defaults.len(), 5);
    }

    #[test]
    fn stage_works_as_hashset_member() {
        // A `HashSet<Stage>` exercises the `Hash + Eq` derives the same
        // way a `HashMap<Stage, _>` does, but on the *set* side of the
        // collection: the set is responsible for hashing, equality, and
        // deduplication of every variant we insert. Build one from the
        // public `Stage::all()` iterator and verify the set reports the
        // correct cardinality — duplicates collapse, every distinct
        // variant survives, and `contains` round-trips for each one.
        let set: HashSet<Stage> = Stage::all().into_iter().collect();
        assert_eq!(
            set.len(),
            Stage::all().len(),
            "HashSet cardinality should equal the number of Stage variants",
        );

        // Re-inserting a variant that is already present must be a
        // no-op: this is what the `Eq + Hash` contract on `Stage` buys
        // us, and it is the whole point of choosing a set here.
        let mut set_with_dupes: HashSet<Stage> = HashSet::new();
        for _ in 0..3 {
            set_with_dupes.insert(Stage::Container);
            set_with_dupes.insert(Stage::X11);
            set_with_dupes.insert(Stage::Container);
        }
        assert_eq!(set_with_dupes.len(), 2);
        assert!(set_with_dupes.contains(&Stage::Container));
        assert!(set_with_dupes.contains(&Stage::X11));
        assert!(!set_with_dupes.contains(&Stage::BareMetal));

        // Every public variant must be retrievable from the canonical
        // set, and no variant should be missing — this catches a
        // future variant that is added to the enum but forgotten by
        // the set-under-test.
        for stage in Stage::all() {
            assert!(
                set.contains(&stage),
                "Stage::{stage:?} should be a member of the set built from Stage::all()",
            );
        }
    }

    #[test]
    fn kdesktop_error_from_io_error_via_question_mark() {
        // The `From<std::io::Error> for KDesktopError` impl is the
        // contract that lets any helper returning `Result<_, KDesktopError>`
        // propagate an I/O error with the `?` operator without writing
        // per-helper conversion glue. Pin that contract end-to-end:
        // build a real `std::io::Error`, run it through `?` into a
        // `KDesktopError`, and assert it lands in the `Io` variant with
        // the original error preserved as the source.

        // A guaranteed-failing read produces a real `std::io::Error`
        // (kind: `NotFound`) that we can convert.
        let io_result: std::io::Result<Vec<u8>> =
            std::fs::read("/this/path/definitely/does/not/exist/kvdesktop-test");
        let io_err = io_result.expect_err("read against a missing path must fail");

        // `?` must apply the `From<std::io::Error>` impl for us.
        let converted: KDesktopError = (|| -> Result<(), KDesktopError> {
            let _bytes = std::fs::read("/another/missing/kvdesktop/path")?;
            Ok(())
        })()
        .expect_err("the closure must short-circuit with the I/O error");

        // The produced error must be the `Io` variant, not the
        // `InvalidStage` variant — i.e. the conversion routes to the
        // expected arm and not some other variant that happens to
        // also exist on the enum.
        match &converted {
            KDesktopError::Io(src) => {
                assert_eq!(
                    src.kind(),
                    io_err.kind(),
                    "the converted error must preserve the original io::ErrorKind",
                );
            }
            KDesktopError::InvalidStage(_) => {
                panic!("I/O error was routed to InvalidStage instead of Io");
            }
        }

        // The `Display` chain must announce the I/O wrapper, so logs
        // and `?`-propagated error chains stay informative.
        let rendered = converted.to_string();
        assert!(
            rendered.contains("I/O error"),
            "Display output should announce the I/O wrapper, got: {rendered:?}",
        );
    }
}
