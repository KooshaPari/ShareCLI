# Selected topic excerpts

Recovered from local October 12 export; selected passages only, no current runtime proof.


## [Agents] Agent concurrency optimization - parked

Conversation: 6a77d52d-85a4-83e8-9d71-0433df6a1b4e

Source: /Users/kooshapari/Downloads/10ea54c5392cf14bed909b25ffdafdd39809f215131b03f58e3b724288246dc6-2026-10-12-04-03-21-43ebde5e0b504b4fa9f3c44f5024dd1c/conversations-012.json


## [Travel] Packing list - parked

Conversation: 6aacb294-1394-83e8-8f5f-cb0bda6e7d91

Source: /Users/kooshapari/Downloads/10ea54c5392cf14bed909b25ffdafdd39809f215131b03f58e3b724288246dc6-2026-10-12-04-03-21-43ebde5e0b504b4fa9f3c44f5024dd1c/conversations-013.json


## [Platform] Compare remote storage options - active

Conversation: 6ac83cfc-b6ec-83ea-8043-5ec0d618366e

Source: /Users/kooshapari/Downloads/10ea54c5392cf14bed909b25ffdafdd39809f215131b03f58e3b724288246dc6-2026-10-12-04-03-21-43ebde5e0b504b4fa9f3c44f5024dd1c/conversations-013.json

Message 163373bf-5e15-417c-b6ea-5b654c713497; role assistant; timestamp 1791508820.617564

Draft PR #878 recovers substantial missing ShareCLI design: resource models, equivalence, coalescing, speculation, pressure response, and adaptive policies. I’ll reconcile these with Fabric rather than duplicate them.

Message 414e10a3-52f1-4a93-b3d5-d7da46abcb65; role assistant; timestamp 1791508043.450559

Understood. I'll treat the ShareCLI–PhenoFabric scheduler work as a **deep research, architecture, and optimization program**, not a conventional feature-expansion exercise.

The increased reasoning budget is most valuable if it changes the rigor of the work, not just the length of the response.

I'll prioritize:

1. **Recovering the original design:** GitHub history, branches, ADRs, specifications, prior discussions, and any surviving evidence of the uncommitted iterations. I'll distinguish recovered intent from new proposals.
2. **Expanding the research landscape:** OS and hardware scheduling, compiler optimization, distributed systems, databases, networking, storage, operations research, control theory, agent orchestration, and unconventional approaches.
3. **Developing a rigorous strategy taxonomy:** Not merely listing techniques, but defining applicability, composition, conflicts, costs, correctness conditions, and implementation alternatives.
4. **Challenging architectural assumptions:** Including whether a shared optimizer is appropriate, whether ShareCLI should remain independent, and where Fabric's authority begins.
5. **Iterative refinement:** Multiple competing designs, adversarial reviews, counterexamples, failure analysis, and explicit reasons for rejecting alternatives.
6. **Empirical validation:** Benchmarks, simulation, baselines, correctness tests, and regression gates before claiming optimization gains.

Three principles will govern the work:

**First, optimize useful outcomes rather than resource utilization alone.** Eliminating unnecessary work may be more valuable than executing it faster.

**Second, optimization overhead is itself a scheduling problem.** The optimizer must justify the resources it consumes.

**Third, sophistication is not evidence of superiority.** A simple policy that consistently beats a complicated one should win.

I also want to investigate the deeper possibility you're describing: **a semantic resource operating system**, where work, data, agents, hardware, and their intentions are modeled together, enabling transformations that traditional schedulers cannot perform.

That possibility deserves serious research, but it must compete against simpler architectures rather than becoming the predetermined conclusion.

I'll keep the results divided into four categories: **verified existing implementation, recovered historical design, externally supported research, and new architectural proposals**. That prevents us from confusing ambitious specifications with working capabilities.

The target is a defensible architecture and executable evaluation program—not another enormous document containing hundreds of unvalidated optimization ideas.

Message 8496d4c5-4f76-4ee9-8120-8815649e078a; role assistant; timestamp 1791512535.575336

