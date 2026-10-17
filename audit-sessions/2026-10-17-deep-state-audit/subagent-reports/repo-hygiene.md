# repo-hygiene lane (subagent 4/8)

Repo: `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli`
HEAD: `3d6a41c972c2c86bf1374f224019c365ce3aa646` (2026-10-16 21:31:33 -0700)
Tracked files: 1185 | Local branches: 27 | Remote refs: 1003 | .gitignore: 69 lines

## Headline
- Repo hygiene is GOOD with three remediation items: tracked dhat-heap.json profiling artifact (HIGH), tracked runtime `.lock` files inside crates/harness-native/var (MEDIUM), and missing explicit `.env` pattern in .gitignore (LOW).
- 1: tracked dhat profiling output + runtime lock files leak build state into history
- 2: .gitignore has no explicit `.env` / `secret*.yml` / `credentials*` patterns
- 3: local branch count is healthy (27); all < 30 days stale; no janitor work needed

## .gitignore
- Present: Y (69 lines, `.gitignore:1`)
- Covers:
  - target/: Y (`/target/`, `.gitignore:23`) plus per-branch targets `target-ac*/`, `target-cow-parity/`, `target-fuse-smoke/` (`.gitignore:48-54`)
  - node_modules/: Y (`.gitignore:39`)
  - .DS_Store / Thumbs.db: Y (`.gitignore:7`, `.gitignore:46`)
  - .env / .envrc / secrets.yml: N -- no explicit pattern
  - large files: N -- no `*.bin`, `*.elf`, `*.dll`, `*.so`, `*.dylib` patterns
- Gaps:
  - `.env`, `.env.*`, `*.env`, `.envrc` (template `.env.example` is intentionally tracked at `.env.example`; actual `.env` is currently untracked but not explicitly ignored)
  - `*.lock` is too broad (would break Cargo.lock which is intentional); need narrower `**/var/**/.lock` or `crates/harness-native/var/**/*.lock`
  - `*.dll`/`*.elf`/`*.so`/`*.dylib` would catch stray binaries
  - `*.heapsnapshot`, `dhat-heap.json` to keep profiling artifacts out of history
- Note: `.gitignore` is well-commented and sectioned (RUST/SWIFT/ZIG/AGENT/EDITORS/OS) -- structure is exemplary even if a few narrow gaps remain.

## Tracked build artifacts
- Files: 5 (3 runtime locks + 1 profiling heap + 1 ELF stub)
  - `crates/harness-native/var/sharecli-hypervisor/a04613e3a977d4fa9771dfa73b1ac5fbb4683198d4b6ede700621ccaa0a1d481.lock` (content-addressed runtime lock)
  - `crates/harness-native/var/sharecli-hypervisor/fea52e90a964a61017acfe39964c9b86059367a0652ff593db323748186509d9.lock` (same)
  - `crates/harness-native/var/sharecli-hypervisor/queue/missing.slot0.lock`
  - `crates/harness-native/var/sharecli-hypervisor/queue/true.slot0.lock`
  - `dhat-heap.json` (212458 bytes, DHAT profiler heap snapshot)
- Also tracked but INTENTIONAL: `Cargo.lock`, `fuzz/Cargo.lock`, `lib/teamcomm/Cargo.lock`, `package-lock.json` (Rust + JS lockfiles should be tracked; correct).
- Risk: MEDIUM (dhat-heap.json: build artifact / profiling output leaking into history; `.lock` runtime files: state from prior daemon runs).

## Tracked secrets
- Files matched by keyword scan: 9, ALL FALSE POSITIVES
  - `CHANGELOG.md`, `README.md`, `PERSONAL_EVALUATION_GUIDE.md`, `THREAT_MODEL.md`, `justfile` -- documentation referencing "secret"/"password"/"webhook_secret" in threat-model + changelog context
  - `src/commands/upgrade.rs:11` -- comment "L112 secrets landing"
  - `crates/gateway-tools/src/main.rs:161-170,334-335` -- HMAC/JWT code using parameter name `secret: String` (no hardcoded value)
  - `audit/rubric/X-packaging-distribution-L108-L122.md:85,89,151` -- acceptance criteria mentioning "secrets" + reference to `secrets.*` in workflow grep
  - `sharecli/preserved/migrated-from-10-override-batch/KodeVibe/engine/pkg/auth/{auth.go,middleware_test.go}` -- vendored source with parameter `secret []byte`
