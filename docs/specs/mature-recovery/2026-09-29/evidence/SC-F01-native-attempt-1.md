# Native evidence receipt — FR-008 semantic identity attempt 1

Date: 2026-09-29.

## Subject
ShareCLI Git-mode result reuse after file bytes change while HEAD/status remain equivalent.

## Candidate
- Adversarial test commit: `f1dd74bfed1f08d4a2cc1ab7deba63b7b3b6b1a9`
- Workflow run: `36615343370`
- re-run job: `109572692771`

## Result
The exact re-run **did not execute the adversarial test**.

The job failed during workspace build because Linux `tray-icon 0.25.0` was compiled without either required `libappindicator` or `ksni` feature. The later unit-test stage was skipped.

## Classification
**ORACLE NOT EXECUTED / INFRASTRUCTURE-BLOCKED / PRODUCT RESULT UNKNOWN.**

This failure is not evidence for or against SC-F01.

## Corrective evidence action
The same test has been copied under `crates/sharecli-core/tests/` and a dedicated recovery workflow now runs only:

`cargo test -p sharecli-core --test recovery_fr008_semantic_identity -- --nocapture`

This isolates the real Hypervisor/cache path from unrelated tray targets without changing product behavior. The earlier source/model counterexample remains valid as model evidence, but native reproduction is not upgraded until the targeted workflow executes.
