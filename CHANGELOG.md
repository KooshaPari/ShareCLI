# Changelog

All notable changes to **sharecli** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **sharecli** — Shared CLI process manager for multi-project agent orchestration.
> Repository: <https://github.com/KooshaPari/sharecli>

---

## [Unreleased]

### Added
- **Wave 17 / Plan 804 (T-935): Cross-platform CI** — `C07 L69` lift `2 → 3`
  (#812, `701c67e`).
- **Personal evaluation guide** — end-to-end evaluator walkthrough (#811, `7f75861`).
- **README badges** — *AI slop inside* + downloads badges (#810, `59959e7`).

### Fixed
- **Thermal module** — resolve panic-on-runtime nesting and switch to dynamic
  version reporting (`d4a58b6`).
- **SonarCloud** — resolve 10 open vulnerabilities on `main` (#805, `b6a3045`).
- **CI** — resolve Build and SonarCloud failures on `main` (#798, `fefd0f1`).

### Documentation
- **Governance SCORECARD** — Plan 806 reconciliation post-#812 Plan 804 lock
  (#813, `c377fc0`).

---

## [1.0.0] - 2026-08-31

The first stable major release. Lifts the scorecard to **A+ (99.1 %)** and ships
**Wave 17 / Wave 18 / Wave 19 / Wave 20** work as top-level entries.

Release commit: `a290e3a` — *chore(release): bump version to 1.0.0*
(also bumps intermediate `v0.4.0` and ships the *Wave 20 spec — v0.5.0 roadmap*).

### Wave 20 — Roadmap to v0.5.0 (top entry)

- **Wave 20 spec — v0.5.0 roadmap** (`docs/ops/governance/WAVE20_SPEC.md`,
  `a290e3a`): foundation document for the next minor release, base-locked on
  `v0.4.0` (`PR #810`).
- **Fuzz CI resilience** — `continue-on-error: true` on `cargo fuzz` matrix
  to absorb Zig mirror transient failures (`.github/workflows/fuzz.yml`,
  `a290e3a`).
- **wave-20 parser series**:
  - `feat(tar_header)` — POSIX ustar header parser (#168, `aa9d423`).
  - `feat(elf_section)` — ELF section header walker (#167, `f3bf2da`).

### Wave 19 — Parser series (top entry)

- `feat(toml_lite)` — minimal TOML parser (#166, `2b66c76`).
- `feat(wasm_opcode)` — WebAssembly MVP opcode decode (#165, `283519f`).

### Wave 18 — Gap remediation & accessibility (top entry)

- **Wave 18 final lock** (#796, `cd50688`) and **Wave 18 gap remediation lock**
  (#791, `ce4e340`).
- **`feat(wave18): gap remediation`** (#787, `17e02d0`) — closes post-Wave 17
  audit gaps across FR-003.
- **`fix/version-accessibility-contract-clean-20260816`** (#795, `4ee0f9b`) —
  version/UI accessibility contract cleanup, landed as part of the Wave 18
  backlog sweep.

### Wave 17 — Governance series (selected plans)

- **Plan 806 / 805 (T-940)** — `C09 L81.9` Undo/restore model `2 → 3`
  (`e9765ae`).
- **Plan 805 (T-940)** — `C09 L81.9` Undo/restore model `2 → 3`
  (`e9765ae`).
- **Plan 804 (T-935)** — `C07 L69` Cross-platform CI `2 → 3` (#812, `701c67e`).
- **Plan 803 (T-930)** — `C07 L67` Fuzz harness `2 → 3` (#804, `8cab7cd`).
- **Plan 802 (T-925)** — `C02 L22` Crypto & key management FR-003 gates `2 → 3`
  (#802, `c1720fd`).
- **Plan 801 (T-920)** — `C02 L24` Privacy & tenancy FR-003 gates `2 → 3`
  (#800, `f9cbe52`).
- **Plan 800 (T-915)** — `C00 L5` Observability FR-003 gates `2 → 3`
  (#797, `6dee96f`).
- **Plan 796 (T-910)** — `C09 L81.12` history + `L81.15` CTA tokens `2 → 3`
  (#789, `4766abf`).
- **Plan 795 (T-900)** — `C07 L68` flake-tracker dashboard source `2 → 3`
  (#786, `1c6756e`).
- **Plan 794 (T-890)** — `C02 L26` overflow fix + FR-003 acceptance gates
  `2 → 3` (#784, `c509771`).
- **Plan 783 (T-880)** — `C11 L111` soft auto-update probe `1 → 2`
  (#782, `76e8f21`).
- **Plan 782 (T-870)** — `C05 L49` Grafana provisioning as code `2 → 3`
  (#781, `5ae9ec2`).
- **Plan 776 (T-860)** — `C04 L34` `2 → 3` verified merges (#780, `8f1990d`).
- **Plan 778b (T-850)** — `C06` SLSA generator re-pin `@v2` → commit SHA
  (#777, `02c805a`).
- **Plan 777 (T-840)** — `C06` SLSA Build `L2 → L3` generator (#776, `5a32630`).
- **T-810 (Plan 775)** — `C01` `--lib` coverage pin + Windows compat cfg gates
  (#775, `691bde6`).

### Release engineering

- **Version bump `0.3.0 → 1.0.0`** with intermediate `v0.4.0` release artifact
  (`a290e3a`, `Formula/sharecli.rb`).
- **Dependency refresh** — `substrate` and `runtime-process` bumped to
  `3e93ff5` (#808, #809, `be042e2`, `2676dab`).
- **`uuid` 1.24.1 → 1.26.0** in the `cargo-minor` group (#807, `4c8450b`).
- **`winfsp`** bump in `cargo-patch` group (#767, `c3fbdb5`).
- **GitHub Actions** — bump 11 actions in one PR (#793, `d11ea61`).

### Platform & tray

- **Ghostty terminal control plane** — surgical re-land of #647
  (#703, `7b94265`).
- **Sparkle delta updates** — channel picker wired into dashboard toolbar,
  `UpdateChannel` enum + `ChannelPicker`, channel-aware `appcast` generation
  (`847372a`, `e252aab`, `2b4c993`, `62a29eb`, `26eadb1`).
- **Sparkle 2.9.5 pinned** in `Package.resolved`; `Sparkle.framework`
  bundled in `install-tray-macos.sh` (`e6319ce`, `83a34df`).
- **Sparkle auto-update** (Q6 / P4-16) shipped (#9d3486d).
- **macOS tray notarization** — `notarize-tray-macos.sh` (P4-17, `dfdf8c3`).
- **`UpdateChannel`** wired through `UpdaterView` and shared `HealthPill` /
  `relativeTimestampFormatter` into `ResourcesView` (`522bf06`, `c5133be`,
  `beccabd`).
- **Tray UX fixes** — right-click menu, anchor below menubar, Pause/Resume
  + Quit (`2d6645d`).
- **ProcessesTreeCanvasView** — Canvas DAG renderer (P2-9, `11532a9`).
- **Dashboard surfaces** — `DashboardOverview` (`q-dash-2`), 8th sidebar page
  + `Cmd+1..8` navigation (`40b8d02`, `6ada85b`).
- **Sparkle resources** — `ResourcesExtras` section (P2-12 / Q5, `118f77d`),
  `FlameChart` (P2-10 / Q4, `7d6b641`), FlameChart layered over TrendsView
  (P2-10 / Q3 wire, `6af3fcc`).
- **UpdaterView wired** into DashboardView toolbar (Q10, `88676e6`).
- **Dead code removed** — `TreeView` + orphaned helpers (Q11, `f1ae476`).
- **Selected-row export** — JSON / CSV from `ShareCLITray` (P2-11, `8ee58ab`).
- **SpawnView** — persistent spawn history (P1-7, `9b2f760`).
- **`AppState`** — connection-state machine + retry telemetry
  (`9598ea3`, `f317250`).
- **IPC retry** — `IPCClient` retry timeout + `ProcessesPage` empty-state
  refresh (`fc9e501`).

### FUSE / sharecli-fuse

- **Cross-platform mount** — Windows WinFsp FUSE backend, NTFS ADS
  provenance, xattr Unix-only (#646, `219ad6f`).
- **Test depth** — 12 new FUSE tests covering backend selection + live
  mount lifecycle (#410–412 era, `b6b3739`, `a79e3be`).
- **In-memory index backend** — refactor `sharecli-fuse` (`2704a67`).
- **Per-crate `cargo-mutants` config** (T-86, `8f4cbd0`); matrix-extend to
  `sharecli-fuse` (`dcd3f99`); mutants output `gitignore`d.
- **Smoke fuser backend re-export restored** (`669c00e`).
- **Read-cache test flake fix** — serialize `read_cache` tests to stop global
  meter flake (`bf0bac3`).
- **`smoke_fuser_config_for_backend`** re-export restored (#728, `669c00e`).
- **Session-registry fixture key normalization** for `no-mount` tests
  (macOS parity, #740, `742`).
- **Windows build compile fix** — `xattr` Unix-only, NTFS ADS provenance,
  WinFsp API (#646, `219ad6f`).
- **Rebase-marker scrub** in `lib.rs` (Q1, Q2; `54277b0`, `8201c44`,
  `98f56e7`).
- **macOS-safe mount config + Linux `AutoUnmount` ACL** (#561, `1315244`).
- **Negative-dentry cache** for `ENOENT` lookups (FR-009 AC-009.7,
  `c1a8560`).
- **Opt-in privileged mount smoke** (FR-009 AC-009.7, #404, `145c153`).
- **Per-agent CoW FUSE parity** + Feb oracle vault (#551, `5ce13c0`,
  `342b398`).
- **Write-provenance xattrs** stamped on `write_rel` / `commit_rel`
  (`3792e4a`).
- **FUSE mount/backing path remap + spawn lifecycle** (AC-009.14,
  `ae669e4`).
- **FUSE session wired into `SpawnOutcome`** (AC-009.13, `8c7528c`).
- **Hypervisor coalesce session wired into FUSE mount** (AC-009.12,
  `fbd310d`).
- **Write-serialize CoW meters** in status/TUI (AC-009.10, `eda629e`).
- **Provenance inspect CLI** for backing write xattrs (AC-009.11,
  `380d048`).
- **Provenance xattrs verified in mount smoke** (AC-009.6 × AC-009.8,
  `a98418a`).

### Hypervisor / FR-008

- **Speculative-execution Hypervisor** for FR-008 (`19e5eca`).
- **`Hypervisor::run` wired** with FR-007 FD/net watch (AC-007.6,
  `d361eec`).
- **FR-008 debounce on every coalesce miss path** (AC-008.6, `1b77230`).
- **Harness queue strategies** via `Hypervisor::run_queued`
  (AC-008.16, #479, `a3cf015`).
- **Harness coalesce / cache** via `Hypervisor::run` (AC-008.17, `fecb30f`).
- **Harness debounce via `Hypervisor::run`** (AC-008.18, `11e48bc`).
- **`SlotQueue` Critical-before-Normal** on Hypervisor nocache lane
  (AC-008.14, `a8e0d4b`).
- **Queue priority** into `SpawnRequest` (AC-008.15, `1a86d0e`).
- **Hypervisor `CoalesceCache` TTL eviction + debounce share window**
  (`6851658`).
- **Hypervisor coalesce meters** in status/TUI (AC-008.11, `1c8731a`).
- **`SlotQueue` meters** in status/TUI (AC-008.12, #428, `b491f06`).
- **Agent-aware thermal gate** for spawn back-pressure (AC-011.4,
  `8589675`).
- **`Hypervisor` nocache e2e re-exec + queue serialize**
  (FR-008 AC-008.10, `2c20247`).
- **Hypervisor primitives** — coalesce, debounce, queue, cache, fuse_io
  (L121, #187, `2e858d9`).

### Fleet & IPC

- **Fleet diff view + spawn history** wiring (P1-8 + P1-7 close,
  `7e15d1c`).
- **Feb harness recovery A+ path** (FUSE/IPC/mesh, #397, `b71460f`).
- **Live process-tree agent scan** — `/proc` + ancestors (#387, `dd41b9c`).
- **`CompositeHealth` + composite metric (0–100, 4 bands)** — `T95`,
  wired into 3 surfaces (#634 follow-on, `54e8ded`, `2b13b29`).
- **Health pill + relative timestamp** extracted to shared file and wired
  into `ResourcesView` (`c5133be`, `beccabd`).
- **Health-pill + relative-timestamp shared module** (refactor,
  `beccabd`).
- **`MiniCompositeHealthCard`** compact tile + sidebar footer wiring
  (`522bf06`, `b58e1f5`).
- **`UpdateChannel`** enum + `ChannelPicker` (`62a29eb`).
- **`fleetHistory` ring buffer on `AppState`** (`b698f85`).
- **`ProcessSummary`** extended with `ppid/cwd/env/state/disk/fd`
  metrics (`6512538`).
- **`ProcessInfo` + `MonitoringProcessEntry`** extended similarly
  (`a514f23`).
- **`FleetSample`** for fleet-history ring buffer (`b698f85`).
- **Per-agent PID resource watch** for thermal TUI + `ps --all`
  (`2793fb6`).
- **FR-007 CPU/MEM resource watch** via RSS + load avg (AC-007.7/8,
  `30a4508`).
- **FR-007 FD and net resource watch** (AC-007.4/5, `b8a409d`).
- **`IPCClient` WS client module** to `sharecli-ipc` (#57, `8671766`).
- **Coalesce/debounce/queue scaffold** — `sharecli-ipc` (#31, `8d924b7`).
- **FUSE IO-interception scaffold** — `sharecli-fuse` (fuser,
  cross-platform, #30, `ab700c5`).

### Health / FR-007

- **Health/pool/status `--csv` with operator companion rows** (AC-007.82,
  #544, `9155a80`).
- **`ps --all --csv` with operator companion rows** (AC-007.83, #545,
  `0999178`).
- **Operator envelope matrix parity suite** (AC-007.84, #546,
  `82c596a`).
- **`proc --tree` operator envelope** locked in parity (AC-007.85,
  #547, `8e9e368`).
- **`proc --pid` operator envelope** locked in parity (AC-007.86,
  #548, `ec47769`).
- **`text --watch` same-tick flush matrix** (AC-007.97, #567,
  `1eedd05`).
- **`text --watch` footer same tick** (AC-007.96, #566, `82e03ca`).
- **`CSV --watch` same-tick flush matrix** (AC-007.95, #565, `ed83792`).
- **`CSV --watch` footer same tick** (AC-007.94, #564, `37ce6c4`).
- **`--watch` frame-marker parity smoke** (AC-007.93, #563, `36a346d`).
- **`--pid` inventory combo loud-reject** (AC-007.92, #562,
  `413441c`).
- **`proc --pid --csv --watch`** (AC-007.91, #558, `35d5d35`).
- **`report --format csv --watch`** (AC-007.90, #557, `fd92809`).
- **`operator sibling --csv --watch`** (AC-007.89, #556, `cad1410`).
- **`proc --csv --watch` text/NDJSON** (AC-007.88, #553, `092e960`).
- **`proc --pid --watch` text/NDJSON** (AC-007.87, #552, `1eec75e`,
  `6320389`).
- **`proc --pid --watch`** (AC-007.87, `dfe7de7`).
- **Pool/status CSV companions** on `proc --csv` (AC-007.79, #541,
  `9167b71`).
- **`IPC health/pool/status` embed pool+status siblings** (AC-007.78,
  #540, `a8a0b57`).
- **Pool/status siblings** in operator JSON paths (AC-007.77, #539,
  `5aa7661`).
- **`health/pool/status/ps --all` text pool/proc-scan operator lines**
  (AC-007.76, #538, `535d228`).
- **`proc` text paths emit pool/proc-scan operator lines** (AC-007.75,
  #537, `dc09fd2`).
- **`report` text pool/proc-scan operator lines** (AC-007.74, #536,
  `92f8472`).
- **Pool/status embedded in `report --format json`** (AC-007.73, #535,
  `8e4b5fc`).
- **Pool/status embedded in `monitoring.report` IPC** (AC-007.72,
  #534, `f84c4b2`).
- **Thermal-TUI pool/status operator panels** (AC-007.71, #533,
  `35a3fca`).
- **`serve` dashboard WS pool/status operator envelope** (AC-007.70,
  #532, `2ee2fe5`).
- **Tray consumes pool/status IPC in operator panels** (AC-007.69,
  #531, `58a8aa6`).
- **Tray pool/status IPC wire parity** for tray clients (AC-007.68,
  #530, `c05acab`).
- **`IPC pool.status + status.snapshot` gate/host_watch** (AC-007.67,
  #529, `03dc173`).
- **`status --watch` gate/host_watch parity** (AC-007.66, #528,
  `4f8a68f`).
- **`pool --watch` gate/host_watch parity** (AC-007.65, #527,
  `08cb3c8`).
- **`health --watch` gate/host_watch parity** (AC-007.64, #526,
  `0366fb6`).
- **`status --json` gate + host_watch parity** (AC-007.25, #483,
  `009f5c7`).
- **ResourceWatchSample host parity** — `proc text/CSV` (AC-007.14, #465,
  `cb2f121`); `proc JSON` (AC-007.13, #464, `0467172`); `proc tree JSON`
  (AC-007.15, #466, `193246f`).
- **Thermal TUI host watch net RX/TX parity** (AC-007.12, #463,
  `85c22f2`).
- **`proc` NDJSON watch includes state** (AC-006.37, #459, `92b4ff3`).
- **`proc` watch lines flushed + piped watch timing stabilized**
  (AC-006.37, #460, `e0e693d`).
- **`--exclude-family` filter** (FR-006 AC-006.38, #458, `3f0b5d7`).
- **`--sort state`** orders inventory by process state (AC-006.36,
  #457, `99c38ce`).
- **Tree JSON state parity** (AC-007.18, #469, `33accdd`).
- **`proc --pid` gate JSON + text section** (AC-007.17, #468, `5f3f2d2`).
- **`proc --pid` host_watch JSON + text footer** (AC-007.16, #467,
  `bf4f3e5`).
- **CSV gate companion for flat/tree `proc`** (AC-007.19, #470,
  `6e86a2e`).
- **Tree text gate section before host watch** (AC-007.20, #471,
  `af28840`).
- **Flat text + watch gate ordering** (AC-007.21/22, #472, `51cf2e0`).
- **One-shot JSON gate key ordering** (AC-007.24, #474, `37d77e0`).
- **Tree watch gate ordering per refresh** (AC-007.23, #473,
  `ffc4f36`).
- **`ps --all` gate → host_watch stdout parity** (AC-007.38, #496,
  `2544ac1`).
- **`health/pool` gate → host_watch stdout parity** (AC-007.37, #495,
  `ad7506c`).
- **`report` text gate → host_watch stdout parity** (AC-007.39,
  `81cb4f0`).
- **`report --format json` gate + host_watch siblings** (AC-007.40,
  #498, `7122255`).
- **Dashboard WS operator envelope** (AC-007.41, `5f1e8da`).
- **`status` text stderr silence** (AC-007.36, #494, `e9347d2`).
- **`proc` watch text stderr silence** (AC-007.35, #493, `e5e9155`).
- **`proc` one-shot text stderr silence** (AC-007.34, #492,
  `0672cc4`).
- **`proc --csv` stderr silence** (AC-007.33, #491, `1428d59`).
- **`status --json` stderr silence** (AC-007.32, #490, `88c047f`).
- **`proc --pid --json` stderr silence** (AC-007.31, #489,
  `8294b6c`).
- **One-shot JSON stderr silence** (AC-007.30, #488, `501edf8`).
- **Tree watch NDJSON stderr companions** (AC-007.29, #487,
  `95e1e91`).
- **`proc` watch NDJSON stderr gate→host_watch companions**
  (AC-007.28, #486, `2fa1450`).
- **`status` text gate before host watch ordering** (AC-007.27,
  #485, `796da66`).
- **Thermal-TUI gate RSS-aware snapshot parity** (AC-007.26, #484,
  `e0da863`).
- **FUSE read-coalesce global meters** surfaced in status (AC-007.9,
  #412, `8418c9d`).
- **Host resource watch sample** surfaced in status (AC-007.10,
  #413, `9fcdc80`).
- **Thermal-TUI host watch + FUSE coalesce panels** (AC-007.11,
  #414, `e4aa2b0`).
- **Thermal-TUI `DetectedAgent` panel** from proc scan
  (FR-006 AC-006.9, #417, `8b220b6`).
- **FUSE neg-dentry meters** in green headless render
  (`6943240`).
- **AGENT column in `ps`** via `proc_scan` (#415, `35755fb`).
- **Maildir queue depth** in status and thermal TUI (AC-010.11,
  #429, `ae86a9e`).
- **Mesh `Maildir` status/reclaim CLI** (FR-010 AC-010.9..10, #403,
  `02dff49`).
- **Thermal TUI fits FUSE neg-dentry meters** in green headless
  render (`6943240`).
- **DetectedAgent wired** into thermal TUI + hypervisor spawn context
  (`b2fcf7e`).
- **AC-009.13 spawn-outcome wiring** (`8c7528c`).

### Proc / FR-006

- **`--sort cpu/age/name` + `--limit` cap** verified — FR-006 AC-006.41
  (`67d77e1`).
- **`sharecli proc` CLI** — cmdline fingerprints + RSS-aware gate
  (FR-006 AC-006.11–006.14, #434, `f605c2b`).
- **`proc --watch` live refresh** (FR-006 AC-006.15, #435, `8742d94`).
- **`proc --tree` parent-child agent forests** (FR-006 AC-006.16, #436,
  `86709a7`).
- **`--family` and `--min-rss` filters** (AC-006.17, #437, `2c5563a`).
- **NDJSON stream for `--watch --json`** (AC-006.18, #438, `1f89db4`).
- **`--sort rss|fd|pid` for agent inventory** (AC-006.19, #439,
  `3881053`).
- **`amp` family + expanded cmdline fingerprints** (AC-006.20, #440,
  `5250ccf`).
- **Bare `comm` basename hits** for ambiguous agents (AC-006.1, #441,
  `226b3c9`).
- **`sharecli proc --limit N`** (AC-006.21, #442, `8988902`).
- **Thermal TUI agent tree from `build_agent_forests`** (AC-006.22,
  `ef7fbf2`).
- **`sharecli proc --pid N` detail view** (AC-006.23, #444,
  `98aa481`).
- **`sharecli proc --csv` flat inventory export** (AC-006.24, #445,
  `33a1181`).
- **`sharecli proc --ppid N` parent filter** (AC-006.25, #446,
  `b877e56`).
- **`sharecli proc --tree --csv` forest export** (AC-006.26, #447,
  `dbb8f95`).
- **`sharecli proc --max-rss` upper RSS bound filter** (AC-006.27,
  #448, `d176e7b`).
- **`--min-fd` / `--max-fd` inventory band filters** (AC-006.28,
  #449, `252785e`).
- **`--comm COMM` substring filter** (AC-006.29, #450, `f3ce90d`).
- **`--cmdline` pattern filter** (AC-006.30, #451, `fd7aa36`).
- **`--state` process-state filter** (AC-006.31, #452, `7f8348c`).
- **State exported on JSON/CSV rows** (AC-006.32, #453, `98357b4`).
- **State exposed on text inventory + pid detail** (AC-006.33, #454,
  `d02f3b4`).
- **State on `--tree` text and JSON nodes** (AC-006.34, #455,
  `395135c`).
- **Live state resolved for all forest PIDs on `--tree`** (AC-006.35,
  #456, `d5f0b8c`).
- **Thermal TUI agent-tree process state letters** (AC-006.39, #461,
  `3e8136c`).
- **Thermal TUI flat agent-lines process state letters** (AC-006.40,
  #462, `46b6f3e`).
- **`proc --sort` help text updated** to mention `cpu/age/name` keys
  (`d8b0d41`).
- **`proc` resolve unused import warning + test compilation**
  (`40e9494`).

### Serve / dashboard

- **`KeepAlive` LaunchAgent** runs `sharecli serve` (deploy, #389, #675,
  `2f03540`, `2ccbb98`).
- **Dashboard skeleton loading states** (C10 L99, FR-003, #396, `711b3a9`).
- **Dashboard WS operator envelope** (AC-007.40 + 41, `5f1e8da`).
- **Dashboard WS operator envelope spec** kept (`ebc82e3`).
- **Dashboard tier-1 disconnect error illustration** (L101, FR-003,
  #581, `0184745`).
- **Dashboard `tokens.css` lock** — hex → tokens (FR-003, #571,
  `7c1c8fb`).
- **Dashboard PNG baseline bytes + soft diff** (FR-003, `6466b4e`).
- **Dashboard assets route documented** (`e5e4719`).
- **Dashboard expansion** — 7 pages, IPC additions, polish
  (`6002f96`, `2704a67`).
- **Dashboard WS extension with pool/status operator envelope**
  (AC-007.70, #532, `2ee2fe5`).
- **Static dashboard served at `GET /`** (#53, `86b3ad4`).
- **Lock-guarded HTTP+WS dashboard server subcommand** (#52,
  `dfbe223`).
- **Thermal pressure monitoring** wired into HTTP+WS server
  (#54, `3daccd5`).
- **Live TUI monitor + `sharecli thermal` subcommand** (#45,
  `a6f14bc`).
- **Hypervisor thermal-gate spawn back-pressure** (#40, `bbcb2cd`).
- **ThermalGovernor wired into `Hypervisor::run`** (#38, `df43b2f`).

### CLI / API surface

- **`text` `--watch` operator flush matrix** (AC-007.98, #568,
  `f81fb49`).
- **`cli::list`** enumerate + `version` Backbone-2 splash (#178,
  `a46ba19`).
- **`util` CLI subcommand** — `base85/crc64/csv/hash/json/sha/uuid/xml/markdown/rng`
  (#156, `494e3c9`, `98ad9a1`, `f851a69`).
- **`serve` subcommand** with axum 0.8 REST cockpit (#180, `33a5980`).
- **`sharecli completions <shell>`** (#61, `a9b540a`).
- **`sharecli report`** subcommand for fleet analytics (#55,
  `1d68bb2`); `--watch` and `--sort` (#56, `7044921`).
- **`sharecli fleet`** subcommand + cross-device deploy doc
  (#24, `830a0a8`).
- **`sharecli cast`** subcommand (T59 cast backlog, #32, `fee481c`).
- **CLI binary + 9 integration tests** (`31cf316`).
- **`--theme` flag** + Backbone-2 Rust `theme.rs` (#164, `3bb5aee`).
- **`Backbone-2` light theme** + soft hermetic offline build
  (#265, `3898e80`).

### Security / FR-004

- **OSV scan + Dependabot groups + container hardening** (FR-004, #243,
  `ab5590c`).
- **OSV ignore `rsa/paste` advisories with rationale** + `event-listener`
  bump 5.4.2 (#714, `6d47aac`).
- **Trunk-CI cosign action re-pinned** (#722, `ec353cb`).
- **Gitleaks pinned binary + repo config** for CI (#741, `b30932a`).
- **Dual secret scanners** — gitleaks + trufflehog (C04 L31 2→3, #374,
  #375, `48cf350`, `9ab9527`).
- **Soft DCO signed-commits policy** (FR-004, #260, `8cf704e`).
- **GPG Verified commits guide for L34** (#401, `9b9fca7`).
- **JWT federated AuthN for serve** (FR-012 W5.1, #229, `7a85f9d`).
- **Audit log retention + AuthN burn alerts** (W5.2, #230, `92f8cae`).
- **W5.3 post-federation threat-model review** (#231, `033f7be`).
- **`h2` 0.4.16** — `RUSTSEC-2026-0258` (#747, `175a836`).
- **`lru` 0.18.2** — `RUSTSEC-2026-0253` (#739, `dc17155`).
- **`jsonwebtoken` 9.3.1 → 11.0.0** (#613, `5e38eaa`).
- **`base64` 0.22.1 → 0.23.1** (#725, `0485b90`).
- **`rusqlite` 0.32.1 → 0.40.1** (#615, `aebbdc6`).
- **`serial_test` 3.5.0 → 4.0.1** (#614, `576557f`).
- **`uuid` 1.23.4 → 1.23.5** (#216, `262ad59`).
- **`clap_mangen` 0.2.33 → 0.3.0** (#575, `ceb8c67`).
- **`fuser` 0.17.0 → 0.18.0** (#576, `7b7ab41`).
- **`async-nats` 0.49.1 → 0.50.0** (#577, `1280b3f`).
- **`windows` 0.61.3 → 0.62.2** (#578, `013b1f1`).
- **`tokio-tungstenite` 0.29.0 → 0.30.0** (#217, `c7de855`).
- **`criterion` 0.5.1 → 0.8.2** (#215, `2d2f775`).
- **`vite` 6.4.3** — `GHSA-4w7w-66w2-5vf9`,
  `GHSA-fx2h-pf6j-xcff`, `GHSA-v6wh-96g9-6wx3` (#2d5548b, `978139c`).
- **`esbuild` 0.25.0** — `GHSA-67mh-4wv8-2f99` (`1936d9f`).
- **`postcss` 8.5.23** — `CVE-2026-69153` (`b1c3047`).
- **`playwright` 1.62.0 → 1.62.1** (#634, `5c80b96`).
- **`playwright` 1.61.1 → 1.62.0** (#612, `b82c6b5`).
- **`playwright` 1.49.0 → 1.61.1** (#559, `56eaa3a`).
- **`jsdom` 30.0.0 → 30.0.1** (#625, `227e522`).
- **`jsdom` 29.1.1 → 30.0.0** (#617, `cd6700f`).
- **`jsdom` → 29.1.1** (#249, `85a8f22`).
- **`axe-core` 4.12.1 → 4.13.0** (#723, `16e9d9a`).
- **`axe-core` 4.10.2 → 4.12.1** (#244, `42fe4c5`).
- **`pixelmatch` 6.0.0 → 7.2.0** (#395, `0117bde`).
- **`clap_complete` 4.6.5 → 4.6.7** (#171, `7f0b98b`).
- **`notify` 7.0.0 → 8.2.0** (#170, `3e1c8f4`).
- **`tokio-tungstenite` 0.26.2 → 0.29.0** (#172, `9333648`).
- **`notify-rust` 4.17.0 → 4.18.0** (#173, `4bd7d82`).
- **`tauri-winrt-notification` 0.7.2 → 0.7.3** (drop `quick-xml`)
  (`02d283a`).
- **`bytes` 1.12.0 → 1.12.1** (`6c64050`).
- **`sha2` 0.10 → 0.11** (#35, #196, `11f9cb5`, `e61c744`).
- **`config` 0.14 → 0.15** (#34, `336aa09`).
- **`sysinfo` 0.30 → 0.39** (#6, `cdfb21b`).
- **`itertools` 0.12 → 0.15** (#13, `105feb4`).
- **`toml` 0.8 → 1.1** (#2, `6714fca`).
- **`thiserror` 1 → 2** (#12, `efebda7`).
- **`async-nats` 0.38 → 0.49** (#36, `9ccc46c`).
- **`reqwest` 0.13** bump + substrate git deps refresh (`b19fbef`).

### CI / governance

- **Trunk-CI soft-gate workflow validation fixes** — round-2 (codeql
  languages scalar, job-level `hashFiles`, dup env keys, #642,
  `12ee872`).
- **Trunk-CI soft-gate workflow validation fixes** — round-1 (4 errors,
  FR-001, #640, `210c335`).
- **`cargo fmt --all`** to restore `ci/lint` gate (#705, `0565b7b`).
- **Orphaned-history PR freshness gate** (#701, `6347563`).
- **Trunk-check + `cargo-mutants` matrix** — green (#722, `ec353cb`).
- **OpenAPI drift gate** for `serve` routes (FR-004, #241,
  `9089100`).
- **Soft container cosign gate** — L56 sign-blob on main (FR-002,
  #254, `8d5c21b`).
- **Trunk.io / Renovate / Mergify / CircleCI / Scorecard workflows**
  pinned to stable gate names (`0c3147d`, `b14d283`).
- **Pre-commit config added** with trufflehog (#707, `ece86d5`).
- **Pre-commit hook for Airlock Bot** (daemon-shield, `407c31d`).
- **`gitleaks` daemon-shield** — block rebase-marker commits on
  `lib.rs` (`66cf9fd`).
- **Zig installation** for `spawn-core-sys` jobs, OSV scan-args,
  npm deps in release test (#710, `c3832f0`).
- **Node 22 for release a11y npm gate** (jsdom 30 engine floor, #711,
  #712, `4c770f2`, `7b1940d`).
- **Release tests** — align release-matrix tests + drop `dhat` from
  release test step (#713, `bb6437a`).
- **`osv-scanner` ignore `rsa/paste`** with rationale (#714, `6d47aac`).
- **Daemons** — multiple `wip: auto-commit daemon` checkpoints (Aug
  2026) merged via provenance closeouts (#761–770).
- **Trunk-CI renovate / `.mergify` / `.pre-commit-config.yaml`
  workflows added** (`fd799be`, `0d3b39b`, `0a0811e`).

### Tray / desktop

- **Sparkle appcast XML template** for stable channel with delta
  example (`2b4c993`).
- **`build-appcast.sh`** — channel-aware appcast generation
  (`e252aab`).
- **`Sparkle` delta updates release process** doc (`b0895ea`).
- **Tray channels (alpha/beta/stable)** — Sparkle release channels
  (`f263b28`, `a2c2f23`).
- **`UpdateChannel` wired** into `UpdaterView` (`26eadb1`).
- **Wire `UpdaterView` into DashboardView toolbar** (Q10, `dad1c54`,
  `88676e6`).
- **Wire `ResourcesExtrasSection` into `ResourcesView`** (Q9,
  #98e0cf1, `b429133`).
- **Wire `HealthPill` + `relativeTimestampFormatter` into
  `ResourcesView`** (`c5133be`).
- **`ProcessesPage` — grouped section + sticky header + live update**
  (`1e29804`).
- **Polish pass** — empty states + micro-interactions + `Cmd+K`
  palette + `Cmd+/` help + `Cmd+,` preferences (`4b0f99f`).
- **`EmptyStateView`** — applied to 6 empty-state pages (`5273c67`).
- **`Sidebar` restructure with `Cmd+1..7` shortcuts** (PR 9,
  `99d2470`).
- **Logs page** with live tail + filter + export (PR 8, `596418b`).
- **Config page expansion** (PR 7, `4581def`).
- **`process.cmdline` IPC + Agent Detail drill-down** (PR 5,
  `57cfd5e`).
- **Pool + Pool Effectiveness pages** (PR 3 + PR 4, `13165e5`).
- **Agents page** (PR 1, `6767538`).
- **Processes page** with `All` / `By Project` / `By Harness`
  (PR 2, `5da4264`).
- **CPU/MEM `cpu_percent` IPC + dashboard column** (PR 2b,
  `5de6ea0`).
- **`Spawn` + `Presets` subpages on Processes** (`7ce6750`).
- **`Resources` subpage on Processes** (`621a07c`).
- **`I/O` column on `Processes/All`** — disk read+write bytes
  (`74421bd`).
- **`FDs` column on `Processes/All`** — file-descriptor heat bar
  (`58a6c6e`).
- **`Trends` subpage on Processes** — fleet memory + CPU over 60s
  (`f4a5bbf`).
- **WinUI tray cross-module notification + dyld rpath + icon
  resources fix** (`265424d`).
- **`build.sh → install-tray-macos.sh`** — `--system /Applications`
  target (`a34fd4c`).
- **Linux tray** — upgrade to `ksni 0.3` blocking + kill actions
  (#21, `f8d2b68`).
- **SwiftUI tray wire-up** + `Info.plist` + `sharecli.icns` (L102,
  #161, `abb084b`).
- **`Cargo.lock` sync** for `--locked` CI compat (#605,
  `e6c9ae9`).
- **`fuser::mount2 → mount`** rename for 0.18.0 (FR-001, #606,
  `ccb1c02`).
- **WinUI tray** — `TCP IPC` bridge + C# FFI (#18, `e58404f`).
- **Tray on Linux** — `ksni` (#19, `2e93e7f`).
- **Tray + native client (Swift/macOS)** over Rust core — config +
  observability (#17, `8cd8510`).
- **Daemon tray fixes** — right-click menu, anchor below menubar,
  Pause/Resume + Quit (`2d6645d`).

### Build / packaging

- **C06 SLSA Build `L2 → L3` generator** (#776, `5a32630`).
- **`repro-check` CI + deny sources** (FR-002, #232, `5195285`).
- **MVP finality + OS parity builds** (FR-004, #247, `e5be75b`).
- **W4.2 Homebrew `sha256` from v0.3.0** (#226, `5a35296`).
- **`cargo-cyclonedx` `sharecli.json` → `sharecli.cdx.json`**
  (release, #225, `0e8e6f7`).
- **Zig 0.14.1 install** for darwin/linux artifact builds (#224,
  `86bc1e4`).
- **C06 release artifact pin** → C06 60% C (#223, `efb5cb7`).
- **OCI Containerfile** for `sharecli` (#66, `84dd809`).
- **Sidecar install script** — `launchd` (macOS) + `systemd`
  (Linux) (T28, #26, `b16c357`).
- **Release pipeline** — `cargo-dist` + tray installers (#39,
  `5b604bd`).
- **GitHub Actions release workflow** with binary artifacts (T36,
  #29, `a39e07e`).
- **Windows CI lane** (C07 L69 → score 2, #205, `2021ee5`).
- **`cargo fmt --all`** for Wave1 CI fmt gate (`103f590`).
- **CI enforces unit coverage with `llvm-cov` at 85%** (`9312ce9`).
- **Windows Rust stub for `spawn-core-sys`** — crate compiles without
  Zig POSIX (`f804f35`).
- **`ci/lint` gate restored** via `cargo fmt --all` (#705,
  `0565b7b`).
- **`docker`-style macOS-no-mount fuser + WinUI SDK bump + loom
  wiring** (#698, `8f62879`).
- **`Compress-Archive` on Windows** for matrix archive (#648,
  `6fffaa6`).
- **Scorecard publish permissions + deploy-docs YAML** (FR-001,
  #250, `d619424`).
- **`osv-scanner` ignore `rsa/paste`** with rationale (#714,
  `6d47aac`).
- **Scorecard publish permissions + deploy-docs YAML** (FR-001,
  #250, `d619424`).
- **`Merge branch 'recovery/preserve-20260717-0532-sharecli'`** —
  shared state recovery (`8d0c9a4`).

### Mesh / FR-010

- **Maildir queue depth** surfaced in status and thermal TUI
  (AC-010.11, #429, `ae86a9e`).
- **Maildir status/reclaim CLI** (FR-010 AC-010.9..10, #403,
  `02dff49`).
- **CoW commit/discard + `smart_merge`/`worktree` pool** (#400,
  `9a8ca99`).

### Config / governance

- **Config schema validation** with clear error messages on load
  failure (#63, `d0f6ab8`).
- **Config file watching + hot-reload** (#59, `44ecc54`).
- **Process-compose YAML integration** (#60, `f27600c`).
- **Project group bulk operations** — start/stop/restart/status
  (#58, `b236c0f`).
- **`process-compose` integration + docs** (Phase 2-3,
  `b8d2274`).
- **Phase 2 — shared runtime pool + project limits** (`b0a8cd6`).

### Docs / governance

- **Backbone-2 light theme** soft hermetic offline build
  (`3898e80`, `3bb5aee`).
- **Soft codesign runbook + MCP N/A ADR** (FR-004, #264,
  `5a5ffbb`).
- **Pyroscope soft push path for C05** (FR-003, #255, `cd14a0a`).
- **Soft concurrency map + memory budgets** (FR-001, #258,
  `e28ca22`).
- **Visual identity contract** published (FR-003, #261, `47e5891`).
- **Inclusive-language lint** + help golden (L81.10, #367,
  `ac2bbc8`).
- **Adaptive TUI + dashboard breakpoints** (FR-004, #240,
  `34d107b`).
- **Playwright Tab-cycle + design-system doc** (L81.3/L81.8 2→3,
  `4647bd5`).
- **Playwright keyboard gate** — fall back to system Edge/Chrome
  (#366, `19260dc`).
- **Adaptive TUI** + dashboard breakpoints (FR-004, #240,
  `34d107b`).
- **OpenAPI drift gate** for `serve` routes (FR-004, #241,
  `9089100`).
- **WCAG Level A `axe-core`** for dashboard (FR-004, #238,
  `86267e9`).
- **Table-header contrast → 5.16:1** (FR-004, #237, `eab25b8`).
- **`hyperfine` /healthz JSON CI artifacts** (FR-004, #236,
  `a753da9`).
- **Homebrew `sha256` from v0.3.0** (W4.2, #226, `5a35296`).
- **SBOM-in-release gate** (L9 2→3, C00 87%→90% A, #369,
  `5cb57be`).
- **FR↔test SSOT gate** (L12 2→3, #368, `408bc9f`).
- **`thiserror` exit codes + secrets runtime contract**
  (L14/L18 2→3, #372, `65469cd`).
- **CancellationToken shutdown + 10% bench-gate** (L4/L6 2→3, #373,
  `d231263`).
- **C03 re-score L30.1/L30.3/L30.9 stale 2→3** (C03 92%→100% A, #370,
  `8b2dab5`).
- **Wave 14 closeout sync** after #337–#391 (FR-003, `3998268`).
- **Wave 13 closeout sync** after #332–#335 (FR-003, #336,
  `1ade83e`).
- **Wave 12 closeout sync** after #326–#330 (FR-003, #331,
  `29c31c6`).
- **Wave 12 WBS/GAP/DAG/RC/PERT sync** (FR-003, #325, `7d45f9c`).
- **Wave 15 reconcile v6** after #396/#399 (FR-003, #570,
  `a0e3cbd`).
- **Wave 16 closeout** — coverage pin 80.51% at `e89755c`
  (T-730, #752, `eb2b865`).
- **Wave 16 soft stubs** — soft Harbor stub (T-720, #751,
  `75ebab1`); soft Pyroscope stub (T-710, #750, `8bd2665`).
- **Wave 17 kickoff T-800** (#755, `89b8806`).
- **Wave 17 W17.2 IN_PROGRESS** at `e298e0f` T-810 6 tests (#773,
  `2a490be`).
- **SCORECARD 2026-08-26 Wave17 W17.2** (#774, `13817d8`).
- **Fuzz harness** — protocol parser fuzz targets for DNS, SNMPv3,
  SSH, CoAP, LDAP (`1f4e787`).
- **Replace `unwrap()`/`expect()` with proper error handling** in
  production code (`fa887e9`).
- **Session+coordination coverage** for T-810 full lift (#771,
  `e298e0f`).
- **CI Scorecard + genuine files** (`93830c2`).
- **CODEOWNERS added** (`0c8944f`).
- **`Plan 802` SCORECARD reconciliation** (#803, `4ddcacd`).
- **`Plan 800` SCORECARD reconciliation** (#801, `986e5ed`).
- **`Plan 797` SCORECARD reconciliation** (#799, `8d446e1`).
- **`Plan 789` SCORECARD reconciliation** (#790, `ec0c3be`).
- **`Plan 786` SCORECARD reconciliation** (#788, `0509a52`).
- **`Plan 784` SCORECARD reconciliation** (#785, `70893b3`).
- **`Plan 782` SCORECARD reconciliation** (#783, `b2997fa`).
- **Wave 16 → Wave 17 WBS-PHASED sync** (#759, `779edb3`).

### Tests

- **Coverage pin @ `fa887e9` + Windows compat cfg gates**
  (T-810, #775, `691bde6`).
- **`dashboard_assets` + `session` registered in root allowlist**
  (#700, `16fe737`).
- **`fr007` health-pool-status-csv export + agent-call admission
  kernel** (#731, `e2ceacd`).
- **Test repair for `StatusJson::log_location`** (#733,
  `2989bba`).
- **Test prep lift 82 %** — FR-003 helpers for T-810 (#760,
  `3520208`).
- **FR-003 coverage climb-2** for fuse/ipc/mesh/fleet/core surfaces
  (#586, `f7716a3`).
- **Coverage pin post-#583** — broad-workspace at 81.17% (#585,
  `b26c2b0`).
- **Clippy clear sharecli-fuse `-D` warnings blockers** (#584,
  `8c42dce`).
- **Coverage climb toward 85 %** (#583, `8c68bb5`).
- **Honest 80.51% broad-workspace coverage pin** (#582,
  `5dbcb23`).
- **Remeasure broad-workspace coverage pin** (#580, `a85660f`).
- **Flaky thermal poll drop on CI runners** (`9bdb95e`).
- **Coverage lift toward 85 %** (`79236fe`, `884537f`).
- **Alloc label test aligned with `--all-features dhat`** (`3fad35c`).
- **`jemalloc`/`dhat` mutually exclusive under `--all-features`**
  (`e8013b5`).
- **`ps --all --watch text stderr silence`** (AC-007.50, #512,
  `b24d946`).
- **AC-007.49 `ps --all --watch --json` NDJSON gate/host_watch
  parity** (#511, `6f5fc12`).
- **AC-007.46 IPC `monitoring.report` gate + host_watch** (#508,
  `d2d324e`).
- **AC-007.45 IPC `health.status` gate + host_watch** (#507,
  `aef60ca`).
- **AC-007.44 `proc health/pool --json` gate + host_watch siblings**
  (#506, `a4b088f`).
- **AC-007.43 `ps --all --json` gate + host_watch siblings** (#505,
  `df8da89`).
- **AC-007.42 `report` watch NDJSON gate + host_watch per refresh**
  (#503, `5c5e7c6`).
- **Drain stdout+stderr in `report` watch JSON gate tests** (#504,
  `cb9bb2a`).
- **`fr007` `lock proc --pid` operator envelope in parity suite**
  (AC-007.86, #548, `ec47769`).
- **Audit-log emit unit test de-flake** (#209, `e631962`).
- **CI green lanes** — mutation-testing, trunk-check, a11y
  keyboard, scorecard, quality-gate (#719, `eff4f3f`).
- **CI green lanes** — security-scan, a11y, SAST, scorecard,
  trunk-check (#717, `a958f78`).
- **CI green lanes** — `deny`, `audit`, `ci-gate fmt`, docs-build
  (#715, `d564b55`).
- **Daemon-shield** — pre-commit hook + `AGENTS.md` note for Airlock
  Bot (`407c31d`).
- **WBS T-95 composite health metric (0–100, 4 bands)** — `54e8ded`,
  `2b13b29`.
- **WIP 5-subpage segmented Config editor** restored (#761,
  `86c7066`).
- **L100 first-run empty-state timer** restored (#764, `e5cea02`).

### Removed / Deprecated

- **Dead `TreeView` + orphaned helpers** removed (Q11, `f1ae476`).
- **Legacy `ProcessTableView` shim** removed (`8bf0a0b`).
- **`contextMenu` property** removed from `lib.rs` (`98f56e7`).
- **`MockProcessRunner` `dead_code` allow** (cast integration tests,
  #46, `6ce68cf`).
- **Absorbed deprecated `PhenoProc` deps** replaced with local
  runtime types (`011b9a6`).

### Fixed

- **`fuse::mount` rebase-marker cleanup** (`98f56e7`, `8201c44`,
  `54277b0`).
- **`fuse` `smoke_fuser_config_for_backend` re-export restored**
  (`669c00e`).
- **`Cargo.lock` ignored** `bench/` (#605, `e6c9ae9`).
- **`unused import` warning + test compilation** (`40e9494`).
- **`session watcher` pointed at installed `sharecli`** (`b26f361`).
- **`disconnect.svg` repaired** so `fs::read_to_string` succeeds
  (#708, `440b47c`).
- **`fr003` heap reports explicit in profiling facade** (#738,
  `34f1b31`).
- **`make ShareCLI validation deterministic`** (#744, `633790b`).
- **`T-692` dashboard hex drift** closed (FR-003 C10 L98, #748,
  `fe3f921`).
- **`WinUI 3` Window attrs dropped**, use `ListView` (XamlCompiler
  MSB3073, #702, `2aa7d7e`).
- **Thermal sysfs read resilient** + `jsonwebtoken rust_crypto`
  provider (#633, `5bc01eb`).
- **Tray `CShareCLIFFI.o` emitted** + FFI link paths aligned
  (#391, `0fa1fd0`).
- **`session` clippy persistence lint** satisfied (`97d5ab5`).
- **`session` list CLI covered** (`058152a`).
- **`session` adapters and tests formatted** (`d9c4175`).
- **`session` zmx + ghostty capability adapters** (`fdd4cc9`).
- **`session` confidence + harness recipes** (`b9f1651`).
- **`session` durable RPC core** (`12d867c`).
- **`dhat-heap.json` added to `.gitignore`** (`b2d3ba1`).
- **`bench/` added to `.gitignore`** (`cd1d5fb`).
- **`.build/` added to `.gitignore`** (SwiftPM/Xcode build cache,
  `7bafd60`).
- **Trunk-CI cosign action re-pinned** (`ec353cb`).
- **`wip: triage checkpoint` — preserve externally-modified files
  before surgical cleanup** (`24a5d6c`).
- **`session-recovery-freshness-20260820`** landed (#768,
  `0377fcb`).
- **`fix/session-recovery-freshness-20260820` provenance** (#768,
  `0377fcb`).
- **`fuse` rebuild markers + dead `contextMenu`** (`98f56e7`).
- **Tray UX fixes** — `2d6645d`.

---

## [0.8.0] - 2026-09-12

Release tagged `v0.8.0` (`1c4d3ff`). Version drift fix aligning canonical
version across Cargo.toml, Info.plist, and all workspace crates.

### Added
- **T-70 appcast pipeline** — release notes for Sparkle appcast generation.

### Fixed
- **Version drift** — canonical version bumped from `0.3.0` to `0.8.0` across
  root `Cargo.toml`, all `crates/*/Cargo.toml`, and macOS tray `Info.plist`.

---

## [0.3.0] - 2026-07-04

Tagged release `v0.3.0` (`c203265`). Snapshot of mid-summer 2026 work:
federated AuthN, OSV scanning, FR-001–FR-012 lifts, full FUSE backend,
Ghostty terminal control plane, and dashboard expansion.

### Added

- **Wave 0 / Wave 1 / Wave 2 governance** — CI macOS matrix, perf
  gate, SBOM/OpenAPI/brew HEAD (#204, `9b89f6b`).
- **Wave 1 lift** — C03/C07/C08/C11 lifts merged (#202, `4b5b86b`).
- **Audit (v38) — re-score C03/C07/C08/C11 after Wave1 lifts**
  (`ccc43c5`).
- **`v38`-audit CI truth** — required gates stabilised
  (#198, `05c7950`).
- **T-300 unhappy-path friction** — C03 → 92 % A (#222, `ae2c2c5`).
- **T-250 golden CLI/TUI fixtures** — C03 → 89 % B (overall 65 % C,
  #221, `ae2c2c5`).
- **T-240 Quick Start outside-in journey** — L30.6 → 3 (#220,
  `ba34cec`).
- **T-230 FR-005 acceptance suites** — AC-005.1..005.5 (#219,
  `5151377`).
- **T-220 FR-004 acceptance suites** — AC-004.1..004.5 (#214,
  `25f2bdb`).
- **T-210 FR-003 acceptance + claim-lock + loop budgets** (#213,
  `eeea69f`).
- **T-200 FR-002 + threat/release lifts** — overall → 64 % C (#212,
  `f6693b4`).
- **Alertmanager routing + nightly bench trends** (C05 L48, #211,
  `299ee56`).
- **`pprof` flamegraph + OTel 0.32.1** — C05 → 67 % C (#210,
  `8efdc20`).
- **Serve Bearer AuthN + audit log + measured C08 baselines**
  (#208, `cdace94`).
- **OTel + RED metrics + Grafana** — C05 → 60 % C (#206,
  `68475cf`).
- **C09 audit ladder** — `L81.13 FAQ + clap_mangen man page` (2→3)
  (#377, #376, `97981d7`, `35d5355`).
- **C04 dual secret scanners** — gitleaks + trufflehog (L31 2→3,
  #374, #375, `48cf350`, `9ab9527`).
- **C10 empty/zero-data CTAs** (L100 2→3, #378, `e7aaaab`).
- **C05 L46 MWMB burn-rate + error budget policy** (2→3, #380,
  `a162208`).
- **C07 L64 e2e/chaos test pyramid tier** (2→3, #384, `943de7a`).
- **C10 L101 dashboard disconnect error view** (2→3, #383,
  `3e60f03`).
- **C06 L54 + C07 L70 + C11 L115 lifts** — unweighted 90 % A (#381,
  #382, `17bebc8`, `c42c536`).
- **Audit (v38) — complete C00–C11 scorecard + close CI truth gaps**
  (`ec8e87a`).
- **Audit (v38) — re-score C05 to D and C10 after observability
  /identity lifts** (`0de6445`).
- **C07 proptest expand boundary + registry + replay** (L66 2→3,
  #364, `3a2570c`).
- **Wave 0 governance** — `sharecli-ci-test` repair (#198,
  `05c7950`); `sharecli-ci-rustsec` (#198, `a45681a`);
  `sharecli-ci-fr` (#198, `24134f9`); `sharecli-ci-test` (#198,
  `e6f4aaa`).
- **C09 Playwright Tab-cycle + design-system doc** (L81.3/L81.8 2→3,
  #365, `4647bd5`).
- **C09 audit ladder** — L81.10 Vale inclusive-language lint + help
  golden (#367, `ac2bbc8`).
- **C09 audit ladder** — L81.13 FAQ + clap_mangen man page (#377,
  `97981d7`).
- **Audit (v38) — defer Dependabot PRs until Wave1 green**
  (`8aa2df1`).
- **`v38`-audit dep PR** — `sharecli-deps-remaining` (#203,
  `2a3909b`).
- **Recovery** — `recovery/preserve-20260717-0532-sharecli`
  (`8d0c9a4`).

### Fixed

- **Audit (v38) — `sharecli C04` scorecard Security (L31–L40)**
  (#131, `e5d447c`).
- **Audit (v38) — Wave0 failure matrix for PR #198 CI lanes**
  (`3ce7355`).
- **Audit (v38) — record `thegent/sharecli` boundary alignment**
  (`196429e`).
- **CI `rustfmt + -D warnings`** leaf blockers for Linux CI
  (`7558f01`).
- **CI enforce unit coverage with `llvm-cov` at 85%** (`9312ce9`).
- **CI add FR annotation to integration_cli tests** (`28ce925`).
- **CI stabilize `serve_lock` tests under Linux `flock` + parallel
  env** (`1f91015`).
- **CI repair corrupted Go template syntax** (`b504ab6`).
- **`tauri-winrt-notification` 0.7.2→0.7.3** (drop `quick-xml`,
  `02d283a`).
- **`sharecli` doctests failing `cargo test --doc`** (#198,
  `022f236`).
- **Clippy — `question_mark` and `derivable_impls` under
  `-D warnings`** (`108e750`).
- **Clippy — `manual_pattern_char_comparison` under `-D warnings`**
  (`3742e36`).
- **`bytes` 1.12.0 → 1.12.1** (`6c64050`).
- **`sharecli Wave0`** — `serve_lock` merge (`049a9a6`).
- **`spawn-core-sys` Windows Rust stub** so crate compiles without
  Zig POSIX (`f804f35`).
- **`sharecli-ci-test` doctest fixes** into `v38-audit-ci-truth`
  (`05c7950`).
- **`sharecli-ci-fr` + `sharecli-ci-rustsec` + `sharecli-ci-test`
  merges** (`a45681a`, `e6f4aaa`, `24134f9`).
- **`deny.toml` allows substrate git source + rustfmt** (`3d61321`).
- **CI install zig and satisfy gates** (`33a0864`).
- **`spawn-core` bundle zig compiler runtime** (`fe01d02`).
- **`spawn-core` handle `execve` failure for zig** (`c2a9796`).
- **CI pin workflow actions** (`7c8e369`).

### Removed

- **Dependabot `sha2-0.11.0`** merge (#194, `7db512f`).
- **Dependabot `bytes-1.12.1`** merge (#199, `f26f58a`).

---

## [0.1.0] - 2026-07-02

Tagged release `v0.1.0` (`464e737`). The first public release of
**sharecli** — a shared CLI process manager for multi-project agent
orchestration. Snapshot of work through 2026-07-02.

### Added

- **`sharecli-fleet` scaffold** — NATS registry + thermal-governor
  stubs (T10, #22, `066703a`).
- **NATS device registry impl** (T11, #23, `e5b99b4`).
- **`sharecli fleet`** subcommand + cross-device deploy doc
  (T15+T16, #24, `830a0a8`).
- **`ThermalGovernor`** — macOS pressure + Linux temp (T12+T13,
  #25, `87b53c6`).
- **Sidecar install script** — `launchd` (macOS) + `systemd`
  (Linux) (T28, #26, `b16c357`).
- **NATS connect/announce/subscribe to real coordinator** (T47,
  #27, `52543ff`).
- **`health_beat`** — periodic `DeviceRecord` announce loop (T54,
  #28, `464e737`).
- **GitHub Actions release workflow** with binary artifacts (T36,
  #29, `a39e07e`).
- **`sharecli-fuse` IO-interception crate** (fuser, cross-platform,
  #30, `ab700c5`).
- **`sharecli-ipc` coalesce/debounce/queue crate** (Lock-Wait-Cache,
  #31, `8d924b7`).
- **`cast` subcommand** — Ghostty + wezterm casters (T59 cast
  backlog, #32, `fee481c`).
- **`sharecli-core` hypervisor engine scaffold** — spawn + coalesce
  entry point (#33, `ae81b3e`).
- **Caster** — rewrite Ghostty caster + implement Windows Terminal
  caster (Tasks 3+5, #37, `bb5d85c`).
- **Release pipeline** — `cargo-dist` + tray installers (#39,
  `5b604bd`).
- **`Hypervisor::run` — thermal gate before spawn** (#38,
  `df43b2f`).
- **Hypervisor thermal-gate spawn back-pressure** (#40, `bbcb2cd`).
- **`p3` remediate top audit findings** (#41, `11af729`).
- **Tray — Linux `ksni 0.3` blocking + kill actions** (#21,
  `f8d2b68`).
- **Tray — Linux `ksni`** (#19, `2e93e7f`).
- **Tray — Windows WinUI 3 (TCP IPC bridge + C# FFI)** (#18,
  `e58404f`).
- **Tray + native client (Swift/macOS)** over Rust core — config +
  observability (#17, `8cd8510`).
- **Build-contention throttle** — semaphore + taskpolicy +
  cargo-jobs + sccache (#16, `a3e308e`).
- **Absorb native harness runtime crate** (#10, `e9459ad`).
- **Forward args to spawned process + fix config module import**
  (#15, `6967024`).
- **Phase 3 — spec+test+traceability layer (FR-001..FR-005)** (#8,
  `5d1dcd3`).
- **`sharecli` boundary lock** (`b89bf73`).
- **Tier-0 hygiene snapshot 2026-06-20** (`f877e27`).
- **Consume substrate `ProcessPort`, drop duplicated process-pool
  code** (`39b92fa`).
- **Clippy warnings + CI failures resolved** (`a4000df`).
- **`spawn-core` handle `execve` failure for zig** (`c2a9796`).

### Fixed

- **`dyn-workflows` architecture sketch + codex-exec prompt (recon)**
  (`96a689d`).
- **CLI forward args to spawned process** (#15, `6967024`).
- **`substrate` git source allow in `deny.toml`** + rustfmt
  (`3d61321`).

### Security

- **Dependabot `thiserror-2`** (#12, `d677e1a`).
- **`thiserror` 1 → 2** (`efebda7`).
- **`itertools` 0.12 → 0.15** (#13, `105feb4`).
- **`toml` 0.8 → 1.1** (#2, `6714fca`).
- **`sysinfo` 0.30 → 0.39** (#6, `cdfb21b`).
- **`config` 0.14 → 0.15** (#34, `336aa09`).
- **`sha2` 0.10 → 0.11** (#35, `11f9cb5`).
- **`async-nats` 0.38 → 0.49** (#36, `9ccc46c`).

---

## Pre-release history (2026-03-25 → 2026-07-01)

Initial development predating the first tagged release.

### Added

- **Initial `sharecli` process manager** (`cd3ad4c`, 2026-03-25).
- **Phase 1 — process manager** (`9a2e030`).
- **Phase 2 — shared runtime pool + project limits** (`b0a8cd6`).
- **Phase 2-3 — process-compose integration + docs** (`b8d2274`).
- **TEST_COVERAGE_MATRIX.md** (`29be7a5`).
- **SPEC.md with architecture + API spec** (`b25af77`).
- **Comprehensive documentation** (`78217cc`).
- **AgilePlus scaffolding** (`927aea9`).
- **Journeys, stories, traceability docs** (`cb585b4`).
- **Agent-readiness governance templates** (`e123575`).
- **Standardized infrastructure files** (`acfd498`).
- **VitePress skeleton + deploy workflow** (`6b2075f`, `9626652`).
- **Reusable workflows from template-commons** (`2917150`).
- **Consolidate sharecli with dedup/queue modules from
  `thegent-sharecli`** (`14b3807`, `ea46668`).
- **Replace broken `PhenoProc` deps with local runtime types**
  (`011b9a6`).
- **CODE_OF_CONDUCT.md, LICENSE** (`4d2e2dd`).
- **ShareCLI boundary lock** (`b89bf73`).
- **Phase 3 — spec+test+traceability layer (FR-001..FR-005)**
  (`5d1dcd3`).
- **Absorb native harness runtime crate** (`e9459ad`).
- **Build-contention throttle** (`a3e308e`).
- **Tray + native client (Swift/macOS)** (`8cd8510`).
- **Forward args to spawned process + fix config module import**
  (`6967024`).
- **Apply phenotype governance standards** (`a405be7`).
- **Missing governance files** (`.gitignore`, `CLAUDE.md`)
  (`fb6ac55`).
- **CLIPPY / CI fixes** (`a4000df`).
- **Consume substrate `ProcessPort`, drop duplicated process-pool
  code** (`39b92fa`).

### Fixed

- **CI — pin workflow actions** (`7c8e369`).
- **CI — install zig and satisfy gates** (`33a0864`).
- **`spawn-core` — handle `execve` failure for zig** (`c2a9796`).
- **`spawn-core` — bundle zig compiler runtime** (`fe01d02`).
- **CLI — forward args to spawned process** (`6967024`).
- **CI — repair corrupted Go template syntax** (`b504ab6`).
- **`deny.toml` — allow substrate git source + rustfmt**
  (`3d61321`).

### Security

- **Dependabot `thiserror-2`** (`d677e1a`).
- **`thiserror` 1 → 2** (`efebda7`).

---

## Release tag anchors

| Tag      | Commit    | Date       | Notes                                            |
|----------|-----------|------------|--------------------------------------------------|
| `v1.0.0` | `a290e3a` | 2026-08-31 | First stable major release. Wave 20 spec ships.  |
| `v0.8.0` | `1c4d3ff` | 2026-09-12 | Version drift fix + T-70 appcast pipeline.        |
| `v0.3.0` | `c203265` | 2026-07-04 | Mid-summer lift to overall ~64 % C.              |
| `v0.1.0` | `464e737` | 2026-07-02 | First tagged public release (`health_beat` T54). |

[Unreleased]: https://github.com/KooshaPari/sharecli/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/KooshaPari/sharecli/releases/tag/v0.8.0
[1.0.0]: https://github.com/KooshaPari/sharecli/releases/tag/v1.0.0
[0.3.0]: https://github.com/KooshaPari/sharecli/releases/tag/v0.3.0
[0.1.0]: https://github.com/KooshaPari/sharecli/releases/tag/v0.1.0
