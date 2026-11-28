# Dependency-based finish plan and effort ranges

Planning basis: source snapshot `e182151d4183c964547c017a4cc3f606a7cfb8ae`, recovered goals and existing open remediation/specification PRs. These are engineering work estimates requested for planning, not measured durations, sprint promises or calendar dates.

## Horizon and estimation method

CVP means the core value proposition is observed: real work stays correct, contention is bounded, heavy work goes to the selected stronger host and the operator sees honest optimization/benefit. MVP adds the repository's existing four-host usability/distribution floor. Full maturity additionally closes retained optional/native and accepted proposed/coordination obligations. This is a sequencing proposal; it grants no waiver, retirement or product-scope change.

Ranges include scoped implementation, review, meaningful regression/negative controls, integration and receipts. They assume a competent engineer familiar with Rust/process supervision, reuse of reviewed existing PRs, access to desk/native profiles and no major dependency replacement. Each package excludes separately estimated packages. One engineer-day is about eight focused hours. Low/high describe plausible effort under current uncertainty, not statistical percentiles. External waiting and unrelated repository work are excluded. Do not divide mechanically by the number of agents: shared runtime files, native environments and acceptance owners serialize work.

## Work packages

| ID  | Outcome / acceptance                                                                                                                                         | Existing work to reconcile first                             | Dependencies                                 | Effort, engineer-days | Confidence                                                 |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ | -------------------------------------------- | --------------------- | ---------------------------------------------------------- |
| W0  | Freeze accepted profiles,public entrypoints,consumer/API baseline and artifact receipt format; nominate integration/assurance owner                          | PR878; PR858 cleanup; current FRs/ADRs                       | None                                         | 1-2                   | Medium                                                     |
| W1  | Reconcile overlapping correctness/dependency/CI candidates without duplicate rewrites; produce one reviewable integration base                               | PR876,893,896,899,906-909,918,921,930                        | W0                                           | 2-5                   | Low; candidate overlap and shared dependency state         |
| W2  | One truthful public execution path,stdout/stderr/exit status and owned child identity; retire misleading success through approved consumer contract          | Issue886; real Hypervisor and existing ProcessPort           | W0,W1                                        | 3-7                   | Medium; source defect clear,integration boundary open      |
| W3  | Explicit pure-task/effect eligibility,input/env/credential identity and generation-safe coalescing; different inputs never incorrectly share                 | PR878,927; cache/queue code                                  | W0,W2                                        | 4-8                   | Low; validity domains need design+native negatives         |
| W4  | Admission covers child lifetime;cancelling one subscriber preserves others;crash/timeout/restart reclaim only observed released capacity                     | Issue886; PR876; guard predicates                            | W2,W3                                        | 3-6                   | Low; cross-process races                                   |
| W5  | Cross-host resource/pressure signals with honest unavailable policy,power source and bounded observation overhead                                            | Fleet watch/thermal code; FR007/011                          | W0,W1                                        | 3-6                   | Low; Windows/WSL power/thermal semantics                   |
| W6  | Complete live fleet identity/leases and automatic heavy-work placement;credentials/effects remain scoped;remote results/cancel/reconnect return correctly    | Issue897; NATS registry and Maildir; peer placement contract | W2,W4,W5                                     | 7-15                  | Low; external authority/transport contract                 |
| W7  | Functional reports-only console,learned strategies/explanations,actions and traceable projected savings;fix invalid idle/RSS inference                       | Issue897; dashboard/serve/tray candidates                    | W0,W5; actions depend W2/W4/W6               | 5-10                  | Low; full page/interaction inventory not frozen            |
| W8  | Real two-host dogfood with identical and deceptive tasks,foreground workload protection,baseline comparisons and realized/projection uncertainty             | Dossier pilot; existing benchmarks/meters                    | W2-W7; baseline setup starts after W0        | 3-6                   | Low; first real result may require upstream repairs        |
| W9  | Current exact signed artifact,notarization where applicable,provenance and persistent deployment/telemetry receipt                                           | Signing/release workflows; PR908/918                         | W1; final receipt after W8                   | 3-7                   | Medium-low; signing/packaging pipeline integration         |
| W10 | Complete four-host CLI/serve/tray/dashboard floor,installer/update/rollback and scoped control parity                                                        | Finality matrix; existing native surfaces                    | W2,W5,W7,W9                                  | 5-12                  | Low; native UI and WSL boundaries                          |
| W11 | If adopted,wire nested transaction guards into durable dispatch/authority,subscriber budgets and uncertain-effect fencing                                    | Proposed FR013; ADR0009                                      | W3,W4,W6; explicit scope decision            | 8-20                  | Low; distributed authority is not a local predicate change |
| W12 | Qualify retained optional native FUSE on required platforms:mount/read/write/CoW/provenance/remap/cancel/crash cleanup                                       | PR919; FR009 tests/probes; native driver profiles            | W3,W4,W10; driver access                     | 6-15                  | Low; privileged native behavior                            |
| W13 | Close retained Airlock parity consumers:CAS notes,counters,generation events,lease-token lifecycle through existing boundary                                 | ADR0008; pinned parity contract                              | W0,W4,W6; consumer decision                  | 4-10                  | Low; only retained consumers counted                       |
| W14 | Independent assurance:fixed denominators,negative-control qualification,security remediation/profile review,consumer preservation and release evidence index | Existing coverage/security/FFI/tray PRs; docs5 gates         | W0; final acceptance after relevant packages | 4-8                   | Low; uncovered required profiles unresolved                |