**[Download the 60-page PDF](sandbox:/mnt/data/ShareCLI_PhenoFabric_Research_Dossier.pdf)**  
**[Download the complete ZIP — PDF, editable sources, contracts, tests, and implementation handoff](sandbox:/mnt/data/ShareCLI_PhenoFabric_Research_and_Implementation_Pack.zip)**

The package brings the storage-streaming discussion and the expanded scheduler design together, with **82 strategy variants across 17 families**, **68 source records**, and a conversation-to-deliverable traceability map.

## The most important findings

**Some of the “missing” scheduler expansion is recoverable—not merely something to reconstruct.** ShareCLI’s draft PR **#878** contains the product-thesis correction and scheduling obligations that explicitly preserve resource packing, coalescing, speculation, pressure response, and adaptive optimization as core scope. It was open and unmerged when inspected. The packet distinguishes that branch from `main`, and does not claim to have recovered every local uncommitted iteration. fileciteturn19file0L2-L6 fileciteturn20file0L2-L6

**The recovery branch has more integration than some of its documentation suggests.** An opt-in resource-admission execution path exists, and the inspected default path separates in-flight sharing from durable cache replay. That is meaningful implementation evidence—but not proof of a complete, qualified default scheduler. The audit records the source/documentation discrepancies rather than choosing whichever version sounds more finished. fileciteturn31file0L2-L6

**The architectural recommendation preserves your broader intent:** ShareCLI remains a substantial local work-and-resource optimizer; Fabric extends it through distributed placement, object residency, topology, and authority. Optimization actions are separated from their algorithms, mechanisms, and security policies—not flattened into one list of “rules.”

## What is in the handoff

| Area | Included |
|---|---|
| **Repository and history audit** | Exact revisions, inspected paths, recovered design lineage, static risks, contradictions, and unresolved recovery gaps. |
| **Scheduler architecture** | Intent/work/resource models; elimination, sharing, incremental reuse, speculation, packing, fairness, preemption, placement, pressure control, and optimizer overhead. |
| **Strategy atlas** | 82 variants with applicability, failure modes, baselines, falsification tests, mechanisms, conflicts, and proposed rollout stages. |
| **Storage and platform design** | Independent data-versus-compute placement; peer and consumer-cloud storage; RAM-backed workspaces; durability/publication protocols; Windows, macOS, and Linux capability gates. |
| **Research and alternatives** | Native build caches/jobservers, distributed runtimes, storage tools, query optimizers, scheduling research, and exotic research paths—with inspection limits stated. |
| **Implementation plan** | 12 dependency-linked work packages, agent instructions, proposed machine-readable contracts, benchmark profiles, security/failure matrices, and a non-destructive recovery runbook. |

## What actually passed

**53 tests passed on the separately written reference model.** They cover accounting, admission, dependency readiness, sharing boundaries, cancellation, publication/eviction ordering, and scheduling tradeoffs. The model also reproduces arithmetic counterexamples relevant to the inspected admission code. Five proposed schema/example pairs passed structural validation.

**
[Truncated]

Message 8e7985df-7875-4b18-ade3-5a05d3b1efff; role assistant; timestamp 1791507814.94434

# PhenoFabric × ShareCLI: distributed compute and storage research

The central architectural question is whether **ShareCLI should evolve into a distributed resource scheduler, or whether PhenoFabric should own distribution while ShareCLI remains its local scheduling engine**.

I located your actual <Link url="https://github.com/KooshaPari/PhenoFabric" title="KooshaPari/PhenoFabric repository"/> through your connected GitHub account. That gives us a concrete starting point rather than designing solely from the product descriptions.

My working hypothesis is that these products should share scheduling primitives without necessarily sharing their entire control plane.

The distinction matters because storage streaming is only one part of the problem. A complete system must decide whether to move data, move computation, replicate a working set, or defer work based on the available resources.

## Research findings

The repository evidence materially changes the assessment. **PhenoFabric already specifies much of the architecture we were discussing.** It would be wasteful to design another independent storage-placement system without first reconciling those specifications.

### 1. What PhenoFabric already covers

