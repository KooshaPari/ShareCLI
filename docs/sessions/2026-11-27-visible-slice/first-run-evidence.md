# ShareCLI first visible slice — evidence report

Date: 2026-11-27 (local) / 2026-11-28T00:0xZ execution
Host: nixos (16-core Ryzen 7 1800X, 31.3 GiB)
Repo: /home/kooshapari/CodeProjects/ShareCLI @ b05da595
Worktree: /home/kooshapari/CodeProjects/ShareCLI-space/feat/visible-slice/wtree (branch feat/visible-slice)
Installed binary: /home/kooshapari/.local/bin/sharecli
sha256: d29083662acf9cb67b118d1bae1a2cf26e80ccd35b82b36d033c58d5dfa731d7
size: 16,657,336 bytes (16M)
No screenshots. Every output below is verbatim from the terminal.

## BUILD

Prescribed command (spec §1), verbatim:

    nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc --command bash -c 'cargo build --release -p sharecli'

Result: **rc=101**, NOT_FIXABLE with the prescribed toolchain as written. First error verbatim:

    error: failed to run custom build command for `spawn-core-sys v0.8.1
      (/home/kooshapari/CodeProjects/ShareCLI-space/feat/visible-slice/wtree/crates/spawn-core-sys)`
      --- stderr
      Error: failed to run `zig build`: No such file or directory (os error 2)
      Is zig installed and on PATH?

This is a missing build tool, not a code error: crates/spawn-core-sys/build.rs shells out to
`zig build` in crates/spawn-core. Zig is required and pinned: justfile:342 says
"zig 0.14.1 required (see .devcontainer/post-create.sh)".

Attempt 2 with nixpkgs#zig (0.17.1): **rc=101**, real API break, verbatim:

    build.zig:23:8: error: no field or member function named 'linkLibC' in 'Build.Step.Compile'

Attempt 3 with the pinned toolchain — the same command plus `nixpkgs#zig_0_14` (0.14.1):

    nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc nixpkgs#zig_0_14 \
      --command bash -c 'zig version; cargo build --release -p sharecli'

Result: **rc=0** in 9m23s, `zig version` printed `0.14.1`, `Finished \`release\` profile [optimized] target(s)`.

Install (spec §2):

    install -m755 target/release/sharecli ~/.local/bin/sharecli
    ~/.local/bin/sharecli --version  ->  sharecli 0.8.1

Verdict: the binary is real and runs. The documented build recipe is incomplete (missing zig 0.14.1);
a user following the spec's command honestly receives rc=101.

## VERBS

Every row below was executed against the installed binary. Full transcripts:
/home/kooshapari/CodeProjects/temp/visible-slice-verbs.log

| verb | verdict | evidence (verbatim) |
|---|---|---|
| `sharecli --help` | WORKS | Lists 37 commands (`session surface ps start stop status proc health config project optimize prune pool run limits check thermal serve report fleet mesh fuse cast proc-compose completions man list soak upgrade version uninstall undo help`), rc=0 |
| `sharecli ps` | PARTIAL | rc=0 but it lists *managed* processes only: `Total: 0 processes, 0 MB memory` / `No managed processes yet. Get started: sharecli start <project> <harness>`. It does NOT see the 25 live agents on this host. Premise in the spec ("proc-scan detection") is wrong for `ps`; detection lives in `proc`/`status`. |
| `sharecli status` | WORKS | rc=0; full snapshot: `Total: 0 processes`, pool `node 0/0 idle · bun 0/0 idle · max 5 · healthy`, `Proc scan 25 · watched 24 · 0 managed · 24 agent row(s)`, `Used: 8273 MB / 32013 MB (25%)`, `Thermal level: GREEN`, `Detected agents: 25`, `Gate decision: [DENY]` |
| `sharecli health` | WORKS | rc=0; `Shared runtime health: HEALTHY` / `No runtime issues detected.` / pool per-harness 0 total 0 idle / `Max per harness type: 5`, plus thermal gate block |
| `sharecli pool` | WORKS | rc=0; `node 0 0 5` / `bun 0 0 5`, `Max per type: 5`, `Health Check: Status: HEALTHY` |
| `sharecli limits` | PARTIAL | no args: rc=2, clap `error: the following required arguments were not provided: <PROJECT>`. With args: rc=0, `Set limits for project 'demo-proj': Memory: 512 MB / Max processes: 4`. BUT the store is per-process in memory (src/runtime/resources.rs:62 `RwLock<HashMap>`), so a later `sharecli check demo-proj` reports defaults `Memory: 748 MB / 1024 MB`, `Processes: 17 / 10`. The set is silently discarded when the process exits. |
| `sharecli run -- echo hi` | MISSING | rc=74, `SHARECLI_ERROR_CODE=io error: pooled run unavailable: no command execution backend is wired; no work was executed`. `--help` itself says "currently unavailable; returns an error without executing work". There is no supervised command executor. |
| `sharecli optimize` | PARTIAL | rc=0; prints `Analyzing resource usage...` with a 0-row table, then the heading `=== Optimization Suggestions ===` and nothing after it. No suggestions, no "none" line, and the 25 detected agents are ignored. |
| `sharecli mesh status` | PARTIAL | no `--queue`: rc=2 clap `error: the following required arguments were not provided: --queue <QUEUE>`. With a real Maildir: rc=0, `mesh queue: .../maildir` / `ready: 0` / `in_flight: 0` / `pending: 0`. The verb works; the spec invocation was incomplete. |

Extra verb executed (not in spec): `sharecli version` -> ASCII banner + `sharecli 0.8.1` / `multi-agent process manager`, rc=0.

