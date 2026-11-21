# ShareCLI console and optimization system design

Status: proposed design, 2026-10-14 Pacific. This report changes documentation only. No implemented capability or measured saving is implied by a proposal below.

## 1. Product promise and current evidence

ShareCLI should improve the user's ability to get work done on a host, including when no coding harness is installed or running. Agent recognition supplies useful workload semantics; it must not gate observation, opportunities, strategy authoring, or optimization experiments. Fino Fabric extends this control and evidence model across compute, storage, I/O, and peripheral capabilities on multiple nodes. Byteport consumes deployment capabilities rather than making AWS the assumed destination.

Audit baseline: local ShareCLI HEAD `fec57c5ce45935203c63e5b4c487fddf642ff18c`. Repository findings supplied by the independent audit lane:

| Existing evidence | Consequence for design |
|---|---|
| `src/dashboard.js:23` prompts zero-pool users to start their first harness | Replace the dead-end with a working host optimization path. |
| `crates/sharecli-fleet/src/detect.rs` and `proc_scan.rs:89` use known agent patterns, including an extensible patterns file | Existing configuration extension is useful but is not autonomous discovery of previously unknown families. |
| `src/commands/serve.rs:247-255` registers the dashboard and APIs; the audit found no dashboard.js asset route despite `dashboard.html:808` referencing it | Consolidate asset delivery and test every supported entry path before relying on richer panels. |
| `src/dashboard.js:534` falls back from saved RSS to observed agent RSS | Remove this fallback. Current consumption is not saved consumption, and missing saving is not zero. |
| `dashboard.js:525-530` computes total minus idle and labels the result idle | Define and test metric semantics in a shared contract. |
| `crates/spawn-core-sys/src/windows_stub.rs:99-119` returns Unsupported; `build.rs:43-47` skips Zig on Windows | Native Windows management and a WSL backend are required implementation work. |
| Existing dispatcher rules and non-agent passthrough are reported in the audit | Do not market general process scheduling as already implemented. |

Exact source findings and their limitations belong to the companion audit. Proposed page names, APIs, data models, priorities, and tests below are new design decisions.

## 2. Information architecture

Use a persistent node selector, time window, observation freshness, and operating mode at the top. Modes are Observe, Simulate, Experiment, and Enforce. Changing nodes must never silently widen authority. The default landing page is Overview, independent of harness count.

| Page | Primary question | Main contents and actions |
|---|---|---|
| Overview | What is limiting my work, and what can I do now? | Host pressure timeline, foreground responsiveness, active work, ranked opportunities, experiment outcomes, missing capabilities. |
| Workloads | What is consuming resources? | All processes grouped by workload ancestry, known harnesses, unknown candidates, unmanaged applications, protected processes. |
| Opportunities | Where can performance improve? | Evidence, affected workload, proposed intervention, expected benefit range, cost, confidence, prerequisites, experiment action. |
| Rules and strategies | What behavior have I authorized? | Searchable strategy library, declarative rules, scope, conditions, exceptions, precedence, dry run and revision history. |
| Learned optimizations | What has actually worked here? | Validated findings, failed and inconclusive experiments, applicability envelope, recency, regression signals, promote or retire action. |
| Experiments and savings | What changed, and how certain is it? | Baselines, counterfactuals, actual outcomes, projected outcomes, uncertainty, regression guardrails, evidence export. |
| Nodes and capabilities | Where can work run or data live? | Local host, WSL distributions, VMs, remote hosts, storage-only providers, permissions, health and compatibility. |
| Activity and evidence | Why did the system act? | Ordered decisions, rule revisions, observations, action receipt, rollback receipt, redacted exports. |
| Settings and permissions | What may ShareCLI observe and control? | Sampling budget, retention, scope, allowlists, foreground protection, credential references, node trust and recovery. |

Keep the default screen comprehensible: one bottleneck narrative, three leading opportunities, and a link to full details. Advanced controls remain available in detail views. A global search finds workloads, strategies, evidence, and nodes. Tables support keyboard operation, explicit units, sortable freshness, and textual status alongside color. Text-based fixture tests and DOM/accessibility inspection satisfy the no-screenshot requirement.

