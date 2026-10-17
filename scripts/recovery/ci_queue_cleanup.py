#!/usr/bin/env python3
"""Cancel superseded read-only CI for this recovery pair; never qualify a product.

Default is dry-run. --apply is restricted to the exact same-repository PR event.
No branch, release, workflow, log, artifact or required-check configuration is deleted.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

BRANCH = "spec/mature-recovery-2026-09-29"
SCOPES = {"KooshaPari/ShareCLI": 878, "KooshaPari/BytePort": 427}
COMMON = {".github/workflows/ci.yml"}
ALLOW = {
    "KooshaPari/ShareCLI": COMMON | {
        ".github/workflows/live-pool-soft.yml", ".github/workflows/load-soft.yml",
        ".github/workflows/soak-soft.yml", ".github/workflows/visual-soft.yml",
        ".github/workflows/visual.yml", ".github/workflows/rss.yml",
    },
    "KooshaPari/BytePort": COMMON | {
        ".github/workflows/mature-recovery-oracle.yml",
        ".github/workflows/mature-recovery-contract.yml",
        ".github/workflows/mature-recovery-artifact.yml",
        ".github/workflows/mature-recovery-build-prototype.yml",
    },
}
ACTIVE = {"queued", "pending", "in_progress", "waiting", "requested"}
SHA = re.compile(r"[0-9a-f]{40}\Z")


def protected_head(pr: dict, repo: str) -> str:
    if repo not in SCOPES or pr.get("number") != SCOPES[repo]:
        raise ValueError("wrong repository or pull request")
    head = pr.get("head", {})
    sha = head.get("sha", "")
    if (pr.get("state") != "open" or pr.get("merged") or
            head.get("ref") != BRANCH or
            head.get("repo", {}).get("full_name") != repo or
            not SHA.fullmatch(sha)):
        raise ValueError("untrusted, closed, or incomplete recovery PR identity")
    return sha


def eligible(run: dict, repo: str, head: str, own_run: int) -> bool:
    """Selection is necessary but not sufficient: ancestry is checked separately."""
    return bool(
        repo in SCOPES and SHA.fullmatch(head) and
        type(run.get("id")) is int and run["id"] > 0 and run["id"] != own_run and
        run.get("event") == "pull_request" and run.get("head_branch") == BRANCH and
        run.get("head_repository", {}).get("full_name") == repo and
        SHA.fullmatch(run.get("head_sha", "")) and run["head_sha"] != head and
        run.get("path") in ALLOW[repo] and run.get("status") in ACTIVE and
        any(p.get("number") == SCOPES[repo] for p in run.get("pull_requests", []))
    )


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise RuntimeError("unexpected API redirect; token not forwarded")


class API:
    def __init__(self, repo: str, token: str):
        if repo not in SCOPES or not token:
            raise ValueError("scoped repository and token required")
        self.base = f"https://api.github.com/repos/{repo}"
        self.token = token
        self.opener = urllib.request.build_opener(NoRedirect())

    def call(self, path: str, method: str = "GET") -> dict:
        if not path.startswith("/") or ".." in path or method not in {"GET", "POST"}:
            raise ValueError("invalid API request")
        req = urllib.request.Request(
            self.base + path, method=method,
            headers={"Authorization": f"Bearer {self.token}",
                     "Accept": "application/vnd.github+json",
                     "User-Agent": "recovery-ci-queue-cleanup"},
            data=b"" if method == "POST" else None,
        )
        with self.opener.open(req, timeout=20) as response:
            raw = response.read()
            return json.loads(raw) if raw else {}


def cleanup(api, repo: str, own_run: int, apply: bool, max_cancel: int = 200, emit=None) -> dict:
    if repo not in SCOPES or not 0 <= max_cancel <= 200:
        raise ValueError("unsupported repository or cancellation budget")
    emit = emit or (lambda event: None)
    head = protected_head(api.call(f"/pulls/{SCOPES[repo]}"), repo)
    report = {"schema": "recovery-ci-maintenance/v1", "repository": repo,
              "protected_candidate": head, "mode": "apply" if apply else "dry-run",
              "selected": [], "cancel_requested": [], "confirmed_cancelled": [],
              "unconfirmed": [], "listing_complete": False, "head_changed": False,
              "qualification": "NOT_PRODUCT_EVIDENCE"}
    runs = []
    for page in range(1, 11):
        batch = api.call(f"/actions/runs?branch={BRANCH}&per_page=100&page={page}").get("workflow_runs")
        if not isinstance(batch, list):
            raise ValueError("workflow run listing unavailable")
        runs.extend(batch)
        if len(batch) < 100:
            report["listing_complete"] = True
            break
    ancestry = {}
    for run in runs:
        if not eligible(run, repo, head, own_run):
            continue
        sha = run["head_sha"]
        if sha not in ancestry:
            comparison = api.call(f"/compare/{sha}...{head}")
            ancestry[sha] = (comparison.get("status") == "ahead" and
                             comparison.get("merge_base_commit", {}).get("sha") == sha)
        if not ancestry[sha]:
            continue  # Diverged/newer/unknown candidates are never cancelled.
        if len(report["selected"]) >= max_cancel:
            break
        report["selected"].append({"id": run["id"], "sha": sha, "path": run["path"]})
        if not apply:
            continue
        if protected_head(api.call(f"/pulls/{SCOPES[repo]}"), repo) != head:
            report["head_changed"] = True
            break
        fresh = api.call(f"/actions/runs/{run['id']}")
        if (not eligible(fresh, repo, head, own_run) or fresh.get("head_sha") != sha):
            continue
        emit({"event": "cancel_request_started", "repository": repo,
              "run_id": run["id"], "old_candidate": sha, "protected_candidate": head})
        try:
            api.call(f"/actions/runs/{run['id']}/cancel", "POST")
        except urllib.error.HTTPError as exc:
            if exc.code == 409:  # Finished between recheck and cancellation.
                report["unconfirmed"].append(run["id"])
                continue
            raise  # Do not claim permission/provider errors were successful cleanup.
        report["cancel_requested"].append(run["id"])
        emit({"event": "cancel_request_accepted", "run_id": run["id"]})
        after = api.call(f"/actions/runs/{run['id']}")
        key = "confirmed_cancelled" if after.get("status") == "completed" and after.get("conclusion") == "cancelled" else "unconfirmed"
        report[key].append(run["id"])
        emit({"event": key, "run_id": run["id"]})
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    if repo not in SCOPES:
        raise ValueError("repository outside approved recovery pair")
    if args.apply:
        if os.environ.get("GITHUB_EVENT_NAME") != "pull_request":
            raise ValueError("apply requires same-repository recovery pull_request event")
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        protected_head(event.get("pull_request", {}), repo)
    report = cleanup(API(repo, os.environ.get("GH_TOKEN", "")), repo,
                     int(os.environ.get("GITHUB_RUN_ID", "0")), args.apply,
                     emit=lambda event: print(json.dumps(event), flush=True))
    print(json.dumps(report, indent=2))
    Path("ci-queue-cleanup-report.json").write_text(json.dumps(report, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, RuntimeError, KeyError, OSError) as error:
        # No headers/tokens or provider response bodies are emitted.
        print(f"CI maintenance failed: {type(error).__name__}", file=sys.stderr)
        raise SystemExit(1)
