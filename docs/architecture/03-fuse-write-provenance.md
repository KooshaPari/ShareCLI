# 03 — FUSE write-provenance intercept

This diagram traces what happens when an agent writes a file
through a FUSE mount created by `sharecli fuse mount`. It is
the source of truth for **FR-009** (FUSE IO intercept) and the
write-provenance xattr contract that stamps every
agent-originated write.

```mermaid
flowchart TD
  classDef agent fill:#1e293b,stroke:#94a3b8,color:#f1f5f9
  classDef kernel fill:#0c4a6e,stroke:#7dd3fc,color:#e0f2fe
  classDef fuse fill:#14532d,stroke:#86efac,color:#dcfce7
  classDef core fill:#7c2d12,stroke:#fdba74,color:#ffedd5
  classDef store fill:#581c87,stroke:#d8b4fe,color:#f3e8ff

  A[Agent process<br/>open/write/close<br/>on /mnt/agent]:::agent
  VFS[Linux VFS]:::kernel
  F3[fuse3 kernel module]:::kernel
  PROC[FUSE userspace<br/>sharecli-fuse]:::fuse
  COW{COW / CoW?}<br/>(interceptor flag):::fuse
  STAGE[Stage in /tmp/sharecli-cow/]:::fuse
  BAK[Backing file<br/>~/work/foo.txt]:::fuse
  PROV[provenance::annotate_write]:::fuse
  XATTR1[setxattr<br/>user.sharecli.session]:::store
  XATTR2[setxattr<br/>user.sharecli.written_at]:::store
  QUERY[sharecli fuse provenance FILE<br/>reads the attrs back]:::core
  CLI[sharecli fuse commit<br/>--agent myagent]:::core

  A -->|write fd| VFS
  VFS -->|fuse3 ops| F3
  F3 -->|/dev/fuse| PROC
  PROC --> COW
  COW -->|yes| STAGE
  COW -->|no| BAK
  STAGE -->|deferred| BAK
  BAK --> PROV
  PROV --> XATTR1
  PROV --> XATTR2
  XATTR1 -.->|read back| QUERY
  XATTR2 -.->|read back| QUERY
  BAK -.->|sharecli fuse commit| CLI
  CLI -->|merge stage -> backing| BAK
```

## Reading the diagram

1. The agent writes to a file under the FUSE mount point
   (default `/mnt/agent`, configured via `--mountpoint`).
2. The Linux VFS routes the `write(2)` syscall to the `fuse3`
   kernel module via the well-known `/dev/fuse` device.
3. `sharecli-fuse` (the userspace daemon) receives the
   `fuse_write` callback. The crate is split into a portable
   core (`src/lib.rs`) and per-OS adapters:
   - **Unix**: `src/unix_mount.rs` — direct FUSE loop.
   - **macOS**: `src/macfuse_mount.rs` — macFUSE equivalent.
   - **Windows**: `src/winfsp_mount.rs` — WinFsp, with NTFS
     alternate data streams in place of xattrs.
4. If the path is in the CoW set (see `sharecli-fuse list
   --agent`), the write is staged in
   `/tmp/sharecli-cow/<agent>/<path>`. Otherwise it goes
   straight to the backing file.
5. Either way, after the write completes,
   `provenance::annotate_write(&path, &session_id)` is
   called. This sets two xattrs (or ADS on Windows) on the
   **backing file**:
   - `user.sharecli.session = <session-id>` (opaque, UTF-8)
   - `user.sharecli.written_at = <unix-secs>` (decimal ASCII)
6. The provenance record lives **only on the backing file**.
   `sharecli-fuse` does not depend on `sharecli-session` and does
   not append writes to any ledger, so there is no per-session
   replay log of the write sequence — the audit surface is the
   current xattr/ADS pair on each file, read back with
   `sharecli fuse provenance <path>` (add `--json` for machine
   output; it prints `null` when the attrs are absent).
7. The CoW stage is only promoted to the backing file when
   the operator runs `sharecli fuse commit --agent myagent`.
   `sharecli fuse discard --agent myagent` is the no-merge
   counterpart.

## Why this shape

- **The backing file is the source of truth, not the FUSE
  layer.** Even if `sharecli-fuse` is killed mid-write, the
  xattrs on the backing file are still queryable
  (`sharecli fuse provenance <path>` reads them). The FUSE layer is a
  routing/auditing wrapper, not a content store.
- **Provenance is set after the write succeeds, never
  before.** The contract is: a file with a `user.sharecli.*`
  xattr is guaranteed to have the matching content. We never
  set an xattr speculatively and then fail to write the
  bytes.
- **Failures are loud.** From `provenance.rs:5`:
  > "Failures are loud: never silently skip provenance when
  > a write succeeds."
  If `setxattr` returns EPERM or ENOTSUP, the write is
  considered failed and the FUSE layer returns EIO to the
  agent. The agent sees a write error; the operator sees
  nothing half-written.
- **CoW + commit is a two-phase commit, not a transaction.**
  An agent can write, then write more, then the operator
  either commits or discards. There is no automatic commit
  on unmount — `sharecli fuse unmount` leaves staged writes
  in `/tmp/sharecli-cow/`.

## Cross-platform behaviour

| Backend | Attribute name | Storage mechanism |
|---------|---------------|-------------------|
| Linux (fuse3) | `user.sharecli.session` | `setxattr(2)` |
| Linux (fuse3) | `user.sharecli.written_at` | `setxattr(2)` |
| macOS (macFUSE) | `user.sharecli.session` | `setxattr(2)` |
| macOS (macFUSE) | `user.sharecli.written_at` | `setxattr(2)` |
| Windows (WinFsp) | `sharecli_session` (ADS) | NTFS alternate data stream |
| Windows (WinFsp) | `sharecli_written_at` (ADS) | NTFS alternate data stream |

Both the Unix xattr and the Windows ADS names are
deliberately namespaced (`user.sharecli.*` on Unix,
`sharecli_*` on Windows ADS) so they survive agent processes
that write their own xattrs/ADS without colliding.

See [docs/adr/ADR-009-harbor-soak-alternative.md](../../adr/ADR-009-harbor-soak-alternative.md)
for the rationale on the FUSE-on-Windows path
(WinFsp with NTFS alternate data streams) and why we
chose ADS over a custom xattr shim.
