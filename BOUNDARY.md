# Boundary Lock: ShareCLI agent runtime / hypervisor

**Status:** ACTIVE — OS/kernel-adjacent agent runtime / hypervisor for multi-agent hosts.

## Owns
- Cross-project CLI process lifecycle (spawn, supervise, teardown)
- Shared orchestration hooks consumed by agent tooling

## Does NOT own
- Agent runtime / tool registry (`thegent` — absorb later when stable)
- Code review CLI (`tehgent`)

## Duplicate
`thegent-sharecli` is a fork-line duplicate — canonical repo is **sharecli**. Absorb into `thegent` only after runtime boundary stabilizes.