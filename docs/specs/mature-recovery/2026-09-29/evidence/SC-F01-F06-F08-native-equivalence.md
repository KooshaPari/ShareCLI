# Native evidence receipt — FR-008 equivalence-domain falsification

Date: 2026-09-30.

## Exact execution
- Candidate branch commit: `ed0616d8ff03488e3c73981c95851513ee42103b`
- Mature Recovery Oracle run: `36686200573`
- Job: `109792530472`
- Zig: 0.14.1 provisioned
- Product test package: `sharecli-core`

## Results

### SC-F01 — Git mode changed bytes
**NATIVE COUNTEREXAMPLE REPRODUCED.**

A real Git fixture retained equivalent HEAD/status shape while `input.txt` changed from `first edit` to `second edit`. The second real Hypervisor invocation returned `first edit` from cache.

### SC-F06 — Args mode crosses workspaces
**NATIVE COUNTEREXAMPLE REPRODUCED.**

Same argv executed in two distinct workspaces whose files returned `workspace-a` vs `workspace-b`. Args mode replayed the first workspace result in the second because cwd/input identity is deliberately absent from the key.

### SC-F07 — Git mode ignores environment
**NATIVE COUNTEREXAMPLE REPRODUCED.**

The same Git state/argv with relevant environment changing from `one` to `two` replayed `one`.

### SC-F08 — Time mode replays external state
**NATIVE COUNTEREXAMPLE REPRODUCED.**

A command reading external state changed from `outside-one` to `outside-two` while the Time-mode key dimensions remained stable. Hypervisor replayed `outside-one`.

## Secondary observations

FUSE mounting failed in the CI environment because `allow_other` is not enabled in `/etc/fuse.conf`; core explicitly continued without FUSE interception. This is useful evidence for truthful optional/degraded-mode reporting but is not the cause of the cache failures.

Cargo emitted a duplicate-key diagnostic from a fetched PhenoShared/agileplus-cache manifest during the second command, yet the targeted equivalence matrix compiled and ran all three tests. Treat that dependency diagnostic as a separate source-quality finding, not as the product result.

## Architecture conclusion

The evidence falsifies the hypothesis that the mature solution is a single generic cache-key mode over arbitrary commands. All three historical key modes have semantic domains in which replay is incorrect.

The architecture gate should therefore prefer:
1. default execution/bypass for unknown equivalence;
2. explicit tool/operation equivalence adapters;
3. separate in-flight duplicate suppression from durable replay;
4. delegation to stronger tool-native caches where available;
5. evidence of adapter identity and relevant inputs before a result becomes shareable.

No production remediation is included in this receipt.
