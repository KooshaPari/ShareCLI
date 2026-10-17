# ShareCLI — mature contract recovery and verticalization, pass 1

**NOT FROZEN.** Source `4f01d0199e82b62bcf20399afcc102f58a10ad07`, inspected 2026-09-29. Read SOURCE-COVERAGE-LEDGER and SEMANTIC-FINDINGS first. Existing FR IDs are preserved. The statements below organize recovered intent/design and propose necessary refinements; they are not an automatically accepted replacement requirement catalog.

## Recovered horizon and existence question

The current accepted-marked lineage and functional index describe an OS-adjacent operator runtime for many agents: observe and supervise workloads, expose contention, share provably equivalent work, coordinate selected filesystem/worktree/queue operations, and provide tray/serve control without requiring vendor-binary replacement for primary detection. Harbor-style agent benchmarking remains outside the product boundary. February FUSE/mesh lineage must be recovered, not erased by the March supervisor-era implementation.

This does NOT establish that every detected agent's invocation is automatically mediated. Detection, explicit managed execution, optional filesystem interception, and global optimization are different capabilities. Prove each reachability/control boundary. The high-risk thesis is useful cross-agent coordination **without incorrect reuse, inappropriate process control or unacceptable foreground interference**. The alternative stack and custom-subsystem justification are in the corresponding PhenoRegistry SOTA dossier. Existence and architecture gates remain open.

## Ontology: independent projections, not one feature tree

- Product/capability: observation; supervised lifecycle; project configuration; resource/admission policy; safe invocation sharing; filesystem/worktree coordination; operator experience; distribution/support.
- Runtime identities: host, principal, registered project, workspace/input snapshot, executable/tool identity, invocation, supervised process identity (PID plus start/generation), lease, cache entry/result artifact, resource observation and optional FUSE session.
- Experience: CLI, machine IPC/API, tray/serve and supported platform-specific interfaces; each maps to journeys, not assumed parity.
- Lifecycle: requested/running/exited/failed/unknown; cache eligible/ineligible/stale/conflicting; lease held/expired/reclaimed; interception required/optional/unavailable. State names are a draft model requiring reconciliation with existing types.
- Verification and quality: acceptance criteria and observations qualify exact subjects/configurations. Reliability, performance, security, accessibility and provenance are shared overlays, not cloned FRs.

A project is not a process, a process ID is not persistent identity, an invocation key is not proof of equivalent inputs, and cache freshness is not correctness. Detection cannot by itself authorize intervention. Do not introduce a new blackboard/BFT/task platform to fill ontology boxes.

## Semantic obligation candidates and existing trace anchors

| Candidate refinement (not newly accepted FR) | Source/parent | Positive and counterexample acceptance design | Growth disposition |
|---|---|---|---|
| Preserve truthful owned-process lifecycle | FR001, SC-S02; PR876 is candidate evidence only | Start/list/stop real child; foreign live PID must survive and refusal reach CLI/IPC/tray; reused PID must not inherit ownership | Mature spine; extend OS adapters |
| Reuse only an authorized equivalence class | FR008, SC-S05/06/14 | Two equivalent immutable-input reads may share; changed file with unchanged Git status, changed executable/env/principal or unavailable fingerprint must not share by assumption | Safe bypass initially; widen declared adapters, not unsafe default sharing |
| Keep observation and actual enforcement distinct | FR004/005/007/011 | Report measured resource/limit state; a monitor-only configuration must never claim it enforced a ceiling | Shared state and capability reporting |
| Preserve durable coordination across interruption | FR008/010, SC-S07 | Worker crash/replacement, stale lease, same priority ticket burst and restart do not lose accepted tasks or falsely reclaim a live owner | Lease/generation semantics before wider concurrency |
| Make interception mode and failure explicit | FR009 and SC-S08/14 | Required mode refuses unavailable interception; optional mode reports degradation and cannot claim intercepted evidence | Platform adapters; no silent claim of parity |

These are semantic refinements for review, not a complete set. For eventual accepted requirements record ID, obligation, rationale, authority/provenance, parent, dependencies, product role, stage, journey, positive/negative acceptance, applicable quality policy, implementation surfaces, verifier and transition disposition. Shared invariants must not be multiplied merely to increase row count.

## Actor-to-outcome journeys and stage projection proposal

| Journey | Closed outcome | Source anchor | Current evidence |
|---|---|---|---|
| SC-J01 Operator → register/configure → start → observe → stop owned workload → reopen | Correct real process, truthful refusal and preserved configuration | FR001–005 | Incomplete; source/past PR assertions only |
| SC-J02 Concurrent authorized workers → equivalent request → correct shared output → changed-input rerun | Reduced executions without semantic false hits | FR008 | Model counterexample defeats broad current key interpretation; no native pass |
| SC-J03 Operator under pressure → observe → apply bounded admission policy → recover | Real signal and stated control, not synthetic health | FR007/011 | Not experimentally verified |
| SC-J04 Coordinated work → lease/worktree/CoW operation → conflict/reclaim/restart | Durable ownership and honest conflict outcome | FR009/010 | Mapping and failure experiments outstanding |
| SC-J05 User → install/update/rollback/remove → retained or explicitly removed state | Supported, secure installed lifecycle | Distribution docs/quality overlay | Not verified |

**CVP proposal:** close SC-J01 and a narrow SC-J02 using an explicitly eligible read-only adapter; unsupported sharing safely bypasses. Preserve identities, durable configuration and evidence shape from the mature horizon. A generic cache badge is not a usable slice.

**MVP/Beta projections:** widen eligible adapters and coordination journeys, supported OS paths and failure coverage. **GA projection:** independently verified supported journey/platform matrix, signed distribution, migration and operator recovery. **Mature:** all distinct recovered and accepted obligations, not a newly invented feature bucket. Stage boundaries remain reviewable and cannot hide failed mandatory controls.

## Transition debt and work packages

SC-TD01 old argv/TTL cache entries need a new semantic-key namespace or invalidation, never reinterpretation as trusted evidence. SC-TD02 historical PID/config/state identifiers need migration/ownership policy. SC-TD03 FUSE best-effort vs required behavior needs explicit configuration compatibility. SC-TD04 removed public surfaces from PR858 need consumer-aware parity review, not wholesale restoration.

| Work package | Dependencies and trace | Exit evidence | State |
|---|---|---|---|
| SC-W1 Frozen source/coverage/lineage | SC-S01–13 | Exact revisions plus open semantic ledger | Initial pass recorded, not exhaustive |
| SC-W2 Authority/history closure | W1, donor revisions, raw conversations | Accepted scope and resolved contradictions | OPEN |
| SC-W3 Alternatives and architecture experiments | W1/W2, SOTA dossier, SC-F01–05 | Safe sharing/recovery/interception comparison against composed baseline | OPEN |
| SC-W4 Mature ontology/requirements/journeys | W2/W3 | Semantic coverage and justified stage projections | DRAFT |
| SC-W5 Independent oracle and actual mapping | W4 plus current implementation candidate | Sealed criteria, reachable traces and adversarial runtime receipts | DESIGN STARTED |
| SC-W6 Fresh falsification review | All above | Reviewer actively finds no blocking unexplained behavior after adversarial attack | NOT RUN |

No production acceptance, maturity percentage or final architecture choice follows from this document.
