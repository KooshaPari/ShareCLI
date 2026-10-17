#!/usr/bin/env python3
"""Exact-checkout positive queue oracle. Build errors/zero tests/skips fail closed."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time
from run_negative_oracle import digest, invoke, executable_from_build


def classify_positive(code, text, expected, timed_out=False):
    text = re.sub(r"\x1b\[[0-9;]*m", "", text)
    observed = re.findall(r"(?m)^test (\S+) \.\.\. (ok|FAILED|ignored)\s*$", text)
    summaries = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;", text)
    return (not timed_out and code == 0 and len(expected) > 0
            and len(observed) == len(expected)
            and len({name for name, _ in observed}) == len(expected)
            and {name for name, _ in observed} == set(expected)
            and all(status == "ok" for _, status in observed)
            and summaries == [("ok", str(len(expected)), "0", "0")])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    receipt = {"schema":"sharecli-owned-waiter-qualification/v1", "product":"KooshaPari/ShareCLI",
               "candidate":args.candidate,"result":"COLLECTOR_FAILURE","product_accepted":False,
               "scope":"native queue ownership, Linux local filesystem",
               "run":os.getenv("GITHUB_RUN_ID"),"attempt":os.getenv("GITHUB_RUN_ATTEMPT"),
               "timestamp":time.strftime("%Y-%m-%dT%H:%M:%SZ",time.gmtime()),
               "environment":{"platform":platform.platform(),"python":sys.version},
               "verifier_sha256":digest(Path(__file__)),"cases":[]}
    exit_code = 1
    try:
        actual = subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip()
        receipt["tested_checkout"] = actual
        if actual != args.candidate or not re.fullmatch(r"[0-9a-f]{40}",actual):
            receipt["result"] = "WRONG_CANDIDATE"
            raise ValueError("requested candidate differs from checkout")
        if subprocess.check_output(["git","diff","HEAD","--name-only"],text=True).strip():
            raise ValueError("DIRTY_TRACKED_SOURCE")
        if sys.platform != "linux":
            raise ValueError("WRONG_ENVIRONMENT: Linux required")
        for tool, command in [("cargo",["cargo","--version"]),("rustc",["rustc","--version"]),("zig",["zig","version"])]:
            code, output, timeout = invoke(command,args.output/(tool+".log"),30)
            if code != 0 or timeout: raise ValueError("verifier tool unavailable: "+tool)
            receipt["environment"][tool]=output.strip()
        unit_source=Path("crates/sharecli-ipc/src/queue_tests.rs")
        names=re.findall(r"#\[test\]\s*(?:#\[[^\]]+\]\s*)*fn (\w+)\(",unit_source.read_text())
        if not names or len(names)!=len(set(names)):raise ValueError("invalid queue test inventory")
        targets=[("sharecli_ipc",["--lib"],"queue::tests::",["queue::tests::"+name for name in names]),
                 ("recovery_queue_pid_reuse",["--test","recovery_queue_pid_reuse"],"stale_ticket_with_reused_live_pid_must_not_block_new_waiter",["stale_ticket_with_reused_live_pid_must_not_block_new_waiter"]),
                 ("recovery_queue_crash",["--test","recovery_queue_crash"],"panicking_owner_releases_slot_and_lane_remains_usable",["panicking_owner_releases_slot_and_lane_remains_usable"]),
                 ("recovery_queue_lease_crash",["--test","recovery_queue_lease_crash"],"killed_waiter_releases_ownership_without_ticket_deletion",["killed_waiter_releases_ownership_without_ticket_deletion"])]
        source_paths=[Path("crates/sharecli-ipc/src/queue.rs"),unit_source,Path("Cargo.lock")]
        source_paths += [Path("crates/sharecli-ipc/tests/"+target+".rs") for target,_,_,_ in targets[1:]]
        receipt["sources"]={str(p):digest(p) for p in source_paths}
        for target, selector, test_filter, expected in targets:
            command=["cargo","test","-p","sharecli-ipc",*selector,"--no-run","--message-format=json"]
            code, output, timeout=invoke(command,args.output/(target+"-build.log"),900)
            if timeout: raise ValueError("BUILD_TIMEOUT: "+target)
            binary=executable_from_build(code,output,target)
            command=[str(binary),test_filter,"--test-threads=1","--color","never"]
            if len(expected)==1: command += ["--exact"]
            code, output, timeout=invoke(command,args.output/(target+"-tests.log"),120)
            passed=classify_positive(code,output,expected,timeout)
            receipt["cases"].append({"target":target,"expected_tests":expected,"command":command,
                                      "binary_sha256":digest(binary),"exit_code":code,"timed_out":timeout,
                                      "result":"PASS" if passed else "FAIL"})
            if not passed:
                receipt["result"] = "FAIL" if code == 101 else "INVALID_ORACLE"
                raise ValueError("named positive oracle failed: "+target)
        receipt["result"]="PASS"
        exit_code=0
    except (OSError,ValueError,subprocess.SubprocessError) as exc:
        receipt["error"]=str(exc)
    finally:
        receipt["raw_artifacts"]={p.name:digest(p) for p in sorted(args.output.glob("*.log"))}
        (args.output/"receipt.json").write_text(json.dumps(receipt,indent=2)+"\n")
        print(json.dumps(receipt,indent=2))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