- High-confidence scan (`ghp_*`, `sk-*`, `aws_access_key_id`, `BEGIN RSA|OPENSSH PRIVATE|PRIVATE KEY`): 0 matches.
- Severity: NONE (no actual secrets).

## Large files (>5MB threshold = none; >100KB reported)
- Top tracked files by size:
  - `recovery/feb-2026-agent-harness/artifacts/harness-fuse.elf` -- 2,001,024 bytes (~2 MB ELF binary)
  - `windows/ShareCLITray/sharecli_ffi.dll` -- 591,360 bytes (~591 KB Windows FFI binary)
  - `assets/dashboard/ui/video/brand_intro.mp4` -- 553,973 bytes (~554 KB marketing video)
  - `assets/dashboard/ui/banners/hero_1920x600.png` -- 357,662 bytes
  - `assets/icons/sharecli.icns` -- 294,196 bytes (macOS icon bundle)
  - `assets/dashboard/ui/icons/phenotype_icon.png` -- 244,148 bytes
  - `assets/dashboard/ui/banners/og_1200x630.png` -- 214,667 bytes
  - `dhat-heap.json` -- 212,458 bytes (PROFILING ARTIFACT, should not be tracked)
  - `Cargo.lock` -- 179,671 bytes (intentional for Rust)
  - `fuzz/Cargo.lock` -- 147,353 bytes (intentional for fuzz workspace)
  - `assets/icons/sharecli.iconset/icon_1024x1024.png` -- 152,364 bytes
- All except `dhat-heap.json` and the ELF/DLL binaries are intentional (assets, lockfiles, icons). The ELF + DLL are the only files that warrant Git LFS consideration if repo size becomes a problem; current total pack size is 257.21 MiB which is comfortable.

## Stale branches
- Local branches: 27
- Remotes: 1003 (including 109+ `pr/N` head refs from dependabot/automation)
- Stale (>30 days no commits, cutoff 2026-09-17): 0
  - All 27 local branches last touched between 2026-10-12 and 2026-10-16 -- 4 to 8 days ago.
  - Oldest local: `fix/sharecli-phase-05-ipc` (2026-10-12 00:04:33 -0700) -- 5 days.
- Stale remote refs: many `pr/N` and `dependabot/*` and `audit/*` refs are months old but are standard CI/dependabot archaeology; no manual cleanup needed.

## Working tree
- Status: clean (`git status --short` returns empty)
- Untracked (per `git status --ignored`): all are properly ignored
  - `.DS_Store` (top + `output/.DS_Store`)
  - `.audit/` (assessment workspace)
  - `.vale/styles/Microsoft/` (synced styles, regenerated by CI)
  - `crates/spawn-core/.zig-cache/`, `crates/spawn-core/zig-out/` (Zig build)
  - `target/` (Rust build)

## Submodules
- `.gitmodules`: NOT PRESENT (no submodules).

## Cargo.lock / lockfile tracking
- Tracked (intentional for binary crate):
  - `Cargo.lock` -- 179,671 bytes (root workspace)
  - `fuzz/Cargo.lock` -- 147,353 bytes
  - `lib/teamcomm/Cargo.lock` -- (vendored teamcomm workspace)
  - `package-lock.json` -- 111,668 bytes
- Tracked (PROBLEMATIC, runtime state):
  - `crates/harness-native/var/sharecli-hypervisor/*.lock` -- 3 files (runtime locks, not dependency locks)

