# GitHub Support Ticket — Actions Webhook Missing

## Subject
Push/PR events not triggering Actions workflows — internal webhook appears missing

## Repository
KooshaPari/sharecli (private)

## Description
GitHub Actions workflows stopped triggering on `push` and `pull_request` events. The internal Actions webhook appears to be missing from the repository.

**Evidence:**

1. **Actions is enabled** — `GET /repos/KooshaPari/sharecli/actions/permissions` returns `{"enabled": true, "allowed_actions": "all"}`

2. **Workflows are active** — `GET /repos/KooshaPari/sharecli/actions/workflows` shows all workflows in `state: "active"`

3. **Zero webhooks configured** — `GET /repos/KooshaPari/sharecli/hooks` returns `[]` (empty array). The internal GitHub Actions webhook that delivers push/PR events appears to be missing.

4. **Push events silently dropped** — 13 commits were pushed to `main` on 2026-09-16 but no `push`-event workflow runs were created. The last push-triggered runs were on 2026-09-15.

5. **Dependabot still works** — Dependabot `dynamic` event runs continue to execute (different internal delivery path, not webhook-dependent).

6. **Workflows don't have `workflow_dispatch`** — Cannot manually re-trigger to verify workflow logic is still correct.

## Impact
- CI does not run on any push or PR
- Code quality gates (lint, test, type-check) are bypassed
- Branch protection checks that depend on Actions status are effectively disabled

## Request
Please investigate why the internal Actions webhook is missing from this repository and restore it. Alternatively, advise on how to re-enable push/PR event delivery to Actions.
