# ShareCLI — source coverage ledger, pass 1

Status: **OPEN DENOMINATOR / NOT SPECIFICATION-COMPLETE**. Observation date: 2026-09-29. Source: `4f01d0199e82b62bcf20399afcc102f58a10ad07`; registry: `85d7cd00cf59c379c05b740e8130a85b0d5bd31b`. These are analyzed snapshots, not branch-head acceptance. See `SNAPSHOT.json`.

This is an additive recovery layer. Existing FR identifiers, accepted ADRs and historical artifacts remain intact. A source being read does not close its family. Full inventory, semantic reconciliation, mounted-call mapping, journey implications and oracle consequences must all be recorded before closure. No coverage percentage is currently defensible. The family rows below are a discovery checklist, not a synthetic requirement catalog.

## Authority rules

USER_INTENT is a user statement with provenance. ACCEPTED_DESIGN is a decision record with its actual authority, not automatically a user quotation. CURRENT_IMPLEMENTATION is an exact source observation, not intended behavior. HISTORICAL_IMPLEMENTATION, PROPOSAL, EXPERIMENT, ASSISTANT_SUGGESTION and EXTERNAL_PRIOR_ART remain distinct. A confidence score cannot upgrade authority. Duplicate registry mirrors count once as evidence, not independent corroboration.

## Inspected source records

All product paths below are at the source SHA above. Each record remains partially resolved unless explicitly stated otherwise.

| ID | Source / extent / blob where captured | Classification and meaning | Obligations, contradictions, journey and verification consequences | Resolution |
|---|---|---|---|---|
| SC-S01 | `README.md`, full; `cf711afaca3947bda0a3d1c1a52cfc5c0d6886b6` | Supporting product description: OS-adjacent multi-agent runtime plus supervisor/tray/serve | Separate observation, intervention and sharing; installation/port/parity assertions require artifact tests. README is not execution proof. | PARTIAL |
| SC-S02 | `FUNCTIONAL_REQUIREMENTS.md`, full; `b5cb4474489cb0c1ba2c469cd3ede3ef51b1f599` | Existing contract index; preserve FR-001 onward and legacy aliases | Lifecycle/config/projects/health/limits/detection/watch/coalesce/FUSE/mesh/thermal; FR-012 mentioned without matching body in this index; cast extensions outside base index. Trace targets are claims until callers/tests checked. Resource checking is not enforcement. | PARTIAL; detailed FR/trace files outstanding |
| SC-S03 | `docs/adr/0006-feb-harness-recovery-lineage.md`, full; `30699abfaa643707b2a0e909d60f85f80cccae18` | ACCEPTED_DESIGN record, dated 2026-07-19, maintainers named collectively | February agent-harness included FUSE; March twin is not February donor; June ownership flip; recover rather than rewrite; tray/serve current UX; Harbor outside product scope. Donor equivalence still needs evidence. | PARTIAL |
| SC-S04 | `docs/ops/feb-recovery.md`, full; `5cf76f11ed12f8df49f5458f68a0441fbf3d1a01` | Supporting/historical recovery map and author-reported completion | Vault is not a Git clone and lacks original FUSE Rust source. Prior mount/signing blockers are historical observations, not current green/red. Ports need behavior-level comparison. | PARTIAL |
| SC-S05 | `crates/sharecli-ipc/src/cache_key.rs`, full; `4ade430be6d82243b5d4059bdfb22fb52c1d5b1d` | CURRENT_IMPLEMENTATION | Git key uses status+HEAD rather than content; Time has no content identity; Args ignores cwd/env. Semantic collision oracle required before accepting result reuse. Tests currently assert mode shape, not full equivalence. | PARTIAL; source-derived counterexample recorded separately |
| SC-S06 | `crates/sharecli-ipc/src/lib.rs`, lines 1–230; `0304afd84bfa6ca68c633be599e721a5b5d2b136` | CURRENT_IMPLEMENTATION | TTL defaults to 300s; CachedResult has exit/stdout/stderr. TTL is freshness policy, not proof of equal inputs. Lock deadline uses wall clock. Inspect remaining store/lock/error paths and callers. | PARTIAL |
| SC-S07 | `crates/sharecli-ipc/src/queue.rs`, implementation through guard; test section truncated | CURRENT_IMPLEMENTATION | u8 cast wraps aging at 256 steps; lexical ticket ordering differs from sequence ordering; failed unlink can be ignored before guard release. Require live-peer/orphan distinction and bounded crash recovery, not a rank-only assertion. | PARTIAL; full tests and runtime missing |
| SC-S08 | `crates/sharecli-core/src/lib.rs`, lines 180–380 and 450–690 plus search excerpt for key call; `cb35ab8d126c4571a2f329869efaec5102d94906` | CURRENT_IMPLEMENTATION | Hypervisor configuration defaults to Time mode; SpawnOutcome separates cached result from FUSE/watch fields. Best-effort FUSE fallback needs explicit mode contract; unsupported platform/failure must not masquerade as enforced interception. Full run and CLI wiring not yet traced. | PARTIAL |
| SC-S09 | Registry `docs/governance/atlas/products/ShareCLI/STATE.md`, full; `be62b47e1f15f38ddccb4ce0e10678d3d529bee0` | Historical bounded assessment at `19cdb88da5e79f0b06953499d51444016c07e8d2` | PR858 compression parity explicitly unestablished; do not revert cleanup or infer no public consumers from absence of local calls. | PARTIAL; migration contract review needed |
| SC-S10 | Selected `harness` commit metadata, oldest-first first 8 results | HISTORICAL_IMPLEMENTATION pointers / author assertions | Includes `e9459ad8c223b8d2b179135f44073b3c47f4b908` absorption, `ab700c5c1e6f181fe55efa2cc4a012f44bafd271` FUSE scaffold. Not full ancestry/diff review. | PARTIAL |
| SC-S11 | Open PR #876 description | Imported implementation-candidate assertions | IPC spawn/kill/deadline remediation is separate from main; reported full integration UNKNOWN. No candidate result is imported into snapshot grading. | PARTIAL; exact head and changes outstanding |
| SC-S12 | Registry search `agent-harness`: `handbook/specs/planning/agent-mesh-wbs-plan-v2.md` excerpt | Historical plan / proposal | February phases and core.sh/bin/harness terminology expand alias search. Do not inherit every later mesh proposal as accepted ShareCLI scope. | PARTIAL |
| SC-S13 | Prior conversation retrieval by names and concepts | USER_INTENT where directly attributed; otherwise ASSISTANT_SUGGESTION | Recovered requests for intent recovery and product work; September assistant formulations do not define authority. One narrow lineage retrieval failed; failure is not absence. Raw transcript/older conceptual search remains open. | PARTIAL |