## Totals and dependency structure

| Horizon              | Included packages | Remaining total                       | Principal qualification                                               |
| -------------------- | ----------------- | ------------------------------------- | --------------------------------------------------------------------- |
| CVP                  | W0-W9             | 34-72 engineer-days (272-576 hours)   | Limited supported two-host slice; no relaxation of future obligations |
| MVP                  | CVP + W10,W14     | 43-92 engineer-days (344-736 hours)   | Existing four-host floor and independent acceptance                   |
| Full mapped maturity | MVP + W11,W12,W13 | 61-137 engineer-days (488-1096 hours) | Conditional proposed/retained scope must be decided                   |

```text
W0 -> W1 -> W2 -> W3 -> W4 --------> W6 -----> W8 -> W9 -> W10
 |                          ^        ^         ^              |
 +----------> W5 ------------+--------+--> W7 --+              |
 |                                                            |
 +--> W14 (assurance in parallel; final receipt after outputs) <-+

W3/W4/W6 -> W11 (if adopted)
W3/W4/W10 -> W12 (optional native profile qualification)
W0/W4/W6 -> W13 (retained coordination consumers)
```

W7 can build a truthful read-only/report experience early; executable controls wait for the relevant safe backend. W9 pipeline repair can proceed after W1, but its accepted artifact is the final integration candidate, not an earlier unrelated build. W14 starts by qualifying instruments and remains independent of the implementation owner's completion claims.

## First bounded execution wave

1. W0 owner reconcile source/consumer profiles and PR878 with the actual integration branch; record the accepted public entrypoint and cache eligibility decisions.
2. W1 integrator reviews existing PR876/CI repairs rather than dispatching overlapping changes. Separate native/hosted/runtime gates and report exact local versus remote evidence.
3. Parallel disjoint lanes: W2 execution semantics; W5 pressure contracts; W7 truthful display/report design. Shared runtime/serve files have one writer; tests and UI split only at explicit boundaries.
4. Assurance prepares deceptive-equivalence,cancellation and foreground-impact fixtures before implementation acceptance. No large native/build wave on the laptop; use the stronger host with controlled parallelism.

## Estimate revision and finish evidence

Re-estimate after W0/W1, after the first public-entrypoint run and after the first remote result round trip. Record old range,new range and the source of change. A discovered failure is new work within its package until the assumptions no longer hold; do not hide it in a completion percentage.

CVP acceptance requires correct outputs/effects,one physical execution where sharing is safe,independent subscribers,no false savings,capacity reclaimed only after termination,and a measured foreground/resource benefit. MVP additionally requires the published install/update/rollback and four-host floor. Full maturity closes retained wider contracts or records explicit owner-approved replacement/sunset decisions with consumer receipts. A failed advantage hypothesis can justify a narrower or dependency-based design; it must not be masked by scorecard changes.
