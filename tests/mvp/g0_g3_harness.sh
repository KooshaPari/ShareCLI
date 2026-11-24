#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Red-first acceptance harness for the ShareCLI per-device MVP (gates G0–G3).
#
# Public interface ONLY: every assertion drives the `sharecli` binary the way an
# operator or an agent does (separate OS processes, real config, real signals).
# No internal crate APIs are touched, so this harness stays green only while the
# product property holds, not while a helper function happens to exist.
#
#   G0  baseline        the binary runs and reports a parseable inventory
#   G1  single authority one resident authority owns the pool: a process started
#                        by invocation A is visible to, and killable by,
#                        invocation B
#   G2  lifetime admission a build slot is held for the WHOLE child lifetime, not
#                        just until spawn returns
#   G3  coalescing       (out of MVP slice — recorded NOT_RUN, never PASS)
#
# Exit: 0 = every in-scope gate PASS, 1 = at least one FAIL, 2 = harness fault.
set -uo pipefail

BIN=${SHARECLI_BIN:-sharecli}
WORK=$(mktemp -d "${TMPDIR:-/tmp}/sharecli-harness.XXXXXX") || exit 2
BINPATH="$WORK/bin"
CFG="$WORK/config"
LOGF="$WORK/harness.log"
mkdir -p "$BINPATH" "$CFG/sharecli"
: >"$LOGF"

export XDG_CONFIG_HOME="$CFG"
export SHARECLI_STATE_DIR="$WORK/state"
export SHARECLI_HARNESS_LOG="$LOGF"
export PATH="$BINPATH:$PATH"

now_ms() { python3 -c 'import time;print(int(time.time()*1000))'; }

FAILED=0
declare -a ROWS=()
record() { # gate name status detail
  ROWS+=("$1|$2|$3|$4")
  [ "$3" = "FAIL" ] && FAILED=1
  printf '  %-6s %-22s %-5s %s\n' "$1" "$2" "$3" "$4"
}

# A fake build harness: records its own start/end so overlap is observable from
# outside, without trusting sharecli's reported timestamps.
cat >"$BINPATH/cargo" <<EOF
#!/usr/bin/env bash
echo "START \$(python3 -c 'import time;print(int(time.time()*1000))') \$*" >>"$LOGF"
sleep "\${FAKE_BUILD_SECS:-4}"
echo "END \$(python3 -c 'import time;print(int(time.time()*1000))') \$*" >>"$LOGF"
EOF
chmod +x "$BINPATH/cargo"

cat >"$CFG/sharecli/config.toml" <<'EOF'
[spawn_policy]
nice_level = 0
max_concurrent_builds = 1
use_sccache = false
EOF

echo "== G0 baseline =="
if "$BIN" --version >/dev/null 2>&1; then
  ver=$("$BIN" --version 2>&1 | head -1)
  record G0 version PASS "$ver"
else
  record G0 version FAIL "sharecli --version failed; binary=$BIN"
  echo "harness: cannot continue without a working binary" >&2
  exit 2
fi

if "$BIN" ps --all --json >"$WORK/ps0.json" 2>"$WORK/ps0.err"; then
  if python3 -c 'import json,sys;json.load(open(sys.argv[1]))' "$WORK/ps0.json" 2>/dev/null; then
    record G0 inventory PASS "ps --all --json parses"
  else
    record G0 inventory FAIL "ps --all --json is not JSON: $(head -c 120 "$WORK/ps0.json")"
  fi
else
  record G0 inventory FAIL "ps --all --json exited non-zero: $(head -c 120 "$WORK/ps0.err")"
fi

echo "== G1 single resident authority =="
# Invocation A starts a normal (non-build) harness.
"$BIN" start mvp --harness sleep --cwd "$WORK" -- 25 >"$WORK/startA.out" 2>&1
PID_A=$(grep -oE 'Started process [0-9]+' "$WORK/startA.out" | grep -oE '[0-9]+' | head -1)

if [ -z "${PID_A:-}" ]; then
  record G1 "start reports pid" FAIL "no pid in output: $(head -c 200 "$WORK/startA.out")"
else
  record G1 "start reports pid" PASS "pid=$PID_A"
fi

