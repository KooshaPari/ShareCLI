// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C06 L54 — network-block hermetic hard gate wired into `ci-success` (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C06 L54 — the netblock hermetic gate must be hard and must run on
/// PRs. The `netblock` job used to live in the deleted
/// `.github/workflows/ci.yml` (feeding the `ci-success` aggregator); it now
/// lives in `ci-gate.yml` as the `ci-netblock` job.
#[test]
fn fr003_ci_gate_netblock_hard_gate_wired() {
    let ci = fs::read_to_string(repo_root().join(".github/workflows/ci-gate.yml"))
        .expect("read ci-gate.yml");

    assert!(
        ci.contains("name: netblock hermetic (required)"),
        "ci-gate.yml must define the hard netblock job"
    );
    assert!(
        ci.contains("bash scripts/ci/netblock_check.sh"),
        "ci-gate.yml netblock job must run netblock_check.sh"
    );

    // PR-time coverage must be restored: the deleted ci.yml `netblock` job fed
    // the `ci-success` aggregator on every PR.
    assert!(
        ci.contains("pull_request:"),
        "ci-gate.yml must trigger on pull_request so the deleted ci.yml netblock coverage is not lost"
    );

    let netblock_section = ci.split("ci-netblock:").nth(1).expect("netblock job block");

    // The job block comments explain that `netblock-soft.yml` runs the same
    // script under `continue-on-error: true`; only real YAML keys count here.
    let netblock_keys = netblock_section
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !netblock_keys.contains("continue-on-error: true"),
        "netblock job must not use continue-on-error"
    );

    // A hard job that nothing depends on does not gate anything. The gate
    // only blocks a merge when a REQUIRED context fails because of it.
    // `ci / lint` is a required branch-protection context and is also the
    // context `.mergify.yml` gates automatic merges on, so `ci-lint` must
    // declare `needs: [ci-netblock]`.
    //
    // Parse the workflow instead of slicing text: a text slice of `ci-lint:`
    // runs on into the LATER jobs, so a `needs:` belonging to another job
    // could satisfy a substring check that does not actually constrain
    // `ci-lint`.
    let doc: serde_yaml::Value = serde_yaml::from_str(&ci).expect("ci-gate.yml must be valid YAML");
    let jobs = doc
        .get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .expect("ci-gate.yml must define a jobs mapping");

    let job = |id: &str| -> serde_yaml::Value {
        jobs.get(serde_yaml::Value::String(id.to_string()))
            .cloned()
            .unwrap_or(serde_yaml::Value::Null)
    };

    let lint = job("ci-lint");
    let needs = lint.get("needs").expect("ci-lint job must declare needs");
    let need_ids: Vec<String> = match needs {
        serde_yaml::Value::String(s) => vec![s.clone()],
        serde_yaml::Value::Sequence(seq) => {
            seq.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
        }
        other => panic!("ci-lint needs must be a string or a sequence, got {other:?}"),
    };
    assert!(
        need_ids.iter().any(|n| n == "ci-netblock"),
        "the ci-lint job (required context 'ci / lint') must depend on ci-netblock, otherwise a failed netblock gate does not block the merge; got needs={need_ids:?}"
    );

    // The dependency alone is not enough: with the default `success()`
    // condition a skipped or failed dependency would skip `ci-lint` entirely
    // instead of failing it, and a skipped required context does not block.
    // `if: always()` is what makes the gate result reach the failure step.
    assert_eq!(
        lint.get("if").and_then(serde_yaml::Value::as_str),
        Some("always()"),
        "ci-lint must use `if: always()` so a failed dependency is evaluated rather than skipping the required context"
    );

    // The failure must actually be propagated: the job needs a step that
    // receives the dependency result and exits non-zero on failure. The
    // result reaches the step through an `env:` binding, so assert that
    // binding rather than a literal expression inside the script.
    let steps = lint
        .get("steps")
        .and_then(serde_yaml::Value::as_sequence)
        .expect("ci-lint job must declare steps");
    let gate_step = steps
        .iter()
        .find(|step| {
            step.get("name")
                .and_then(serde_yaml::Value::as_str)
                .is_some_and(|n| n.contains("hard gates"))
        })
        .expect("ci-lint must have a step that verifies the hard gates");
    let env = gate_step.get("env").and_then(serde_yaml::Value::as_mapping);
    let env_pairs: Vec<(String, String)> = env
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        env_pairs
            .iter()
            .any(|(_, v)| v.contains("needs.ci-netblock.result")),
        "the gate-verification step must bind needs.ci-netblock.result into its environment, otherwise it cannot detect a failed gate; got env={env_pairs:?}"
    );
    let run = gate_step.get("run").and_then(serde_yaml::Value::as_str).unwrap_or_default();
    assert!(
        run.contains("exit 1"),
        "the gate-verification step must exit non-zero when a hard gate fails, otherwise the required context still passes"
    );
}

/// FR-003 / C06 L54 — hermetic contract documents hard CI enforcement.
#[test]
fn fr003_hermetic_builds_doc_hard_gate() {
    let doc = fs::read_to_string(repo_root().join("docs/ops/hermetic-builds.md"))
        .expect("read hermetic-builds.md");

    assert!(
        doc.contains("ci-gate.yml"),
        "hermetic-builds.md must document the ci-gate.yml hard gate"
    );
    assert!(doc.contains("ci-netblock"), "hermetic-builds.md must reference the ci-netblock job");
    assert!(doc.contains("netblock"), "hermetic-builds.md must reference netblock enforcement");
}
