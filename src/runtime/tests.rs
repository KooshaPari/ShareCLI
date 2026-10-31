// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Unit tests for `runtime` — loaded as `crate::runtime::tests` via
//! `#[path = "tests.rs"]` declared in `runtime/mod.rs`. The nested
//! `mod tests` block below creates `runtime::tests::tests`; `use super::*;`
//! pulls in the `crate::runtime::*` aliases from this file's outer scope.

use crate::runtime::*;
use std::time::Instant;
use substrate::ProcessHandle;
use sysinfo::Pid;

mod tests {
    use super::*;

    async fn insert_unowned_handle(pool: &ProcessPool) -> u32 {
        let pid = std::process::id();
        let system = pool.system.read().await;
        let info = ProcessInfo::from_sysinfo(Pid::from_u32(pid), "fixture".into(), &system)
            .expect("test process must be visible");
        drop(system);
        pool.processes.write().await.insert(
            pid,
            ManagedProcess { info, handle: ProcessHandle { id: Default::default(), pid } },
        );
        pid
    }

    #[tokio::test]
    async fn failed_kill_retains_tracking() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        assert!(pool.kill(pid).await.is_err());
        assert!(pool.processes.read().await.contains_key(&pid));
    }

    #[tokio::test]
    async fn failed_kill_all_reports_error_and_retains_tracking() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        let result = pool.kill_all().await;
        assert!(result.is_err(), "failed stop must not acknowledge success");
        assert!(pool.processes.read().await.contains_key(&pid));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_all_removes_successful_children_but_retains_failures() {
        let pool = ProcessPool::new();
        let child = pool.spawn("sleep", &["30".into()], None, None, None).await.unwrap();
        let failed_pid = insert_unowned_handle(&pool).await;
        assert!(pool.kill_all().await.is_err());
        let tracked = pool.processes.read().await;
        assert!(tracked.contains_key(&failed_pid));
        assert!(!tracked.contains_key(&child.pid));
        assert_eq!(tracked.len(), 1);
    }

    #[tokio::test]
    async fn test_process_pool() {
        let pool = ProcessPool::new();

        #[cfg(unix)]
        let (cmd, args) = ("sleep", vec!["1".to_string()]);
        #[cfg(windows)]
        let (cmd, args) = (
            "cmd",
            vec![
                "/C".to_string(),
                "ping".to_string(),
                "127.0.0.1".to_string(),
                "-n".to_string(),
                "2".to_string(),
            ],
        );

        let info = pool.spawn(cmd, &args, None, None, None).await;
        assert!(info.is_ok(), "process-pool spawn failed: {info:?}");

        let list = pool.list().await;
        assert!(!list.is_empty());

        pool.kill_all().await.unwrap();
    }

    // -----------------------------------------------------------------------
    // spawn_capability
    // -----------------------------------------------------------------------

    #[test]
    fn spawn_capability_uses_harness_when_provided() {
        assert_eq!(spawn_capability("cargo", &Some("rustc".into())), "rustc");
    }

    #[test]
    fn spawn_capability_falls_back_to_cmd_name() {
        assert_eq!(spawn_capability("cargo", &None), "cargo");
    }

    #[test]
    fn spawn_capability_empty_harness_returns_empty_string() {
        // Some("") is Some, so unwrap_or_else does NOT trigger the fallback.
        assert_eq!(spawn_capability("node", &Some(String::new())), "");
    }

    // -----------------------------------------------------------------------
    // ProcState conversions
    // -----------------------------------------------------------------------

    #[test]
    fn proc_state_default_is_unknown() {
        let s = ProcState::default();
        assert_eq!(s, ProcState::Unknown);
    }

    #[test]
    fn proc_state_from_sysinfo_variants() {
        // Map every sysinfo ProcessStatus to a ProcState and check known ones.
        use sysinfo::ProcessStatus;
        let pairs = vec![
            (ProcessStatus::Idle, ProcState::Idle),
            (ProcessStatus::Run, ProcState::Run),
            (ProcessStatus::Sleep, ProcState::Sleep),
            (ProcessStatus::Stop, ProcState::Stop),
            (ProcessStatus::Zombie, ProcState::Zombie),
            (ProcessStatus::Tracing, ProcState::Tracing),
            (ProcessStatus::Dead, ProcState::Dead),
        ];
        for (sys, expected) in pairs {
            let state: ProcState = sys.into();
            assert_eq!(state, expected, "mapping for {:?}", sys);
        }
    }

    #[test]
    fn proc_state_is_serializable_round_trip() {
        let states = vec![
            ProcState::Idle,
            ProcState::Run,
            ProcState::Sleep,
            ProcState::Stop,
            ProcState::Zombie,
            ProcState::Tracing,
            ProcState::Dead,
            ProcState::Unknown,
        ];
        for s in states {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: ProcState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, s, "round-trip for {:?}", s);
        }
    }

    #[test]
    fn proc_state_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&ProcState::Run).unwrap(), "\"run\"");
        assert_eq!(serde_json::to_string(&ProcState::Sleep).unwrap(), "\"sleep\"");
        assert_eq!(serde_json::to_string(&ProcState::Zombie).unwrap(), "\"zombie\"");
    }

    // -----------------------------------------------------------------------
    // SharedRuntime
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn shared_runtime_status_initially_empty() {
        let rt = SharedRuntime::new(4);
        let status = rt.status().await;
        assert_eq!(status.node_total, 0);
        assert_eq!(status.node_idle, 0);
        assert_eq!(status.bun_total, 0);
        assert_eq!(status.bun_idle, 0);
        assert_eq!(status.max_per_type, 4);
    }

    #[tokio::test]
    async fn shared_runtime_acquire_unsupported_type_fails() {
        let rt = SharedRuntime::new(4);
        let result = rt.acquire("python").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported harness type"), "got: {err}");
    }

    #[tokio::test]
    async fn shared_runtime_release_unsupported_type_fails() {
        let rt = SharedRuntime::new(4);
        let result = rt.release("python", 123).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unsupported harness type"), "got: {err}");
    }

    #[tokio::test]
    async fn shared_runtime_release_nonexistent_pid_is_ok() {
        let rt = SharedRuntime::new(4);
        // Releasing a PID that doesn't exist should succeed silently.
        assert!(rt.release("node", 99999).await.is_ok());
    }

    #[tokio::test]
    async fn shared_runtime_health_check_empty_pools() {
        let rt = SharedRuntime::new(4);
        let health = rt.health_check().await;
        assert!(health.healthy);
        assert!(health.issues.is_empty());
        assert_eq!(health.node_in_use, 0);
        assert_eq!(health.bun_in_use, 0);
    }

    // -----------------------------------------------------------------------
    // ProjectResources
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn project_resources_get_default_when_unset() {
        let _ = crate::config::init_global();
        let pr = ProjectResources::new();
        let limits = pr.get_limits("nonexistent").await;
        // Default should come from config::global().project_limits
        let cfg = crate::config::global();
        assert_eq!(limits.memory_limit_mb, cfg.project_limits.memory_limit_mb);
        assert_eq!(limits.max_processes, cfg.project_limits.max_processes);
        assert!(limits.cpu_affinity.is_none());
    }

    #[tokio::test]
    async fn project_resources_set_and_get() {
        let pr = ProjectResources::new();
        let custom = ProjectLimits {
            memory_limit_mb: 2048,
            max_processes: 8,
            cpu_affinity: Some(vec![0, 1]),
        };
        pr.set_limits("my-project", custom.clone()).await;
        let got = pr.get_limits("my-project").await;
        assert_eq!(got.memory_limit_mb, 2048);
        assert_eq!(got.max_processes, 8);
        assert_eq!(got.cpu_affinity, Some(vec![0, 1]));
    }

    #[tokio::test]
    async fn project_resources_check_limits_returns_ok() {
        let _ = crate::config::init_global();
        let pr = ProjectResources::new();
        // check_limits should succeed even for empty projects (0 processes <= max_processes)
        let check = pr.check_limits("nonexistent-project").await;
        assert!(check.is_ok());
        let rc = check.unwrap();
        assert!(rc.processes_ok);
        assert!(rc.overall_ok);
    }

    // -----------------------------------------------------------------------
    // ProjectLimits Default
    // -----------------------------------------------------------------------

    #[test]
    fn project_limits_default_uses_config() {
        let _ = crate::config::init_global();
        let limits = ProjectLimits::default();
        let cfg = crate::config::global();
        assert_eq!(limits.memory_limit_mb, cfg.project_limits.memory_limit_mb);
        assert_eq!(limits.max_processes, cfg.project_limits.max_processes);
    }

    // -----------------------------------------------------------------------
    // ProcessInfo construction helpers
    // -----------------------------------------------------------------------

    #[test]
    fn process_info_debug_clone() {
        let info = ProcessInfo {
            pid: 42,
            name: "test-proc".into(),
            cmd: vec!["test".into(), "--flag".into()],
            memory_mb: 128,
            start_time: 1000,
            cpu_percent: 50.5,
            project: Some("alpha".into()),
            harness: Some("cargo".into()),
            ppid: Some(1),
            cwd: Some("/tmp".into()),
            env_count: 3,
            state: ProcState::Run,
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: Some(10),
            thread_count: Some(4),
        };
        let cloned = info.clone();
        assert_eq!(cloned.pid, 42);
        assert_eq!(cloned.name, "test-proc");
        assert_eq!(cloned.cmd, vec!["test", "--flag"]);
        assert_eq!(cloned.memory_mb, 128);
        assert_eq!(cloned.cpu_percent, 50.5);
        assert_eq!(cloned.project, Some("alpha".into()));
        assert_eq!(cloned.harness, Some("cargo".into()));
        assert_eq!(cloned.ppid, Some(1));
        assert_eq!(cloned.cwd, Some("/tmp".into()));
        assert_eq!(cloned.env_count, 3);
        assert_eq!(cloned.state, ProcState::Run);
        assert_eq!(cloned.fd_count, Some(10));
        assert_eq!(cloned.thread_count, Some(4));
    }

    #[test]
    fn process_info_defaults_for_optional_fields() {
        let info = ProcessInfo {
            pid: 1,
            name: "minimal".into(),
            cmd: vec![],
            memory_mb: 0,
            start_time: 0,
            cpu_percent: 0.0,
            project: None,
            harness: None,
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        };
        assert!(info.project.is_none());
        assert!(info.harness.is_none());
        assert!(info.ppid.is_none());
        assert!(info.cwd.is_none());
    }

    // -----------------------------------------------------------------------
    // ProcessPool new + default
    // -----------------------------------------------------------------------

    #[test]
    fn process_pool_new_is_default() {
        let a = ProcessPool::new();
        let b = ProcessPool::default();
        // Both should be constructible. Check list returns empty for fresh pools.
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let la = a.list().await;
            let lb = b.list().await;
            assert!(la.is_empty());
            assert!(lb.is_empty());
        });
    }

    // -----------------------------------------------------------------------
    // ProcessPool system_memory_usage
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn system_memory_usage_returns_nonzero() {
        let pool = ProcessPool::new();
        let (used, total) = pool.system_memory_usage().await;
        // A running system must have >0 total memory.
        assert!(total > 0, "total memory should be non-zero");
        assert!(used <= total, "used should be <= total");
    }

    // -----------------------------------------------------------------------
    // ProcessPool find filters
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn find_all_returns_managed_processes() {
        let pool = ProcessPool::new();
        let results = pool.find(ProcessFilter::All).await;
        // With no spawned processes, should be empty.
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn find_by_project_filters_correctly() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        {
            let mut procs = pool.processes.write().await;
            if let Some(m) = procs.get_mut(&pid) {
                m.info.project = Some("alpha".into());
            }
        }
        // Refresh so sysinfo can see the process.
        pool.refresh().await;
        let alpha = pool.find(ProcessFilter::ByProject("alpha".into())).await;
        assert!(!alpha.is_empty());
        assert!(alpha.iter().all(|p| p.project.as_deref() == Some("alpha")));

        let beta = pool.find(ProcessFilter::ByProject("beta".into())).await;
        assert!(beta.is_empty());
    }

    #[tokio::test]
    async fn find_by_harness_filters_correctly() {
        let pool = ProcessPool::new();
        let pid = insert_unowned_handle(&pool).await;
        {
            let mut procs = pool.processes.write().await;
            if let Some(m) = procs.get_mut(&pid) {
                m.info.harness = Some("cargo".into());
            }
        }
        pool.refresh().await;
        let cargo = pool.find(ProcessFilter::ByHarness("cargo".into())).await;
        assert!(!cargo.is_empty());
        assert!(cargo.iter().all(|p| p.harness.as_deref() == Some("cargo")));

        let node = pool.find(ProcessFilter::ByHarness("node".into())).await;
        assert!(node.is_empty());
    }

    // -----------------------------------------------------------------------
    // PooledProcess defaults
    // -----------------------------------------------------------------------

    #[test]
    fn pooled_process_clone() {
        let pp = PooledProcess {
            pid: 100,
            name: "node".into(),
            in_use: true,
            last_used: Instant::now(),
        };
        let cloned = pp.clone();
        assert_eq!(cloned.pid, 100);
        assert_eq!(cloned.name, "node");
        assert!(cloned.in_use);
    }

    // -----------------------------------------------------------------------
    // RuntimeHealth + PoolStatus + ResourceCheck
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_health_clone() {
        let h = RuntimeHealth {
            healthy: false,
            issues: vec!["oops".into()],
            node_in_use: 2,
            bun_in_use: 1,
        };
        let cloned = h.clone();
        assert!(!cloned.healthy);
        assert_eq!(cloned.issues, vec!["oops"]);
        assert_eq!(cloned.node_in_use, 2);
        assert_eq!(cloned.bun_in_use, 1);
    }

    #[test]
    fn pool_status_clone() {
        let s =
            PoolStatus { node_total: 3, node_idle: 1, bun_total: 2, bun_idle: 0, max_per_type: 5 };
        let cloned = s.clone();
        assert_eq!(cloned.node_total, 3);
        assert_eq!(cloned.bun_idle, 0);
    }

    #[test]
    fn resource_check_clone() {
        let rc = ResourceCheck {
            memory_mb: 100,
            memory_limit_mb: 200,
            memory_ok: true,
            process_count: 5,
            max_processes: 10,
            processes_ok: true,
            overall_ok: true,
        };
        let cloned = rc.clone();
        assert!(cloned.overall_ok);
        assert_eq!(cloned.memory_mb, 100);
    }

    // -----------------------------------------------------------------------
    // EnvGuard drop
    // -----------------------------------------------------------------------

    #[test]
    fn env_guard_restores_previous_value() {
        unsafe {
            let key = "_JCODE_TEST_ENV_GUARD_RESTORE_";
            std::env::remove_var(key);
            {
                let _guard = EnvGuard { key: key.to_string(), prev: None };
                std::env::set_var(key, "temporary");
                assert_eq!(std::env::var(key).unwrap(), "temporary");
            }
            // After drop, variable should be removed (prev was None).
            assert!(std::env::var(key).is_err());
        }
    }

    #[test]
    fn env_guard_restores_previous_value_when_some() {
        unsafe {
            let key = "_JCODE_TEST_ENV_GUARD_RESTORE_SOME_";
            std::env::set_var(key, "original");
            {
                let _guard = EnvGuard { key: key.to_string(), prev: Some("original".to_string()) };
                std::env::set_var(key, "temporary");
                assert_eq!(std::env::var(key).unwrap(), "temporary");
            }
            // After drop, should restore the original value.
            assert_eq!(std::env::var(key).unwrap(), "original");
            std::env::remove_var(key);
        }
    }

    // -----------------------------------------------------------------------
    // ProcessFilter debug
    // -----------------------------------------------------------------------

    #[test]
    fn process_filter_debug_impl() {
        let all = ProcessFilter::All;
        let proj = ProcessFilter::ByProject("x".into());
        let harness = ProcessFilter::ByHarness("y".into());
        assert_eq!(format!("{:?}", all), "All");
        assert_eq!(format!("{:?}", proj), "ByProject(\"x\")");
        assert_eq!(format!("{:?}", harness), "ByHarness(\"y\")");
    }
}
