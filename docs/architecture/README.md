# sharecli architecture

This directory holds the canonical architecture diagrams for
sharecli. The diagrams are Mermaid; the source files can be
edited as plain Markdown and rendered by GitHub, Obsidian, or
the `mmdc` CLI.

| Diagram | File | Purpose |
|---------|------|---------|
| System overview (crate topology) | [01-system-overview.md](01-system-overview.md) | Which crate depends on which, and the role of each |
| Process lifecycle data flow | [02-process-lifecycle.md](02-process-lifecycle.md) | How a `sharecli start` command flows from shell to a managed process |
| FUSE write-provenance intercept | [03-fuse-write-provenance.md](03-fuse-write-provenance.md) | How the FUSE layer tags every write with `user.sharecli.session` and `user.sharecli.written_at` |

## Reading order

1. **01-system-overview** — start here. Read once to learn the crate
   names and what each one does. Re-visit when you need to know
   "where does this functionality live?".
2. **02-process-lifecycle** — read when working on `start`, `ps`,
   `stop`, `status`, or the ProcessPool itself. The data-flow
   diagram is the source of truth for "who calls who" during a
   process-management operation.
3. **03-fuse-write-provenance** — read when working on the FUSE
   layer, the mount/unmount flow, or the xattr-based auditing.
   The diagram pins the order of operations in the FUSE
   write callback.

## Cross-references

- `docs/adr/0007-ports-adapters-hexagonal.md` — explains the
  `sharecli-core` ↔ `sharecli-fleet` ↔ `sharecli-thermal-tui`
  hexagonal split that 01-system-overview diagrams.
- `docs/adr/0009-global-nested-ledger-local-admission.md` —
  explains why `sharecli-session` keeps its own ledger and does
  not depend on `sharecli-fleet`.
- `docs/adr/0008-airlock-convergence-contract-capture.md` —
  explains the contract-capture surface between the CLI binary
  and the sharecli-fleet fleet-aggregator.
- `FUNCTIONAL_REQUIREMENTS.md` — pins the FR-001..FR-011 IDs
  that map to the runtime behaviour shown here.
