#!/usr/bin/env python3
"""Validate mature-recovery trace records.

This validator intentionally does not infer trace edges from filenames, FR tags,
or commit annotations.
"""
import json, sys
from pathlib import Path

REQUIRED={"trace_id","obligation","decision","implementation","oracle","evidence_state","authority"}
BAD_GREEN={"NOT_RUN","SKIPPED","STALE","WRONG_CANDIDATE","UNKNOWN","COLLECTOR_FAILURE"}

def die(m):
    print("ERROR:",m,file=sys.stderr); raise SystemExit(1)

def main(path):
    d=json.loads(Path(path).read_text())
    rows=d.get("traces")
    if d.get("schema")!="mature-recovery-trace/v1" or not isinstance(rows,list):
        die("invalid trace manifest")
    ids=set()
    for r in rows:
        missing=REQUIRED-set(r)
        if missing: die(f"{r.get('trace_id','?')}: missing {sorted(missing)}")
        if r["trace_id"] in ids: die(f"duplicate trace_id {r['trace_id']}")
        ids.add(r["trace_id"])
        if not r["obligation"] or not r["implementation"] or not r["oracle"]:
            die(f"{r['trace_id']}: relation-less trace")
        if r.get("satisfies") is True and r["evidence_state"] in BAD_GREEN:
            die(f"{r['trace_id']}: {r['evidence_state']} cannot satisfy")
        if r["authority"]=="generated_catalog" and r.get("satisfies") is True:
            die(f"{r['trace_id']}: generated catalog cannot establish acceptance")
    print(json.dumps({"valid":True,"trace_count":len(rows)},indent=2))

if __name__=="__main__":
    if len(sys.argv)!=2: die("usage: validate_trace.py <trace.json>")
    main(sys.argv[1])
