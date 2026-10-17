#!/usr/bin/env python3
"""Validate a mature-recovery evidence receipt."""

import json, re, sys
from pathlib import Path

SHA = re.compile(r"^[0-9a-f]{40}$")
RESULTS = {"PASS","FAIL","EXPECTED_FAIL","SKIPPED","NOT_RUN","COLLECTOR_FAILURE","STALE","WRONG_CANDIDATE","WRONG_CONFIGURATION","INVALID_ORACLE","UNKNOWN"}
GREEN = {"PASS"}

def die(m):
    print("ERROR:", m, file=sys.stderr)
    raise SystemExit(1)

def main(path):
    d=json.loads(Path(path).read_text())
    required=["schema","evidence_id","product","subject","criterion","contract_revision","candidate_revision","configuration","environment","verifier","verifier_revision","evaluation_run","timestamp","result","provenance"]
    for k in required:
        if k not in d or d[k] in (None,"",[]):
            die(f"missing required {k}")
    if d["schema"]!="mature-recovery-evidence/v1":
        die("unsupported schema")
    for k in ("candidate_revision","verifier_revision"):
        if not SHA.fullmatch(d[k]):
            die(f"{k} must be exact git SHA")
    if d["result"] not in RESULTS:
        die("invalid result")
    if d.get("green") is True and d["result"] not in GREEN:
        die(f"{d['result']} cannot be green")
    if d["result"]=="PASS":
        if not d.get("raw_artifacts"):
            die("PASS requires raw_artifacts")
        if d.get("candidate_matches_subject") is not True:
            die("PASS requires candidate_matches_subject=true")
        if d.get("oracle_executed") is not True:
            die("PASS requires oracle_executed=true")
    if d["result"] in {"SKIPPED","NOT_RUN","COLLECTOR_FAILURE","STALE","WRONG_CANDIDATE","WRONG_CONFIGURATION","INVALID_ORACLE","UNKNOWN"} and d.get("satisfies_criterion") is True:
        die(f"{d['result']} cannot satisfy criterion")
    print(json.dumps({"valid":True,"evidence_id":d["evidence_id"],"result":d["result"],"green":d.get("green",False)},indent=2))

if __name__=="__main__":
    if len(sys.argv)!=2: die("usage: validate_evidence.py <receipt.json>")
    main(sys.argv[1])
