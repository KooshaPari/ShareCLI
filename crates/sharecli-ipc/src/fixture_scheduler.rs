use super::*;

fn demand(id:&str,cpu:f64,mem:u64,priority:i64)->(WorkItem,ResourceVector){
    (WorkItem{id:id.into(),dependencies:vec![],priority,estimated_duration_ms:None,estimate_provenance:None},
     ResourceVector{cpu:Some(cpu),memory_bytes:Some(mem),gpu_count:Some(0),vram_bytes:Some(0),disk_bytes:Some(0),io_weight:Some(0.0),capabilities:vec![]})
}

fn fits(d:&ResourceVector,free_cpu:f64,free_mem:u64)->bool{
    d.cpu.is_some_and(|v|v<=free_cpu)&&d.memory_bytes.is_some_and(|v|v<=free_mem)
}

#[test]
fn deterministic_fixture_requires_queue_when_naive_concurrency_exceeds_envelope(){
    let (_a,ra)=demand("a",3.0,6,0);
    let (_b,rb)=demand("b",3.0,6,0);
    let (_c,rc)=demand("c",2.0,4,0);
    let cap_cpu=4.0; let cap_mem=8;
    assert!(ra.cpu.unwrap()+rb.cpu.unwrap()>cap_cpu);
    assert!(ra.memory_bytes.unwrap()+rb.memory_bytes.unwrap()>cap_mem);
    assert!(fits(&ra,cap_cpu,cap_mem));
    assert!(fits(&rc,cap_cpu,cap_mem));
}

#[test]
fn simple_best_fit_fixture_never_exceeds_hard_envelope(){
    let items=vec![demand("a",3.0,6,0),demand("b",1.0,2,0),demand("c",2.0,4,0)];
    let cap_cpu=4.0; let cap_mem=8;
    let mut free_cpu=cap_cpu; let mut free_mem=cap_mem; let mut admitted=vec![]; let mut queued=vec![];
    for (w,r) in items {
        if fits(&r,free_cpu,free_mem){
            free_cpu-=r.cpu.unwrap(); free_mem-=r.memory_bytes.unwrap(); admitted.push(w.id);
        } else { queued.push(w.id); }
    }
    assert_eq!(admitted,vec!["a","b"]);
    assert_eq!(queued,vec!["c"]);
    assert!(free_cpu>=0.0 && free_mem<=cap_mem);
}

#[test]
fn unknown_demand_is_not_admitted_by_fixture(){
    let r=ResourceVector::unknown();
    assert!(!fits(&r,4.0,8));
}