| Existing area | Repository evidence | Implication |
|---|---|---|
| Object residency | `architecture/object-plane.md` | Natural home for remote-backed data and cache placement |
| Heterogeneous scheduling | `architecture/scheduler.md` | Natural home for compute-versus-data placement decisions |
| Node capabilities | `fabric-capability`, `fabric-graph` | Can describe storage, compute, network and locality |
| Authority and leases | Control-plane architecture | Foundation for safe migration and ownership |
| Cross-device operation | Two-node harness and inter-node wire specification | Existing integration direction |
| Agent execution | P4 TaskSpec and ephemeral-realm roadmap | Intended consumer of storage and compute placement |

The repository also contains a dedicated <Link url="https://github.com/KooshaPari/PhenoFabric/blob/main/sota/storage-memory-fabric-class.md" title="storage and memory fabric research survey"/> covering technologies such as UCX and NVIDIA NIXL.

This is more than a conceptual overlap: PhenoFabric's domain model already defines versioned objects, residency, mutability, consistency and durability.

However, **specification coverage is not proof of working implementation**. The repository's own work plan places the object plane and explicit task placement in P4, and identifies more advanced interposition mechanisms as research-gated. I found a two-node test-harness design and implementation references, but did not execute the code or verify production-grade cross-device storage migration.

### 2. The ShareCLI boundary

The ShareCLI model previously established includes resource vectors, scheduling envelopes, work items, durable caching and in-flight work merging.

Those are useful independently of PhenoFabric.

I would divide responsibility this way:

<box border radius="lg" padding={3} gap={2}>
  <box background="surface-secondary" radius="md" padding={3} gap={1}>
    **PhenoFabric — distributed resource authority**

    Node discovery, topology, storage residency, global placement, cross-host leases, migration, network-aware scheduling and recovery.
  </box>
  <row justify="center">
    <icon name="arrow-up-down" color="secondary" size="lg"/>
  </row>
  <box background="surface-secondary" radius="md" paddin
[Truncated]


## [Agents] Critique agent architecture - active

Conversation: 6ac8868e-22b8-83e9-9ec8-b1a9cfbfa07b

Source: /Users/kooshapari/Downloads/10ea54c5392cf14bed909b25ffdafdd39809f215131b03f58e3b724288246dc6-2026-10-12-04-03-21-43ebde5e0b504b4fa9f3c44f5024dd1c/conversations-013.json

Message 58e2be1d-9793-51d9-a76f-618001c1135e; role assistant; timestamp 1791526583.572811

**Your largest gap is authority, not agent orchestration.** You have several components capable of scheduling work, changing state, and reporting budgets—but no complete specification for which component owns each decision.

My predicted first failure is **duplicate or stale execution after a timeout**, followed by **resource contention and provider throttling**. The most consequential failure is **an agent reaching an external effect through a path that bypasses approval**.

These are architectural predictions, not measured findings. **Unsure:** I have not inspected your forks, HERDR implementation, actual worker profiles, provider entitlements, or deployed configuration. The listed model routes are treated as your intended configuration, not verified availability.

## 1. Missing pieces

Some are mentioned in your picture but lack the machinery that makes them enforceable.

| Missing contract or service | What it must do | Failure without it |
|---|---|---|
| **Authority map** | Assign one writer/decision owner for task admission, execution, approvals, spend, artifacts, and organizational state | Hermes, Paperclip, Temporal, and JetStream disagree about what should run |
| **Task/run/attempt identity** | Separate persistent agent identity, logical task, workflow, execution attempt, and container slot | A retry inherits stale permissions or appears to be a new authorized action |
| **Host execution supervisor** | Launch, reconcile, fence, cancel, and clean up processes independently of the AI worker | Temporal retries while the original container continues working |
| **Effect executor** | Hold final-effect credentials and execute only validated, approved action envelopes | Approval is recorded, but nothing actually prevents bypass |
| **Admission controller** | Atomically reserve resources, budget, provider quota, and delegation allowance before dispatch | Hundreds of logical agents become hundreds of runnable processes |
| **Credential broker and egress enforcement** | Issue task-scoped capabilities; prevent direct provider/service access outside approved routes | A worker bypasses budgets, exports data, or uses cached credentials |
| **Reconciliation service** | Compare desired state against containers, workflow state, artifacts, and external effects | Orphan containers and ambiguous writes accumulate |
| **Independent verification** | Test acceptance criteria against immutable artifacts with separately controlled verification | Workers mark their own plausible output “done” |
| **Memory governance** | Preserve provenance, access scope, freshness, and distinctions between evidence and inference | Prompt injection becomes persistent organizational “knowledge” |
| **Recovery specification** | Define restore order, reconciliation, key recovery, RPO/RTO, and approval invalidation | A successful database restore re-enables previously consumed approvals |