## 3. First run and zero-harness behavior

Represent discovery and readiness separately. `0 recognized harnesses` may mean none are running, an adapter lacks visibility, permissions are restricted, discovery is stale, or detection does not recognize a family. Never turn these possibilities into a false claim that no agents exist.

First-run flow:

1. Identify host and capabilities using process/file/CLI observations. Show exactly what is visible and what is unavailable.
2. Start low-overhead observation within a declared collection scope. Record a baseline and collector cost; no process mutation is required.
3. Populate all-process Workloads and offer host-level opportunities with their evidence requirements.
4. Offer optional harness recognition and explicit workload registration. Continue when skipped.
5. Permit simulations and controlled experiments only where a supported actuator and authorized target exist.

Concrete initial screen text:

> **Your host is ready to observe**  
> Recognized running harnesses: 0 · Process scan: current · Visibility: current user  
> ShareCLI can still inspect resource pressure and suggest improvements. Recognition improves attribution as workloads appear.  
> **Collect baseline** · **Inspect workloads** · **Review strategies**  
> Savings: not measured. Estimates appear after relevant evidence is collected.

During an empty observation window:

> No sustained resource bottleneck observed in this window. Continue observing or select a workload to measure. No changes applied.

When visibility is incomplete:

> Harness count is incomplete: process details are unavailable for part of this host. Last successful scan: [time]. Inspect capability status.

When pressure exists but control is unavailable:

> Memory pressure is elevated. Evidence identifies [workload] as a contributor. This installation can observe it; the proposed control is unavailable with current permissions. Review a manual strategy or choose an owned workload experiment.

Zero harnesses must not produce fabricated opportunity counts, artificial savings, empty navigation, or a requirement to launch an agent. Capability-specific absence is displayed where it matters, not as a product-wide failure.

## 4. Open-world workload and harness discovery

Maintain an all-process inventory as the foundation. Identify an instance by node, boot/session identity, PID, and process start time to avoid PID-reuse mistakes. Associate executable identity, parent ancestry, owner, bounded launch metadata, and observed lifecycle. Redact arguments before persistence; never collect command secrets, environment dumps, chat contents, or credentials by default.

Detection is a layered classifier:

1. Known signatures: installed and running are separate states; match versioned executable/path/package/protocol signals.
2. Generic workload clusters: repeated CLI trees, child-tool launches, resource bursts, and known runtime relationships identify an *unknown workload candidate*, not a proven AI agent.
3. Evidence enrichment: inspect explicitly permitted installation metadata and manifests; do not run an unknown executable to learn what it does.
4. Local user mapping: identify or dismiss a candidate, register a custom executable, and preview matches before saving.
5. Optional reviewed adapter promotion: convert confirmed local evidence into a versioned adapter with positive and negative fixtures. Never silently upload local paths or process details.

Display candidate evidence and conflicting signals. Do not pretend a heuristic score is a calibrated probability. Distinguish recognized agent, user-labeled agent, unknown CLI workload, generic application, and excluded process. Multiple signals derived from the same ancestry count as correlated evidence. A shell running `node` alone is insufficient to identify an agent.

Adapters declare supported OS, signature version, evidence requirements, confidence semantics, and test fixtures. New adapter versions first run in shadow mode. Negative controls include editors, ordinary shells, language servers, package managers, build tools, renamed executables, wrappers, and synthetic PID reuse. Missed candidates remain visible as general workloads.

## 5. Opportunity engine and strategy library

Pipeline: Observe → normalize → attribute → find constraints → propose → estimate → simulate → experiment → evaluate → learn → optionally enforce. Each stage has a version and an evidence receipt. An empty or failed stage produces a reason, not a guessed result.

Candidates can arise from resource contention, duplicated work, excessive launch churn, redundant polling, avoidable rebuilds, locality problems, scheduling conflicts, repeated failures, or unsuitable placement. These are hypotheses until a relevant observation supports them. Rank by user objective, expected net benefit, evidence quality, action reversibility, prerequisite cost, and regression risk. Show Pareto alternatives when throughput and responsiveness conflict; never reduce every outcome to a misleading universal score.

