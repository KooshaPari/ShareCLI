//! Deterministic scheduling benchmark simulator.
//! It evaluates scheduling policy mechanics without spawning real workloads.

use crate::scheduling::{ResourceEnvelope, ResourceVector, WorkItem};

#[derive(Debug, Clone)]
pub struct BenchmarkWork {
    pub item: WorkItem,
    pub demand: ResourceVector,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkMetrics {
    pub makespan_ms: u64,
    pub peak_cpu: f64,
    pub peak_memory_bytes: u64,
    pub hard_envelope_violations: u64,
    pub completed: usize,
}

fn demand(work: &BenchmarkWork) -> Option<(f64,u64)> {
    Some((work.demand.cpu?, work.demand.memory_bytes?))
}

pub fn simulate_naive_all_at_once(envelope: &ResourceEnvelope, work: &[BenchmarkWork]) -> BenchmarkMetrics {
    let peak_cpu: f64 = work.iter().filter_map(|w| demand(w).map(|x| x.0)).sum();
    let peak_memory_bytes: u64 = work.iter().filter_map(|w| demand(w).map(|x| x.1)).sum();
    let cap_cpu = envelope.capacity.cpu.unwrap_or(0.0);
    let cap_mem = envelope.capacity.memory_bytes.unwrap_or(0);
    BenchmarkMetrics {
        makespan_ms: work.iter().map(|w| w.duration_ms).max().unwrap_or(0),
        peak_cpu,
        peak_memory_bytes,
        hard_envelope_violations: u64::from(peak_cpu > cap_cpu || peak_memory_bytes > cap_mem),
        completed: work.len(),
    }
}

pub fn simulate_bounded_fifo(envelope: &ResourceEnvelope, work: &[BenchmarkWork]) -> BenchmarkMetrics {
    let cap_cpu = envelope.capacity.cpu.unwrap_or(0.0);
    let cap_mem = envelope.capacity.memory_bytes.unwrap_or(0);
    let mut now = 0u64;
    let mut waiting: Vec<usize> = (0..work.len()).collect();
    let mut running: Vec<(usize,u64)> = Vec::new();
    let mut completed = 0usize;
    let mut peak_cpu = 0.0f64;
    let mut peak_mem = 0u64;

    while completed < work.len() {
        let mut used_cpu = 0.0;
        let mut used_mem = 0u64;
        for (idx,_) in &running {
            if let Some((cpu,mem)) = demand(&work[*idx]) { used_cpu += cpu; used_mem += mem; }
        }

        let mut admitted = Vec::new();
        for (pos,idx) in waiting.iter().enumerate() {
            let Some((cpu,mem)) = demand(&work[*idx]) else { continue };
            if used_cpu + cpu <= cap_cpu && used_mem.saturating_add(mem) <= cap_mem {
                used_cpu += cpu; used_mem += mem;
                running.push((*idx, now + work[*idx].duration_ms));
                admitted.push(pos);
            } else {
                break;
            }
        }
        for pos in admitted.into_iter().rev() { waiting.remove(pos); }

        peak_cpu = peak_cpu.max(used_cpu);
        peak_mem = peak_mem.max(used_mem);

        if running.is_empty() {
            // Unschedulable/unknown work; reference simulator stops rather than pretending success.
            break;
        }

        let next = running.iter().map(|(_,finish)| *finish).min().unwrap();
        now = next;
        let before = running.len();
        running.retain(|(_,finish)| *finish > now);
        completed += before - running.len();
    }

    BenchmarkMetrics {
        makespan_ms: now,
        peak_cpu,
        peak_memory_bytes: peak_mem,
        hard_envelope_violations: 0,
        completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rv(cpu:f64,mem:u64)->ResourceVector{
        ResourceVector{cpu:Some(cpu),memory_bytes:Some(mem),gpu_count:Some(0),vram_bytes:Some(0),disk_bytes:None,io_weight:None,capabilities:vec![]}
    }
    fn wi(id:&str,duration:u64,cpu:f64,mem:u64)->BenchmarkWork{
        BenchmarkWork{
            item:WorkItem{id:id.into(),dependencies:vec![],priority:0,estimated_duration_ms:Some(duration),estimate_provenance:Some("fixture".into())},
            demand:rv(cpu,mem),duration_ms:duration,
        }
    }

    #[test]
    fn same_workload_naive_overcommits_while_bounded_fifo_does_not() {
        let env=ResourceEnvelope{id:"host".into(),observed_at_unix_ms:1,source:"fixture".into(),capacity:rv(4.0,8_000)};
        let work=vec![wi("a",100,2.0,4_000),wi("b",100,2.0,4_000),wi("c",100,2.0,4_000),wi("d",100,2.0,4_000)];
        let naive=simulate_naive_all_at_once(&env,&work);
        let bounded=simulate_bounded_fifo(&env,&work);
        assert_eq!(naive.hard_envelope_violations,1);
        assert_eq!(bounded.hard_envelope_violations,0);
        assert_eq!(bounded.completed,4);
        assert!(bounded.peak_cpu<=4.0);
        assert!(bounded.peak_memory_bytes<=8_000);
        assert_eq!(bounded.makespan_ms,200);
    }

    #[test]
    fn benchmark_does_not_claim_bounded_fifo_is_faster_than_invalid_naive_baseline() {
        let env=ResourceEnvelope{id:"host".into(),observed_at_unix_ms:1,source:"fixture".into(),capacity:rv(2.0,4_000)};
        let work=vec![wi("a",100,2.0,4_000),wi("b",100,2.0,4_000)];
        let naive=simulate_naive_all_at_once(&env,&work);
        let bounded=simulate_bounded_fifo(&env,&work);
        assert!(naive.makespan_ms < bounded.makespan_ms);
        assert_eq!(naive.hard_envelope_violations,1);
        assert_eq!(bounded.hard_envelope_violations,0);
    }
}
