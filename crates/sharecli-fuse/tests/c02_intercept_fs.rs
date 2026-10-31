// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-003 / FR-009
//!
//! C02 — InterceptFs no-mount surfaces: session id, agents.conf, write
//! serialization flag, negative-dentry probe/invalidate, and the per-agent
//! CoW stage/commit/discard/pending contract. These are the pure-filesystem
//! methods that mutation coverage requires; none need a live FUSE mount.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use sharecli_fuse::{
    global_neg_dentry_meters, read_provenance, remap_mount_to_backing, smoke_fuser_config,
    smoke_fuser_config_for_backend, AgentCowStore, AgentsConf, CowMountHandle, FuseMountOptions,
    InodeMap, InterceptFs, InterceptFsOptions, NegativeDentryCache, ReadCacheMeters,
    ReadContentCache, WriteSerialize,
};
use tempfile::TempDir;

/// InterceptFs::with_session must plumb the explicit session id (a mutant
/// deleting the field from the options expression would yield the default).
#[test]
fn c02_with_session_plumbs_explicit_id() {
    let dir = TempDir::new().expect("tempdir");
    let fs = InterceptFs::with_session(dir.path(), "sess-42");
    assert_eq!(fs.session_id(), "sess-42");
    assert_eq!(fs.default_agent(), "sess-42");
}

/// An empty session id falls back to the default id.
#[test]
fn c02_empty_session_id_falls_back_to_default() {
    let dir = TempDir::new().expect("tempdir");
    let fs = InterceptFs::with_options(
        dir.path(),
        InterceptFsOptions { session_id: String::new(), ..Default::default() },
    );
    assert!(!fs.session_id().is_empty());
    assert_eq!(fs.session_id(), fs.default_agent());
}

/// agents_conf must surface a loaded agents.conf (a mutant replacing the
/// accessor with None is only observable when a conf is actually loaded).
#[test]
fn c02_agents_conf_loaded_from_options() {
    let dir = TempDir::new().expect("tempdir");
    let conf = dir.path().join("agents.conf");
    fs::write(&conf, "# comments ignored\nclaude\n  code\n\n").expect("conf");
    let fs = InterceptFs::with_options(
        dir.path(),
        InterceptFsOptions { agents_conf: Some(conf), ..Default::default() },
    );
    let loaded = fs.agents_conf().expect("agents.conf loaded");
    assert_eq!(loaded.patterns(), &["claude", "code"]);
}

/// serialize_writes reflects the construction flag (default true; a mutant
/// forcing `true` is only observable when the flag is false).
#[test]
fn c02_serialize_writes_honors_no_serialize() {
    let dir = TempDir::new().expect("tempdir");
    let serialized = InterceptFs::with_options(dir.path(), Default::default());
    assert!(serialized.serialize_writes(), "default serialize is on");
    let bare = InterceptFs::with_options(
        dir.path(),
        InterceptFsOptions { serialize: false, ..Default::default() },
    );
    assert!(!bare.serialize_writes(), "--no-serialize must disable locks");
}

/// exists_rel reports backing truth (file present vs absent).
#[test]
#[serial_test::serial]
fn c02_exists_rel_reports_backing_truth() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("present.txt"), b"x").expect("file");
    let fs = InterceptFs::new(&backing);

    assert!(fs.exists_rel(Path::new("present.txt")).expect("exists"));
    assert!(!fs.exists_rel(Path::new("missing.txt")).expect("missing"));
}

/// exists_rel must propagate non-ENOENT errors (ENOTDIR when a path
/// component is a regular file); a mutant widening the guard to `true`
/// would swallow the error into Ok(false).
#[test]
#[serial_test::serial]
fn c02_exists_rel_propagates_non_notfound_errors() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("leaf"), b"x").expect("leaf");
    let fs = InterceptFs::new(&backing);

    // `leaf/child` cannot resolve: ENOTDIR, not ENOENT.
    let result = fs.exists_rel(Path::new("leaf/child"));
    assert!(result.is_err(), "ENOTDIR must surface as Err; got {result:?}");
}

