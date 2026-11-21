# ShareCLI / Fino Fabric dream loop — registry-ready proposal

Status: PROPOSED; 2026-10-14 Pacific. This is a planning artifact, not a change to PhenoRegistry, shipped behavior, an approved kernel intervention, or measured savings. Exact operator brief is preserved in `operator-brief.md`.

## Intent and ownership

ShareCLI owns local software scheduling and resource supervision across all processes: agent adapters enrich identity but do not gate usefulness. Fino Fabric extends local resource capabilities into a user-oriented mesh of hosts, VMs, compute, storage, peripherals and I/O. Byteport consumes those capabilities to build and deploy projects from declared repository paths and optional explicit commands. Providers may include self-hosted nodes, Vercel and Supabase; AWS is optional rather than the default economic assumption.

A full host advertises granted execution/control capabilities; administrator access alone does not imply trust or an automatic grant. A client may only request access. A storage-only provider such as Google Drive advertises storage operations and quotas, never executable capacity. Peripherals and I/O require exclusive/shared lease semantics, device ownership, latency budgets and loss recovery. Do not pretend transparent distributed memory or arbitrary GPU pooling follows from networking nodes.

## Canonicalization evidence and unresolved history

Current local registry `handbook/specs/program-docset/products/ShareCLI/DOSSIER.md` calls ShareCLI an OS-adjacent process/resource supervisor and requires correctness, compatible coalescing, comparative measurement and negative controls. Its `FEDERATION.md` proposes two hosts sharing a permitted compatible instance. BytePort/FEDERATION.md proposes deployment/status/rollback contracts and scoped process supervision. These support the boundaries above. Older `ECOSYSTEM_MAP.md` and rationalization records describe archived/absorbed projects; those historical dispositions conflict with newer product records and must not authorize deletion or source movement.

No exact dream-loop definition was recovered from the bounded local registry Markdown search. The phrase therefore has no verified existing canonical semantics in this audit. Adopt the definition below as a proposal, retain a supersedes field for any subsequently recovered source, and attach the recovered October 12 chat snapshots before claiming historical continuity. The available October 4 Plan 100x Revenue export is a different conversation ID from the requested active Branch: [Platform] chat; it does not prove that active branch was recovered.

## Proposed canonical definition

A dream loop is a bounded, evidence-producing cycle which converts desired user outcomes and observed friction into ranked hypotheses, executes one reversible experiment, independently checks correctness and value, and persists either a scoped optimization or a rejected hypothesis with its evidence. Each cycle has a workload, owner, budget, stop condition, baseline, counterfactual, provenance and rollback. Ideation may be broad; enactment remains limited to granted capabilities.

State machine: intake → inventory → model → rank → simulate → experiment → verify → decide → learn → observe. Decisions: promote, retain as suggestion, reject, pause for missing evidence, or rollback. A failed hypothesis is useful learned evidence; repeated identical retries without a new hypothesis are not progress.

Record schema: id, revision, source_refs, product_boundary, outcome, owner, capability_grants, workload_signature, hypothesis, constraints, baseline_revision, candidate_revision, experiment_budget, metrics, negative_controls, projected_effect_range, measured_effect, confidence_method, external_validity, acceptance_gate, stop_reason, rollback_receipt, learned_rule_refs, supersedes, next_review_trigger. Financial metrics require actual invoices/receipts or explicit estimates and never equate avoided CPU time with revenue.

## Delivery DAG and acceptance gates

| Work package | Depends on | Deliverable and exit evidence |
|---|---|---|
| DL-01 Source recovery | none | Immutable brief, source register, selected chat excerpts with IDs/timestamps and access status; unresolved active branch clearly marked |
| DL-02 Capability atlas | DL-01 | Actual code/build/runtime map, source ownership and supported OS capabilities; baseline and unknowns |
| DL-03 Zero-harness console | DL-02 | System workload overview, rules and strategies usable with zero recognized harnesses; fixture proves absent, permission-denied, stale and empty are distinct |
| DL-04 Discovery | DL-02 | Explainable unknown workload candidates; passive metadata first; adapter proposal and operator override; false-positive and spoofing controls |
| DL-05 Rule ledger | DL-03 | Versioned rule strategies, simulation, permission scope, provenance, enable/disable and rollback; learned optimization lifecycle |
| DL-06 Controlled experiments | DL-05 | Baseline vs candidate with independently held negative cases, same workload, counterbalanced order, repeated samples, confidence bounds and overhead |
| DL-07 Windows native | DL-02 | Windows PE binary, native host discovery/control and spawn-core WSL backend using wsl.exe; lifecycle/cancellation/native isolation evidence |
| DL-08 Fabric contract pilot | DL-06, DL-07 | Two granted hosts plus storage-only provider; capability negotiation, leases, partition behavior and revoke/reconnect proof |
| DL-09 Byteport journey | DL-08 | One project/subrepo build and deployment plus status/rollback observed from both applications without duplicate action |
| DL-10 Registry handoff | all scoped gates | Additive pattern/ADR, ownership map, linked work packages and evidence index; source owner accepts explicit unresolved items |