# Verifier for "ps --all --json lists this pid": PASS only when a JSON object's
# own "pid" field equals the integer. A flattened substring search would let pid
# 12 match 312 (prefix) or a non-pid field such as cmdline, which is the defect
# this gate exists to catch.
ps_has_pid() { # <json-file> <pid>
  python3 - "$1" "$2" <<'PY'
import json,sys
data=json.load(open(sys.argv[1])); pid=int(sys.argv[2])
def has_pid(o):
    if isinstance(o, dict):
        return any((k == "pid" and isinstance(v, int) and v == pid) or has_pid(v)
                   for k, v in o.items())
    if isinstance(o, list):
        return any(has_pid(v) for v in o)
    return False
sys.exit(0 if has_pid(data) else 1)
PY
}

# Negative controls for the verifier itself. These MUST NOT match; if any does,
# a stale entry or a longer pid reports G1 PASS on zero evidence.
printf '{"processes":[{"pid":999,"cmdline":"sleep 1234"}]}' >"$WORK/ps-neg-substring.json"
printf '{"processes":[{"pid":312}]}' >"$WORK/ps-neg-prefix.json"
printf '{"processes":[{"pid":1234}]}' >"$WORK/ps-pos.json"
G1_VERIFIER_BAD=0
ps_has_pid "$WORK/ps-neg-substring.json" 1234 && G1_VERIFIER_BAD=1
ps_has_pid "$WORK/ps-neg-prefix.json" 12 && G1_VERIFIER_BAD=1
ps_has_pid "$WORK/ps-pos.json" 1234 || G1_VERIFIER_BAD=1
if [ "$G1_VERIFIER_BAD" -eq 0 ]; then
  record G1 "pid match is exact" PASS "non-pid and prefix matches rejected; exact pid field accepted"
else
  record G1 "pid match is exact" FAIL "verifier matched a non-pid field or a pid prefix, or missed an exact pid"
fi

# Invocation B — a different OS process — must see it.
if [ -n "${PID_A:-}" ] && "$BIN" ps --all --json >"$WORK/ps1.json" 2>/dev/null; then
  if ps_has_pid "$WORK/ps1.json" "$PID_A"; then
    record G1 "visible across invocations" PASS "pid $PID_A in a ps --all \"pid\" field"
  else
    record G1 "visible across invocations" FAIL "pid $PID_A missing from ps --all (fresh pool per invocation)"
  fi
else
  record G1 "visible across invocations" FAIL "ps --all unavailable"
fi

# Invocation C must actually terminate it.
if [ -n "${PID_A:-}" ]; then
  "$BIN" stop --pid "$PID_A" >"$WORK/stop.out" 2>&1
  sleep 1
  if kill -0 "$PID_A" 2>/dev/null; then
    record G1 "stop reaches the child" FAIL "pid $PID_A still alive after stop: $(head -c 160 "$WORK/stop.out")"
    kill -9 "$PID_A" 2>/dev/null
  else
    record G1 "stop reaches the child" PASS "pid $PID_A terminated"
  fi
fi

echo "== G2 full-lifetime admission (cap=1) =="
: >"$LOGF"
FAKE_BUILD_SECS=4 "$BIN" start mvp --harness cargo --cwd "$WORK" -- build >"$WORK/b1.out" 2>&1 &
B1=$!
sleep 1
FAKE_BUILD_SECS=4 "$BIN" start mvp --harness cargo --cwd "$WORK" -- build >"$WORK/b2.out" 2>&1 &
B2=$!
wait "$B1" "$B2" 2>/dev/null
# Give any queued-but-not-admitted child time to run.
sleep 6

# Classify a build log for the cap=1 lifetime property. Prints
# "<verdict> <first_start> <first_end> <second_start>", where verdict is one of:
#   fail_first_unobserved  build 1 has no complete START/END pair
#   fail_second_missing    build 1 complete, but no second START at all
#   fail_overlap           build 2 started before build 1 ended
#   pass                   build 1 complete and build 2 started at/after its end
# Absent timestamps are printed as 0, but a 0 is never treated as evidence: the
# verdict is decided inside this one function so the checks below and the
# self-tests below exercise identical logic.
g2_verdict() { # <logfile>
  python3 - "$1" <<'PY'
import sys
starts=[];ends=[]
for line in open(sys.argv[1]):
    p=line.split()
    if len(p)>=2 and p[1].isdigit():
        (starts if p[0]=='START' else ends).append(int(p[1]))
starts.sort()
first_start = starts[0] if starts else 0
first_end   = min(ends) if ends else 0
second_start = starts[1] if len(starts)>1 else 0
if first_start == 0 or first_end == 0 or first_end < first_start:
    verdict = "fail_first_unobserved"
elif second_start == 0:
    verdict = "fail_second_missing"
elif second_start < first_end:
    verdict = "fail_overlap"
else:
    verdict = "pass"
print(verdict, first_start, first_end, second_start)
PY
}

