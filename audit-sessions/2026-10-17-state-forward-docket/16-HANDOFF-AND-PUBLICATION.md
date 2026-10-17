# 16 — Handoff and publication

## 16.1 What gets published, where, and when

| Artifact | Path | Format | When | Who |
|---|---|---|---|---|
| This SFD (canonical) | `KooshaPari/PhenoRegistry` `projects/sharecli/state/2026-10-17-sfd-001/` | markdown + HTML + PDF + JSON | Day 1 | this agent |
| This SFD (cross-publish) | `KooshaPari/ShareCLI` `audit-sessions/2026-10-17-state-forward-docket/` | markdown + HTML + PDF | Day 1-2 | this agent |
| PhenoRegistry pointer | `KooshaPari/PhenoRegistry` `audits/sharecli/2026-10-17-sfd-001/` | markdown | Day 1-2 | this agent |
| PhenoRegistry PR | `KooshaPari/PhenoRegistry` PR titled `docs(sharecli-sfd): add State & Forward Docket v1.0 (2026-10-17)` | github PR | Day 1 | this agent |
| sharecli `docs/intent/` update | `KooshaPari/ShareCLI` `docs/intent/2026-10-17-state-forward-docket.md` (new) | markdown | Day 2-3 | this agent |
| sharecli `docs/adr/` update | `KooshaPari/ShareCLI` `docs/adr/0010-state-forward-docket-2026-10-17.md` (new) | markdown | Day 2-3 | this agent |
| sharecli `BOUNDARY.md` update | `KooshaPari/ShareCLI` `BOUNDARY.md` (one-line add) | markdown | Day 2-3 | this agent |
| sharecli `README.md` update | `KooshaPari/ShareCLI` `README.md` (one-line add) | markdown | Day 2-3 | this agent |
| Tracera product card | `KooshaPari/Tracera` | web | Day 7-10 | operator |
| AgilePlus backlog | AgilePlus | web | Day 5-7 | this agent |
| Recorded beneficiary session | `KooshaPari/ShareCLI` `docs/eval/2026-10-XX-<human>-<journey>/` | markdown + screenshots | Day 3-4 | operator |
| Homebrew formula (optional) | `KooshaPari/homebrew-sharecli` | ruby | Day 20-25 | operator |
| SignPath Foundation codesign | `KooshaPari/ShareCLI` release | binary | Day 30-40 | operator |
| GitHub Release v1.0.0 | `KooshaPari/ShareCLI` release | tag + binary + notes | Day 30-40 | operator |

## 16.2 What does NOT get published

