#!/usr/bin/env python3
"""50-process native admission comparison for ShareCLI mature recovery.

The same subprocess workload is executed twice:
1. naive all-at-once;
2. bounded batches.

Children allocate real memory and wait on a release barrier so the parent can
measure a guaranteed overlap window. The fixture derives its policy envelope
from a calibrated child RSS. It proves hard-envelope safety only; it does not
claim the bounded policy is throughput-optimal.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import time
from dataclasses import dataclass


def rss_bytes(pid: int) -> int | None:
    try:
        with open(f"/proc/{pid}/status", "r", encoding="utf-8") as handle:
            for line in handle:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1]) * 1024
    except FileNotFoundError:
        return None
    return None


def child(mib: int, hold_seconds: float) -> int:
    buf = bytearray(mib * 1024 * 1024)
    for index in range(0, len(buf), 4096):
        buf[index] = 1

    print("READY", flush=True)
    # The parent releases an entire group after all siblings are ready. This
    # creates a deterministic measurement window instead of depending on
    # process-start timing.
    sys.stdin.readline()
    time.sleep(hold_seconds)
    return 0


@dataclass
class RunReceipt:
    process_count: int
    concurrency: int
    peak_rss_bytes: int
    elapsed_ms: int
    completed: int


def spawn_one(mib: int, hold_seconds: float) -> subprocess.Popen[str]:
    return subprocess.Popen(
        [
            sys.executable,
            __file__,
            "--child",
            "--mib",
            str(mib),
            "--hold-seconds",
            str(hold_seconds),
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def wait_ready(proc: subprocess.Popen[str]) -> None:
    assert proc.stdout is not None
    line = proc.stdout.readline().strip()
    if line != "READY":
        stderr = proc.stderr.read() if proc.stderr else ""
        raise RuntimeError(f"child readiness failed: {line!r} {stderr}")


def release(proc: subprocess.Popen[str]) -> None:
    assert proc.stdin is not None
    proc.stdin.write("\n")
    proc.stdin.flush()


def group_rss(group: list[subprocess.Popen[str]]) -> int:
    total = 0
    for proc in group:
        if proc.poll() is None:
            value = rss_bytes(proc.pid)
            if value is not None:
                total += value
    return total


def run_workload(
    *,
    total: int,
    concurrency: int,
    mib: int,
    hold_seconds: float,
) -> RunReceipt:
    started = time.monotonic()
    completed = 0
    peak = 0

    while completed < total:
        count = min(concurrency, total - completed)
        group = [spawn_one(mib, hold_seconds) for _ in range(count)]

        try:
            for proc in group:
                wait_ready(proc)

            # Every child in this batch is alive and blocked on the release
            # barrier, making this a real simultaneous RSS observation.
            for _ in range(4):
                peak = max(peak, group_rss(group))
                time.sleep(0.01)

            for proc in group:
                release(proc)

            for proc in group:
                code = proc.wait(timeout=5)
                if code != 0:
                    stderr = proc.stderr.read() if proc.stderr else ""
                    raise RuntimeError(f"child exit {code}: {stderr}")
                completed += 1
        finally:
            for proc in group:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()

    return RunReceipt(
        process_count=total,
        concurrency=concurrency,
        peak_rss_bytes=peak,
        elapsed_ms=int((time.monotonic() - started) * 1000),
        completed=completed,
    )


def calibrate(mib: int) -> int:
    proc = spawn_one(mib, 0.01)
    try:
        wait_ready(proc)
        peak = 0
        for _ in range(5):
            value = rss_bytes(proc.pid)
            if value is not None:
                peak = max(peak, value)
            time.sleep(0.01)
        release(proc)
        if proc.wait(timeout=5) != 0:
            raise RuntimeError("calibration child failed")
        if peak <= 0:
            raise RuntimeError("calibration produced no positive RSS observation")
        return peak
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--child", action="store_true")
    parser.add_argument("--processes", type=int, default=50)
    parser.add_argument("--bounded-concurrency", type=int, default=5)
    parser.add_argument("--mib", type=int, default=1)
    parser.add_argument("--hold-seconds", type=float, default=0.08)
    args = parser.parse_args()

    if args.child:
        return child(args.mib, args.hold_seconds)

    if not sys.platform.startswith("linux") or not os.path.isdir("/proc"):
        print(json.dumps({"status": "SKIPPED", "reason": "Linux /proc required"}))
        return 0

    if args.processes < 1:
        raise SystemExit("--processes must be positive")
    if not 1 <= args.bounded_concurrency <= args.processes:
        raise SystemExit("--bounded-concurrency must be between 1 and process count")

    calibrated = calibrate(args.mib)

    # Leave substantial headroom above the bounded group while remaining far
    # below the all-at-once population. This avoids inventing a product SLO;
    # the envelope exists only to falsify unsafe admission on the same host.
    envelope = math.ceil(calibrated * (args.bounded_concurrency + 2.5))

    naive = run_workload(
        total=args.processes,
        concurrency=args.processes,
        mib=args.mib,
        hold_seconds=args.hold_seconds,
    )
    bounded = run_workload(
        total=args.processes,
        concurrency=args.bounded_concurrency,
        mib=args.mib,
        hold_seconds=args.hold_seconds,
    )

    receipt = {
        "fixture": "sharecli-native-50-process-envelope-v1",
        "same_workload": True,
        "processes": args.processes,
        "bounded_concurrency": args.bounded_concurrency,
        "child_payload_mib": args.mib,
        "single_child_calibrated_rss_bytes": calibrated,
        "synthetic_policy_envelope_bytes": envelope,
        "naive": naive.__dict__,
        "bounded": bounded.__dict__,
        "naive_violates_envelope": naive.peak_rss_bytes > envelope,
        "bounded_violates_envelope": bounded.peak_rss_bytes > envelope,
        "interpretation": (
            "same real subprocess workload; acceptance is hard-envelope safety "
            "and complete execution, not bounded makespan dominance"
        ),
    }
    print(json.dumps(receipt, indent=2))

    if naive.completed != args.processes or bounded.completed != args.processes:
        return 2
    if not receipt["naive_violates_envelope"]:
        return 3
    if receipt["bounded_violates_envelope"]:
        return 4
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
