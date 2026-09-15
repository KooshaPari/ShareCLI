# sharecli

<p align="center">
  <a href="assets/icons/sharecli-512x512.png"><img src="assets/icons/sharecli-512x512.png" alt="sharecli" width="160" height="160"></a>
</p>
<p align="center"><em>OS-adjacent agent runtime — detect, watch, coalesce/debounce/queue, FUSE, mesh, thermal.</em></p>
<p align="center">
  <a href="https://github.com/KooshaPari/sharecli/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/KooshaPari/sharecli/ci.yml?branch=main&label=CI" alt="CI"></a>
  <a href="https://crates.io/crates/sharecli"><img src="https://img.shields.io/crates/v/sharecli.svg" alt="crates.io"></a>
  <a href="https://github.com/KooshaPari/sharecli/releases"><img src="https://img.shields.io/github/v/release/KooshaPari/sharecli?label=release" alt="GitHub release"></a>
</p>
<p align="center"><sub>Backbone-2 graphite palette · brand SVG ships in `sharecli-iconset` worktree (Backbone-2 source-of-truth) · <a href="docs/assets/identity/">visual identity demo</a></sub></p>

[![AI slop inside](https://sladge.net/badge.svg)](https://sladge.net) [![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/KooshaPari/sharecli/total)](https://github.com/KooshaPari/sharecli/releases)

---

`sharecli` is an **OS/kernel-adjacent agent runtime** for hosts running many
coding agents at once. It does **not** wrap vendor agent binaries (for example
Claude Code). It discovers agents by scanning processes and matching known
patterns, watches CPU / memory / network / FDs and syscall-relevant IO, and
under contention **speculatively coalesces** redundant concurrent work across
agents (read coalesce, lock-wait cache, thermal gate). An **agent mesh**
coordinates that shared substrate.

Core crates:

| Crate | Role |
| ----- | ---- |
| `sharecli-core` | Hypervisor loop, thermal gate, coalesce orchestration |
| `sharecli-fuse` | FUSE IO intercept for shared cwd / build-cache reads |
| `sharecli-ipc` | CoalesceCache and IPC debounce/queue |
| `sharecli-fleet` | Fleet registry + thermal governor |

Harbor / agent-eval soft harness lives **outside** this repo: suite-facing
stubs in [`phenotype-tooling/crates/benchora`](https://github.com/KooshaPari/phenotype-tooling)
(`harbor-soft/`); Harbor fork/env in
[`portage-temp`](https://github.com/KooshaPari/portage-temp) (standing home
while `portage` awaits GH restore). See [ADR 0002](docs/adr/0002-eval-surface-out-of-scope.md).

The CLI also ships a declarative supervisor surface (TOML config, hot-reload,
HTTP health/metrics, tray/dashboard, plugins) for process lifecycle on top of
that hypervisor stack.

## Installation

Install `sharecli` with one of the following methods:

```bash
# macOS / Linux (curl + sh)
curl -fsSL https://raw.githubusercontent.com/KooshaPari/sharecli/main/install.sh | sh
```

```bash
# Install from source via crates.io
cargo install sharecli

# Install a prebuilt binary via cargo-binstall
cargo binstall sharecli
```

Homebrew formula lives at `Formula/sharecli.rb` (version tracks `Cargo.toml` /
latest tag `v0.3.0`). Bottle `sha256` is filled for `v0.3.0` darwin — see
[`docs/ops/brew-bottle.md`](docs/ops/brew-bottle.md) and [`docs/deploy.md`](docs/deploy.md).

```bash
brew install --HEAD Formula/sharecli.rb
# or from tap when published:
brew install sharecli
```

Remove local config/state after uninstalling the binary:

```bash
sharecli uninstall              # package-manager guidance + path listing
sharecli uninstall --purge-data # delete ~/.config/sharecli and state dirs
```

Container (non-root + `/healthz` healthcheck):

```bash
podman build -f Containerfile -t sharecli .
podman run --rm -p 9000:9000 sharecli
```

### Per-host install (parity floor)

Same capability set on every host: CLI + `serve`, process verbs, tray/menubar (or WSL bridge), dashboard at `http://127.0.0.1:9000/` (macOS also native window). Details: [`docs/deploy/FINALITY.md`](docs/deploy/FINALITY.md).

#### macOS (optimal: Swift tray + `DashboardView`)

```bash
cargo binstall sharecli
# or: gh release download <tag> -p 'sharecli-*-apple-darwin.tar.gz'
# tray/desktop: sharecli-desktop-macos-*.tar.gz from the same release
```

#### Windows (optimal: WinUI tray + web cockpit)

```powershell
# CLI zip from GitHub Releases: sharecli-*-pc-windows-msvc.zip
# Tray: sharecli-tray-windows-*.zip (beta; unsigned)
# Dashboard: start `sharecli serve` then http://127.0.0.1:9000/
```

#### Linux (optimal: StatusNotifier tray + web cockpit)

```bash
# gh release download <tag> -p 'sharecli-*-unknown-linux-gnu.tar.gz'
# tray: sharecli-tray-linux-*.tar.gz
cargo binstall sharecli   # CLI alternative
```

#### WSL (CLI in WSL + Windows/WSLg tray bridge)

```bash
# Inside WSL (same Linux CLI artifact):
sharecli serve --bind 0.0.0.0:9000
# From Windows: http://127.0.0.1:9000/  or ShareCLITray → that port
# Optional: WSLg + sharecli-tray-linux
just wsl-parity-check
```

Deploy surface matrix: [`docs/deploy.md`](docs/deploy.md).  
Finality + OS parity: [`docs/deploy/FINALITY.md`](docs/deploy/FINALITY.md).

## Uninstall

Remove the binary for your install method, then optionally delete local data:

```bash
# crates.io / cargo-binstall
cargo uninstall sharecli

# Homebrew (once tapped)
brew uninstall sharecli

# Manual / from-source install
rm -f "$(command -v sharecli)"
# or: rm -f /usr/local/bin/sharecli ~/.local/bin/sharecli ~/.cargo/bin/sharecli
```

Optional data cleanup (config, pane map, runtime lock):

| Path | Contents |
| ---- | -------- |
| `~/.config/sharecli/` (Linux/macOS XDG) | `config.toml`, `pane-map.toml` |
| `%APPDATA%\sharecli\` (Windows) | same |
| `$XDG_RUNTIME_DIR/sharecli*` or temp dir | serve lock files |

```bash
rm -rf ~/.config/sharecli
# Windows PowerShell: Remove-Item -Recurse -Force "$env:APPDATA\sharecli"
```

## Features

- **Agent detect / watch / coalesce** — proc-pattern discovery (no vendor-bin
  wrap), resource watch, thermal gate, CoalesceCache + optional FUSE IO share
  (`sharecli-core` / `fuse` / `ipc` / `fleet`).
- **Config hot-reload** — uses `notify` to watch the config file and apply
  changes to the running supervisor in place (no restart required).
- **Health-check scheduler** — runs periodic HTTP/TCP/exec probes against
  each managed process and tracks pass/fail history.
- **Schema validation** — every config is validated against a strict JSON
  Schema before reload, so a typo never crashes the running supervisor.
- **Desktop + webhook notifications** — surface state transitions
  (`started`, `crashed`, `unhealthy`, `recovered`) to the OS notification
  daemon and to arbitrary HTTP webhooks with HMAC-SHA256 signatures.
- **Shell completions** — generates `bash`, `zsh`, `fish`, and `powershell`
  completions from the live CLI definition.
- **`proc-compose` integration** — discovers and supervises the services
  declared in a `proc-compose.toml` alongside the main config.
- **Prometheus metrics** — exposes counters, gauges, and histograms for
  process state, restarts, health checks, and request latency at
  `/metrics/prometheus`.
- **Plugin registry** — discover, install, and run executable plugins
  that extend `sharecli` with new subcommands (see
  `sharecli plugin`).

## Install (details)

### From source (recommended)

```bash
cargo install sharecli
```

This installs the `sharecli` binary into `~/.cargo/bin`.

### From a pre-built release

Download a release archive from the
[releases page](https://github.com/KooshaPari/sharecli/releases) and
extract the binary somewhere on your `PATH`:

```bash
tar -xzf sharecli-<target>.tar.gz
sudo install -m 0755 sharecli /usr/local/bin/sharecli
```

> Note: tags `v0.1.0`–`v0.3.0` currently have no attached assets. The
> release workflow matrix builds `sharecli-x86_64-unknown-linux-gnu.tar.gz`
> and `sharecli-aarch64-apple-darwin.tar.gz` for future publishes.

### Build from a git checkout

```bash
git clone https://github.com/KooshaPari/sharecli.git
cd sharecli
cargo build --release
./target/release/sharecli --version
```

## Quick Start

1. Drop a config file at `./sharecli.toml`:

   ```toml
   [server]
   bind = "127.0.0.1:9090"

   [[process]]
   name = "echo"
   command = ["sh", "-c", "while true; do echo tick; sleep 5; done"]

   [process.healthcheck]
   kind = "tcp"
   port = 0
   interval = "10s"
   ```

2. Start the supervisor:

   ```bash
   sharecli serve
   ```

3. Inspect managed processes from the CLI:

   ```bash
   sharecli proc-compose status
   ```

4. Generate shell completions (example for `zsh`):

   ```bash
   sharecli completions zsh > "${fpath[1]}/_sharecli"
   # restart your shell, or: autoload -U compinit && compinit
   ```

Other shells: `sharecli completions bash`, `sharecli completions fish`,
`sharecli completions powershell`.

## Configuration

The full config schema is documented at
`docs/configuration.md`; the minimal shape is:

```toml
# sharecli.toml

[server]
bind          = "127.0.0.1:9090"
log_level     = "info"        # trace | debug | info | warn | error
config_path   = "./sharecli.toml"

[notifications]
desktop       = true
webhook_url   = "https://example.com/sharecli-hook"
webhook_secret = "env:SHARECLI_WEBHOOK_SECRET"  # HMAC-SHA256 signing key

[healthcheck]
default_interval = "10s"
default_timeout   = "2s"

[[process]]
name     = "api"
command  = ["./bin/api", "--port", "8080"]
cwd      = "./"
env      = { RUST_LOG = "info" }
restart  = "on-failure"        # no | always | on-failure
backoff  = { initial = "1s", max = "30s", multiplier = 2.0 }

[process.healthcheck]
kind     = "http"
url      = "http://127.0.0.1:8080/healthz"
interval = "5s"
timeout  = "1s"
```

Secrets prefixed with `env:` are resolved from the process environment at
start time and never written to disk.

## API

`sharecli serve` exposes the following endpoints on the configured bind
address (default `127.0.0.1:9090`). All endpoints respond with JSON unless
noted; non-`200` responses include a `{"error": "..."}` body.

| Method | Path                   | Description                                                    |
| ------ | ---------------------- | -------------------------------------------------------------- |
| GET    | `/health`              | Liveness probe for the supervisor itself. Always `200` if up.  |
| GET    | `/health/processes`    | Per-process status (state, PID, uptime, last health check).   |
| GET    | `/config`              | Effective config (secrets redacted) currently in effect.       |
| GET    | `/metrics/prometheus`  | Prometheus exposition format (text/plain; `version=0.0.4`).   |

A request that targets an unknown process can append
`?name=<process-name>` to `/health/processes`.

## License

Dual-licensed under MIT or Apache 2.0, at your option.
See [LICENSE](LICENSE) and [LICENSE-APACHE](LICENSE-APACHE).
""  
""  
"## ci-gate-refresh $(date -u +%FT%TZ)"  
