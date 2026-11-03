// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C08 L76 FR-003 gate - Harbor 7d soak mirrored visibility (Wave17 onward)
//! FR: FR-003
//! Scope: honest mirror of the EXTRACTED Harbor soak; asserts 0/7 (no false claim).

use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    fs::read_to_string(path).expect("file must exist")
}

#[test]
fn c08_l76_harbor_7d_mirror_exists() {
    // The visibility mirror must exist so sharecli auditors see the honest soak state.
    assert!(
        Path::new("docs/eval/harbor-7d.log").exists(),
        "docs/eval/harbor-7d.log mirror must exist"
    );
}

#[test]
fn c08_l76_mirror_is_marked_zero_of_seven() {
    // Honest status: the 7d soak is NOT complete. The mirror must say 0/7.
    let content = read("docs/eval/harbor-7d.log");
    assert!(
        content.contains("0 / 7") || content.contains("0/7"),
        "mirror must state the soak is 0/7, not claim completion"
    );
}

#[test]
fn c08_l76_mirror_does_not_claim_completion() {
    // Guard against a false lift: the mirror must never advertise 7/7.
    let content = read("docs/eval/harbor-7d.log");
    assert!(
        !content.contains("7 / 7") && !content.contains("7/7"),
        "mirror must not claim a completed 7-day soak"
    );
}

#[test]
fn c08_l76_mirror_points_to_canonical_lane() {
    // The mirror must hand off to the canonical EXTRACTED soak location.
    let content = read("docs/eval/harbor-7d.log");
    assert!(
        content.contains("benchora/harbor-soft"),
        "mirror must reference the canonical benchora/harbor-soft lane"
    );
    assert!(
        content.contains("portage-temp"),
        "mirror must reference the Harbor fork/env portage-temp"
    );
}

#[test]
fn c08_l76_stub_doc_still_marks_extracted() {
    // The soft-stub doc remains valid: it must still state EXTRACTED for the hard log.
    let content = read("docs/eval/harbor-soft-stub.md");
    assert!(content.contains("EXTRACTED"), "stub doc must still mark EXTRACTED");
    assert!(content.contains("benchora/harbor-soft"), "stub doc must reference EXTRACTED lane");
}

#[test]
fn c08_l76_no_live_harbor_workflow() {
    // Per ADR 0002 sharecli does not host Harbor workflows. Mirror is doc-only.
    assert!(
        !Path::new(".github/workflows/harbor.yml").exists(),
        "no live Harbor workflow should exist in sharecli main"
    );
}
