# ShareCLI bidirectional trace slice 01 — work sharing, admission, filesystem

Status: **DRAFT STRUCTURAL TRACE / not full graph**  
Date: 2026-09-30.  
Frozen source: `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

This graph does not inherit old FR-tag edges merely because names match. Every row states the semantic relation and evidence authority.

## Trace nodes

| Trace ID | Product obligation / decision | Authority / provenance | Implementation relation | Oracle | Exact evidence | Current state |
|---|---|---|---|---|---|---|
| SC-T01 | Unknown work equivalence → execute/bypass rather than durable replay | SC-AD-02; native falsification SC-F01/06/07/08; provisional accepted architecture | current `Hypervisor::run` violates broad interpretation because all modes feed durable CoalesceCache | changed bytes/workspace/env/external-state matrix | run `36686200573`, job `109792530472`; prior SC-F01 run `36627656798` job `109609724650` | **implementation conflict / native reproduced** |
| SC-T02 | In-flight suppression and durable replay are separate semantics | SC-AD-01; SOTA singleflight/specialized cache evidence | current Lock-Wait-Cache couples lock-time sharing and TTL durable cache | `recovery_fr008_inflight_vs_durable.rs` | test committed; exact current execution pending | **oracle pending** |
| SC-T03 | Equivalence adapter identity + relevant inputs bind shareability | ontology v0.1 + SC-AD-02 + native matrix | current generic CacheKeyMode has no adapter identity | wrong tool/env/workspace/external input controls | SC-F06/07/08 exact native failures | **contract supported; implementation missing** |
| SC-T04 | Observed != mediated != optimization-eligible | capability truth matrix; dispatcher/proc source inspection | proc scan and `helios-shield` proxy are separate source paths | observed-only vs proxy-passthrough vs optimized fixture | source evidence only; public runtime matrix pending | **partially mapped** |
| SC-T05 | Admission provider is explicit; native protocols preferred when applicable | SC-AD-03 + admission slice | current SlotQueue and historical jobserver-named strategies need provider mapping | native jobserver/nested-build prototype | not run | **architecture experiment pending** |
| SC-T06 | Queue ownership uses generation-safe identity | SC-MO-LEASE-001; donor liveness behavior; PID reuse threat | current ticket = priority.wallclock.PID.seq; no generation identity | dead PID + reused-live-PID fixtures | dead waiter/internal tests + `recovery_queue_pid_reuse.rs`; current exact workflow pending | **implementation conflict suspected; native current receipt pending** |
| SC-T07 | Aging monotonic; FIFO semantic not filename lexicographic | SC-MO-FAIR-001/002 | current u8 cast + string min | 256-step wrap + seq2/seq10 tests | source/model reproduced; native internal tests committed; current exact receipt pending | **blocked on native receipt** |
| SC-T08 | FUSE policy is off/optional/required; optional degradation truthful; required fails closed | SC-AD-04 + filesystem slice | current `FuseGuard::try_mount` is always best-effort/no-op on failure | optional-unavailable and future required-unavailable controls | native SC-F01 run recorded FUSE unavailable + command continued | **optional behavior observed; required mode missing** |
| SC-T09 | FUSE provenance does not imply result equivalence | ontology/capability matrix + SC-AD-04 | current cache lookup occurs before FUSE; cache hit has no FUSE session | cached-result-without-FUSE provenance negative | source trace + SC-F01 run | **semantic distinction established** |

## Reverse trace requirements

For every implementation surface above, reverse navigation must answer:
- which accepted/provisional obligation authorizes it;
- which decision permits the mechanism;
- which oracle can falsify it;
- which exact evidence currently qualifies it;
- whether the evidence was produced by the same candidate/configuration;
- whether the mechanism is commodity/integrated/custom.

Example reverse path:

`cache_key.rs::Git → Hypervisor::run lookup → SC-T01/T03 → recovery_fr008_* → run/job receipt → NATIVE COUNTEREXAMPLE → SC-AD-01/02 architecture change`.

A relation-less `FR-008` commit annotation does not satisfy this graph.

## Orphan candidates surfaced by this slice

- historical `CacheKeyMode::Args/Time/Git` as mature durable-reuse modes: no surviving semantic authorization after native falsification; classify as transition/compatibility candidates pending migration plan.
- current global best-effort FUSE attempt on every cache miss: mechanism exists, but no accepted evidence says every miss should attempt FUSE; evaluate cost/necessity and possibly narrow to explicit adapters.
- jobserver configuration naming without verified native protocol participation: cannot remain a capability claim.

## Slice exit

Close this trace slice only when pending in-flight, queue ownership/fairness and required-FUSE oracles execute; decisions reach accepted status; and implementation candidate/remediation mapping is added.