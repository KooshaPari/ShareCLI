mod tests {
    use sharecli_fleet::collect_forest_pids;
    use sharecli_fleet::proc_scan::{DetectedAgent, FakeProcSource, ProcSnapshot};

    use crate::commands::proc::*;

    /// Host inventory from a proc source (used by `ps --all` tests).
    fn host_agent_inventory_from_source(
        source: &dyn sharecli_fleet::ProcSource,
    ) -> (Vec<DetectedAgentWatch>, usize) {
        let agents = sharecli_fleet::scan_agents(source);
        let watched = watch_detected_agents(&agents);
        (watched, agents.len())
    }

    #[test]
    fn agent_row_from_watch_formats_rss() {
        let row = agent_row_from_watch(
            &DetectedAgentWatch {
                agent: DetectedAgent { pid: 42, family: "claude", comm: "claude".into() },
                resource: sharecli_fleet::AgentResourceSample {
                    mem_rss_bytes: 52_428_800,
                    fd_count: Some(10),
                },
            },
            &HashMap::from([(42, 'R')]),
        );
        assert_eq!(row.mem_rss, "50M");
        assert_eq!(row.fd_count, Some(10));
        assert_eq!(row.state, "R");
    }

    #[test]
    fn host_inventory_from_fixture() {
        let src = FakeProcSource::new(vec![ProcSnapshot {
            pid: 100,
            ppid: 1,
            comm: "claude".into(),
            cmdline: vec!["claude".into()],
            state: 'R',
        }]);
        let (watched, scanned) = host_agent_inventory_from_source(&src);
        assert_eq!(scanned, 1);
        assert_eq!(watched.len(), 0, "fixture PID is not live on host");
        let _ = watched;
    }

    #[test]
    fn zero_watch_interval_is_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(crate::commands::proc::run(
                false,
                false,
                false,
                Some(0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ))
            .expect_err("watch 0 MUST fail");
        assert!(
            err.to_string().contains(">= 1"),
            "error MUST mention minimum interval; got: {err}"
        );
    }

    #[test]
    fn ndjson_line_includes_ts_and_agents() {
        let line = AgentProcNdjsonLine {
            ts: 1_750_000_000,
            snapshot: AgentProcSnapshot {
                agents: vec![],
                scanned: 0,
                watched: 0,
                gate: sharecli_fleet::GateStatusSnapshot {
                    thermal_pressure: "GREEN".into(),
                    detected_agents: 0,
                    agent_total_rss_bytes: 0,
                    agent_contention: "OK".into(),
                    gate_decision: "ADMIT".into(),
                },
                host_watch: HostResourceWatchJson::default(),
                pool: None,
                status: None,
            },
        };
        let json = serde_json::to_string(&line).expect("serialize");
        assert!(json.contains("\"ts\":1750000000"));
        assert!(json.contains("\"agents\":[]"));
        assert!(json.contains("\"host_watch\""));
    }

    #[test]
    fn ndjson_line_agent_rows_include_state() {
        let line = AgentProcNdjsonLine {
            ts: 1_750_000_000,
            snapshot: AgentProcSnapshot {
                agents: vec![AgentProcRow {
                    pid: 42,
                    family: "claude".into(),
                    comm: "claude".into(),
                    state: "R".into(),
                    mem_rss_bytes: 100,
                    mem_rss: "100B".into(),
                    fd_count: None,
                }],
                scanned: 1,
                watched: 1,
                gate: sharecli_fleet::GateStatusSnapshot {
                    thermal_pressure: "GREEN".into(),
                    detected_agents: 1,
                    agent_total_rss_bytes: 100,
                    agent_contention: "OK".into(),
                    gate_decision: "ADMIT".into(),
                },
                host_watch: HostResourceWatchJson::default(),
                pool: None,
                status: None,
            },
        };
        let json = serde_json::to_string(&line).expect("serialize");
        assert!(
            json.contains("\"state\":\"R\""),
            "NDJSON watch line MUST include agent state (AC-006.37); got: {json}"
        );
    }

    #[test]
    fn build_forest_state_map_includes_child_pids() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot {
                pid: 50,
                ppid: 1,
                comm: "claude".into(),
                cmdline: vec!["claude".into()],
                state: 'S',
            },
            ProcSnapshot {
                pid: 51,
                ppid: 50,
                comm: "node".into(),
                cmdline: vec!["node".into()],
                state: 'R',
            },
        ]);
        let forests = sharecli_fleet::build_agent_forests(&src);
        assert_eq!(collect_forest_pids(&forests), vec![50, 51]);
        let map = build_forest_state_map(&src, &forests);
        assert_eq!(map.get(&51), Some(&'R'));
    }

    #[test]
    fn tree_json_from_fixture() {
        let src = FakeProcSource::new(vec![
            ProcSnapshot { pid: 1, ppid: 0, comm: "init".into(), cmdline: vec![], state: 'R' },
            ProcSnapshot {
                pid: 50,
                ppid: 1,
                comm: "cursor-agent".into(),
                cmdline: vec!["cursor-agent".into()],
                state: 'R',
            },
            ProcSnapshot {
                pid: 51,
                ppid: 50,
                comm: "node".into(),
                cmdline: vec!["node".into()],
                state: 'R',
            },
        ]);
        let forests = sharecli_fleet::build_agent_forests(&src);
        let state_by_pid = HashMap::from([(50, 'R'), (51, 'R')]);
        let snap = AgentTreeSnapshot {
            forests: forests
                .iter()
                .map(|root| agent_tree_node_to_json(root, &state_by_pid))
                .collect(),
            roots: forests.len(),
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            status: None,
        };
        assert_eq!(snap.roots, 1);
        assert_eq!(snap.forests[0].state, "R");
        assert_eq!(snap.forests[0].children.len(), 1);
        assert_eq!(snap.forests[0].children[0].pid, 51);
        assert_eq!(snap.forests[0].children[0].state, "R");
    }

    #[test]
    fn build_proc_detail_missing_pid_fails() {
        let src = FakeProcSource::new(vec![]);
        let err = build_proc_detail(&src, 42).expect_err("missing pid");
        assert!(
            err.to_string().contains("not found"),
            "error MUST mention missing process; got: {err}"
        );
    }

    #[test]
    fn pid_csv_watch_zero_interval_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(crate::commands::proc::run(
                false,
                true,
                false,
                Some(0),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(42),
                None,
            ))
            .expect_err("pid+csv+watch 0 MUST fail");
        assert!(
            err.to_string().contains("--watch") || err.to_string().contains(">= 1"),
            "error MUST mention watch interval; got: {err}"
        );
    }

    #[test]
    fn pid_tree_combo_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(crate::commands::proc::run(
                false,
                false,
                true,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(42),
                None,
            ))
            .expect_err("pid+tree MUST fail");
        assert!(
            err.to_string().contains("--tree") && err.to_string().contains("AC-007.92"),
            "error MUST mention --tree / AC-007.92; got: {err}"
        );
    }

    #[test]
    fn pid_family_sort_limit_combo_rejected() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let err = rt
            .block_on(crate::commands::proc::run(
                false,
                false,
                false,
                None,
                Some("claude".into()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("rss".into()),
                Some(5),
                Some(42),
                None,
            ))
            .expect_err("pid+family+sort+limit MUST fail");
        let msg = err.to_string();
        assert!(msg.contains("--family"), "got: {msg}");
        assert!(msg.contains("--sort"), "got: {msg}");
        assert!(msg.contains("--limit"), "got: {msg}");
        assert!(msg.contains("AC-007.92"), "got: {msg}");
    }

    fn fixture_row(pid: u32, family: &'static str, comm: &str, rss: u64) -> DetectedAgentWatch {
        DetectedAgentWatch {
            agent: DetectedAgent { pid, family, comm: comm.into() },
            resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: Some(0) },
        }
    }

    fn fixture_inventory() -> Vec<DetectedAgentWatch> {
        vec![
            fixture_row(100, "claude", "claude", 50_000_000),
            fixture_row(50, "claude", "claude", 200_000_000),
            fixture_row(75, "claude", "claude", 100_000_000),
            fixture_row(25, "claude", "claude", 75_000_000),
            fixture_row(150, "claude", "claude", 300_000_000),
            fixture_row(10, "claude", "claude", 10_000_000),
        ]
    }

    #[test]
    fn sort_rss_desc_then_limit_caps_rows() {
        // AC-006.41 / AC-006.21: --sort rss --limit 5 returns at most 5 rows, RSS descending.
        let inventory = fixture_inventory();
        let sorted = sort_watched_agents(&inventory, ProcSort::Rss, &HashMap::new());
        let limited = limit_watched_agents(sorted, Some(5));
        assert!(limited.len() <= 5, "--limit 5 MUST cap at 5 rows; got {}", limited.len());
        // Confirm strict descending RSS order across the limited slice.
        let rss_seq: Vec<u64> = limited.iter().map(|r| r.resource.mem_rss_bytes).collect();
        let mut prev = u64::MAX;
        for rss in &rss_seq {
            assert!(*rss <= prev, "RSS MUST be descending; saw {rss} after {prev}");
            prev = *rss;
        }
        // First row MUST be the largest RSS fixture (pid 150 = 300M).
        assert_eq!(limited[0].agent.pid, 150);
    }

    #[test]
    fn sort_pid_asc_then_limit_caps_rows() {
        // AC-006.19 / AC-006.21: --sort pid --limit 3 returns at most 3 rows, PID ascending.
        let inventory = fixture_inventory();
        let sorted = sort_watched_agents(&inventory, ProcSort::Pid, &HashMap::new());
        let limited = limit_watched_agents(sorted, Some(3));
        assert!(limited.len() <= 3, "--limit 3 MUST cap at 3 rows; got {}", limited.len());
        let pid_seq: Vec<u32> = limited.iter().map(|r| r.agent.pid).collect();
        let mut prev = 0u32;
        for pid in &pid_seq {
            assert!(*pid > prev, "PID MUST be ascending; saw {pid} after {prev}");
            prev = *pid;
        }
        assert_eq!(limited[0].agent.pid, 10);
        assert_eq!(limited[1].agent.pid, 25);
        assert_eq!(limited[2].agent.pid, 50);
    }

    #[test]
    fn sort_name_ascending_alphabetical() {
        // AC-006.41: --sort name sorts by COMM alphabetical; PID tie-break ascending.
        let inventory = vec![
            fixture_row(3, "claude", "zsh", 1),
            fixture_row(1, "claude", "bash", 1),
            fixture_row(2, "claude", "alpha", 1),
        ];
        let sorted = sort_watched_agents(&inventory, ProcSort::Name, &HashMap::new());
        let comms: Vec<&str> = sorted.iter().map(|r| r.agent.comm.as_str()).collect();
        assert_eq!(comms, vec!["alpha", "bash", "zsh"]);
    }

    #[test]
    fn sort_name_pid_tie_break_ascending() {
        // Same COMM, different PID → PID tie-break ascending.
        let inventory = vec![
            fixture_row(30, "claude", "claude", 1),
            fixture_row(10, "claude", "claude", 1),
            fixture_row(20, "claude", "claude", 1),
        ];
        let sorted = sort_watched_agents(&inventory, ProcSort::Name, &HashMap::new());
        let pids: Vec<u32> = sorted.iter().map(|r| r.agent.pid).collect();
        assert_eq!(pids, vec![10, 20, 30]);
    }

    #[test]
    fn proc_sort_parses_cpu_age_name() {
        // AC-006.41: --sort accepts cpu, age, name keys alongside the historical set.
        assert_eq!("cpu".parse::<ProcSort>().unwrap(), ProcSort::Cpu);
        assert_eq!("age".parse::<ProcSort>().unwrap(), ProcSort::Age);
        assert_eq!("name".parse::<ProcSort>().unwrap(), ProcSort::Name);
        assert_eq!("NAME".parse::<ProcSort>().unwrap(), ProcSort::Name);
    }

    #[test]
    fn proc_sort_unknown_key_lists_new_options() {
        // AC-006.41: error message MUST enumerate cpu/age/name as accepted sort keys.
        let err = "bogus".parse::<ProcSort>().expect_err("unknown sort key MUST fail");
        let msg = err.to_string();
        assert!(msg.contains("cpu"), "error MUST list 'cpu'; got: {msg}");
        assert!(msg.contains("age"), "error MUST list 'age'; got: {msg}");
        assert!(msg.contains("name"), "error MUST list 'name'; got: {msg}");
    }

    #[test]
    fn parse_proc_limit_zero_is_rejected() {
        // AC-006.21: --limit 0 MUST be rejected as invalid (>= 1).
        let err = parse_proc_limit(Some(0)).expect_err("--limit 0 MUST fail");
        assert!(
            err.to_string().contains(">= 1") || err.to_string().contains("must be"),
            "error MUST mention minimum; got: {err}"
        );
    }

    #[test]
    fn limit_under_inventory_size_returns_inventory() {
        // AC-006.21: --limit N larger than inventory returns the full slice, capped order intact.
        let inventory = fixture_inventory();
        let sorted = sort_watched_agents(&inventory, ProcSort::Pid, &HashMap::new());
        let limited = limit_watched_agents(sorted, Some(50));
        assert_eq!(limited.len(), inventory.len(), "limit MUST NOT pad below inventory size");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: comm_matches_pattern / cmdline_matches_pattern
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn comm_matches_pattern_case_insensitive() {
        assert!(comm_matches_pattern("Claude", "claude"));
        assert!(comm_matches_pattern("CLAUDE", "claude"));
        assert!(comm_matches_pattern("claude", "CLAUDE"));
    }

    #[test]
    fn comm_matches_pattern_no_match() {
        assert!(!comm_matches_pattern("node", "claude"));
        assert!(!comm_matches_pattern("", "claude"));
    }

    #[test]
    fn cmdline_matches_pattern_case_insensitive() {
        assert!(cmdline_matches_pattern("/usr/bin/Claude --help", "claude"));
        assert!(cmdline_matches_pattern("claude serve", "SERVE"));
    }

    #[test]
    fn cmdline_matches_pattern_empty_string() {
        assert!(!cmdline_matches_pattern("", "anything"));
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: parse_proc_state edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_proc_state_all_valid_uppercase() {
        for ch in ['R', 'S', 'D', 'Z', 'T', 'X', 'K', 'W', 'P', 'I'] {
            let result = parse_proc_state(&ch.to_string()).expect("valid state MUST parse");
            assert_eq!(result, ch, "uppercase {ch} must normalize to itself");
        }
    }

    #[test]
    fn parse_proc_state_lowercase_normalizes_to_uppercase() {
        for (input, expected) in [
            ('r', 'R'),
            ('s', 'S'),
            ('d', 'D'),
            ('z', 'Z'),
            ('k', 'K'),
            ('w', 'W'),
            ('p', 'P'),
            ('i', 'I'),
        ] {
            let result = parse_proc_state(&input.to_string()).expect("valid lowercase MUST parse");
            assert_eq!(result, expected, "lowercase '{input}' must normalize to '{expected}'");
        }
    }

    #[test]
    fn parse_proc_state_lowercase_t_and_x_preserved() {
        assert_eq!(parse_proc_state("t").unwrap(), 't');
        assert_eq!(parse_proc_state("x").unwrap(), 'x');
    }

    #[test]
    fn parse_proc_state_empty_rejected() {
        parse_proc_state("").expect_err("empty MUST fail");
        parse_proc_state("  ").expect_err("whitespace-only MUST fail");
    }

    #[test]
    fn parse_proc_state_multi_char_rejected() {
        parse_proc_state("RS").expect_err("multi-char MUST fail");
        parse_proc_state("ab").expect_err("multi-char MUST fail");
    }

    #[test]
    fn parse_proc_state_invalid_char_rejected() {
        let err = parse_proc_state("Q").expect_err("invalid char MUST fail");
        assert!(err.to_string().contains("Q"), "error MUST mention the bad char; got: {err}");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: parse_fd_count
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_fd_count_valid_number() {
        assert_eq!(parse_fd_count("0", "--min-fd").unwrap(), 0);
        assert_eq!(parse_fd_count("42", "--max-fd").unwrap(), 42);
        assert_eq!(parse_fd_count("1024", "--min-fd").unwrap(), 1024);
    }

    #[test]
    fn parse_fd_count_invalid_string_rejected() {
        let err = parse_fd_count("abc", "--min-fd").expect_err("non-numeric MUST fail");
        assert!(err.to_string().contains("abc"), "error MUST mention the bad value; got: {err}");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: csv_escape_field
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn csv_escape_field_no_escaping_needed() {
        assert_eq!(csv_escape_field("hello"), "hello");
        assert_eq!(csv_escape_field(""), "");
    }

    #[test]
    fn csv_escape_field_comma_wrapped_in_quotes() {
        assert_eq!(csv_escape_field("a,b"), "\"a,b\"");
    }

    #[test]
    fn csv_escape_field_double_quote_escaped_and_wrapped() {
        assert_eq!(csv_escape_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_escape_field_newline_wrapped_in_quotes() {
        assert_eq!(csv_escape_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn csv_escape_field_carriage_return_wrapped() {
        assert_eq!(csv_escape_field("a\rb"), "\"a\rb\"");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: format_cmdline
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn format_cmdline_empty_returns_placeholder() {
        assert_eq!(format_cmdline(&[]), "(empty)");
    }

    #[test]
    fn format_cmdline_single_element() {
        assert_eq!(format_cmdline(&["claude".into()]), "claude");
    }

    #[test]
    fn format_cmdline_multiple_elements_joined() {
        let args = vec!["claude".into(), "serve".into(), "--port".into(), "8080".into()];
        assert_eq!(format_cmdline(&args), "claude serve --port 8080");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: state_letter_for_pid / state_json_from_char / state_text_from_detail_state
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn state_letter_for_pid_present() {
        let map = HashMap::from([(42, 'R'), (99, 'S')]);
        assert_eq!(state_letter_for_pid(&map, 42), "R");
        assert_eq!(state_letter_for_pid(&map, 99), "S");
    }

    #[test]
    fn state_letter_for_pid_missing_returns_empty() {
        let map = HashMap::from([(42, 'R')]);
        assert_eq!(state_letter_for_pid(&map, 999), "");
    }

    #[test]
    fn state_json_from_char_normal() {
        assert_eq!(state_json_from_char('R'), "R");
        assert_eq!(state_json_from_char('S'), "S");
        assert_eq!(state_json_from_char('Z'), "Z");
    }

    #[test]
    fn state_json_from_char_question_returns_empty() {
        assert_eq!(state_json_from_char('?'), "");
    }

    #[test]
    fn state_text_from_detail_state_empty_returns_dash() {
        assert_eq!(state_text_from_detail_state(""), "-");
    }

    #[test]
    fn state_text_from_detail_state_nonempty_returns_self() {
        assert_eq!(state_text_from_detail_state("R"), "R");
        assert_eq!(state_text_from_detail_state("S"), "S");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: render_proc_detail_csv
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn render_proc_detail_csv_includes_header_and_body() {
        let detail = ProcDetailSnapshot {
            pid: 1234,
            ppid: 1,
            parent_comm: Some("init".into()),
            comm: "claude".into(),
            state: "R".into(),
            cmdline: vec!["claude".into()],
            family: Some("claude".into()),
            agent_ancestor: None,
            mem_rss_bytes: 52_428_800,
            mem_rss: "50M".into(),
            fd_count: Some(12),
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 1,
                agent_total_rss_bytes: 52_428_800,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            status: None,
        };
        let csv = render_proc_detail_csv(&detail);
        assert!(csv.starts_with("pid,ppid,comm,state,mem_rss_bytes,mem_rss,fd_count\n"));
        assert!(csv.contains("1234,1,claude,R,52428800,50M,12\n"));
    }

    #[test]
    fn render_proc_detail_csv_empty_fd_count() {
        let detail = ProcDetailSnapshot {
            pid: 100,
            ppid: 1,
            parent_comm: None,
            comm: "node".into(),
            state: "S".into(),
            cmdline: vec![],
            family: None,
            agent_ancestor: None,
            mem_rss_bytes: 0,
            mem_rss: "0B".into(),
            fd_count: None,
            gate: sharecli_fleet::GateStatusSnapshot {
                thermal_pressure: "GREEN".into(),
                detected_agents: 0,
                agent_total_rss_bytes: 0,
                agent_contention: "OK".into(),
                gate_decision: "ADMIT".into(),
            },
            host_watch: HostResourceWatchJson::default(),
            pool: None,
            status: None,
        };
        let csv = render_proc_detail_csv(&detail);
        // fd_count column should be empty when None
        assert!(
            csv.contains(",\n") || csv.ends_with(",\n"),
            "missing fd MUST produce empty field; got: {csv}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: append_gate_csv_companion
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn append_gate_csv_companion_appends_to_existing() {
        let gate = sharecli_fleet::GateStatusSnapshot {
            thermal_pressure: "GREEN".into(),
            detected_agents: 3,
            agent_total_rss_bytes: 1_000_000,
            agent_contention: "OK".into(),
            gate_decision: "ADMIT".into(),
        };
        let base = "header\nrow1\n".to_string();
        let result = append_gate_csv_companion(base.clone(), &gate);
        assert!(result.starts_with(&base), "base content MUST be preserved");
        assert!(result.len() > base.len(), "companion MUST append content");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: render_agent_inventory_csv
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn render_agent_inventory_csv_header_and_rows() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "cursor", "cursor-agent", 2_000_000),
        ];
        let state_by_pid = HashMap::from([(10, 'R'), (20, 'S')]);
        let csv = render_agent_inventory_csv(&inventory, &state_by_pid);
        assert!(csv.starts_with("pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"));
        assert!(csv.contains("10,claude,claude,R,1000000,"));
        assert!(csv.contains("20,cursor,cursor-agent,S,2000000,"));
    }

    #[test]
    fn render_agent_inventory_csv_empty_inventory() {
        let csv = render_agent_inventory_csv(&[], &HashMap::new());
        assert!(csv.starts_with("pid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"));
        // Empty inventory still has header + trailing newline
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "empty inventory MUST have only header; got {} lines",
            lines.len()
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: filter_watched_agents
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn filter_watched_agents_family_filter() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "cursor", "cursor-agent", 2_000_000),
        ];
        let filter = ProcFilter { family: Some("claude".into()), ..Default::default() };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 10);
    }

    #[test]
    fn filter_watched_agents_comm_filter() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "claude", "node", 2_000_000),
        ];
        let filter = ProcFilter { comm: Some("node".into()), ..Default::default() };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 20);
    }

    #[test]
    fn filter_watched_agents_empty_filter_returns_all() {
        let inventory = fixture_inventory();
        let filter = ProcFilter::default();
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), inventory.len());
    }

    #[test]
    fn filter_watched_agents_rss_bounds() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 500),
            fixture_row(20, "claude", "claude", 1500),
            fixture_row(30, "claude", "claude", 2500),
        ];
        let filter = ProcFilter {
            min_rss_bytes: Some(1000),
            max_rss_bytes: Some(2000),
            ..Default::default()
        };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 20);
    }

    #[test]
    fn filter_watched_agents_state_filter() {
        let inventory = vec![
            fixture_row(10, "claude", "claude", 1_000_000),
            fixture_row(20, "claude", "claude", 2_000_000),
        ];
        let state_by_pid = HashMap::from([(10, 'R'), (20, 'S')]);
        let filter = ProcFilter { state: Some('R'), ..Default::default() };
        let filtered = filter_watched_agents(
            &inventory,
            &filter,
            &HashMap::new(),
            &HashMap::new(),
            &state_by_pid,
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].agent.pid, 10);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: limit_agent_forests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn limit_agent_forests_none_returns_all() {
        let forests = vec![
            AgentTreeNode { pid: 10, ppid: 1, comm: "a".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 20, ppid: 1, comm: "b".into(), family: None, children: vec![] },
        ];
        let result = limit_agent_forests(forests.clone(), None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn limit_agent_forests_some_caps() {
        let forests = vec![
            AgentTreeNode { pid: 10, ppid: 1, comm: "a".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 20, ppid: 1, comm: "b".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 30, ppid: 1, comm: "c".into(), family: None, children: vec![] },
        ];
        let result = limit_agent_forests(forests, Some(2));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].pid, 10);
        assert_eq!(result[1].pid, 20);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: sort_agent_forests by name
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn sort_agent_forests_by_name_ascending() {
        let forests = vec![
            AgentTreeNode { pid: 3, ppid: 1, comm: "zsh".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 1, ppid: 1, comm: "alpha".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 2, ppid: 1, comm: "beta".into(), family: None, children: vec![] },
        ];
        let result = sort_agent_forests(
            &forests,
            ProcSort::Name,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        );
        let comms: Vec<&str> = result.iter().map(|n| n.comm.as_str()).collect();
        assert_eq!(comms, vec!["alpha", "beta", "zsh"]);
    }

    #[test]
    fn sort_agent_forests_by_rss_desc() {
        let forests = vec![
            AgentTreeNode { pid: 10, ppid: 1, comm: "a".into(), family: None, children: vec![] },
            AgentTreeNode { pid: 20, ppid: 1, comm: "b".into(), family: None, children: vec![] },
        ];
        let rss_by_pid = HashMap::from([(10, 500), (20, 1000)]);
        let result = sort_agent_forests(
            &forests,
            ProcSort::Rss,
            &rss_by_pid,
            &HashMap::new(),
            &HashMap::new(),
        );
        assert_eq!(result[0].pid, 20, "higher RSS MUST sort first");
        assert_eq!(result[1].pid, 10);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: ProcFilter::active
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn proc_filter_active_empty_is_false() {
        assert!(!ProcFilter::default().active());
    }

    #[test]
    fn proc_filter_active_with_family_is_true() {
        let f = ProcFilter { family: Some("claude".into()), ..Default::default() };
        assert!(f.active());
    }

    #[test]
    fn proc_filter_active_with_state_is_true() {
        let f = ProcFilter { state: Some('R'), ..Default::default() };
        assert!(f.active());
    }

    #[test]
    fn proc_filter_active_with_rss_bounds_is_true() {
        let f = ProcFilter { min_rss_bytes: Some(1000), ..Default::default() };
        assert!(f.active());
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: ProcSort::from_cli
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn proc_sort_from_cli_none_returns_none() {
        assert!(ProcSort::from_cli(None).unwrap().is_none());
    }

    #[test]
    fn proc_sort_from_cli_valid_key() {
        assert_eq!(ProcSort::from_cli(Some("rss")).unwrap(), Some(ProcSort::Rss));
        assert_eq!(ProcSort::from_cli(Some("fd")).unwrap(), Some(ProcSort::Fd));
        assert_eq!(ProcSort::from_cli(Some("pid")).unwrap(), Some(ProcSort::Pid));
        assert_eq!(ProcSort::from_cli(Some("state")).unwrap(), Some(ProcSort::State));
        assert_eq!(ProcSort::from_cli(Some("cpu")).unwrap(), Some(ProcSort::Cpu));
        assert_eq!(ProcSort::from_cli(Some("age")).unwrap(), Some(ProcSort::Age));
        assert_eq!(ProcSort::from_cli(Some("name")).unwrap(), Some(ProcSort::Name));
    }

    #[test]
    fn proc_sort_from_cli_invalid_returns_error() {
        let err = ProcSort::from_cli(Some("bogus")).expect_err("unknown sort key MUST fail");
        assert!(err.to_string().contains("bogus"), "error MUST mention the bad key; got: {err}");
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: render_agent_tree_csv
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn render_agent_tree_csv_header_and_body() {
        let root = AgentTreeNode {
            pid: 50,
            ppid: 1,
            comm: "claude".into(),
            family: Some("claude"),
            children: vec![AgentTreeNode {
                pid: 51,
                ppid: 50,
                comm: "node".into(),
                family: None,
                children: vec![],
            }],
        };
        let rss_by_pid = HashMap::from([(50, 1_000_000), (51, 500_000)]);
        let fd_by_pid = HashMap::from([(50, 10), (51, 5)]);
        let state_by_pid = HashMap::from([(50, 'R'), (51, 'S')]);
        let csv = render_agent_tree_csv(&[root], &rss_by_pid, &fd_by_pid, &state_by_pid);
        assert!(csv.starts_with(
            "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"
        ));
        assert!(csv.contains("0,0,50,1,claude,claude,R,1000000,"));
        assert!(csv.contains("0,1,51,50,,node,S,500000,"));
    }

    #[test]
    fn render_agent_tree_csv_empty_forests() {
        let csv = render_agent_tree_csv(&[], &HashMap::new(), &HashMap::new(), &HashMap::new());
        assert!(csv.starts_with(
            "root_index,depth,pid,ppid,family,comm,state,mem_rss_bytes,mem_rss,fd_count"
        ));
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines.len(),
            1,
            "empty forests MUST have only header; got {} lines",
            lines.len()
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: ProcSort FromStr edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn proc_sort_from_str_case_insensitive() {
        assert_eq!("PID".parse::<ProcSort>().unwrap(), ProcSort::Pid);
        assert_eq!("RSS".parse::<ProcSort>().unwrap(), ProcSort::Rss);
        assert_eq!("Fd".parse::<ProcSort>().unwrap(), ProcSort::Fd);
    }

    #[test]
    fn proc_sort_from_str_fd_and_state_parse() {
        assert_eq!("fd".parse::<ProcSort>().unwrap(), ProcSort::Fd);
        assert_eq!("state".parse::<ProcSort>().unwrap(), ProcSort::State);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: agent_row_from_watch with missing state
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn agent_row_from_watch_missing_state_is_empty() {
        let row = agent_row_from_watch(
            &DetectedAgentWatch {
                agent: DetectedAgent { pid: 99, family: "claude", comm: "claude".into() },
                resource: AgentResourceSample { mem_rss_bytes: 1024, fd_count: None },
            },
            &HashMap::new(), // no state entries
        );
        assert_eq!(row.state, "", "missing state MUST produce empty string");
        assert_eq!(row.fd_count, None);
        assert_eq!(row.mem_rss_bytes, 1024);
    }

    // ──────────────────────────────────────────────────────────────────────
    // NEW: parse_proc_limit edge cases
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn parse_proc_limit_none_returns_none() {
        assert!(parse_proc_limit(None).unwrap().is_none());
    }

    #[test]
    fn parse_proc_limit_valid_number() {
        assert_eq!(parse_proc_limit(Some(1)).unwrap(), Some(1));
        assert_eq!(parse_proc_limit(Some(100)).unwrap(), Some(100));
    }
}
