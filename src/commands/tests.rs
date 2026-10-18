//! Unit tests for the `commands` module decomposition.
//!
//! Tests cover the project-group filter (`filter_by_project`, originally
//! in `mod.rs`) and the `force_kill_requires_confirmation` guard (originally
//! in `mod.rs` next to `stop`). They live here as a sibling `#[path]`-loaded
//! module so that the `commands` module root can stay a thin re-export shell.

#[cfg(test)]
mod project_group_tests {
    use crate::commands::process_ops::force_kill_requires_confirmation;
    use crate::commands::ps::filter_by_project;
    use crate::runtime::{ProcState, ProcessInfo};

    fn make_proc(
        pid: u32,
        name: &str,
        project: Option<&str>,
        harness: Option<&str>,
    ) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cmd: vec![],
            memory_mb: 100,
            start_time: 0,
            cpu_percent: 0.0,
            project: project.map(str::to_string),
            harness: harness.map(str::to_string),
            ppid: None,
            cwd: None,
            env_count: 0,
            state: ProcState::default(),
            disk_read_bytes: None,
            disk_write_bytes: None,
            fd_count: None,
            thread_count: None,
        }
    }

    #[test]
    fn filter_returns_only_matching_project() {
        let procs = vec![
            make_proc(1, "alpha", Some("proj-a"), Some("cargo")),
            make_proc(2, "beta", Some("proj-b"), Some("node")),
            make_proc(3, "gamma", Some("proj-a"), Some("bun")),
            make_proc(4, "delta", None, None),
        ];
        let result = filter_by_project(&procs, "proj-a");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.project.as_deref() == Some("proj-a")));
    }

    #[test]
    fn filter_returns_empty_when_no_match() {
        let procs = vec![
            make_proc(1, "alpha", Some("proj-a"), Some("cargo")),
            make_proc(2, "beta", Some("proj-b"), Some("node")),
        ];
        let result = filter_by_project(&procs, "proj-c");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_ignores_processes_with_no_project() {
        let procs = vec![
            make_proc(1, "untagged", None, None),
            make_proc(2, "tagged", Some("proj-a"), Some("cargo")),
        ];
        let result = filter_by_project(&procs, "proj-a");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pid, 2);
    }

    #[test]
    fn filter_returns_all_when_all_match() {
        let procs = vec![
            make_proc(1, "a", Some("myproj"), Some("cargo")),
            make_proc(2, "b", Some("myproj"), Some("node")),
            make_proc(3, "c", Some("myproj"), Some("bun")),
        ];
        let result = filter_by_project(&procs, "myproj");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn filter_is_case_sensitive() {
        let procs = vec![
            make_proc(1, "a", Some("Proj-A"), Some("cargo")),
            make_proc(2, "b", Some("proj-a"), Some("cargo")),
        ];
        let result = filter_by_project(&procs, "proj-a");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].pid, 2);
    }

    #[test]
    fn filter_on_empty_list_returns_empty() {
        let procs: Vec<ProcessInfo> = vec![];
        let result = filter_by_project(&procs, "any-project");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_preserves_all_fields() {
        let procs = vec![make_proc(42, "my-proc", Some("target"), Some("cargo"))];
        let result = filter_by_project(&procs, "target");
        assert_eq!(result.len(), 1);
        let p = result[0];
        assert_eq!(p.pid, 42);
        assert_eq!(p.name, "my-proc");
        assert_eq!(p.harness.as_deref(), Some("cargo"));
        assert_eq!(p.memory_mb, 100);
    }

    #[test]
    fn force_kill_requires_confirmation_when_force_without_yes() {
        assert!(force_kill_requires_confirmation(true, false));
    }

    #[test]
    fn force_kill_skips_confirmation_when_yes() {
        assert!(!force_kill_requires_confirmation(true, true));
    }

    #[test]
    fn force_kill_skips_confirmation_when_not_force() {
        assert!(!force_kill_requires_confirmation(false, false));
    }

    // --- filter_by_project edge cases ---

    #[test]
    fn filter_by_project_with_multiple_harnesses_same_project() {
        let procs = vec![
            make_proc(1, "a", Some("proj"), Some("cargo")),
            make_proc(2, "b", Some("proj"), Some("node")),
            make_proc(3, "c", Some("proj"), Some("bun")),
        ];
        let result = filter_by_project(&procs, "proj");
        assert_eq!(result.len(), 3);
    }

    // --- force_kill_requires_confirmation edge cases ---

    #[test]
    fn force_kill_both_true_skips_confirmation() {
        assert!(!force_kill_requires_confirmation(true, true));
    }
}
