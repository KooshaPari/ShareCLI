# Traceability slice 02 — vertical journeys

Date: 2026-09-30. Supplements TRACEABILITY-SLICE-01.

| Trace ID | Journey criterion | Current source relation | Oracle/contract | State |
|---|---|---|---|---|
| SC-T10 | durable session observation survives controller/store reopen | SQLite append-only observations + materialized session row | SC-VS-S01 | source/unit evidence exists; public slice pending |
| SC-T11 | recovery decision carries ProcessGeneration/OwnershipClaim | ProcessEvidence exists on SurfaceRecord, but AgentSession/ResumeRecipe drops it before RecoveryExecutor | SC-VS-S04/S05 + VERTICAL-SLICE-SESSION-RECOVERY | **structural implementation gap** |
| SC-T12 | recovery dry-run default has no launch | RecoveryExecutor::dry_run + CLI default semantics | SC-VS-S02 | strong primitive; mounted receipt pending |
| SC-T13 | ambiguous evidence never unattended launch | auto_resumable confidence gate | SC-VS-S03 | unit evidence; public-path receipt pending |
| SC-T14 | exited runtime observation has explicit logical-session recovery semantics | recovery_plan skips ObservationKind::Exited while executor permits SessionState::Exited | exited-session contradiction oracle | **semantic contradiction open** |
| SC-T15 | recovery operation survives controller restart/idempotent retry | no first-class RecoveryOperation identity mapped | SC-VS-S06/S12 | missing |
| SC-T16 | successful recovery creates/link new ProcessGeneration observation | executor reports Resumed after spawn; no generation/evidence link in result | SC-VS-S07/S09 | incomplete |
| SC-T17 | stale observation excluded | recovery_plan max-age logic exists | SC-VS-S10 | source/unit evidence; mounted receipt pending |
| SC-T18 | provider degradation remains explicit | surface provider/degraded contracts exist | SC-VS-S11 | partial |

Reverse path example:
`SessionObservation.surface.process → recovery_plan strips process evidence into AgentSession → RecoveryExecutor → SC-T11 → v1.1 OwnershipClaim/ProcessGeneration obligation`.

This is why existing recovery tests cannot close SC-J05 even though persistence/dry-run primitives are substantial.