| Strategy family | Preconditions | Intervention concept | Required outcome and guardrail |
|---|---|---|---|
| Concurrency shaping | Independent queued jobs, known ownership | Bound concurrent builds or tool launches | Completion time/throughput and queue delay; no starvation. |
| Foreground protection | Explicit foreground preference and controllable background work | Lower background scheduling priority or pause new admission | Interaction latency plus background deadline compliance. |
| Launch/poll coalescing | Repeated equivalent operations with understood semantics | Coalesce safe requests or adjust opt-in polling cadence | Spawn/syscall count per completed task plus freshness bound. |
| Shared immutable cache | Content identity and cache validity proven | Reuse build/download results | Avoided work plus correctness and storage costs. |
| Memory admission | Repeated peak-memory contention | Queue owned jobs before pressure becomes critical | Peak memory, stall duration and completion time; no forced data loss. |
| I/O locality | Measured read/write behavior and eligible target | Place scratch/data near execution | I/O wait and total transfer cost; integrity checks. |
| Cross-node placement | Fabric capability, trusted target and portable workload | Move eligible queued work | End-to-end time including transfer, startup and return. |
| Adapter optimization | Known harness supports a documented setting | Recommend or apply scoped setting | Versioned behavior and task-quality regression checks. |

Avoid promising generic syscall deduplication: syscalls carry ordering, ownership, and side-effect semantics. Initial coalescing belongs at explicit application/tool boundaries. OS scheduling integration is a capability layer with platform-specific mechanisms; hardware scheduler control is research unless a documented supported interface is proven.

## 6. Rule editor and learned optimization pages

A rule contains an ID, revision, owner, intent, target selector, trigger, evidence threshold, capability prerequisites, action parameters, maximum scope, exclusions, cooldown, budget, guardrails, rollback recipe, expiry, and audit links. Render human-readable logic and an advanced declarative view from the same model.

Example proposed rule:

> When this node shows sustained memory pressure and more than the configured number of **owned build jobs** are queued or running, stop admitting additional builds. Keep foreground and explicitly protected workloads exempt. Resume admission after the configured recovery window. Never kill an existing process. Preview shows the exact selected jobs and exclusion reasons.

Resolve conflicts deterministically: protected scopes and explicit deny take precedence, then authority scope, then documented priority and specificity. Reject contradictory equal-precedence mutations rather than relying on evaluation order. Show a rule explanation for every decision. Rules are distinct from harness configuration files; changes to shared harness policy require the established all-three-harness update workflow.

A strategy is a reusable hypothesis/template. A rule is an authorized executable policy. A learned optimization is a contextual finding backed by experiments. Promotion between these concepts is explicit.

Learned findings show workload signature, machine/OS envelope, strategy version, tested conditions, sample size, effect range, confidence method, regression outcomes, freshness, provenance, and supersession. Failed experiments remain searchable to prevent repeated mistakes. New OS, hardware, adapter or workload versions can invalidate applicability. Promotion to enforcement requires a valid rollback and policy authorization; learning alone grants no control authority.

## 7. Measurement and honest savings

Use four labels everywhere: **observed**, **estimated**, **experiment result**, and **not available**. A model estimate never fills an observed-savings field. Missing values remain null with a reason. A zero value is valid data, so truthiness fallbacks are prohibited.

Metrics include completed useful tasks, end-to-end wall time, queue delay, interaction latency, CPU time, peak and integrated memory, pressure/stall duration, bytes transferred, I/O latency, spawn count, supported syscall counts, energy where measured, and actual monetary cost where a price and accounting basis exist. Track result quality and errors alongside speed. Lower RSS alone can mean the workload was delayed or failed.

Platform contracts:

