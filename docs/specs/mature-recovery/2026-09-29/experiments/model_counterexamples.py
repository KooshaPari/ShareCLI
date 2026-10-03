#!/usr/bin/env python3
"""Source-derived counterexamples, NOT execution of ShareCLI's Rust binaries.

Requires Python 3.10+ and git. Creates only a disposable local Git fixture.
Prints inspectable JSON, fails if a claimed counterexample cannot be reproduced.
"""
from __future__ import annotations
import datetime as dt
import hashlib
import json
import pathlib
import platform
import shutil
import subprocess
import tempfile
import uuid

SOURCE = "4f01d0199e82b62bcf20399afcc102f58a10ad07"

def git(cwd: pathlib.Path, *args: str) -> bytes:
    return subprocess.run(["git", "-C", str(cwd), *args], check=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=15).stdout

def key(cwd: pathlib.Path, argv: list[str]) -> tuple[str, str, str]:
    # Exact ASCII-input translation of cache_key.rs Git mode. No input digest.
    status = git(cwd, "status", "--porcelain")
    head = git(cwd, "rev-parse", "HEAD")
    payload = b"".join(x.encode() + b"\0" for x in argv)
    payload += b"\x01" + str(cwd).encode() + b"\x01" + status + head
    return hashlib.sha256(payload).hexdigest(), status.decode(), head.decode().strip()

def effective_rank(base: int, milliseconds: int) -> int:
    # Rust `(waited.as_millis() / 1_000) as u8` truncates BEFORE saturating_add.
    return min(255, base + ((milliseconds // 1000) % 256))

def main() -> None:
    if not shutil.which("git"):
        raise SystemExit("UNAVAILABLE: git is required; no experiment result")
    with tempfile.TemporaryDirectory(prefix="sharecli-eq-") as td:
        cwd = pathlib.Path(td)
        git(cwd, "init", "-q")
        git(cwd, "config", "user.name", "Recovery fixture")
        git(cwd, "config", "user.email", "fixture@example.invalid")
        f = cwd / "input.txt"
        f.write_text("base\n")
        git(cwd, "add", "input.txt")
        git(cwd, "commit", "-qm", "fixture baseline")
        observations = []
        for text in ["first edit\n", "other edit\n"]:
            f.write_text(text)
            k, status, head = key(cwd, ["cat", "input.txt"])
            output = subprocess.run(["cat", "input.txt"], cwd=cwd, check=True,
                                    capture_output=True, timeout=5).stdout
            observations.append({"key": k, "status": status, "head": head,
                                 "actual_output": output.decode(),
                                 "actual_output_sha256": hashlib.sha256(output).hexdigest()})
        assert observations[0]["key"] == observations[1]["key"]
        assert observations[0]["actual_output"] != observations[1]["actual_output"]
        unchanged_key = key(cwd, ["cat", "input.txt"])[0]
        assert unchanged_key == observations[1]["key"]
        git(cwd, "add", "input.txt")
        git(cwd, "commit", "-qm", "new baseline")
        after_commit_key = key(cwd, ["cat", "input.txt"])[0]
        assert after_commit_key != unchanged_key
        ranks = {str(s): effective_rank(0, s*1000) for s in [0, 2, 255, 256, 257]}
        assert ranks["256"] == 0 and ranks["255"] == 255
        tickets = ["02.1790700000.1234.2", "02.1790700000.1234.10"]
        assert min(tickets) == tickets[1]
        result = {
            "run_id": str(uuid.uuid4()),
            "observed_at": dt.datetime.now(dt.timezone.utc).isoformat(),
            "verifier": "source-derived-model-counterexamples",
            "verifier_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
            "product": "KooshaPari/ShareCLI", "source_snapshot": SOURCE,
            "candidate_binaries_executed": False,
            "classification": "MODEL_COUNTEREXAMPLES_NOT_PRODUCT_ACCEPTANCE",
            "environment": {"python": platform.python_version(), "os": platform.platform(),
                            "git": subprocess.check_output(["git", "--version"], text=True).strip()},
            "experiments": {
                "SC-EXP-001": {"subject": "crates/sharecli-ipc/src/cache_key.rs Git mode",
                               "source_blob": "4ade430be6d82243b5d4059bdfb22fb52c1d5b1d",
                               "result": "COUNTEREXAMPLE_REPRODUCED", "observations": observations,
                               "positive_controls": {"unchanged_input_same_key": True, "new_commit_changes_key": True},
                               "limitation": "Real git and cat, Python translation of key function; not a Rust cache-hit end-to-end run."},
                "SC-EXP-002": {"subject": "queue.rs effective_rank cast",
                               "result": "COUNTEREXAMPLE_REPRODUCED", "critical_rank_by_wait_seconds": ranks,
                               "limitation": "Exact arithmetic model; no live queue fairness or crash recovery established."},
                "SC-EXP-003": {"subject": "queue.rs ticket-name tie ordering",
                               "result": "COUNTEREXAMPLE_REPRODUCED", "arrival_order": tickets,
                               "lexical_winner": min(tickets),
                               "limitation": "Same PID, second, priority and effective-rank tie; not a concurrent scheduler run."}
            },
            "product_gate": "BLOCKED",
            "native_regressions": "NOT_RUN"
        }
        print(json.dumps(result, indent=2))

if __name__ == "__main__":
    main()