### Your approval broker needs an executor beside it

Hashing the artifact is necessary but insufficient. Approval should bind a **canonical action envelope**, including:

- Task and logical effect IDs.
- Exact artifact digest and dependency/input digests where relevant.
- Operation, destination, account, environment, and relevant parameters.
- Preconditions: expected commit, resource version, current state.
- Maximum spend or other bounded effects.
- Expiry, one-time nonce, policy version, and authenticated owner.
- Execution receipt and resulting external resource identifier.

Approve 
[Truncated]

Message bb8dbe86-129e-496c-b10a-5c9e72e19389; role user; timestamp 1791526543.577

Critique my agent stack architecture. I want what is MISSING, what BREAKS FIRST, and how you would SEQUENCE the build. Be adversarial and specific; no flattery. Mark anything you are unsure of as unsure and cite sources for external facts.

Goal: one human running an agent "civilization/corp" (tens to hundreds of agents) with bounded AI workers and independently enforced approvals.

Current picture (decisions marked D are firm, others are open):
- Hermes (Nous Research hermes-agent) = top-level CEO / living-assistant gateway: channels, cron, memory. D. OpenClaw rejected, I own the iMessage adapter.
- Paperclip (paperclipai) = org-management cockpit view (goals/tasks/budgets), not above Hermes. D.
- Workflow orchestration is a MUST. Candidates compared: Temporal OSS (leading, Python workers, Postgres-only), Hatchet (alt, gated on an open durable-replay crash issue), Kestra, Prefect, Windmill, Activepieces, n8n.
- Workers = Codex, Claude Code, JCode, KCode, Cursor, Forge as bounded CLI agents, one container per task from a shared versioned base image, spawned by the orchestrator with only that task's workspace and scoped credentials, destroyed after. gVisor/Kata only for untrusted code.
- Approvals: independent broker storing a hash of the exact artifact/action; owner-authenticated; agents never hold approval or final-effect credentials. Host drive access = default-deny grants, tool-scoped mounts, session-level only on explicit approval.
- Infra: one Postgres anchor, NATS (JetStream) as agent event bus, MinIO for artifacts. Neo4j rejected for now.
- Routing: OmniRoute rejected; cliproxyapi-plusplus (my fork) or manual routing. Models: OpenCode Go (Step 5 free / MiMo 2.6), MiniMax grandfathered sub as cheap API route (M3), normal = GPT 6 Luna medium, high = GPT 6.1 Sol medium.
- Session visibility: HERDR (terminal workspace multiplexer) on the host; panes' shells are docker exec into agent containers. A read-only web facade over HERDR reaches my assistant through Cloudflare Tunnel + Access.
- Network: Tailscale tailnet = machine-to-machine; Cloudflare Access/Tunnel = identity edge for browser-reached clients; cloudflared runs inside the tailnet. Desktop first, laptop second.
- Hardware: desktop Ryzen 5800X / 64 GB / RTX 3090 Ti + GTX 1080 Ti, 24/7; laptop M1 Pro 16 GB. Bounded warm-container pool per host (about 10-20 concurrent on the laptop, more on the desktop); agent identity lives in state (git/PhenoRegistry), containers are ephemeral slots; idle agents checkpointed.
- Ledger/state: PhenoRegistry repo. Failure-first test plan ("Pilot F": crash after external write, prompt injection, credential revocation, duplicate approval, artifact changed after approval, restore from backup).
- Distributed resource layer (later): PhenoFabric as resource authority, ShareCLI as local scheduler.

