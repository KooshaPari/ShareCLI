//! Unit tests for the sharecli-fuse crate root.
//!
//! Extracted from the original `mod tests { ... }` inline block in
//! `lib.rs` during the decompose-top3 refactor (FR-001, 500L module cap).
//! Covers `InterceptFsOptions`, `FuseBackend`, `FuseBackendDiagnostic`,
//! `FuseCapabilities`, `NegDentryMeters`, `ReadCacheMeters`, and the
//! `InterceptFs` FUSE layer.

#![cfg(test)]

    use crate::*;
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::time::Duration;

    // ------------------------------------------------------------------
    // InterceptFsOptions
    // ------------------------------------------------------------------

    /// Default session_id is "default".
    #[test]
    fn intercept_fs_options_default_session() {
        let opts = InterceptFsOptions::default();
        assert_eq!(opts.session_id, "default");
    }

    /// Default cow is disabled and serialize is enabled.
    #[test]
    fn intercept_fs_options_default_flags() {
        let opts = InterceptFsOptions::default();
        assert!(!opts.cow);
        assert!(opts.serialize);
    }

    /// Default cow_dir and agent are None.
    #[test]
    fn intercept_fs_options_default_nones() {
        let opts = InterceptFsOptions::default();
        assert!(opts.cow_dir.is_none());
        assert!(opts.agent.is_none());
        assert!(opts.agents_conf.is_none());
    }

    // ------------------------------------------------------------------
    // FuseBackend::as_str
    // ------------------------------------------------------------------

    #[test]
    fn fuse_backend_as_str_kernel() {
        assert_eq!(FuseBackend::Kernel.as_str(), "kext");
    }

    #[test]
    fn fuse_backend_as_str_fskit() {
        assert_eq!(FuseBackend::Fskit.as_str(), "fskit");
    }

    #[test]
    fn fuse_backend_as_str_unavailable() {
        assert_eq!(FuseBackend::Unavailable.as_str(), "non-fuse");
    }

    // ------------------------------------------------------------------
    // FuseBackendDiagnostic::message
    // ------------------------------------------------------------------

    #[test]
    fn fuse_backend_diagnostic_no_verified() {
        let msg = FuseBackendDiagnostic::NoVerifiedBackend.message();
        assert!(msg.contains("macFUSE unavailable"));
    }

    #[test]
    fn fuse_backend_diagnostic_fskit_volumes() {
        let msg = FuseBackendDiagnostic::FskitRequiresVolumes.message();
        assert!(msg.contains("/Volumes"));
    }

    // ------------------------------------------------------------------
    // FuseCapabilities
    // ------------------------------------------------------------------

    #[test]
    fn fuse_capabilities_default() {
        let caps = FuseCapabilities::default();
        assert!(!caps.kernel_loaded);
        assert!(!caps.fskit_approved);
    }

    // ------------------------------------------------------------------
    // NegativeDentryMeters edge cases
    // ------------------------------------------------------------------

    #[test]
    fn neg_dentry_meters_hit_rate_pct_zero_events() {
        let m = NegDentryMeters { hits: 0, misses: 0 };
        assert_eq!(m.hit_rate_pct(), 0);
    }

    #[test]
    fn neg_dentry_meters_hit_rate_pct_all_hits() {
        let m = NegDentryMeters { hits: 10, misses: 0 };
        assert_eq!(m.hit_rate_pct(), 100);
    }

    #[test]
    fn neg_dentry_meters_hit_rate_pct_all_misses() {
        let m = NegDentryMeters { hits: 0, misses: 5 };
        assert_eq!(m.hit_rate_pct(), 0);
    }

    #[test]
    fn neg_dentry_meters_hit_rate_pct_split() {
        let m = NegDentryMeters { hits: 3, misses: 7 };
        assert_eq!(m.hit_rate_pct(), 30);
    }

    // ------------------------------------------------------------------
    // ReadCacheMeters edge cases
    // ------------------------------------------------------------------

    #[test]
    fn read_cache_meters_hit_rate_pct_zero_events() {
        let m = ReadCacheMeters { hits: 0, misses: 0 };
        assert_eq!(m.hit_rate_pct(), 0);
    }

    #[test]
    fn read_cache_meters_hit_rate_pct_even_split() {
        let m = ReadCacheMeters { hits: 50, misses: 50 };
        assert_eq!(m.hit_rate_pct(), 50);
    }

    #[test]
    fn read_cache_meters_hit_rate_pct_all_hits() {
        let m = ReadCacheMeters { hits: 42, misses: 0 };
        assert_eq!(m.hit_rate_pct(), 100);
    }

    // ------------------------------------------------------------------
    // WriteSerializeMeters::format_status_section
    // ------------------------------------------------------------------

    #[test]
    fn write_serialize_meters_format_section() {
        let m =
            WriteSerializeMeters { passthrough_writes: 11, stages: 22, commits: 33, discards: 44 };
        let s = m.format_status_section();
        assert!(s.contains("=== FUSE Write Serialize ==="));
        assert!(s.contains("Passthrough:  11"));
        assert!(s.contains("Stages:       22"));
        assert!(s.contains("Commits:      33"));
        assert!(s.contains("Discards:     44"));
    }

    // ------------------------------------------------------------------
    // join_rel / abs_under (pure path logic)
    // ------------------------------------------------------------------

    #[test]
    fn join_rel_empty_parent() {
        assert_eq!(join_rel(Path::new(""), OsStr::new("child")), PathBuf::from("child"));
    }

    #[test]
    fn join_rel_nonempty_parent() {
        assert_eq!(join_rel(Path::new("src"), OsStr::new("main.rs")), PathBuf::from("src/main.rs"));
    }

    #[test]
    fn join_rel_deeply_nested() {
        assert_eq!(join_rel(Path::new("a/b/c"), OsStr::new("d.txt")), PathBuf::from("a/b/c/d.txt"));
    }

    #[test]
    fn abs_under_empty_rel() {
        let backing = Path::new("/workspace");
        assert_eq!(abs_under(backing, Path::new("")), PathBuf::from("/workspace"));
    }

    #[test]
    fn abs_under_nonempty_rel() {
        let backing = Path::new("/workspace");
        assert_eq!(
            abs_under(backing, Path::new("src/main.rs")),
            PathBuf::from("/workspace/src/main.rs")
        );
    }

    #[test]
    fn root_ino_is_one() {
        assert_eq!(ROOT_INO, 1);
    }

    // ------------------------------------------------------------------
    // sanitize_agent_id edge cases
    // ------------------------------------------------------------------

    #[test]
    fn sanitize_agent_id_empty() {
        assert_eq!(sanitize_agent_id(""), "default");
    }

    #[test]
    fn sanitize_agent_id_whitespace_only() {
        assert_eq!(sanitize_agent_id("   "), "default");
    }

    #[test]
    fn sanitize_agent_id_special_chars() {
        assert_eq!(sanitize_agent_id("a/b c!d"), "a_b_c_d");
    }

    #[test]
    fn sanitize_agent_id_valid_preserved() {
        assert_eq!(sanitize_agent_id("agent-1_test"), "agent-1_test");
    }

    // ------------------------------------------------------------------
    // AgentsConf::parse edge cases
    // ------------------------------------------------------------------

    #[test]
    fn agents_conf_parse_empty() {
        let conf = AgentsConf::parse("");
        assert!(conf.patterns().is_empty());
    }

    #[test]
    fn agents_conf_parse_only_comments_and_blanks() {
        let conf = AgentsConf::parse("# line one\n\n# line two\n");
        assert!(conf.patterns().is_empty());
    }

    #[test]
    fn agents_conf_parse_mixed_content() {
        let conf = AgentsConf::parse("claude\n# skip\n\naider\n");
        assert_eq!(conf.patterns(), &["claude", "aider"]);
    }

    #[test]
    fn agents_conf_matches_name_substring() {
        let conf = AgentsConf::parse("claude");
        assert!(conf.matches_name("claude-code"));
        assert!(conf.matches_name("my-claude-build"));
        assert!(!conf.matches_name("cursor"));
    }

    #[test]
    fn agents_conf_matches_name_case_sensitive() {
        let conf = AgentsConf::parse("claude");
        assert!(!conf.matches_name("Claude"));
    }

    #[test]
    fn agents_conf_valid_agent_id() {
        assert!(AgentsConf::is_valid_agent_id("abc123"));
        assert!(AgentsConf::is_valid_agent_id("agent-1"));
        assert!(AgentsConf::is_valid_agent_id("my_agent"));
        assert!(!AgentsConf::is_valid_agent_id(""));
        assert!(!AgentsConf::is_valid_agent_id("has space"));
        assert!(!AgentsConf::is_valid_agent_id("a/b"));
    }

    // ------------------------------------------------------------------
    // NegativeDentryCache construction
    // ------------------------------------------------------------------

    #[test]
    fn neg_dentry_cache_new_default_ttl() {
        let cache = NegativeDentryCache::new();
        assert_eq!(cache.ttl(), DEFAULT_NEG_TTL);
    }

    #[test]
    fn neg_dentry_cache_custom_ttl() {
        let ttl = Duration::from_secs(42);
        let cache = NegativeDentryCache::with_ttl(ttl);
        assert_eq!(cache.ttl(), ttl);
    }

    #[test]
    fn neg_dentry_cache_empty_meters() {
        let cache = NegativeDentryCache::new();
        let m = cache.meters();
        assert_eq!(m.hits, 0);
        assert_eq!(m.misses, 0);
    }

    // ------------------------------------------------------------------
    // ReadCacheMeters::format_status_section
    // ------------------------------------------------------------------

    #[test]
    fn read_cache_meters_format_section() {
        let m = ReadCacheMeters { hits: 10, misses: 90 };
        let s = m.format_status_section();
        assert!(s.contains("=== FUSE Read Coalesce ==="));
        assert!(s.contains("Cache hits:   10"));
        assert!(s.contains("Cache misses: 90"));
        assert!(s.contains("Hit rate:     10%"));
    }

    // ------------------------------------------------------------------
    // NegativeDentryMeters::format_status_section
    // ------------------------------------------------------------------

    #[test]
    fn neg_dentry_meters_format_section() {
        let m = NegDentryMeters { hits: 5, misses: 5 };
        let s = m.format_status_section();
        assert!(s.contains("=== FUSE Negative Dentry ==="));
        assert!(s.contains("Neg hits:     5"));
        assert!(s.contains("Neg misses:   5"));
        assert!(s.contains("Hit rate:     50%"));
    }

    // ------------------------------------------------------------------
    // global_read_cache_meters (currently returns default)
    // ------------------------------------------------------------------

    #[test]
    fn global_read_cache_meters_returns_default() {
        let m = global_read_cache_meters();
        assert_eq!(m.hits, 0);
        assert_eq!(m.misses, 0);
    }

    // ------------------------------------------------------------------
    // WriteSerialize basic construction and ops
    // ------------------------------------------------------------------

    #[test]
    fn write_serialize_staging_root_created() {
        let dir = tempfile::TempDir::new().unwrap();
        let staging = dir.path().join("ws-test");
        let ws = WriteSerialize::with_staging_root(&staging);
        assert_eq!(ws.staging_root(), staging.as_path());
        assert!(staging.exists());
    }

    #[test]
    fn write_serialize_no_pending_initially() {
        let ws = WriteSerialize::new();
        let fake = PathBuf::from("/nonexistent/path");
        assert!(!ws.has_pending(&fake).unwrap());
    }

    #[test]
    fn write_serialize_pending_paths_empty_initially() {
        let ws = WriteSerialize::new();
        assert!(ws.pending_backing_paths().unwrap().is_empty());
    }

    #[test]
    fn write_serialize_stage_commit_and_discard_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let staging = dir.path().join("staging");
        let ws = WriteSerialize::with_staging_root(&staging);

        let backing = dir.path().join("file.txt");
        std::fs::write(&backing, b"original").unwrap();

        // Stage bytes
        ws.stage_bytes(&backing, b"staged").unwrap();
        assert!(ws.has_pending(&backing).unwrap());
        assert_eq!(ws.pending_backing_paths().unwrap().len(), 1);

        // Commit
        ws.commit_pending(&backing).unwrap();
        assert_eq!(std::fs::read(&backing).unwrap(), b"staged");
        assert!(!ws.has_pending(&backing).unwrap());

        // Stage again and discard
        ws.stage_bytes(&backing, b"will-discard").unwrap();
        ws.discard_pending(&backing).unwrap();
        assert_eq!(std::fs::read(&backing).unwrap(), b"staged");
        assert!(!ws.has_pending(&backing).unwrap());
    }

    // ------------------------------------------------------------------
    // InterceptFs construction (platform-gated, macOS/Linux only)
    // ------------------------------------------------------------------

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_new_default_session() {
        let dir = tempfile::TempDir::new().unwrap();
        let fs = InterceptFs::new(dir.path());
        assert_eq!(fs.backing(), dir.path());
        assert!(!fs.session_id().is_empty());
        assert!(!fs.cow_enabled());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_with_session_explicit() {
        let dir = tempfile::TempDir::new().unwrap();
        let fs = InterceptFs::with_session(dir.path(), "test-session");
        assert_eq!(fs.session_id(), "test-session");
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_with_options_cow_enabled() {
        let dir = tempfile::TempDir::new().unwrap();
        let opts = InterceptFsOptions {
            session_id: "opts-test".to_string(),
            cow: true,
            ..InterceptFsOptions::default()
        };
        let fs = InterceptFs::with_options(dir.path(), opts);
        assert_eq!(fs.session_id(), "opts-test");
        assert!(fs.cow_enabled());
        // cow_root should default to {backing}/.sharecli-cow when cow=true
        assert_eq!(fs.cow_root(), dir.path().join(".sharecli-cow"));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_with_options_serialize_flag() {
        let dir = tempfile::TempDir::new().unwrap();
        let opts = InterceptFsOptions { serialize: false, ..InterceptFsOptions::default() };
        let fs = InterceptFs::with_options(dir.path(), opts);
        assert!(!fs.serialize_writes());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_empty_session_falls_back_to_default() {
        let dir = tempfile::TempDir::new().unwrap();
        let opts =
            InterceptFsOptions { session_id: String::new(), ..InterceptFsOptions::default() };
        let fs = InterceptFs::with_options(dir.path(), opts);
        // Empty session_id should fall back to a generated default, not remain empty
        assert!(!fs.session_id().is_empty());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_cache_meters_zero_initially() {
        let dir = tempfile::TempDir::new().unwrap();
        let fs = InterceptFs::new(dir.path());
        let m = fs.cache_meters();
        assert_eq!(m.hits, 0);
        assert_eq!(m.misses, 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_neg_dentry_meters_zero_initially() {
        let dir = tempfile::TempDir::new().unwrap();
        let fs = InterceptFs::new(dir.path());
        let m = fs.neg_dentry_meters();
        assert_eq!(m.hits, 0);
        assert_eq!(m.misses, 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_exists_rel_nonexistent_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let fs = InterceptFs::new(dir.path());
        let result = fs.exists_rel(Path::new("does-not-exist.txt"));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_exists_rel_existing_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let fs = InterceptFs::new(dir.path());
        let result = fs.exists_rel(Path::new("subdir"));
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn intercept_fs_invalidate_neg_rel_no_panic() {
        let dir = tempfile::TempDir::new().unwrap();
        let fs = InterceptFs::new(dir.path());
        // Should not panic even for a path that was never remembered
        fs.invalidate_neg_rel(Path::new("never-seen.txt"));
    }