## GATE — proc-scan detection turns on and off

Claim chosen: "detection must see a live agent process that I start myself" (spec §4).
`sharecli ps` cannot satisfy this (managed-only), so the load-bearing detector is `sharecli proc`
(the same scan `status` summarises as "Detected agents").

Method: copy bash to a binary named `jcode` (comm becomes `jcode`, an exact match in
crates/sharecli-fleet/src/detect.rs:387), start it, query, kill it, query again.

Proof the process is real and named as claimed:

    jcode-pid=3169458 comm=jcode
    zzz-pid=3169460 comm=zzz-plain

GATE ON (process alive):

    3169458  jcode            S      4M         3        jcode
    Total: 29 agent process(es)

Baseline immediately before: `Total: 24 agent process(es)` (host churn moves this number).

GATE OFF (same pid, after `kill`):

    my jcode pid present count: 0
    Total: 25 agent process(es)

So: OFF -> ON -> OFF is demonstrated on the exact pid I started. Detection works.

### The gate is also too loose (false positives, measured)

Same harness, a process whose **comm is not an agent at all** but whose argv contains the literal
string `jcode`:

    3169460  jcode            S      4M         3        zzz-plain
    Total: 1 agent process(es)

`zzz-plain` is classified `FAMILY=jcode`. Cause: cmdline substring fingerprints
(crates/sharecli-fleet/src/detect.rs CMDLINE_FINGERPRINTS, `("jcode", &["jcode", ".jcode", ...])`)
match anywhere in argv. Observed on the live host, unfiltered: `systemd-inhibit` (argv wraps a
`~/.jcode/...` path), `rustc`, and `sharecli` itself (argv contains `--family jcode`) were all
reported as `FAMILY=jcode`. The "Detected agents: 25" headline on this host therefore over-counts.

### `kcode` is not a known family (measured)

A live process with comm exactly `kcode`:

    kcode pid=3514625 comm=kcode
    sharecli proc --family kcode  ->  No known agent processes detected on this host.

`kcode` is absent from `KNOWN_AGENT_FAMILIES` and `CMDLINE_FINGERPRINTS`
(crates/sharecli-fleet/src/detect.rs:27-56). The operator's primary harness is invisible to the
inventory.

Also, `proc --family <unknown>` prints "No known agent processes detected on this host" while
silently omitting 25-26 real agents, which reads as "host is idle" rather than "your filter matched
nothing".

## GAPS — ranked by what a user hits next

1. **No supervised command execution.** `run` is a stub (rc=74, "no command execution backend is
   wired"). The core value proposition ("process manager" that runs work) is missing on this slice.
   `src/commands/process_ops.rs:154` returns `SharedRuntime::execution_unavailable()`.
2. **`limits` never persists.** Set and check are separate processes; the store is an in-memory
   `RwLock<HashMap>` (`src/runtime/resources.rs:62`). `limits` prints success and forgets.
   `check` also has no real accounting: it counts any process whose cmdline contains the project
   name, so the project string in the caller's own shell inflates the count (observed 17 processes
   for a project that has none).
3. **Agent inventory precision.** Cmdline substring matching produces false positives
   (`systemd-inhibit`, `rustc`, `sharecli` itself as `jcode`), so `Detected agents` / `Agent RSS
   total` over-count. This feeds the gate decision below.
4. **Thermal gate DENIES on a normal working host.** At 25 detected agents the gate prints
   `Agent contention: REFUSE (count warn>=4, refuse>=8)` and `Gate decision: [DENY]` while RSS is
   2.97 GB against a 34 GB refuse threshold. The count thresholds (warn 4 / refuse 8) are far below
   a normal fleet, so any start path is denied by default.
5. **`ps` and `proc` are confusingly split.** `ps` says "0 processes" and "No managed processes yet"
   on a host with 25 live agents; a new user will conclude detection is broken.
6. **`optimize` is inert.** Heading with no content, ignores detected (unmanaged) agents.
7. **Build recipe incomplete.** Requires zig 0.14.1, undocumented in the top-level build command;
   the documented command fails rc=101. Also `crates/spawn-core/build.zig` does not compile under
   zig 0.17.x (`linkLibC` removed), so an unpinned `nixpkgs#zig` is a trap.
8. **`kcode` unrecognised** (see GATE), so the operator's own harness is missing from inventory.
9. **`mesh` verbs require `--queue`** with no default; fine once known, but a bare `mesh status`
   is a clap error.

## UNKNOWN (not measured)

- Live HTTP/WebSocket surface (`sharecli serve`) — not started; no port bound, no browser exercised.
- `start`/`stop`/`prune` lifecycle against a real harness (no harness launch attempted).
- `fleet`, `cast`, `fuse`, `thermal` TUI, `report`, `soak`, `session`, `surface`, `proc-compose`,
  `undo`, `upgrade`, `uninstall` — outside the spec's verb list; not exercised.
- Persistence of `limits` across a daemon (with `serve` running) — untested; the in-memory store is
  per-process, so a long-lived server may hold state, but that was not run.
- Behaviour under `NO_COLOR`/`TERM=dumb` beyond clap help (not measured).
- macOS/Windows behaviour of any of the above.

## Scope rules honoured

- No agents spawned. No merges, no PR closes, no force-push, no test gates weakened.
- Worktree isolated at ShareCLI-space/feat/visible-slice/wtree; ShareCLI-space/fix/* untouched.
- In-repo evidence copy: docs/sessions/2026-11-27-visible-slice/first-run-evidence.md (this file),
  committed with tx-agent: kcode / tx-validated: build.