# Self-tests for the classifier. Each fixture has a REQUIRED verdict; a
# classifier that cannot reject incomplete or overlapping evidence is the
# defect, so any mismatch here is a harness failure, not a product failure.
g2_selftest() { # <name> <expected-verdict> <logfile>
  got=$(g2_verdict "$3" | awk '{print $1}')
  if [ "$got" = "$2" ]; then
    record G2 "classifier self-test: $1" PASS "verdict=$got"
  else
    record G2 "classifier self-test: $1" FAIL "expected verdict=$2, got '$got'"
  fi
}
: >"$WORK/g2-neg-empty.log"
printf 'START %s build\n' "$(now_ms)" >"$WORK/g2-neg-noend.log"
# The exact pre-fix false PASS: two STARTs, no END at all. Old logic read
# FIRST_END=0, found SECOND_START >= 0, and recorded both rows PASS.
printf 'START 100 build\nSTART 200 build\n' >"$WORK/g2-neg-two-start-noend.log"
printf 'START 100 build\nEND 200 build\nSTART 150 build\nEND 250 build\n' >"$WORK/g2-neg-overlap.log"
printf 'START 100 build\nEND 200 build\nSTART 250 build\nEND 350 build\n' >"$WORK/g2-pos.log"
g2_selftest "empty log rejected" fail_first_unobserved "$WORK/g2-neg-empty.log"
g2_selftest "START without END rejected" fail_first_unobserved "$WORK/g2-neg-noend.log"
g2_selftest "two STARTs without END rejected" fail_first_unobserved "$WORK/g2-neg-two-start-noend.log"
g2_selftest "overlap detected" fail_overlap "$WORK/g2-neg-overlap.log"
g2_selftest "serialised builds accepted" pass "$WORK/g2-pos.log"

read -r G2_VERDICT _ FIRST_END SECOND_START <<<"$(g2_verdict "$LOGF")"

case "${G2_VERDICT:-}" in
  fail_first_unobserved)
    record G2 "first build observed" FAIL "build 1 has no complete START/END pair in $LOGF (cap=1 cannot be asserted on zero evidence)"
    record G2 "second build admitted" FAIL "verdict undefined: build 1 never completed"
    ;;
  fail_second_missing)
    # The second build never ran at all: admission held it out entirely, which is
    # still a FAIL for "queued then admitted", but distinguishable from overlap.
    record G2 "second build admitted" FAIL "second build never executed (cap=1 not released)"
    ;;
  fail_overlap)
    record G2 "no build overlap" FAIL "overlap: build2 started ${SECOND_START} before build1 ended ${FIRST_END} (permit dropped at spawn return)"
    record G2 "second build admitted" PASS "second build ran"
    ;;
  pass)
    record G2 "no build overlap" PASS "build2 started ${SECOND_START} >= build1 end ${FIRST_END}"
    record G2 "second build admitted" PASS "queued then admitted"
    ;;
  *)
    record G2 "verdict computed" FAIL "g2_verdict produced '${G2_VERDICT:-<empty>}'"
    ;;
esac

echo "== G3 coalescing (out of MVP slice) =="
record G3 "agent-call coalescing" NOT_RUN "requires the call-capture path; MVP slice is G1+G2"

echo
echo "== summary =="
printf '%s\n' "${ROWS[@]}" | awk -F'|' '{printf "%-6s %-26s %-8s %s\n",$1,$2,$3,$4}'
cp "$LOGF" "$WORK/harness-lifetime.log" 2>/dev/null
echo
echo "artifacts: $WORK"
[ "$FAILED" -eq 0 ] && { echo "RESULT: PASS"; exit 0; }
echo "RESULT: FAIL"
exit 1
