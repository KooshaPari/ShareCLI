# ShareCLI source/contract coverage ledger — pass 2

Status: **DENOMINATOR STRUCTURE ESTABLISHED / semantic closure incomplete**  
Date: 2026-09-30. Frozen source: `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

This supersedes pass-1 status statements. It does not claim product completion.

## Evidence corrections since pass 1

- targeted native recovery oracles **have executed**;
- SC-F01 and the equivalence-domain matrix reproduced durable-reuse counterexamples;
- exact PhenoInfra and PhenoShared dependency revisions are frozen in SNAPSHOT.json;
- direct user intent and February/June lineage were recovered in additional passes;
- ontology v1 candidate and architecture decisions SC-AD-01..04 now constrain interpretation;
- generic requirement-count growth is explicitly not a closure criterion.

## Closure denominator

A family is CLOSED only when:
1. source/history surfaces relevant to that family are enumerated;
2. authority conflicts are resolved or explicitly remain blocking;
3. mature obligations/non-obligations are extracted;
4. implementation and dependency surfaces are mapped;
5. positive + adversarial oracle set is defined;
6. exact native evidence exists for architecture-risk claims where required;
7. reverse trace has no unexplained product-significant orphan.

| Family | Discovery | Authority | Contract | Impl map | Oracle | Native risk evidence | Trace | State |
|---|---|---|---|---|---|---|---|---|
| Product thesis/aliases/lineage | strong | strong | v1 candidate | n/a | n/a | n/a | partial | **NEAR-CLOSED** |
| Observation/process identity | partial | strong | provisional | partial | designed | pending PID-reuse runtime | partial | OPEN |
| Ownership/supervision | partial | strong | provisional | partial | designed | pending | partial | OPEN |
| Mediation/dispatcher | partial | moderate | provisional | source-confirmed Unix path | designed | pending public truth fixture | partial | OPEN |
| Work equivalence | strong | strong | provisional | source-confirmed conflict | strong | **counterexamples reproduced** | strong slice | OPEN pending remediation architecture acceptance |
| In-flight sharing | strong | strong | provisional | source/test mapped | strong | pending current exact run | partial | OPEN |
| Durable result reuse | strong | strong | provisional | current generic modes challenged | strong | **counterexamples reproduced** | strong slice | OPEN |
| Admission/native jobserver | moderate | strong | provisional | current queue mapped | designed | comparative prototype pending | partial | OPEN |
| Lease/liveness/fairness | strong source | strong | provisional | source conflict mapped | strong | PID-reuse/fairness current receipts pending | partial | OPEN |
| Resource/thermal policy | partial | moderate | provisional | partial | designed | pending | weak | OPEN |
| Filesystem interception | strong | strong | provisional | mapped | optional observed; required pending | optional degradation observed | partial | OPEN |
| Mesh/session/distributed scope | partial | unresolved | not mature-frozen | partial | incomplete | none | weak | OPEN |
| Human UI/tray/dashboard | partial | moderate | capability-truth principle | partial | incomplete | none | weak | OPEN |
| CLI/machine APIs/IPC | partial | moderate | partial | partial | partial | selected native tests only | weak | OPEN |
| State/storage/migrations | partial | moderate | partial | partial | partial | queue/cache only | weak | OPEN |
| Security/authorization | partial | strong invariants | partial | partial | designed | incomplete | weak | OPEN |
| Packaging/install/platform parity | partial | moderate | not frozen | partial | incomplete | CI only, not clean-host acceptance | weak | OPEN |
| Release/lifecycle | partial | moderate | not frozen | partial | incomplete | none | weak | OPEN |
| Dependencies/integrations | partial | moderate | adapter principles | exact Pheno revisions frozen | partial | incomplete | partial | OPEN |
| Historical donor equivalence | moderate | moderate | selective recovery | partial | comparison designed | incomplete | partial | OPEN |
| External SOTA/alternatives | strong for equivalence/admission | external | decisions informed | n/a | prototypes pending | n/a | registry dossier | OPEN |
| Docs/support/accessibility | partial | low/moderate | overlay not decomposed | partial | incomplete | none | weak | OPEN |

## Current critical denominator

Architecture/specification cannot freeze until at minimum:
- in-flight-vs-durable split receipt;
- queue PID-reuse/fairness receipt;
- native-jobserver comparison;
- required-FUSE negative prototype;
- capability-truth public fixture;
- mesh/session mature-scope disposition;
- public surfaces and security boundaries decomposed;
- independent review.

The remaining work is therefore semantic and experimental, not “write N more requirements.”