/// invalidate_neg_rel clears a cached negative-dentry entry so a later
/// existence probe observes the file (a no-op mutant keeps the stale miss).
#[test]
#[serial_test::serial]
fn c02_invalidate_neg_rel_clears_cached_miss() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    let fs = InterceptFs::new(&backing);
    let rel = Path::new("created-later.txt");

    assert!(!fs.exists_rel(rel).expect("first probe misses"));
    fs::write(backing.join(rel), b"now").expect("create");
    // Negative dentry is still cached from the first probe.
    assert!(!fs.exists_rel(rel).expect("stale negative dentry"), "miss must remain cached");
    fs.invalidate_neg_rel(rel);
    assert!(fs.exists_rel(rel).expect("after invalidation"), "invalidate must clear the miss");
}

/// commit_all_for_agent promotes staged bytes into the backing file and
/// returns the committed relative paths.
#[test]
fn c02_commit_all_for_agent_promotes_and_returns_rels() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("a.txt"), b"seed-a").expect("seed");
    fs::write(backing.join("b.txt"), b"seed-b").expect("seed");
    let fs = InterceptFs::new(&backing);

    fs.stage_rel_for_agent(Some("agent-a"), Path::new("a.txt"), b"staged-a").expect("stage");
    fs.stage_rel_for_agent(Some("agent-a"), Path::new("b.txt"), b"staged-b").expect("stage");

    let committed = fs.commit_all_for_agent(Some("agent-a")).expect("commit all");
    assert!(committed.contains(&PathBuf::from("a.txt")), "got {committed:?}");
    assert!(committed.contains(&PathBuf::from("b.txt")), "got {committed:?}");
    assert_eq!(fs::read(backing.join("a.txt")).expect("read"), b"staged-a");
    assert_eq!(fs::read(backing.join("b.txt")).expect("read"), b"staged-b");
    assert!(fs.pending_rel_paths().expect("pending").is_empty());
}

/// discard_rel drops staging for a relative path, leaving the backing file
/// byte-identical.
#[test]
fn c02_discard_rel_keeps_backing_unchanged() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("a.txt"), b"seed").expect("seed");
    let fs = InterceptFs::new(&backing);

    fs.stage_rel(Path::new("a.txt"), b"staged").expect("stage");
    assert!(!fs.pending_rel_paths().expect("pending").is_empty());

    fs.discard_rel(Path::new("a.txt")).expect("discard");
    assert_eq!(fs::read(backing.join("a.txt")).expect("read"), b"seed");
    assert!(fs.pending_rel_paths().expect("pending").is_empty());
}

/// discard_rel_for_agent is scoped to the given agent's staging.
#[test]
fn c02_discard_rel_for_agent_scoped() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("a.txt"), b"seed").expect("seed");
    let fs = InterceptFs::new(&backing);

    fs.stage_rel_for_agent(Some("agent-b"), Path::new("a.txt"), b"staged").expect("stage");
    fs.discard_rel_for_agent(Some("agent-b"), Path::new("a.txt")).expect("discard");
    assert_eq!(fs::read(backing.join("a.txt")).expect("read"), b"seed");
    assert!(fs.pending_rel_paths().expect("pending").is_empty());
}

/// discard_all_for_agent returns every discarded relative path and clears
/// that agent's staging (a no-op mutant leaves the staging pending).
#[test]
fn c02_discard_all_for_agent_returns_rels() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("a.txt"), b"seed-a").expect("seed");
    fs::write(backing.join("b.txt"), b"seed-b").expect("seed");
    let fs = InterceptFs::new(&backing);

    fs.stage_rel_for_agent(Some("agent-c"), Path::new("a.txt"), b"staged-a").expect("stage");
    fs.stage_rel_for_agent(Some("agent-c"), Path::new("b.txt"), b"staged-b").expect("stage");

    let discarded = fs.discard_all_for_agent(Some("agent-c")).expect("discard all");
    assert!(discarded.contains(&PathBuf::from("a.txt")), "got {discarded:?}");
    assert!(discarded.contains(&PathBuf::from("b.txt")), "got {discarded:?}");
    assert_eq!(fs::read(backing.join("a.txt")).expect("read"), b"seed-a");
    assert_eq!(fs::read(backing.join("b.txt")).expect("read"), b"seed-b");
    assert!(fs.pending_rel_paths().expect("pending").is_empty());
}