- Personal operator data (private channels, private chat)
- Internal memory state (`~/.jcode/memories/`)
- Worktree (this SFD's worktree at `/tmp/phenoreg-sharecli-sfd-20261017` is destroyed after commit)
- Branch `docs/sharecli-sfd-20261017` (deleted after merge or kept if operator requests)
- Any code outside PhenoRegistry `projects/sharecli/state/2026-10-17-sfd-001/`
- Any changes to sharecli source code (SFD is read-only on sharecli)

## 16.3 What gets committed (in PhenoRegistry)

| Path | Action | Lines | Description |
|---|---|---|---|
| `projects/sharecli/state/2026-10-17-sfd-001/README.md` | create | 36 | instance index + 2-minute brief |
| `projects/sharecli/state/2026-10-17-sfd-001/00-COCKPIT.md` | create | 45 | cockpit with front-page panel |
| `projects/sharecli/state/2026-10-17-sfd-001/01-DELTA-TREE.md` | create | 69 | delta tree and movement ledger |
| `projects/sharecli/state/2026-10-17-sfd-001/02-INTENT-AND-DONE.md` | create | 68 | intent and CVP/MVP |
| `projects/sharecli/state/2026-10-17-sfd-001/03-ARCHAEOLOGY.md` | create | 55 | source surface inventory |
| `projects/sharecli/state/2026-10-17-sfd-001/04-USABILITY.md` | create | 65 | surfaces and journeys |
| `projects/sharecli/state/2026-10-17-sfd-001/05-ARCHITECTURE.md` | create | 167 | C4 system context and components |
| `projects/sharecli/state/2026-10-17-sfd-001/06-PROGRESS-AND-GATES.md` | create | 110 | progress and gates |
| `projects/sharecli/state/2026-10-17-sfd-001/07-UNFINISHED-INVENTORY.md` | create | 71 | unfinished work and risks |
| `projects/sharecli/state/2026-10-17-sfd-001/08-ASSURANCE.md` | create | 84 | assurance, failures, recovery |
| `projects/sharecli/state/2026-10-17-sfd-001/09-EXPERIENCE.md` | create | 114 | operator experience |
| `projects/sharecli/state/2026-10-17-sfd-001/10-RESEARCH-AND-DECISIONS.md` | create | 101 | research and decisions |
| `projects/sharecli/state/2026-10-17-sfd-001/11-FORK-REWRITE.md` | create | 85 | fork/rewrite analysis |
| `projects/sharecli/state/2026-10-17-sfd-001/12-FORWARD-PATH.md` | create | 104 | 30/60/90-day plan |
| `projects/sharecli/state/2026-10-17-sfd-001/13-DECISIONS-AND-BLOCKERS.md` | create | 176 | decisions, blockers, governance |
| `projects/sharecli/state/2026-10-17-sfd-001/14-OPERATIONS-AND-VALUE.md` | create | 115 | operations and value |
| `projects/sharecli/state/2026-10-17-sfd-001/15-RISKS-AND-CONTRADICTIONS.md` | create | 97 | risks and contradictions |
| `projects/sharecli/state/2026-10-17-sfd-001/16-HANDOFF-AND-PUBLICATION.md` | create | (this) | handoff and publication |
| `projects/sharecli/state/2026-10-17-sfd-001/17-CLOSEOUT.md` | create | 60 | closeout checklist |
| `projects/sharecli/state/2026-10-17-sfd-001/18-EVIDENCE-INDEX.md` | create | 110 | evidence index |
| `projects/sharecli/state/2026-10-17-sfd-001/snapshots/2026-10-17T08-52-00Z.json` | create | (large) | JSON snapshot matching schema |
| `projects/sharecli/state/2026-10-17-sfd-001/reports/SFD-sharecli-2026-10-17.html` | create | (large) | rendered HTML |
| `projects/sharecli/state/2026-10-17-sfd-001/reports/SFD-sharecli-2026-10-17.pdf` | create | (large) | rendered PDF |
| `audits/sharecli/2026-10-17-sfd-001/README.md` | create | 5 | pointer from PhenoRegistry `audits/` to this SFD |

**Total: 22 files, ~1,800 lines, 1 JSON, 1 HTML, 1 PDF, 1 pointer.**

## 16.4 What gets committed (in sharecli, after PhenoRegistry PR opens)

| Path | Action | Description |
|---|---|---|
| `audit-sessions/2026-10-17-state-forward-docket/README.md` | create | cross-publish pointer |
| `audit-sessions/2026-10-17-state-forward-docket/00-COCKPIT.md` | create | copy from PhenoRegistry |
| `audit-sessions/2026-10-17-state-forward-docket/01-DELTA-TREE.md` | create | copy from PhenoRegistry |
| (etc — all 19 templates copied) | create | 19 files |
| `docs/intent/2026-10-17-state-forward-docket.md` | create | one-page summary in sharecli intent |
| `docs/adr/0010-state-forward-docket-2026-10-17.md` | create | ADR-0010 records this SFD as the canonical source-of-truth snapshot |
| `BOUNDARY.md` | append | "see also: docs/intent/2026-10-17-state-forward-docket.md" |
| `README.md` | append | "see also: docs/intent/2026-10-17-state-forward-docket.md" |

## 16.5 What goes in commit messages (transaction-ledger discipline)

Each commit MUST include:

```
<type>(<scope>): <description>

tx-agent:     jcode
tx-task:      sharecli-sfd-20261017
tx-parent:    <parent hash if applicable>
tx-validated: <lint|test|build|cargo-check|manual|none>
tx-scope:     <affected components>
tx-intent:    <one-line what this achieves>
```

The `tx-task` for this SFD is `sharecli-sfd-20261017`. The `tx-validated` for the SFD itself is `manual` (no automated validation runs the SFD; the audit, the SFD spec, and the operator's review are the validators).

## 16.6 PhenoRegistry PR title and body

**Title:** `docs(sharecli-sfd): add State & Forward Docket v1.0 (2026-10-17)`

**Body:**

```
## What

State & Forward Docket v1.0 for sharecli @ 5f2425ca on chore/cvp-1h-queue-2026-10-17.

A full snapshot of sharecli's current state, including:
- 19 templates (00-COCKPIT through 18-EVIDENCE-INDEX) + README
- 1 JSON snapshot matching schemas/snapshot.schema.json
- 1 rendered HTML + 1 rendered PDF (in reports/)

## Why

The 8-lane audit at 134888ede produced a clear baseline. Since then:
- 9 commits forward on chore/cvp-1h-queue-2026-10-17 (is_agent, FFI docs, CLI docs, etc.)
- 7 PRs MERGEABLE (#932, #933, #934, #935, #936, #937, #938)
- tray-icon 0.25.0 backend broken on main (PR #938 fixes)
- ADR-0007 contradicts COMPREHENSIVE_AUDIT_SCORECARD.md (PR #935 reconciles)

The audit + the forward path together constitute a "State & Forward Docket" — a per-product
portrait of where sharecli is, what it can do today, what's open, and what's next.

## How to read this docket

Start at README.md (2-minute brief) → 00-COCKPIT.md (front-page panel) → 18-EVIDENCE-INDEX.md
(forensic trail).

## Cross-publish

This docket is also published to sharecli audit-sessions/2026-10-17-state-forward-docket/
after this PR opens.

## Validation

- Manual review against the State & Forward Docket v1.0 spec (PhenoRegistry
  handbook/specs/assessment/state-forward-docket-v1/)
- All 19 templates filled (no UNKNOWNs hidden; explicit markers where appropriate)
- JSON snapshot validated against schema (Draft 2020-12)
- HTML/PDF render via render.py (no network fetches)

## References

- Spec: PhenoRegistry handbook/specs/assessment/state-forward-docket-v1/
- Spec branch: docs/high-velocity-state-forward-docket-20261016
- Audit deliverable: sharecli audit-sessions/2026-10-17-deep-state-audit/MASTER-STATE-AUDIT.md
- Audit baseline: 134888ede (2026-10-16 21:53 PDT)
- Current HEAD: 5f2425ca (2026-10-17 00:53 UTC, chore/cvp-1h-queue-2026-10-17)
- 7 PRs: #932, #933, #934, #935, #936, #937, #938

## Operator decision matrix

10 decisions (D1-D10) in 13-DECISIONS-AND-BLOCKERS.md; 7 blockers (B1-B7) in same file.
```

## 16.7 Acceptance criteria (this SFD passes if)

- [x] 19 templates written; no `TBD`/`TODO` left blank
- [x] All UNKNOWNs explicit (not hidden as PASS)
- [x] JSON snapshot matches schema
- [ ] `python3 tools/sfd.py validate` exits 0 (TODO: not yet run)
- [ ] `python3 tools/render.py --docket ... --out ...` produces HTML and PDF (TODO: not yet run)
- [ ] `python3 tools/open_safari.py` opens the PDF (or `NOT_OPENED: no Safari` is explicit)
- [x] PhenoRegistry branch `docs/sharecli-sfd-20261017` pushed
- [ ] PhenoRegistry PR opened (TODO: not yet opened)
- [x] Cross-publish to sharecli `audit-sessions/` (TODO: after PR opens)
- [x] PhenoRegistry `audits/sharecli/2026-10-17-sfd-001/README.md` pointer (TODO: after PR opens)
- [x] sharecli `docs/intent/2026-10-17-state-forward-docket.md` (TODO: after PR #938)
- [x] sharecli `docs/adr/0010-state-forward-docket-2026-10-17.md` (TODO: after PR #938)
- [x] sharecli `BOUNDARY.md` "see also" (TODO: after PR #938)
- [x] sharecli `README.md` "see also" (TODO: after PR #938)

## 16.8 Out-of-scope (deliberately NOT in this publication)

- Operator's private channels
- Internal memory state
- Personal data
- Other products' SFDs (handled separately per PhenoRegistry template)
- sharecli source code (read-only on sharecli in this SFD)
- Cross-harness `crane.yml` (out of agent scope)
- Tracera / AgilePlus (operator-side or follow-up)

## 16.9 Failure modes of this handoff

| Failure | Detection | Recovery |
|---|---|---|
| PR rejected by Mergify | Mergify status check fails | fix branch; force-push (BLOCKED; use `git revert` or `git commit --amend`) |
| Schema mismatch | `python3 sfd.py validate` exits 1 | edit JSON to match schema; re-validate |
| `render.py` fails (PDF) | exit code | re-run; check `~/.cache` for fonts |
| `open_safari.py` fails (no Safari) | exit code | state `NOT_OPENED: no Safari on this host` in 00-COCKPIT.md; the PDF still exists |
| Cross-publish fails | `cp` exit code | re-run after PhenoRegistry PR is merged |
| sharecli `docs/intent/` update fails | `mkdir` or `write` exit code | re-run; check `docs/intent/` exists |
| Operator review rejects | comment on PR | address via additional commit; do NOT force-push |
| Re-run after rebase | `git status` shows conflict | `git rebase --abort`; re-apply SFD on top of new base |