## CI secrets in workflow
- Hardcoded in workflow: N
- All secrets referenced via `${{ secrets.* }}` pattern (correct usage):
  - `.github/workflows/codesign-soft.yml:34-41` -- `APPLE_CERTIFICATE_BASE64`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_ID`, `WINDOWS_CERT_PFX_BASE64`, `WINDOWS_CERT_PASSWORD` (all `${{ secrets.* }}`, with comment "Referenced so evidence grep finds secrets.*")
  - `.github/workflows/ci.yml:234` -- `GITHUB_TOKEN`
  - `.github/workflows/codesign.yml:43` -- `INFISICAL_TOKEN`
- 9 workflows reference secrets.* (codesign, codesign-soft, ci-gate, appcast, flake-tracker, container-cosign, infisical, gpg-soft, packaging-soft, nightly-bench).
- High-confidence hardcoded scan (`password:`, `secret:`, `api_key:` with literal values): 0 hits.
- `process.env.X` literal usage in JS scripts: 3 references (SHARECLI_VISUAL_FIXTURE, UPDATE_VISUALS -- both flags, not secrets).

## Other hygiene observations
- Nested `.git` (worktree) check: 0 nested repos.
- `.DS_Store`, `Thumbs.db` patterns: Y.
- Agent config dirs ignored (`.claude/`, `.codex/`, `.cursor/`, `.gemini/`, `.idea/`, `.vscode/`): Y (`.gitignore:1-9`).
- `.agileplus`, `.audit`, `.cargo`, `.circleci`, `.config`, `.devcontainer`, `.githooks`, `.trunk`, `.vale`, `.mergify.yml` are all TRACKED (intentional -- these are repo config, not user config). No issues.
- `.gitguardian.yaml` and `.gitleaks.toml` and `.trufflehog.yml` all present and tracked -- dual secret scanners wired (per `CHANGELOG.md:470,906`).
- `.pre-commit-config.yaml` present (3,117 bytes).
- No `.env` files tracked; `.env.example` (777 bytes) is the only template.
- `recovery/` directory contains 84 KB of files including the 2 MB ELF; this is historical artifact retention, intentional.

## CVP/MVP gap -- smallest set of repo-hygiene fixes
| Item | Fix | Estimate |
|---|---|---|
| 1 | Untrack `dhat-heap.json` (212 KB profiling artifact); add `dhat-heap.json` and `**/*.heapsnapshot` to .gitignore | 10m |
| 2 | Add narrow `**/var/**/*.lock` (or `crates/harness-native/var/**/*.lock`) to .gitignore; untrack 3 runtime lock files via `git rm --cached` | 10m |
| 3 | Add explicit `.env`, `.env.*`, `.envrc`, `secrets.yml`, `credentials*` patterns to .gitignore (defense in depth) | 10m |
| 4 | Consider Git LFS for `recovery/.../*.elf` (2 MB) + `windows/ShareCLITray/sharecli_ffi.dll` (~591 KB) | 30m |
| 5 | Add `*.dll`, `*.elf`, `*.so`, `*.dylib` patterns to .gitignore (prevent future stray binaries) | 10m |
| 6 | Decide policy on `recovery/` directory (currently 84 KB of historical agent harness artifacts -- keep, archive, or gitignore) | 1h |
| 7 | Sweep dependabot/pr remote refs (`remotes/origin/pr/*`, `remotes/origin/dependabot/*`) | 30m |
| 8 | Replace binary `recovery/feb-2026-agent-harness/artifacts/harness-fuse.elf` with source or LFS pointer | 4h |

Minimum viable hygiene (MVP): items 1 + 2 + 3 (3 x 10m = 30m total).

## Confidence
- 87%
- Ground truth evidence:
  - HEAD: `git log -1` -> `3d6a41c972c2c86bf1374f224019c365ce3aa646` (2026-10-16 21:31:33 -0700)
  - Working tree: `git status --short` -> empty (clean)
  - .gitignore size: `wc -l .gitignore` -> 69 lines
  - Tracked secret matches: 9 files, all verified as FALSE POSITIVES by direct `rg -n` inspection (CHANGELOG.md:470, README.md:271, justfile:56-58, src/commands/upgrade.rs:11, crates/gateway-tools/src/main.rs:161-335, audit/rubric/X-packaging-distribution-L108-L122.md:85,89,151, sharecli/preserved/.../auth.go:7-44)
  - High-confidence secret scan (ghp_*, sk-*, aws_access_key_id, BEGIN *PRIVATE*): 0 matches across entire repo
  - Stale branch cutoff: 2026-09-17, 0 local branches older
  - Branch count: `git branch | wc -l` -> 27; `git branch -a | wc -l` -> 1003
  - CI secret scan: `rg -n "GH_TOKEN|GITHUB_TOKEN|secrets\." .github/workflows/*.yml` shows 9 workflows using `${{ secrets.* }}`; 0 hardcoded literals
  - Large files: `git ls-files | xargs wc -c` top 20 sorted, reported above
  - dhat-heap.json: 212,458 bytes (single profiling artifact; should be in .gitignore)
  - runtime locks: 3 files under `crates/harness-native/var/sharecli-hypervisor/`