# ShareCLI implementation mapping — mounted surfaces pass 2

Date: 2026-09-30. Frozen source `4f01d0199e82b62bcf20399afcc102f58a10ad07`.

## Root CLI reachability

The actual Clap root mounts substantial surfaces:
- session and terminal-surface recovery/control;
- managed-process start/stop/status;
- host process discovery;
- health/config/project;
- optimize/prune/pool/run;
- project limits/check;
- thermal TUI;
- HTTP/WebSocket serve;
- fleet;
- mesh;
- FUSE;
- cross-machine cast;
- process-compose;
- upgrade/uninstall/undo.

This proves these surfaces are reachable commands, not that every one belongs to the accepted mature spine.

## Presentation contradiction

Root CLI description currently says:

`Shared CLI process manager for multi-project agent orchestration`.

README framing also uses “mesh” and broad orchestration language. SC-AD-05 now provisionally bounds the mature product to local OS-adjacent coordination rather than distributed agent orchestration.

Classification:
- mounted CLI surface = current implementation fact;
- broad “agent orchestration” description = **presentation claim requiring authority reconciliation**;
- Fleet/Cast remote/cross-machine commands = **reachable implementation whose mature-role must be independently justified**.

Do not delete them from the spec branch. Do not count them as mandatory mature obligations merely because Clap exposes them.

## Surface-to-ontology mapping

| Surface | Ontology/capability relation | Current mature disposition |
|---|---|---|
| `proc` | ObservedProcess/resource observation | spine candidate |
| `start/stop/ps/status` | OwnershipClaim/ProcessGeneration/supervision | spine candidate; generation safety open |
| `session/surface` | durable local coordination/recovery | spine candidate; provider capability evidence needed |
| `run/pool` | runtime/admission | needs provider/semantics mapping |
| `limits/check/thermal` | resource observation/policy | spine candidate; enforcement truth mapping open |
| `serve` | machine/operator projection | spine candidate; auth/reachability mapping required |
| `fuse` | FilesystemSession adapter | optional/required capability |
| `mesh` | local durable coordination | retained only for traced local obligations |
| `fleet/cast` | remote/cross-machine coordination | **scope candidate / not automatically mature** |
| `proc-compose` | external integration | adapter candidate |
| `upgrade/uninstall/undo` | product lifecycle/recovery | mature lifecycle candidate |
| `soak` | evaluation/support tooling | not automatically product outcome |

## New falsification questions

1. Can SC-J01 close with Fleet/Cast/Mesh disabled? If yes, they are not spine prerequisites.
2. Does any direct accepted user intent require cross-machine Cast/Fleet in mature ShareCLI?
3. Does `serve` expose capability truth or reproduce legacy “managed/optimized” ambiguity?
4. Can session recovery refuse stale/reused PID identity after controller restart?
5. Do update/uninstall/undo operate on the v1.1 durable identity model or legacy paths?

## Consequence

Current command breadth is not the mature-contract denominator. Mounted-but-unjustified surfaces enter an **orphan/scope-candidate ledger** until authority + journey + oracle establish their role.
