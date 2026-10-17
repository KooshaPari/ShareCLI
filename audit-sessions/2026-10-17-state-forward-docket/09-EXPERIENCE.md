# 09 — Operator experience, dashboard and decision support

## 9.1 Operator-visible surfaces (UI / dashboard)

| Surface | Type | Where | Audience | State |
|---|---|---|---|---|
| `sharecli --help` | CLI | stdout | operator | rendered with clap derive; coverage complete |
| `sharecli ps` | CLI | stdout | operator | tabular output; json option |
| `sharecli status` | CLI | stdout | operator | tabular + json; default `--format table` |
| `sharecli start/stop/run` | CLI | stdout + process side-effects | operator | sysexits-style exit codes |
| `sharecli config` | CLI | stdout + `~/.config/sharecli/config.toml` | operator | get/set/reset/list/validate |
| `sharecli completions {bash,zsh,fish,pwsh}` | CLI subcommand | stdout | ops/install | emitted; not asserted in CI |
| `sharecli man` | CLI subcommand | stdout | ops/install | emitted |
| `sharecli-tray-linux` | tray daemon | desktop tray | operator | icon + menu; libappindicator backend |
| `sharecli-tray-windows` | tray daemon | desktop tray | operator | icon + menu; ksni backend |
| `sharecli-fuse mount_smoke` | smoke binary | stdout | dev/CI | mount + unmount + integrity check |
| `sharecli-dashboard` (web) | web UI | browser | operator | 12 widgets; `axe dashboard (WCAG 2.x Level A)` PASS; `dashboard keyboard Tab-cycle` PASS; `dashboard PNG hard diff` PASS |
| `sharecli-fleet` (in-process pub/sub) | internal | n/a | internal | pub/sub API |
| `sharecli-mesh` (worktree pool) | internal | n/a | internal | cross-agent coordination |
| Rustdoc | docs | browser (cargo doc --open) | developer | 64% on hot crates; rustdoc CI runs |
| `github.com/KooshaPari/ShareCLI` | web (README, FR.md, SPEC.md) | browser | operator, contributor | rendered on every PR |
| Tracera sharecli product card | web | browser | org | not yet created |
| AgilePlus sharecli backlog | web | browser | operator | not yet created |

## 9.2 Live inventory at operator's hand (today)

| Question | How answered | Latency | Caveat |
|---|---|---|---|
| Which agents are running? | `sharecli ps` | <1 sec | supervised only; bare `claude`/`codex` not in this list |
| How much CPU is agent X using? | `sharecli status --format json` | <1 sec | sample-based; not continuous |
| How much MEM is agent X using? | same | <1 sec | RSS sample |
| What is the cap on agent X? | same | <1 sec | from `~/.config/sharecli/config.toml` |
| Is sharecli cooling the agents? | `sharecli status` shows thermal state | <1 sec | thermal governor |
| What did agent X do in the last hour? | `sharecli session show <id>` (proposed, not yet shipped) | n/a | proposed in FR-005 |
| What worktrees are in flight? | `sharecli mesh list` (proposed) | n/a | proposed |
| What is the latest crash? | `sharecli log` (proposed) | n/a | proposed |

## 9.3 Decision-support (operator's decision tree)

```
+-- Operator has sharecli installed?
|   +-- yes --> Go to "live with sharecli"
|   +-- no  --> Go to "install"
|
+-- Install
|   +-- via `cargo install --path .` (current; requires Rust toolchain)
|   +-- via `brew install sharecli` (NOT YET; planned v1.1)
|   +-- via `apt install sharecli` (NOT YET; planned v1.1)
|   +-- via Homebrew tap `KooshaPari/homebrew-sharecli` (NOT YET)
|
+-- Live with sharecli
|   +-- `sharecli ps` (see what's running)
|   +-- `sharecli config show` (see current caps)
|   +-- `sharecli start myproj --harness claude` (add an agent)
|   +-- `sharecli stop myproj` (kill it)
|   +-- `sharecli status` (resource usage)
|   +-- `sharecli session list` (NOT YET; v1.1)
|   +-- `sharecli log` (NOT YET; v1.1)
|
+-- What if something breaks?
|   +-- `sharecli status` first (always)
|   +-- see failure mode table in §08.3
|   +-- `sharecli doctor` (NOT YET; v1.1)
|   +-- `sharecli ps --diagnose` (NOT YET; v1.1)
|
+-- What if I want to file an issue?
|   +-- `gh issue create --repo KooshaPari/ShareCLI` (GitHub)
|   +-- the operator manual at docs/manual.md (proposed; v1.1)
|   +-- the SFD `read-from-the-cockpit` instructions (this document)
|
+-- What if I want to know sharecli's status (as an outsider)?
    +-- Tracera product card (NOT YET)
    +-- AgilePlus backlog (NOT YET)
    +-- this SFD (in PhenoRegistry)
```

## 9.4 Operator experience gaps (today)

| Gap | Surface | Why it's a gap | Proposed path |
|---|---|---|---|
| `sharecli doctor` | CLI | no one-stop diagnostic | FR-005 session 2 (proposed) |
| `sharecli log` | CLI | no per-agent log view | FR-005 session 3 (proposed) |
| `sharecli mesh list` | CLI | no cross-agent view | FR-005 session 4 (proposed) |
| Pre-built binary | distribution | no Homebrew/apt | homebrew-sharecli tap; operator-side |
| Operator manual | docs | only README and FR.md | `docs/manual.md` (v1.1) |
| Tracera card | web | not present | operator adds |
| AgilePlus backlog | web | not present | seed from `WORK_DAG.md` |
| Recorded beneficiary session | evidence | none yet | operator runs one |

## 9.5 Operator experience quality (today)

| Question | Yes / No | Evidence |
|---|---|---|
| Can a new operator install sharecli from source in <10 min? | yes | `Hermetic Build (Hard Gate)` PASS; README step-by-step |
| Can a new operator install with prebuilt binary? | no | not published yet |
| Can an operator recover from a broken tray? | yes | `sharecli status` works headless |
| Can an operator recover from a broken FUSE? | yes | CLI keeps running; FUSE optional |
| Can an operator recover from a stale UDS? | yes | `rm /tmp/sharecli.sock` |
| Can an operator get help from the project? | partial | GitHub issues; no operator manual |
| Can an operator get help in real time? | no | no chat channel, no SRE on call |
| Can an operator get a session replay? | no | `sharecli session show` is v1.1 |
| Can an operator trust the next release? | partial | `cargo bench (gate)` FAIL; perf regression suspected |
| Can an operator trust the security posture? | partial | codeql + trufflehog PASS; cargo-deny FAIL |

## 9.6 What "experience" is *not* in sharecli

- No web UI beyond the dashboard (12 widgets)
- No notifications beyond the tray (no email, no Slack, no webhook)
- No analytics beyond resource_watch (no funnel, no per-agent success rate)
- No SLO/SLI dashboard (no Prometheus exporter, no OpenTelemetry)
- No mobile app (ADR-0001: not in product)
- No real-time chat with the maintainer (no support channel)

These are deliberate omissions per `PRD.md` §Out of product scope and ADRs.
