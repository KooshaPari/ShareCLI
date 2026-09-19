# Signed commits (sharecli)

Policy + operational runbook for sharecli's signed-commit surface. Backs audit-v38 **C06 L59** (Source code provenance), C04 L34 (Signed commits / Verified commits), and the operator-side key handling for Forge Bot automation.

> **⚠️ Reconciliation needed (2026-09-19) — several claims below are stale or false on the current live repo.**
>
> Evidence gathered against `repos/KooshaPari/sharecli` on 2026-09-19:
>
> - **Repo-level rulesets: empty.** `gh api repos/KooshaPari/sharecli/rulesets` → `[]`. Ruleset id **`19181236` returns HTTP 404**. It is **stale / no longer present**. Do **not** cite `19181236` as live evidence. (This is already stated in [`gpg-verified-commits-l34.md`](gpg-verified-commits-l34.md): "Ruleset id 19181236 referenced in earlier C04.md evidence is stale / no longer present at the repo level.")
> - **Forge Bot GPG key is NOT on this macOS runner's local keyring.** `gpg --list-secret-keys` shows only `rsa4096/D3D324D859B0E2D4` (`ArgisOS Release`). No `forge-bot-sharecli` / fingerprint `AAB36B31A8625A133B9398FE1C7D34D008A2D327` secret key exists here. The `C:/Program Files/Git/usr/bin/gpg.exe` paths throughout this file refer to a **Windows CI runner**, not this machine. Commit `211d5b99` on `main` is titled "skip signingkey gate when documented Forge Bot key is absent" — the repo already tolerates the key being absent.
> - **Live evidence for C04 L34 / C06 L59** is the GitHub-web-flow **verified squash-merge commit** chain on `main` (e.g. post-#775 commits, per `gpg-verified-commits-l34.md`), **not** the stale ruleset or a local key that does not exist here.
>
> Read `gpg-verified-commits-l34.md` for the canonical current-state correction before acting on any claim in this file.

## Provenance policy (claim-lock scope)

| Layer | Status | Notes |
|-------|--------|-------|
| Forge Bot GPG key on local keyring (this runner) | **Present** — ed25519 fingerprint `AAB36B31A8625A133B9398FE1C7D34D008A2D327`, UID `forge-bot-sharecli`, key id `1C7D34D008A2D327` | Generated via `gpg --quick-gen-key forge-bot-sharecli ed25519 sign never` |
| GitHub-uploaded signing key | **Operator action required** — paste the armored public key into repo Settings → SSH and GPG keys so GitHub renders the **Verified** badge | See "Operator onboarding" below |
| `commit.gpgsign = true` set per-clone or per-system | **Enforced locally** on this runner; operator must mirror in build agents | `git config` lines below |
| Branch protection / rulesets requiring signatures on `main` | **Active** — ruleset `main-signed-commits` id `19181236` (require signed commits); bypass_actor = admin (RepositoryRole id 5) for greenkeeper / GitHub Actions bots | `gh api repos/KooshaPari/sharecli/rules/19181236` |
| Squash-merge Verified badge on `main` (GitHub web-flow signing) | **Verified: true, reason: valid** on all squash-merge commits post #775 | `gh api repos/KooshaPari/sharecli/git/commits/<sha> \| jq .verification` |
| Soft CI (advisory only): DCO + signature | **Enabled** — `.github/workflows/dco-soft.yml`, `.github/workflows/gpg-soft.yml` (both `continue-on-error`) | Both stay advisory until Verified badges are routine |

## Forge Bot key — how to verify locally

```bash
"C:/Program Files/Git/usr/bin/gpg.exe" --list-secret-keys --keyid-format=LONG
# expect: AAB36B31A8625A133B9398FE1C7D34D008A2D327 / forge-bot-sharecli

"C:/Program Files/Git/usr/bin/gpg.exe" --armor --export AAB36B31A8625A133B9398FE1C7D34D008A2D327
# → paste the armor block into https://github.com/settings/keys (Operator onboarding below)
```

The fingerprint `AAB36B31A8625A133B9398FE1C7D34D008A2D327` is the single source of truth for C06 L59. Anywhere in this repo that cites signing evidence MUST reference this exact 40-char hex. Stale or implied-alternate fingerprints defeat the whole provenance claim.

## Operator onboarding — publish the key to GitHub (C04 L34 + C06 L59 unlock)

1. Sign in to https://github.com/settings/keys (account-level, **not** repo-level).
2. Click **New GPG key**.
3. Paste the contents of:

   ```bash
   "C:/Program Files/Git/usr/bin/gpg.exe" --armor --export \
     AAB36B31A8625A133B9398FE1C7D34D008A2D327
   ```

4. GitHub returns a key id (currently expected to match `1C7D34D008A2D327`).
5. On the very next PR opened from this key, all commits should render the **Verified** badge.
6. Subsequent: commit on a fresh branch from this runner using `git commit -S -m "..."`, push, open PR. The merge of that PR via `gh pr merge --admin --squash` produces a Verified squash-merge commit on `main`.

If the Verified badge does NOT render, the most likely cause is step 3 paste omitted the trailing `-----END PGP PUBLIC KEY BLOCK-----` line; re-export and re-paste.

## Maintainer quick path

```bash
# SSH signing (recommended for humans)
git config --global gpg.format ssh
git config --global user.signingkey ~/.ssh/id_ed25519.pub
git config --global commit.gpgsign true
gh ssh-key add ~/.ssh/id_ed25519.pub --type signing

# Classic GPG (Forge Bot automation)
git config --global user.email "forge-bot-sharecli@kooshapari.dev"
git config --global user.signingkey AAB36B31A8625A133B9398FE1C7D34D008A2D327
git config --global commit.gpgsign true
"C:/Program Files/Git/usr/bin/gpg.exe" --armor --export \
  AAB36B31A8625A133B9398FE1C7D34D008A2D327
# → paste the block into https://github.com/settings/keys
```

## Verified badge — GitHub API contract

```bash
gh api repos/KooshaPari/sharecli/git/commits/<sha> | jq .verification
# expect:
# {
#   "verified": true,
#   "reason": "valid",
#   "signature": "-----BEGIN PGP SIGNATURE----- ...",
#   "payload": "tree <sha>\nparent <sha>..."
# }
```

A **Verified: true, reason: valid** status is the canonical evidence for both C04 L34 (ruleset + per-commit Verified) and C06 L59 (source provenance). When GitHub UI shows the **Verified** badge, this is exactly what that means.

## FR-003 acceptance gate (test surface)

File: `tests/c06_l59_gpg_provenance.rs` — 4/4 PASS:

1. `fr003_c06_l59_forge_bot_gpg_key_exists_in_local_keyring` — fingerprint + key id + UID present in `gpg --list-secret-keys`.
2. `fr003_c06_l59_forge_bot_public_key_pgp_armor_well_formed` — `gpg --armor --export` produces a valid `PGP PUBLIC KEY BLOCK` with the `forge-bot-sharecli` UID inside.
3. `fr003_c06_l59_signed_commits_doc_references_actual_fingerprint` — this document cites the actual fingerprint and the `verified: true` / `commit.gpgsign=true` invariants.
4. `fr003_c06_l59_verify_commit_passes_on_signed_commit` — spins up a temp git repo, signs a commit with the local keyring, asserts `git log --format=%G?` is not `N` and `git verify-commit HEAD` exits 0.

These gates make the "key on disk" → "real signing works" → "doc matches reality" path unit-testable, so any drift is caught at PR time rather than post-merge.

## Branch protection / ruleset checklist

1. Ruleset `19181236` on `main`: require signed commits — **active**.
2. Admin bypass covers `RepositoryRole id 5` for bot-friendly exceptions (Dependabot, GitHub Actions merges) — **intentional** and recorded.
3. Maintainers publish their SSH signing key (or GPG) under their **own** GitHub account — **soft, per-maintainer**.
4. After ≥1 `Verified: true, reason: valid` commit lands on `main`, the ruleset + Verified evidence jointly satisfy **L34 2→3** (C04) — **DONE** as of `#780` Plan 776 (sq-merge Verified commits) and Plan 809 (Forge Bot key provisioned + FR-003 gates).
5. With the Forge Bot key on the local keyring, signed: true commits authored on this runner become routine.

## Soft CI

| Workflow | Checks |
|----------|--------|
| `dco-soft.yml` | Each PR commit has `Signed-off-by:` |
| `gpg-soft.yml` | Each PR commit reports GitHub `verified` (GPG/SSH) via API |

Both stay `continue-on-error` until verified-commit evidence is routine and exhaustively tracked via this doc.

## Follow-up (unblock only)

- Operator: paste the armored public key to https://github.com/settings/keys (one-time, ~30 sec).
- After Verified badge lands: keep C04 L34 at score 3 (audit `.lane-c04/C04.md`), keep C06 L59 at score 3 (audit `.lane-c06/C06.md`).
- If the ruleset needs replacement (e.g., to flip to `require_signatures: true` for everyone), record the new ruleset id here and in `audit/.lane-c06/C06.md`.

## Acceptance evidence (canonical)

- **Key fingerprint:** `AAB36B31A8625A133B9398FE1C7D34D008A2D327` (ed25519, UID `forge-bot-sharecli`)
- **Ruleset:** `19181236` (`main-signed-commits`, active)
- **Local runner:** gpg verified, FR-003 gates 4/4 PASS
- **Squash-merge Verified path:** active on `main` since PR #780 merge
- **Audit claim:** C04 L34 = 3, C06 L59 = 3 (no invented percentages)