/// create_rel creates the file, stamps session provenance xattrs, and clears
/// any cached negative dentry (an after_create_at no-op mutant skips the
/// provenance stamp, so the xattr read differs).
#[test]
#[serial_test::serial]
fn c02_create_rel_stamps_session_provenance() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    let fs = InterceptFs::with_session(&backing, "sess-create");

    // Prime a negative dentry so create must invalidate it.
    assert!(!fs.exists_rel(Path::new("made.txt")).expect("probe misses"));

    fs.create_rel(Path::new("made.txt"), 0o644).expect("create");
    assert!(backing.join("made.txt").is_file(), "file must exist");
    assert!(
        fs.exists_rel(Path::new("made.txt")).expect("exists after create"),
        "create must clear the cached miss"
    );
    let provenance =
        read_provenance(&backing.join("made.txt")).expect("provenance").expect("stamped");
    assert_eq!(provenance.session_id, "sess-create");
}

/// AgentCowStore::serialize must reflect the construction flag (a mutant
/// forcing `true` is only observable when the flag was false).
#[test]
fn c02_agent_cow_serialize_flag_honored() {
    let dir = TempDir::new().expect("tempdir");
    let locked = AgentCowStore::new(dir.path().join("cow-a"), "default", true);
    assert!(locked.serialize());
    let bare = AgentCowStore::new(dir.path().join("cow-b"), "default", false);
    assert!(!bare.serialize());
}

/// AgentCowStore::discard_pending must actually clear the staging (a mutant
/// returning Ok(()) without discarding leaves the path pending, so a later
/// commit_all would promote it).
#[test]
fn c02_agent_cow_discard_pending_clears_staging() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("file.txt");
    fs::write(&backing, b"seed").expect("seed");
    let cow = AgentCowStore::new(dir.path().join("cow"), "default", true);

    cow.stage_bytes(None, &backing, b"staged").expect("stage");
    cow.discard_pending(None, &backing).expect("discard");
    assert!(cow.pending_for_agent(None).expect("pending").is_empty(), "discard must clear staging");
    let committed = cow.commit_all_for_agent(None).expect("commit all");
    assert!(committed.is_empty(), "nothing may remain pending; got {committed:?}");
    assert_eq!(fs::read(&backing).expect("read"), b"seed");
}

/// AgentCowStore::with_locked_path with serialize disabled must skip the
/// per-path lock entirely — no agent store (and thus no staging directory)
/// may be created (a deleted-`!` mutant would materialize `cow/default`).
#[test]
fn c02_agent_cow_no_serialize_skips_store_creation() {
    let dir = TempDir::new().expect("tempdir");
    let cow_root = dir.path().join("cow");
    let cow = AgentCowStore::new(&cow_root, "default", false);
    let path = dir.path().join("x.txt");
    let mut hit = false;
    cow.with_locked_path(None, &path, || {
        hit = true;
    })
    .expect("callback");
    assert!(hit, "callback must still run");
    assert!(
        !cow_root.join("default").exists(),
        "no store may be created when serialize is disabled"
    );
}

/// AgentsConf::load must parse the file (a mutant replacing the body with
/// Ok(Default) would always return an empty pattern set).
#[test]
fn c02_agents_conf_load_parses_file() {
    let dir = TempDir::new().expect("tempdir");
    let conf = dir.path().join("agents.conf");
    fs::write(&conf, "claude\ncode\n").expect("conf");
    let loaded = AgentsConf::load(&conf).expect("load");
    assert_eq!(loaded.patterns(), &["claude", "code"]);
}