## Source-family denominator

| Family | Known surfaces / remaining enumeration | Current disposition and closure work |
|---|---|---|
| Product intent and aliases | SC-S01–04,13; agent-harness, harness-fuse, core.sh, rules.conf, Lock-Wait-Cache, thegent-sharecli | PARTIAL; exact historical utterances and acceptance changes |
| README / docs | README, docs/configuration, docs/deploy and FINALITY pointers | PARTIAL; reconcile claims with installed behavior |
| ADRs | ADR0006 read; ADR0002/0004/0005 referenced | PARTIAL; enumerate and resolve scope decisions |
| Specifications | root index; docs/specs/FR.md, TRACEABILITY.md; cast extensions | PARTIAL; semantic obligations, no target count |
| Source modules | core/ipc/fleet/fuse/mesh, supervisor and platform modules | PARTIAL; exhaustive tree/module and export map |
| CLI | src/main.rs, commands, mesh/fuse/process verbs | OPEN; parser → dispatch → effects → errors |
| Machine APIs / IPC | serve, sharecli-ipc and websocket surfaces | OPEN; mounted routes, auth and protocol shapes |
| MCP | ADR0004 no-first-party-server pointer | OPEN; verify exclusion, do not manufacture an MCP requirement |
| Human interfaces | tray/native shells/dashboard/cast/panes | OPEN; user navigation and reachability, not screenshots alone |
| State / storage / migrations | config registry, cache, tickets, fleet and Maildir queue | PARTIAL; durable identities, locks, migrations, restarts |
| Tests / fixtures / graders | existing FR references; model controls | PARTIAL; collect execution identities, skipped/empty checks and mutations |
| CI / development policy | .github/workflows, imported audit catalogs | OPEN; identify authoritative gates; inherited generic catalog excluded from this program's grading |
| Deployment / distribution | installer, formula, container, OS parity docs | OPEN; signed artifact and actual clean-host verification |
| Release lifecycle | tags, workflows, signing and sunset policies | OPEN; accepted human stable-promotion boundary vs assertions |
| Security / authorization | JWT/IPC permissions/plugin/FUSE/cache trust | OPEN; cross-user/cache leakage, process ownership and secret redaction |
| Reliability / observability | health, resource sampling, thermal, metrics | PARTIAL; live signal quality and overload/recovery experiments |
| Integrations / dependency revisions | Substrate and donor pointers, external tools | OPEN; inspect manifests/locks and freeze every source materially used |
| Historical implementations | February vault, March orphan DAG, June ports, PR858 | PARTIAL; available history not exhausted |
| Registry records and mirrors | atlas and handbook duplicate folders; project card | PARTIAL; resolve duplicates and contradictions without double counting |
| External standards / SOTA / academia | pass-1 research in corresponding registry dossier | PARTIAL; library versions, compatibility, license and project-health review |
| Documentation / support / quality | accessibility, localization, install recovery and help | OPEN; shared overlays tied to applicable journeys |
| Auxiliary/generated/assets | branding, vendored audits, generated artifacts | OPEN; explicit non-normative disposition and exclusion from grading |

## Resolution rule and receipt

A closed record must name its extracted obligations or explicit non-normative judgment; conflicts and their authorized resolution; relevant implementation surfaces; affected stages/journeys; and evidence required to falsify correctness. Family closure requires exhaustive source enumeration as well. Search snippets, a truncated tree, or metadata-only commits cannot establish that.

Local clone failed DNS; Rust/cargo unavailable. No native product suite was executed in this pass. Source-derived model experiments are useful negative evidence but cannot close runtime, parity, journey, or architecture-risk gates. No mature completion or product percentage is assigned.
