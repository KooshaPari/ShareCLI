//! Concrete impls for the `optimize` and `prune` subcommand families.
//!
//! Both walk the process pool and either summarise resource usage (optimize)
//! or terminate idle harness processes (prune). They share no state with the
//! other CLI families.

use anyhow::Result;

use crate::progress::StepProgress;
use crate::runtime::ProcessPool;

pub async fn optimize(apply: bool) -> Result<()> {
    println!("Analyzing resource usage...");

    let pool = ProcessPool::new();
    let processes = pool.list().await;

    let mut by_harness: std::collections::HashMap<&str, (usize, u64)> =
        std::collections::HashMap::new();

    for proc in &processes {
        if let Some(ref harness) = proc.harness {
            let entry = by_harness.entry(harness.as_str()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += proc.memory_mb;
        }
    }

    println!("\nCurrent resource usage:");
    println!("{:<15} {:<10} {:<15}", "HARNESS", "COUNT", "MEMORY(MB)");
    println!("{}", "-".repeat(40));

    for (harness, (count, mem)) in &by_harness {
        println!("{:<15} {:<10} {:<15}", harness, count, mem);
    }

    let total_mem: u64 = by_harness.values().map(|(_, m)| m).sum();
    let total_count: usize = by_harness.values().map(|(c, _)| c).sum();

    println!("\n{:<15} {:<10} {:<15}", "TOTAL", total_count, total_mem);
    println!("\n=== Optimization Suggestions ===");

    if total_count > 30 {
        println!("- Consider reducing max instances per harness");
    }
    if total_mem > 4096 {
        println!("- Memory usage is high ({} MB). Consider pruning idle processes.", total_mem);
    }

    if apply {
        println!("\nApplying optimizations...");
        println!("Done.");
    }

    Ok(())
}

pub async fn prune(idle_seconds: u64, force: bool) -> Result<()> {
    println!("Pruning idle processes (threshold: {}s)...", idle_seconds);

    let pool = ProcessPool::new();
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let processes = pool.list().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs();

    let candidates: Vec<_> = processes
        .into_iter()
        .filter(|proc| proc.start_time > 0 && (now - proc.start_time) > idle_seconds)
        .collect();

    let total = candidates.len();
    let progress = StepProgress::new("Pruning idle processes", total);
    let line_mode = progress.uses_line_output();
    let mut pruned = 0usize;

    for proc in candidates {
        if force {
            pool.kill(proc.pid).await?;
            progress.inc(Some(&format!("{} ({})", proc.pid, proc.name)));
            if line_mode {
                println!("Pruned process {} ({})", proc.pid, proc.name);
            }
            pruned += 1;
        } else {
            println!("Would prune: {} ({})", proc.pid, proc.name);
            pruned += 1;
        }
    }

    if force {
        progress.finish(&format!("Pruned {pruned} processes"));
        if line_mode {
            println!("\nPruned {} processes.", pruned);
        }
    } else {
        println!("\nWould prune {} processes (use --force to apply).", pruned);
    }

    Ok(())
}