/// is_valid_agent_id must accept alnum / `_` / `-` ids (a mutant turning the
/// character alternation into a conjunction rejects every non-empty id).
#[test]
fn c02_agents_conf_is_valid_agent_id() {
    assert!(AgentsConf::is_valid_agent_id("claude-1"));
    assert!(AgentsConf::is_valid_agent_id("_abc_"));
    assert!(!AgentsConf::is_valid_agent_id(""));
    assert!(!AgentsConf::is_valid_agent_id("has space"));
    assert!(!AgentsConf::is_valid_agent_id("dot.txt"));
}

/// pending_by_agent groups staged paths by agent, stripping the backing root
/// (a mutant replacing the grouping with a canned value yields the wrong rels).
#[test]
fn c02_pending_by_agent_groups_staged_paths() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("a.txt"), b"seed-a").expect("seed");
    fs::write(backing.join("b.txt"), b"seed-b").expect("seed");
    let fs = InterceptFs::new(&backing);

    fs.stage_rel_for_agent(Some("agent-d"), Path::new("a.txt"), b"staged-a").expect("stage");
    fs.stage_rel_for_agent(Some("agent-e"), Path::new("b.txt"), b"staged-b").expect("stage");

    let by_agent = fs.pending_by_agent().expect("pending by agent");
    let mut names: Vec<(String, Vec<PathBuf>)> =
        by_agent.into_iter().map(|(a, rels)| (a, rels)).collect();
    names.sort_by(|x, y| x.0.cmp(&y.0));
    assert_eq!(names.len(), 2, "got {names:?}");
    assert_eq!(names[0].0, "agent-d");
    assert_eq!(names[0].1, vec![PathBuf::from("a.txt")]);
    assert_eq!(names[1].0, "agent-e");
    assert_eq!(names[1].1, vec![PathBuf::from("b.txt")]);
}

/// Process-wide negative-dentry meters must count the miss and the cached hit
/// (no-op record mutants leave the counters flat).
#[test]
#[serial_test::serial]
fn c02_neg_dentry_global_meters_count_hits_and_misses() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    let before = global_neg_dentry_meters();
    let fs = InterceptFs::new(&backing);

    assert!(!fs.exists_rel(Path::new("never-here.txt")).expect("first probe misses"));
    assert!(!fs.exists_rel(Path::new("never-here.txt")).expect("cached neg hit"));

    let after = global_neg_dentry_meters();
    assert_eq!(after.misses, before.misses + 1, "miss recorder must count");
    assert_eq!(after.hits, before.hits + 1, "hit recorder must count");
}

/// NegativeDentryCache exposes its TTL and rejects a miss at zero TTL (the
/// boundary mutant `>` -> `>=` would accept `expires_at == now`).
#[test]
fn c02_neg_dentry_ttl_accessor_and_zero_ttl_boundary() {
    let mut cache = NegativeDentryCache::with_ttl(Duration::from_secs(7));
    assert_eq!(cache.ttl(), Duration::from_secs(7));
    cache.remember_miss(PathBuf::from("x.txt"));
    assert!(cache.is_negative(Path::new("x.txt")), "within TTL MUST hit");

    let mut zero = NegativeDentryCache::with_ttl(Duration::ZERO);
    zero.remember_miss(PathBuf::from("y.txt"));
    assert!(!zero.is_negative(Path::new("y.txt")), "zero-TTL miss MUST expire");
}

/// ReadContentCache::read_slice honors offset/size and past-end reads return
/// empty (mutants replacing the slice math or the boundary guard mis-slice).
#[test]
fn c02_read_cache_slice_semantics() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("data.bin");
    fs::write(&path, b"abcdef").expect("write");
    let mut cache = ReadContentCache::new();

    assert_eq!(cache.read_slice(&path, 0, 6).expect("full"), b"abcdef");
    assert_eq!(cache.read_slice(&path, 2, 2).expect("middle"), b"cd");
    assert_eq!(cache.read_slice(&path, 1, 3).expect("offset"), b"bcd");
    assert_eq!(cache.read_slice(&path, 100, 5).expect("past end"), b"");
}

