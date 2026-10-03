# Native counterexample collection contract — 2026-10-02

Scope: SC-WP-A05 / FR-008 evidence correctness. No scheduler, cache, speculation or default execution behavior is changed by this collector patch.

Reviewed parent: `209d3c98a5cbd1c1c6d8fa307605cc0907a0e378`.

## Defect in the previous collector

The FR-008 job ignored two cargo command failures with `|| true` and classified any nonzero result from the in-flight test as the expected native counterexample. A missing compiler, malformed test, missing Zig, failed build, zero-test selection or unrelated panic could therefore be mistaken for evidence of the known cache defect. Unexpected test success was also incorrectly described as proof that the defect remained.

## Replacement

The job checks out the exact candidate head explicitly. The collector records the requested and actual checkout, rejects tracked-source drift, records cargo/rustc/Zig versions, compiles each named integration-test executable separately with `--no-run --message-format=json`, hashes the binary, and invokes each named test directly using exact selection and one test thread.

Expected failure requires the named test, its complete single-test libtest summary, the named assertion panic, the exact source-reviewed assertion message, and exit status 101. The three integration targets cover eight named native cases. A successful test is COUNTEREXAMPLE_NOT_REPRODUCED, requiring review rather than a claim that the product is unsafe. Build errors, missing binaries, unexpected panics, missing tests, ignored tests, malformed output, timeouts and signals cannot satisfy the negative oracle.

Every receipt records `green: false` and `satisfies_criterion: false`: reproducing a defect is diagnostic evidence, never product acceptance. The CI collector can succeed at gathering that evidence without qualifying the cache. Raw logs and receipts are uploaded even after collector failure.

## Executed validation and limits

Seventeen local Python tests pass, including arbitrary-nonzero/build-error rejection, exact-name/assertion binding, current Rust panic thread-ID formatting, missing/ignored/zero-test controls, truncated output, timeout process-group termination, successful-build executable identity and unexpected-pass classification. Python compilation and workflow YAML/shell syntax checks pass. A real invocation with cargo unavailable returned COLLECTOR_FAILURE, zero native cases and non-green flags, as required.

Native Rust tests were NOT_RUN locally because cargo/rustc are unavailable. The Python tests validate the collector and synthetic rejection controls; they do not prove the ShareCLI runtime or B05/B06 product criteria. Native CI is required after this commit.

## B06 findings retained as blockers

Source inspection of `speculation.rs` at this parent found that `ordinary_cache_hits_are_not_speculation_authority` calls `record_eligible_hit(...ExplicitlyReadOnly)` rather than `record_hit`, contradicting its own empty-result assertion. Request/eligibility changes reuse the first stored key entry; expiry does not reset/prune stale entries; eligibility is filtered after truncation. These findings are not repaired or qualified by this collector commit. B06 remains BLOCKED pending a bounded semantic repair and native positive/negative tests.
