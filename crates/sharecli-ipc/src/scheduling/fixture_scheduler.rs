//! Deterministic recovery benchmark for the v1.2 scheduling thesis.
//! This is a reference strategy/oracle, not the production scheduler.

use super::{ResourceVector, WorkItem};

#[derive(Debug, Clone)]
pub struct FixtureWork {
    pub item: WorkItem,
    pub demand: ResourceVector,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave {
    pub ids: Vec<String>,
}

fn fits(cap_cpu:f64, cap_mem:u64, used_cpu:f64, used_mem:u64, d:&ResourceVector)->bool {
    match (d.cpu,d.memory_bytes) {
        (Some(cpu),Some(mem)) => used_cpu+cpu<=cap_cpu && used_mem.saturating_add(mem)<=cap_mem,
        _ => false,
    }
}

/// Deterministic first-fit decreasing by memory, then CPU, then ID.
/// Unknown scalar demand is deferred rather than treated as zero.
pub fn pack_waves(mut work:Vec<FixtureWork>, cap_cpu:f64, cap_mem:u64)->(Vec<Wave>,Vec<String>) {
    work.sort_by(|a,b|{
        b.demand.memory_bytes.unwrap_or(u64::MAX).cmp(&a.demand.memory_bytes.unwrap_or(u64::MAX))
            .then_with(|| b.demand.cpu.unwrap_or(f64::INFINITY).partial_cmp(&a.demand.cpu.unwrap_or(f64::INFINITY)).unwrap())
            .then_with(|| a.item.id.cmp(&b.item.id))
    });
    let mut waves:Vec<(Wave,f64,u64)>=vec![];
    let mut deferred=vec![];
    for w in work {
        if w.demand.cpu.is_none()||w.demand.memory_bytes.is_none(){deferred.push(w.item.id);continue}
        let mut placed=false;
        for (wave,cpu,mem) in &mut waves {
            if fits(cap_cpu,cap_mem,*cpu,*mem,&w.demand){
                *cpu+=w.demand.cpu.unwrap();*mem+=w.demand.memory_bytes.unwrap();
                wave.ids.push(w.item.id.clone());placed=true;break;
            }
        }
        if !placed {
            if !fits(cap_cpu,cap_mem,0.0,0,&w.demand){deferred.push(w.item.id);continue}
            waves.push((Wave{ids:vec![w.item.id]},w.demand.cpu.unwrap(),w.demand.memory_bytes.unwrap()));
        }
    }
    (waves.into_iter().map(|x|x.0).collect(),deferred)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(id:&str,cpu:f64,mem:u64)->FixtureWork{
        FixtureWork{item:WorkItem{id:id.into(),dependencies:vec![],priority:0,estimated_duration_ms:Some(1000),estimate_provenance:Some("fixture".into())},demand:ResourceVector{cpu:Some(cpu),memory_bytes:Some(mem),gpu_count:Some(0),vram_bytes:Some(0),disk_bytes:Some(0),io_weight:Some(0.0),capabilities:vec![]}}
    }

    #[test]
    fn naive_all_at_once_exceeds_fixture_envelope_but_packing_does_not(){
        let work=vec![w("a",2.0,6),w("b",2.0,6),w("c",2.0,4),w("d",2.0,4)];
        let total_mem:u64=work.iter().map(|x|x.demand.memory_bytes.unwrap()).sum();
        assert!(total_mem>10);
        let (waves,deferred)=pack_waves(work,4.0,10);
        assert!(deferred.is_empty());
        assert_eq!(waves.len(),2);
        for wave in waves { assert!(wave.ids.len()<=2); }
    }

    #[test]
    fn unknown_demand_is_deferred(){
        let mut x=w("unknown",1.0,1);x.demand.memory_bytes=None;
        let (_,deferred)=pack_waves(vec![x],4.0,10);
        assert_eq!(deferred,vec!["unknown"]);
    }

    #[test]
    fn impossible_single_item_is_deferred(){
        let (_,deferred)=pack_waves(vec![w("huge",8.0,20)],4.0,10);
        assert_eq!(deferred,vec!["huge"]);
    }
}
