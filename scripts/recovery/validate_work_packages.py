#!/usr/bin/env python3
"""Validate mature-recovery work-package manifests without third-party deps."""

import json
import re
import sys
from pathlib import Path

ID_RE = re.compile(r"^[A-Z]+-WP-([AB])([0-9]{2})$")
SHA_RE = re.compile(r"^[0-9a-f]{40}$")

def fail(msg: str) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    raise SystemExit(1)

def main(path: str) -> None:
    p = Path(path)
    data = json.loads(p.read_text())
    if data.get("schema") != "mature-recovery-work-packages/v1":
        fail("unsupported schema")
    if not SHA_RE.fullmatch(data.get("source_snapshot", "")):
        fail("source_snapshot must be exact 40-char lowercase git SHA")
    packages = data.get("packages")
    if not isinstance(packages, list) or not packages:
        fail("packages must be non-empty list")

    ids = []
    by_id = {}
    for item in packages:
        pid = item.get("id", "")
        m = ID_RE.fullmatch(pid)
        if not m:
            fail(f"invalid package id: {pid!r}")
        if pid in by_id:
            fail(f"duplicate package id: {pid}")
        if item.get("tier") != m.group(1):
            fail(f"{pid}: tier disagrees with ID")
        if item.get("merge_gate") not in {"READY", "BLOCKED"}:
            fail(f"{pid}: invalid merge_gate")
        if item["tier"] == "A" and item["merge_gate"] != "READY":
            fail(f"{pid}: Tier A must be READY")
        if item["merge_gate"] == "BLOCKED" and not item.get("blocked_by"):
            fail(f"{pid}: BLOCKED package requires blocked_by")
        for field in ("critical", "evidence_required", "depends_on"):
            value = item.get(field)
            if not isinstance(value, list):
                fail(f"{pid}: {field} must be list")
            if len(value) != len(set(value)):
                fail(f"{pid}: duplicate {field}")
        if not item["critical"] or not item["evidence_required"]:
            fail(f"{pid}: critical/evidence_required must be non-empty")
        ids.append(pid)
        by_id[pid] = item

    for pid, item in by_id.items():
        for dep in item["depends_on"]:
            if dep not in by_id:
                fail(f"{pid}: unknown dependency {dep}")
            if dep == pid:
                fail(f"{pid}: self dependency")

    # DAG check.
    visiting, done = set(), set()
    def visit(pid):
        if pid in done:
            return
        if pid in visiting:
            fail(f"dependency cycle involving {pid}")
        visiting.add(pid)
        for dep in by_id[pid]["depends_on"]:
            visit(dep)
        visiting.remove(pid)
        done.add(pid)
    for pid in ids:
        visit(pid)

    # A blocked package must never be transitively treated as ready by ordering.
    print(json.dumps({
        "manifest": str(p),
        "product": data["product"],
        "source_snapshot": data["source_snapshot"],
        "package_count": len(ids),
        "ready": [i for i in ids if by_id[i]["merge_gate"] == "READY"],
        "blocked": [i for i in ids if by_id[i]["merge_gate"] == "BLOCKED"],
        "valid": True
    }, indent=2))

if __name__ == "__main__":
    if len(sys.argv) != 2:
        fail("usage: validate_work_packages.py <work-packages.json>")
    main(sys.argv[1])