/// A stale mtime must miss and reload (a guard->true mutant serves stale bytes).
#[test]
fn c02_read_cache_stale_mtime_misses_and_reloads() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("data.bin");
    fs::write(&path, b"v1").expect("write");
    let mut cache = ReadContentCache::new();

    assert_eq!(cache.read_coalesced(&path).expect("first"), b"v1");
    thread::sleep(Duration::from_millis(25));
    fs::write(&path, b"v2-longer-content").expect("rewrite");

    let second = cache.read_coalesced(&path).expect("second");
    assert_eq!(second, b"v2-longer-content", "stale cache MUST NOT be served");
    let m = cache.meters();
    assert_eq!(m.misses, 2);
    assert_eq!(m.hits, 0, "changed mtime must not count as a hit");
}

/// hit_rate_pct math (mutants forcing 0/1 must be observable).
#[test]
fn c02_read_cache_hit_rate_pct() {
    let m = ReadCacheMeters { hits: 3, misses: 7 };
    assert_eq!(m.hit_rate_pct(), 30);
    assert_eq!((ReadCacheMeters { hits: 5, misses: 0 }).hit_rate_pct(), 100);
    assert_eq!(ReadCacheMeters::default().hit_rate_pct(), 0);
}

/// A shorter path that is a prefix of the mountpoint must NOT remap (the
/// `<` -> `==` / `==` -> `!=` mutants return Some for the prefix case).
#[test]
fn c02_path_remap_rejects_shorter_prefix() {
    let mount = Path::new("/tmp/fuse-mp");
    let backing = Path::new("/workspace/proj");
    assert!(remap_mount_to_backing(mount, backing, Path::new("/tmp")).is_none());
    assert!(remap_mount_to_backing(mount, backing, Path::new("/tmp/")).is_none());
    assert!(remap_mount_to_backing(mount, backing, Path::new("/tmp/fuse")).is_none());
}

/// InodeMap::abs_path resolves allocated inodes and rejects unknown ones.
#[test]
fn c02_inode_map_abs_path_resolves_allocated_inodes() {
    let backing = Path::new("/workspace/proj");
    let mut map = InodeMap::new();
    let ino = map.alloc_or_get(PathBuf::from("a/b.txt"));
    assert_eq!(map.abs_path(backing, ino), Some(PathBuf::from("/workspace/proj/a/b.txt")));
    assert_eq!(map.abs_path(backing, 999_999), None);
}

/// CowMountHandle (built from options, no mount) plumbs session id and the
/// CoW stage/commit/pending contract.
#[test]
fn c02_cow_mount_handle_session_and_pending() {
    let dir = TempDir::new().expect("tempdir");
    let backing = dir.path().join("root");
    fs::create_dir(&backing).expect("root");
    fs::write(backing.join("a.txt"), b"seed-a").expect("seed");
    fs::write(backing.join("b.txt"), b"seed-b").expect("seed");

    let cow_dir = dir.path().join("cow");
    let handle = CowMountHandle::from_options(
        &backing,
        &InterceptFsOptions {
            session_id: "sess-77".to_string(),
            cow: true,
            cow_dir: Some(cow_dir.clone()),
            agent: None,
            serialize: true,
            agents_conf: None,
        },
    );
    assert_eq!(handle.session_id(), "sess-77");
    assert_eq!(handle.cow_root(), cow_dir.as_path());

    handle.stage_rel_for_agent(None, Path::new("a.txt"), b"staged-a").expect("stage");
    handle.stage_rel_for_agent(Some("agent-y"), Path::new("b.txt"), b"staged-b").expect("stage");

    let pending = handle.pending_rel_paths().expect("pending");
    assert_eq!(pending, vec![PathBuf::from("a.txt")], "default agent pending; got {pending:?}");
    let pending_y = handle.pending_rel_paths_for_agent(Some("agent-y")).expect("pending");
    assert_eq!(pending_y, vec![PathBuf::from("b.txt")], "got {pending_y:?}");

    let by_agent = handle.pending_by_agent().expect("by agent");
    let mut names: Vec<(String, Vec<PathBuf>)> = by_agent
        .into_iter()
        .filter(|(_, rels)| !rels.is_empty())
        .map(|(a, rels)| (a, rels))
        .collect();
    names.sort_by(|x, y| x.0.cmp(&y.0));
    assert_eq!(names.len(), 2, "got {names:?}");
    assert_eq!(names[0].0, "agent-y");
    assert_eq!(names[0].1, vec![PathBuf::from("b.txt")]);
    assert_eq!(names[1].0, "sess-77");
    assert_eq!(names[1].1, vec![PathBuf::from("a.txt")]);

    handle.discard_rel_for_agent(None, Path::new("a.txt")).expect("discard");
    assert!(handle.pending_rel_paths().expect("pending").is_empty());

    let discarded = handle.discard_all_for_agent(Some("agent-y")).expect("discard all");
    assert_eq!(discarded, vec![PathBuf::from("b.txt")], "got {discarded:?}");
    assert!(handle.pending_rel_paths_for_agent(Some("agent-y")).expect("pending").is_empty());
    assert_eq!(fs::read(backing.join("a.txt")).expect("read"), b"seed-a");
    assert_eq!(fs::read(backing.join("b.txt")).expect("read"), b"seed-b");
}

