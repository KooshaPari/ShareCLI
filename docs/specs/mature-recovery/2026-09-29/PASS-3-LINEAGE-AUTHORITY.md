# ShareCLI — pass 3: recovered lineage authority and mechanism/domain split

Date: 2026-09-29. Frozen analyzed source remains `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

## Commit archaeology

Two important July commits were inspected directly.

### SC-S18 — OS-adjacent thesis restoration

Commit `7d49102f5a2994c28fbbc67c8d7575cb458665c9` (2026-07-19) explicitly says it restores OS-adjacent product framing, moves Harbor evaluation stubs out, and reframes detection/watch/coalesce/mesh. It is co-authored by KooshaPari and Cursor.

Classification: **ACCEPTED-DESIGN CANDIDATE with user co-authorship, not automatically direct user intent.** It is materially stronger than a pure bot scorecard commit, but the original conversation/decision source should still be recovered before treating every mechanism named there as mandatory mature behavior.

### SC-S19 — February harness recovery implementation

Commit `b71460f03c3089082c6eaf9db1ad9853198a59ac` (2026-07-20) ports queue/nocache behavior, Maildir mesh substrate, FUSE read coalescing/write serialization and expands FR-008..010 acceptance. It is co-authored by KooshaPari and Cursor.

Classification: **ACCEPTED/HISTORICAL IMPLEMENTATION RECOVERY**, but mechanism parity is not product authority. The recovery commit proves intent to recover capabilities; it does not prove every historical algorithm is semantically correct or optimal.

## Mechanism versus obligation partition

The mature contract should preserve outcomes, not blindly preserve recovered algorithms.

| Recovered mechanism | Underlying obligation candidate | Mechanism status |
|---|---|---|
| CoalesceCache/TTL/debounce | avoid duplicate authorized equivalent work | contested; durable arbitrary-command replay unsafe without semantic adapter |
| SlotQueue/PriorityQueue | bounded admission/fair ordering under contention | algorithm contested; ownership/fencing/fairness semantics must be defined first |
| FUSE read coalescing/CoW | coordinate filesystem work and optionally reduce duplicate IO/conflicts | adapter candidate; required/optional mode and platform burden unresolved |
| Maildir mesh | durable local work coordination/recovery | implementation candidate, not identity |
| proc scan/agent family detection | observe heterogeneous agent workloads without replacing vendor binaries | strong candidate mature obligation |
| Harbor evaluation | benchmark/evaluate agents | explicitly moved out of ShareCLI product boundary |

This partition is essential for SOTA bootstrap decisions: a better external primitive may replace a historical mechanism without deleting the accepted user outcome.

## Existing native oracle status

The new FR-008 semantic-identity test remains on the specification branch. Exact-current-head CI is queued at this pass; no native result is claimed yet. A failing scorecard or unrelated job cannot substitute for this criterion.

## Next lineage closure

- recover original February agent-harness source revision and user conversation authority;
- compare donor behavior against the July port, not just filenames;
- identify external callers that actually pass through Hypervisor/FUSE/mesh;
- decide which recovered outcomes are mandatory versus optional adapters;
- then derive semantic obligations independent of historical implementation shape.
