#!/usr/bin/env python3
"""Collect named native counterexamples without treating build failures as proof.

EXPECTED_FAIL means the defect was observed, never product acceptance. A repaired
candidate that passes is COUNTEREXAMPLE_NOT_REPRODUCED and requires review.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import signal
import subprocess
import sys
import time
import uuid
from pathlib import Path

CASES = {
    "recovery_fr008_semantic_identity": {
        "fr008_git_mode_must_not_reuse_output_after_content_changes": "Git mode reused output although input bytes changed with the same HEAD/status shape",
        "fr008_time_mode_must_not_reuse_output_after_file_content_changes": "Time mode reused output although cwd/argv/env stayed constant and input bytes changed",
        "fr008_args_mode_must_not_reuse_output_across_distinct_workspaces": "Args mode reused output across distinct workspaces with identical argv",
        "fr008_git_mode_must_not_ignore_environment_that_changes_output": "Git mode reused output although an execution-relevant environment value changed",
    },
    "recovery_fr008_equivalence_matrix": {
        "args_mode_must_not_share_across_different_workspaces": "Args-mode false green: cwd-independent key reused another workspace's result",
        "git_mode_must_not_share_across_different_environment": "Git-mode false green: environment change reused prior result",
        "time_mode_must_not_replay_nondeterministic_external_state": "Time-mode false green: external input change reused prior result",
    },
    "recovery_fr008_inflight_vs_durable": {
        "concurrent_share_is_useful_but_must_not_imply_later_replay": "current in-flight equivalence must not be treated as authority for later durable replay",
    },
}
LOG_LIMIT = 16 * 1024 * 1024


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def classify(code: int | None, output: str, name: str, marker: str, *, timed_out: bool = False) -> str:
    if timed_out or code is None or code < 0:
        return "COLLECTOR_FAILURE"
    text = re.sub(r"\x1b\[[0-9;]*m", "", output)
    named = re.search(r"(?m)^test " + re.escape(name) + r" \.\.\.", text)
    summaries = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", text)
    if not named or len(summaries) != 1 or not re.search(r"(?m)^running 1 test\s*$", text):
        return "INVALID_ORACLE"
    status, passed, failed, ignored = summaries[0]
    if code == 0 and (status, passed, failed, ignored) == ("ok", "1", "0", "0"):
        return "COUNTEREXAMPLE_NOT_REPRODUCED"
    # A nonzero exit alone is not evidence. Require the named assertion, its
    # test panic, the complete libtest summary, and the expected exit status.
    panic = re.search(r"thread '" + re.escape(name) + r"'(?: \([0-9]+\))? panicked at [^\n]+\n", text)
    if (code == 101 and (status, passed, failed, ignored) == ("FAILED", "0", "1", "0")
            and panic and "assertion `left == right` failed: " + marker in text[panic.end():]):
        return "EXPECTED_FAIL"
    return "INVALID_ORACLE"


def executable_from_build(code: int | None, output: str, target: str) -> Path:
    if code != 0:
        raise ValueError("build did not complete successfully")
    paths = set()
    for line in output.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (event.get("reason") == "compiler-artifact"
                and event.get("target", {}).get("name") == target
                and event.get("profile", {}).get("test") is True
                and event.get("executable")):
            paths.add(Path(event["executable"]).resolve())
    if len(paths) != 1:
        raise ValueError("build must identify exactly one test executable")
    path = paths.pop()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError("test executable is missing or not executable")
    return path


def invoke(argv: list[str], log: Path, timeout: float) -> tuple[int | None, str, bool]:
    with log.open("w", encoding="utf-8") as handle:
        try:
            proc = subprocess.Popen(argv, stdout=handle, stderr=subprocess.STDOUT, start_new_session=True)
        except OSError as exc:
            handle.write(f"collector could not start process: {exc}\n")
            return None, "", False
        timed_out = False
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
    if log.stat().st_size > LOG_LIMIT:
        return None, "collector log exceeded parser limit; raw log preserved", True
    return proc.returncode, log.read_text(encoding="utf-8", errors="replace"), timed_out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=sorted(CASES), required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    receipt = {
        "schema": "mature-recovery-negative-oracle/v1", "product": "KooshaPari/ShareCLI",
        "target": args.target, "requested_candidate": args.candidate,
        "result": "COLLECTOR_FAILURE", "satisfies_criterion": False, "green": False,
        "verifier_sha256": digest(Path(__file__)), "run": os.getenv("GITHUB_RUN_ID") or str(uuid.uuid4()),
        "run_attempt": os.getenv("GITHUB_RUN_ATTEMPT"), "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "environment": {"platform": platform.platform(), "machine": platform.machine(), "python": sys.version},
        "configuration": {"package": "sharecli-core", "profile": "test", "exact": True, "test_threads": 1},
        "cases": [],
    }
    try:
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
        receipt["actual_candidate"] = head
        if not re.fullmatch(r"[0-9a-f]{40}", args.candidate) or head != args.candidate:
            receipt["result"] = "WRONG_CANDIDATE"
            raise ValueError("requested candidate is not the tested checkout")
        dirty = subprocess.check_output(["git", "diff", "HEAD", "--name-only"], text=True)
        if dirty.strip():
            raise ValueError("tracked source differs from the recorded candidate")
        for tool in ("cargo", "rustc"):
            code, output, timed_out = invoke([tool, "--version"], args.output / f"{tool}.log", 30)
            if code != 0 or timed_out:
                raise ValueError(f"required verifier tool unavailable: {tool}")
            receipt["environment"][tool] = output.strip()
        # Zig is a transitive build tool for spawn-core-sys, not the oracle
        # verifier itself. Record it when available, but let the exact cargo
        # build determine whether its absence is actually relevant.
        code, output, timed_out = invoke(["zig", "version"], args.output / "zig.log", 30)
        receipt["environment"]["zig"] = output.strip() if code == 0 and not timed_out else None
        command = ["cargo", "test", "-p", "sharecli-core", "--test", args.target, "--no-run", "--message-format=json"]
        receipt["build_command"] = command
        code, output, timed_out = invoke(command, args.output / "build.log", 900)
        if timed_out:
            raise ValueError("test compilation timed out")
        binary = executable_from_build(code, output, args.target)
        receipt["binary_sha256"] = digest(binary)
        for name, marker in CASES[args.target].items():
            command = [str(binary), "--exact", name, "--test-threads=1", "--nocapture", "--color", "never"]
            code, output, timed_out = invoke(command, args.output / f"{name}.log", 60)
            receipt["cases"].append({"test": name, "command": command, "exit_code": code,
                "timed_out": timed_out, "result": classify(code, output, name, marker, timed_out=timed_out)})
        results = [case["result"] for case in receipt["cases"]]
        receipt["result"] = "EXPECTED_FAIL" if all(r == "EXPECTED_FAIL" for r in results) else "INVALID_ORACLE"
        if results and all(r == "COUNTEREXAMPLE_NOT_REPRODUCED" for r in results):
            receipt["result"] = "COUNTEREXAMPLE_NOT_REPRODUCED"
    except (OSError, ValueError, subprocess.SubprocessError) as exc:
        receipt["collector_error"] = str(exc)
    finally:
        receipt["raw_artifacts"] = {p.name: digest(p) for p in sorted(args.output.glob("*.log"))}
        (args.output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(receipt, indent=2))
    return 0 if receipt["result"] == "EXPECTED_FAIL" else 1


if __name__ == "__main__":
    raise SystemExit(main())
