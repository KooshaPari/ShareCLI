#!/usr/bin/env python3
"""Native Linux resource-pressure fixture for ShareCLI mature recovery.

This intentionally avoids OOM. It calibrates one child process, defines a
synthetic policy envelope between two- and four-child aggregate RSS, then
compares naive all-at-once with bounded concurrency using the same work.
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
        with open(f"/proc/{pid}/status", "r", encoding="utf-8") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1]) * 1024
    except FileNotFoundError:
        # The process can exit between poll() and opening /proc/<pid>/status.
        # That is an observation race, not a zero-RSS measurement.
        return None
    # /proc status can outlive or race process teardown and may omit VmRSS
    # during that transition. Preserve this as an unknown observation; callers
    # sample repeatedly and never reinterpret None as a zero-RSS measurement.
    return None


def child(mib: int, seconds: float) -> int:
    buf = bytearray(mib * 1024 * 1024)
    for i in range(0, len(buf), 4096):
        buf[i] = 1
    print("READY", flush=True)
    time.sleep(seconds)
    return 0


@dataclass
class BatchResult:
    peak_rss_bytes: int
    elapsed_ms: int
    completed: int


def spawn_one(mib: int, seconds: float) -> subprocess.Popen[str]:
    return subprocess.Popen(
        [sys.executable, __file__, "--child", "--mib", str(mib), "--seconds", str(seconds)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def wait_ready(p: subprocess.Popen[str]) -> None:
    assert p.stdout is not None
    line = p.stdout.readline().strip()
    if line != "READY":
        stderr = p.stderr.read() if p.stderr else ""
        raise RuntimeError(f"child failed readiness: {line!r} {stderr}")


def sample_group(group: list[subprocess.Popen[str]]) -> int:
    total = 0
    for p in group:
        if p.poll() is None:
            rss = rss_bytes(p.pid)
            if rss is not None:
                total += rss
    return total


def run_batches(total: int, concurrency: int, mib: int, seconds: float) -> BatchResult:
    start = time.monotonic()
    peak = 0
    completed = 0
    remaining = total
    running: list[subprocess.Popen[str]] = []
    try:
        while completed < total:
            while remaining > 0 and len(running) < concurrency:
                p = spawn_one(mib, seconds)
                wait_ready(p)
                running.append(p)
                remaining -= 1

            while True:
                peak = max(peak, sample_group(running))
                finished = [p for p in running if p.poll() is not None]
                if finished:
                    for p in finished:
                        if p.returncode != 0:
                            stderr = p.stderr.read() if p.stderr else ""
                            raise RuntimeError(f"child exit {p.returncode}: {stderr}")
                        running.remove(p)
                        completed += 1
                    break
                time.sleep(0.01)
    finally:
        for p in running:
            if p.poll() is None:
                p.terminate()
        for p in running:
            try:
                p.wait(timeout=1)
            except subprocess.TimeoutExpired:
                p.kill()
    return BatchResult(peak, int((time.monotonic() - start) * 1000), completed)


def calibrate(mib: int) -> int:
    p = spawn_one(mib, 0.30)
    try:
        wait_ready(p)
        peak = 0
        while p.poll() is None:
            rss = rss_bytes(p.pid)
            if rss is not None:
                peak = max(peak, rss)
            time.sleep(0.01)
        if p.returncode != 0:
            raise RuntimeError("calibration child failed")
        return peak
    finally:
        if p.poll() is None:
            p.kill()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--child", action="store_true")
    ap.add_argument("--mib", type=int, default=12)
    ap.add_argument("--seconds", type=float, default=0.20)
    args = ap.parse_args()

    if args.child:
        return child(args.mib, args.seconds)

    if not sys.platform.startswith("linux") or not os.path.isdir("/proc"):
        print(json.dumps({"status": "SKIPPED", "reason": "Linux /proc required"}))
        return 0

    single = calibrate(args.mib)
    envelope = math.ceil(single * 2.75)

    naive = run_batches(total=4, concurrency=4, mib=args.mib, seconds=args.seconds)
    bounded = run_batches(total=4, concurrency=2, mib=args.mib, seconds=args.seconds)

    receipt = {
        "fixture": "sharecli-native-memory-pressure-v1",
        "single_child_calibrated_rss_bytes": single,
        "synthetic_policy_envelope_bytes": envelope,
        "work_items": 4,
        "naive": naive.__dict__,
        "bounded": bounded.__dict__,
        "naive_violates_envelope": naive.peak_rss_bytes > envelope,
        "bounded_violates_envelope": bounded.peak_rss_bytes > envelope,
        "interpretation": "bounded concurrency may increase elapsed time; acceptance requires envelope safety, not naive makespan dominance",
    }
    print(json.dumps(receipt, indent=2))

    if naive.completed != 4 or bounded.completed != 4:
        return 2
    if not receipt["naive_violates_envelope"]:
        return 3
    if receipt["bounded_violates_envelope"]:
        return 4
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