No invented durations. Estimate each package only after inventory exposes its denominator; report owner, elapsed time, evidence and revised ETA when execution begins.

## Windows is a required native product surface

Ship a Windows executable with native host process/resource telemetry and control. Model WSL as a spawn-core backend selected by a structured target containing distribution, Linux user, working directory and argument vector. Invoke wsl.exe through a direct process API rather than concatenated shell strings. Discover installed distributions and version/status; keep Windows and Linux process identity namespaces separate. Detect missing WSL, disabled virtualization, stopped/missing distributions, denied rights and unsupported versions explicitly.

The Windows parent must define stream decoding, Windows/WSL path conversion, quoting, environment allowlists, exit codes, launch timeouts, cancellation escalation, descendant cleanup and reconnect behavior. Killing only wsl.exe is not proof the Linux workload stopped: use a distribution-side scoped supervisor/job token with a verified termination receipt. Native Windows jobs and WSL jobs require separate implementations and integration tests. Do not terminate a whole distribution to cancel one task. Test Unicode/spaces, UNC paths, concurrent distributions, parent crash, Linux grandchildren, output backpressure and client disconnect. A WSL-hosted bridge may remain interim but cannot close DL-07. Microsoft documents Windows-to-WSL command execution through wsl.exe: https://learn.microsoft.com/en-us/windows/wsl/basic-commands and https://learn.microsoft.com/en-us/windows/dev-environment/wsl-interop .

## Optimization sequence and feasibility boundaries

Start with admission control, bounded concurrency, shared read-only work under proven equivalence, cache scoping, polling backoff and priority-aware queues. Progress to OS-specific supported controls only with capability checks and measured benefit. Kernel/syscall or hardware scheduling changes are research lanes requiring an explicit platform mechanism, compatibility analysis, privileges, isolated benchmark and rollback; user-space process deduplication does not establish syscall coalescing. Never merge side-effectful operations solely because command text matches. Credentials, inputs, environment, working directory, tool versions, authorization subject, filesystem state and cancellation consumers affect equivalence.

Zero-harness operation still offers process groups, pressure indicators, optimization opportunities, a dry-run schedule and rule history. Unknown executable metadata produces an unclassified workload candidate, not an invented agent identity. Discovery must distinguish installed, running, connected and controllable states. Read no chat content or secrets merely to classify a workload.

## Measurement and savings contract

Every forecast shows workload, baseline, assumptions, sample window, confidence and expiration. Keep wall time, CPU-seconds, resident/private memory, peak memory, I/O bytes, queue delay and platform pressure metrics separate. Linux PSI, macOS pressure and Windows counters need native definitions and must not be displayed as one interchangeable percentage.

Example arithmetic only: if baseline work consumes 120 CPU-seconds, 30 are proven redundant, and supervision costs 3, projected net reduction is 27 CPU-seconds or 22.5%. This is not a measured ShareCLI result and implies no fixed wall-time improvement. Memory savings require simultaneous lifetime accounting; summing process RSS double-counts shared pages. Cross-host placement evaluates local compute + remote transfer + remote queue + remote execution + result transfer against the local baseline; storage/cache costs and energy are separate. Additive savings cannot sum overlapping rule opportunities; evaluate the combined schedule.

Guardrails: correctness and authorization invariants first; no foreground p95/p99 regression above a predeclared workload threshold; observer overhead bounded before promotion; latency/error/leak negative controls; rollback on breach; kill switch and versioned policy. Report measured benefit with uncertainty and inconclusive outcomes.

## External methods and skills adopted as guidance

Brendan Gregg’s USE method motivates resource-by-resource utilization, saturation and errors rather than attractive but ungrounded dashboards: https://www.brendangregg.com/usemethod.html . Anthropic’s long-running harness account motivates persistent task state and small tested increments: https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents . These are design inputs, not product proof. The local continuous-agent-loop skill recommends bounded loops, quality/eval gates, persistence and recovery from churn; its guidance informs the proposal without invoking an actual execution loop. Any external screenshot-based verification suggestions are excluded by the operator policy; verification here uses text and process-scoped evidence.

## Proposed registry landing, without mutation

Propose `patterns/dream-loop.md` as the reusable method; add product-specific links under the current ShareCLI dossier, a Fino Fabric boundary proposal, and BytePort federation/deployment records. Reconcile `BOUNDARY_OWNERS.md` and historical archive aliases before registering ownership. Use the registry session artifact protocol and link an AgilePlus specification during later implementation; this audit does not claim live engine state or create that spec. Preserve source IDs and hashes, add a claim-to-evidence matrix, document conflicts and define explicit supersession rather than deleting older records.