/// Smoke FUSE configs advertise the smoke FS name; on Linux the ACL is
/// root-and-owner (mutants returning a default Config lose both).
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn c02_session_registry_smoke_config_surfaces() {
    let cfg = smoke_fuser_config();
    let flags = format!("{:?}", cfg.mount_options);
    assert!(flags.contains("sharecli-fuse-smoke"), "smoke FSName must be set; got {flags}");
    #[cfg(target_os = "linux")]
    {
        let acl = format!("{:?}", cfg.acl);
        assert!(acl.contains("RootAndOwner"), "Linux smoke ACL must be root-and-owner; got {acl}");
    }
    let cfg_for_backend = smoke_fuser_config_for_backend(None);
    assert!(format!("{:?}", cfg_for_backend.mount_options).contains("sharecli-fuse-smoke"));
}

/// default_session_id is a process-prefixed, non-empty id (mutants returning
/// "xyzzy" / an empty string differ).
#[test]
fn c02_default_session_id_is_prefixed_and_non_empty() {
    let id = sharecli_fuse::default_session_id();
    assert!(id.starts_with("sharecli-"), "got {id:?}");
    assert!(!id.is_empty());
    assert_ne!(id, "xyzzy");
}

/// FuseMountOptions converts to full InterceptFsOptions (a mutant returning
/// Default loses session/cow/serialize).
#[test]
fn c02_fuse_mount_options_to_intercept_options() {
    let dir = TempDir::new().expect("tempdir");
    let cow_dir = dir.path().join("cow");
    let opts = FuseMountOptions {
        session_id: Some("s-1".to_string()),
        cow: true,
        cow_dir: Some(cow_dir.clone()),
        agent: Some("agent-9".to_string()),
        serialize: false,
        agents_conf: None,
    };
    let intercept = opts.to_intercept_options();
    assert_eq!(intercept.session_id, "s-1");
    assert!(intercept.cow);
    assert_eq!(intercept.cow_dir, Some(cow_dir));
    assert_eq!(intercept.agent, Some("agent-9".to_string()));
    assert!(!intercept.serialize, "serialize=false must carry through");
}

/// Cross-filesystem commit (staging on /tmp, backing on /dev/shm) exercises
/// the EXDEV copy fallback (mutants hardcoding the EXDEV constant break it).
#[test]
fn c02_write_serialize_commit_exdev_falls_back_to_copy() {
    let shm = Path::new("/dev/shm");
    if !shm.is_dir() {
        eprintln!("skipping EXDEV test: {}/ is not present", shm.display());
        return;
    }
    let dir = TempDir::new().expect("tempdir");
    let backing = shm.join(format!("sharecli-exdev-{}", std::process::id()));
    fs::write(&backing, b"seed").expect("seed on tmpfs");

    let ws = WriteSerialize::with_staging_root(dir.path().join("staging"));
    ws.stage_bytes(&backing, b"staged").expect("stage");
    ws.commit_pending(&backing).expect("commit across filesystems");
    assert_eq!(fs::read(&backing).expect("read"), b"staged");
    let _ = fs::remove_file(&backing);
}
