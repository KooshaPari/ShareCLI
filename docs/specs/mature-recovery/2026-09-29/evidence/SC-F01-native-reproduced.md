# Native evidence receipt — FR-008 changed-input semantic identity

Date: 2026-09-30.

## Subject
Git-mode result reuse must not return stale command output when relevant input bytes change while Git HEAD and porcelain status shape remain equivalent.

## Exact evidence
- Product: `KooshaPari/ShareCLI`
- Recovery workflow commit: `4f1b6e7e9b4c31f38965e63b7eae42ccdfbfdda7`
- Workflow run: `36627656798`
- Job: `109609724650` — FR-008 semantic identity oracle
- Test: `crates/sharecli-core/tests/recovery_fr008_semantic_identity.rs::fr008_git_mode_must_not_reuse_output_after_content_changes`
- Toolchain dependency: Zig 0.14.1 provisioned because `spawn-core-sys` requires it.

## Observed result
The real `sharecli-core::Hypervisor` test compiled and executed.

First invocation observed:
`first edit\n`

The fixture then rewrote the same already-modified file to:
`second edit\n`

The second Hypervisor invocation returned:
`first edit\n`

The assertion failed with:

`FR-008 false green: Git-mode cache reused output although input bytes changed`

The same run attempted FUSE interception and logged that `allow_other` was unavailable because `user_allow_other` was not set; core proceeded without FUSE interception. This is useful mode evidence but not the cause of the cache-key false hit.

## Classification
**SC-F01 = NATIVE COUNTEREXAMPLE REPRODUCED.**

This confirms the source/model finding on the real Hypervisor path for the exact recovery candidate/configuration. It does not prove all cache modes fail, that all commands are unsafe to share, or that a particular replacement architecture is correct.

## Architecture consequence
A Git HEAD + porcelain fingerprint is not sufficient input identity for durable arbitrary-command result reuse. Remediation must be defined by an accepted equivalence domain/adapter; simply adding another generic field is not enough to establish correctness.

The FUSE degradation in the same run also reinforces the independent capability model: command mediation/result reuse and filesystem interception have distinct evidence identities.