Questions: (1) What pieces are missing entirely (secrets management, identity per agent, observability/tracing, cost/budget enforcement, evals, backup/DR, policy engine, memory/knowledge layer, rate limiting, audit log, ...)? (2) What breaks first under tens to hundreds of agents on these two machines? (3) Which decisions above are weakest or contradict each other? (4) Give a sequenced build plan with explicit exit tests per phase.


## [Research] Immutable transactional ledger - parked

Conversation: 6acab1d8-c838-83ea-ac05-438390e10140

Source: /Users/kooshapari/Downloads/10ea54c5392cf14bed909b25ffdafdd39809f215131b03f58e3b724288246dc6-2026-10-12-04-03-21-43ebde5e0b504b4fa9f3c44f5024dd1c/conversations-013.json

Message 24293f10-9c06-466d-8c70-902682ac6581; role user; timestamp 1791751963.631888

This research is for ShareCLI and phenofabric.

Message 3cf4e837-ab8c-4cc7-9781-9135dd217c94; role assistant; timestamp 1791752095.772241

**Then this is foundational research for ShareCLI and PhenoFabric’s execution model—not a separate ledger product.** The autonomous-system-agent problem is one stress test of that model, not its entire purpose.

My earlier emphasis on agent governance was too narrow. Here, the ledger would also need to support **work sharing, scheduling, resource ownership, state changes, and coordination between humans and software agents.**

## How it fits the existing architecture

Your October 8 research dossier already establishes this division:

| Project | Existing architectural responsibility |
|---|---|
| **ShareCLI** | Local work admission, optimization, reservations, lifecycle management, and resource-pressure response. |
| **PhenoFabric** | Distributed capabilities, topology, placement, object residency, authority epochs, and cross-host execution contracts. |

Those are documented design responsibilities, not a claim that everything is implemented. fileciteturn1file0L34-L46

**Your ledger proposal would give both a shared transactional foundation across those responsibilities.** Not one ledger for ShareCLI and another for PhenoFabric that we try to reconcile afterward.

That should preserve the existing boundary: ShareCLI remains useful standalone and does not silently become the global coordinator; PhenoFabric does not absorb ShareCLI. fileciteturn1file0L63-L65 How standalone operation joins a broader ledger without identity or authority conflicts becomes a research question—not something to hand-wave away.

## The extension you are proposing

The dossier already distinguishes a **work graph** from a **resource graph**, joined through versioned objects. It also requires plan rewrites to preserve original intent, consumers, and evidence lineage. fileciteturn1file0L18-L27

I would investigate your proposal as the next step:

> **One global, nested transactional history underlies the system’s views of actors, work, resources, authority, and state. Humans and software agents participate through the same Actor abstraction.**

In that formulation, the work graph and resource graph would be related views—not independent sources of truth. The nested ledger would preserve the context in which their relationships changed.

“Global” would describe the shared model and authoritative history. It would not, by itself, prescribe one physical server, universal read access, or one lock around every operation.

## A ShareCLI-specific consequence: share execution without erasing separate commitments

Consider a hypothetical case: a human requests a build, and an agent independently requests an equivalent build.

The proposed model should allow **two intent branches to reference one shared execution**, without merging the actors, their authority, or their acceptance requirements.

That exposes several requirements immediately:

| Event | Required distinction |
|---|---|
| One actor cancels | Cancelling its interest must not automatically terminate work still required by the other. |
| Execution produces an artifact | Sharing computation must not automatically grant both actors access to every output. |
| Resources are consumed | Record physical consumption once; represent any allocation between consumers separately. |
| Work moves to another machine | Placement and execution attempts may change without losing the originating commitments and authority constraints. |
| One consumer accepts the result | That must not silently establish acceptance for 
[Truncated]

Message cceb23f3-45f5-4600-b348-b96ea2b5348f; role assistant; timestamp 1791751987.566832

I’ll anchor this ledger-and-actor model in ShareCLI and PhenoFabric’s existing design, first clarifying their intended roles rather than assuming a new subsystem or boundary.