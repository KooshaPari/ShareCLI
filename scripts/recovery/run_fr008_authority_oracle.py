#!/usr/bin/env python3
"""Exact-head positive verifier for FR-008 in-flight/durable authority separation."""
import argparse,json,os,re,subprocess,sys,time,hashlib
from pathlib import Path

def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=argparse.ArgumentParser(); p.add_argument("--candidate",required=True); p.add_argument("--output",type=Path,required=True); a=p.parse_args()
a.output.mkdir(parents=True,exist_ok=True)
r={"schema":"sharecli-fr008-authority/v1","candidate":a.candidate,"result":"COLLECTOR_FAILURE","product_accepted":False,"timestamp":time.strftime("%Y-%m-%dT%H:%M:%SZ",time.gmtime()),"run":os.getenv("GITHUB_RUN_ID")}
try:
 actual=subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(); r["tested_checkout"]=actual
 if actual!=a.candidate: raise ValueError("WRONG_CANDIDATE")
 source=Path("crates/sharecli-core/tests/recovery_fr008_inflight_vs_durable.rs")
 expected=re.findall(r"#\[tokio::test[^\]]*\]\s*async fn (\w+)",source.read_text())
 if len(expected)!=2 or len(set(expected))!=2: raise ValueError("expected exactly two positive authority tests")
 r["expected_tests"]=expected; r["sources"]={str(source):sha(source),"crates/sharecli-core/src/lib.rs":sha("crates/sharecli-core/src/lib.rs"),"crates/sharecli-ipc/src/lib.rs":sha("crates/sharecli-ipc/src/lib.rs")}
 cmd=["cargo","test","-p","sharecli-core","--test","recovery_fr008_inflight_vs_durable","--","--nocapture","--test-threads=1"]
 cp=subprocess.run(cmd,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=900)
 (a.output/"cargo-test.log").write_text(cp.stdout); r["exit_code"]=cp.returncode
 observed=re.findall(r"(?m)^test (\w+) \.\.\. (ok|FAILED|ignored)\s*$",re.sub(r"\x1b\[[0-9;]*m","",cp.stdout))
 if cp.returncode!=0 or {n for n,s in observed}!=set(expected) or any(s!="ok" for n,s in observed): raise ValueError("positive authority oracle failed")
 r["result"]="PASS"; code=0
except Exception as e:
 r["error"]=str(e); code=1
r["raw"]={p.name:sha(p) for p in a.output.glob("*.log")}
(a.output/"receipt.json").write_text(json.dumps(r,indent=2)+"\n"); print(json.dumps(r,indent=2)); sys.exit(code)