| Platform | Native evidence | Explicit limitations |
|---|---|---|
| Linux | Process counters, cgroup metrics where delegated, CPU/memory/I/O PSI where enabled | PSI is stall time, not CPU utilization; unsupported kernel/cgroup data stays unavailable. |
| macOS | Process CPU/RSS, OS memory-pressure indicators, swap/compression counters where available, workload latency, supported disk metrics | Do not rename memory-pressure categories as Linux PSI or sum category values into a synthetic PSI percentage. Detailed syscall tracing needs a separate supported/permissioned collector. |
| Windows | Native process counters, commit/working-set measurements, paging and CPU/disk counters where available, workload latency | Commit, working set, and physical memory are different measurements. Native Job Object control does not establish control of Linux descendants inside WSL. |
| WSL | Per-distribution Linux evidence collected through a scoped backend plus Windows host evidence | Keep guest and host namespaces separate; avoid double counting VM memory/CPU and guest process totals. |

Linux PSI measures CPU, memory and I/O stalls and may expose per-cgroup pressure with appropriate configuration. See [Linux kernel PSI documentation](https://docs.kernel.org/accounting/psi.html).

Illustrative formulas, with no verified numeric savings claimed:

- Wall-time saving per comparable job: `T_baseline - T_treatment`; relative saving: `(T_baseline - T_treatment) / T_baseline`, only when baseline is positive.
- Projected time saving over a horizon: `eligible_job_count × estimated_effect_per_job − additional_setup_and_transfer_time`. Mark eligibility and effect assumptions separately.
- Memory residence reduction: `integral(M_baseline(t) − M_treatment(t), dt)` in GiB-seconds, on comparable completed work. Peak RSS reduction is a separate number.
- Stall reduction: `stall_seconds_baseline − stall_seconds_treatment`, with absolute percentage-point and relative changes clearly distinguished if displaying pressure ratios.
- Net remote benefit: `local_completion_time − (queue + transfer_out + remote_execution + transfer_back + startup)`. Include remote failure probability and retransfers in uncertainty.
- Projected cost saving: `baseline_billable_units × baseline_rate − treatment_billable_units × treatment_rate − added_costs`. Idle capacity is not cash saved without a real avoidable cost.
- Collector net benefit: `gross_effect − observation_and_control_overhead` in the same unit. Do not subtract unlike units.

Never add overlapping savings from two rules affecting the same task. Deduplicate attribution by experiment and workload/time window; show joint effects or marginal estimates explicitly. Time projections do not imply the user gained equivalent working time unless the work was on their critical path.

Measurement protocol: capture task identity, input digest, completion semantics, relevant node state and strategy version; establish a comparable baseline; randomize or alternate matched windows where safe; account for warm caches, load, thermal effects, network variation and learning effects; include an untreated negative control; predeclare success and regression thresholds; report sample count and interval method. If samples are insufficient or workload drift is material, show inconclusive. For nonrepeatable interactive work, clearly label observational evidence and confounding.

Confidence ranges must come from an identified model or statistical method. Estimate calibration compares forecast intervals with later results, tracks coverage and directional errors, and suspends unreliable models. A single-user low-sample history should not produce precise-looking percentages.

## 8. Capability and control boundaries

Start with observation and application-level admission control for owned work. Then integrate documented OS primitives, validated per OS/version and permission level. Linux cgroup controls require appropriate delegation; Windows may use supported native process/Job Object interfaces; macOS policy mechanisms require independent feasibility verification. The UI only offers an action when its capability probe is positive.

Windows Job Objects can manage process groups and CPU-rate controls, subject to documented nesting and breakaway behavior. They are not a universal cross-OS scheduler. See [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) and [CPU rate control](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_cpu_rate_control_information).

Every mutation revalidates identity and target ownership immediately before acting. Keep protected/system processes excluded unless explicitly supported and separately authorized. Provide global pause, per-rule disable, bounded duration, rollback, offline recovery and a readable audit receipt. An unavailable collector must disable dependent decisions, not preserve stale enforcement as though evidence were current. Prevent oscillation with hysteresis, cooldowns and action budgets.

Do not automatically kill rustc/cargo or other shared processes as an optimization. Do not silently broaden collection, install tracing components, change global harness settings, or alter remote trust. None of those is necessary for zero-harness value.

## 9. Native Windows and WSL backend in spawn-core

Required end state: a Windows-native ShareCLI executable observes and manages its Windows host and invokes WSL through `wsl.exe` using a WSL backend in spawn-core. A Linux service reached through an interim bridge does not meet this requirement.

Proposed backend interface: `probe`, `list_targets`, `spawn`, `status`, `wait`, `cancel`, `read_output`, and `capabilities`. Execution target is a tagged value (`WindowsHost` or `WslDistribution{id,name,user}`); never infer target from path shape. Each spawn request includes an argument vector, explicit working directory, scoped environment overrides, stdio mode, deadline, cancellation semantics, and idempotency key. Return an opaque job identity with host launcher identity and guest process identity when established.

Use a trusted system `wsl.exe` path, supported distribution selection and direct execution options. No generic shell-string interpolation. Validate argument encoding at the Windows process boundary and again at guest execution. Probe actual supported command behavior instead of assuming a particular WSL version. Official command surface: [Microsoft WSL command reference](https://learn.microsoft.com/windows/wsl/reference).

A Windows launcher PID is not a Linux workload PID. Track guest process start identity with a helper/protocol where required. Define whether cancellation terminates only the launched guest job and descendants; do not terminate a whole distribution to cancel one task. If cancellation cannot be established, mark it unsupported and reject workloads requiring it. Propagate output, exit status, timeout and partial failure distinctly. Native spawn and WSL spawn can coexist.

Required contract and integration tests:

1. Native executable starts with WSL absent: Windows observation works, WSL capability is unavailable, no Linux-only dependency aborts startup.
2. Enumerate multiple distributions, including stopped ones and Unicode names; enumeration does not start a stopped distribution without an explicit launch request.
3. Target selection launches only the requested distribution/user; changing the system default does not redirect an existing configured target.
4. Round-trip argument vectors containing spaces, quotes, Unicode, shell metacharacters, and empty strings; arguments are never executed as injected commands.
5. Verify cwd and path conversion for Windows and guest paths; reject unsupported mappings rather than guessing.
6. Environment overrides are scoped to the job; secrets do not enter telemetry or error text.
7. Capture stdout/stderr and exact nonzero exit results; distinguish transport failure from child failure; bound buffering and handle backpressure.
8. Cancellation and timeout affect only the owned workload subtree; unrelated Windows and WSL jobs survive. Verify guest cleanup, not merely launcher exit.
9. Handle guest exit before registration, Windows PID reuse, launcher crash, distribution shutdown and backend restart without attaching to an unrelated process.
10. Text-only native Windows/WSL integration run proves lifecycle behavior on a real Windows host; cross-compilation alone is insufficient.

## 10. Fabric and Byteport composition

Fabric nodes publish separate executable, storage, I/O and peripheral capabilities. Administrator access supports a full host role only after capability probes and trust establish what can actually run. A storage-only service is a storage provider, not a compute node. A restricted client can be an access endpoint without being eligible for arbitrary remote execution.

ShareCLI supplies local workload intent, pressure evidence, safe admission/control and measurements. Fabric supplies authenticated placement, transport, inventory, leases and cross-node recovery. Byteport supplies build/deploy intent, dependency/artifact contracts and provider adapters. Shared job IDs and capability schemas enable composition without collapsing product ownership. Optimize data movement and total completion time rather than merely distributing processes. Peripheral access requires explicit ownership, contention policy and device semantics; it is not equivalent to mounting disk.

## 11. Proposed data and API contracts

Version all envelopes. Store UTC event time, monotonic duration where possible, collector version, node identity and freshness. Redacted exports contain evidence references and hashes, not undisclosed secrets.

| Entity | Minimum contract |
|---|---|
| Capability | node, platform, feature, available/unsupported/denied/unknown, observed_at, reason, probe version |
| Workload | stable process identity, ancestry, recognition class, adapter version, ownership, redacted display, visibility |
| Observation | metric, value or null, unit, scope, time interval, provenance, quality, missing reason |
| Opportunity | hypothesis, evidence references, affected scope, prerequisites, proposed action, estimate model/version, range, costs, conflicts |
| Rule revision | declaration, owner, authority scope, precedence, protection, guardrails, rollback, expiry, dry-run receipt |
| Experiment | assignment, baseline/treatment, task identity, thresholds, outcomes, confounders, analysis version, conclusion |
| Learned finding | experiment references, applicability, effect interval, confidence method, recency, promotion state |
| Action receipt | idempotency key, identity revalidation, rule revision, before state, action, result, rollback status |

Suggested read APIs: `/api/v1/capabilities`, `/workloads`, `/opportunities`, `/strategies`, `/rules`, `/findings`, `/experiments`, `/savings`, `/events`. Mutating APIs require explicit policy scope, revision concurrency checking and idempotency. Stream typed, sequence-numbered updates with a resynchronization endpoint; a dropped connection renders data stale and cannot silently preserve current status. These endpoints are proposals, not existing routes.

## 12. Phased backlog and objective acceptance

No fabricated schedule or savings target is assigned. Each phase depends on evidence from its predecessor; independent UI/data-contract work can proceed in parallel with OS-adapter work.

| Phase | Deliverable | Objective acceptance and negative control |
|---|---|---|
| A: truthful baseline | Fix asset delivery, idle labels and savings semantics; separate discovery/visibility/capabilities | Server entry test loads every declared asset; zero and null contract fixtures remain distinct; observed RSS never appears as saved RSS; idle+busy reconciles to denominator. |
| B: zero-harness console | All-process overview, baseline collection, opportunity empty states and capabilities | Fixtures for zero processes, zero known harnesses with busy generic processes, restricted visibility and stale scans all retain useful navigation; no harness launch is required. Collector overhead is measured against a predeclared budget. |
| C: discovery | Generic candidates, adapter registry, local registration and review workflow | Unknown synthetic workload appears as a candidate; known fixtures match expected adapters; ordinary shell/editor/build negative controls are not asserted to be agents; PID-reuse fixture cannot transfer identity or authority. |
| D: rules and experiments | Strategy library, rule preview, reversible admission-control trial, findings ledger | Dry run lists exact affected/excluded identities; conflicting policies reject safely; no-op negative control reports no causal saving; rollback restores controlled state; failed/inconclusive results remain visible. |
| E: calibrated savings | Comparable baselines, explicit estimates, outcome intervals and deduplicated attribution | Known synthetic effects reproduce expected unit conversions; overlapping effects are not summed twice; unavailable/negative effects render correctly; calibration evaluates held-out outcomes and displays insufficient evidence honestly. |
| F: native Windows | Native Windows process backend and spawn-core WSL backend | All ten native/WSL lifecycle contracts above pass on real Windows; build-only or bridge-only evidence cannot close the phase. |
| G: deeper scheduling | Supported Linux/macOS/Windows controls behind capability gates | Every enabled actuator passes ownership, permissions, stale-identity, starvation, oscillation and rollback tests on its target OS; unsupported hardware claims remain research. |
| H: Fabric/Byteport | Typed capability integration and one bounded cross-node placement experiment | Storage-only node is rejected for execution; transfer/startup/return included in outcome; disconnect and lease-expiry recovery preserve workload ownership; local fallback is explicit. |

Product gate across phases: user-visible opportunities link to evidence; estimates show assumptions and uncertainty; every action explains authority and rollback; a clean host with no recognized harness still has a useful console. Performance acceptance uses completed work and responsiveness jointly, not a single resource counter. Local tests, native runtime qualification, hosted CI, deployment and economic outcomes remain separate evidence gates.

## 13. Immediate implementation handoff

Begin with Phase A and B against the exact audited revision after repository-state reconciliation. Freeze the measurement vocabulary and capability schema before building savings cards. Then implement candidate discovery and one reversible, owned-work admission strategy to prove the full learning loop. Develop native Windows/WSL lifecycle support as a named independent lane sharing the same job contract. Do not begin broad autonomous OS intervention until those foundations are measured and recoverable.

This document is a console and optimization specification. It does not approve production deployment, mutate harness configuration, establish measured gains, or substitute for unavailable chat history and native Windows runtime evidence.
