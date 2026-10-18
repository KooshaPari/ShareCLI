//! Platform-gated FUSE implementation for the sharecli hypervisor.
//!
//! Extracted from the original `mod platform { ... }` inline block in
//! `lib.rs` during the decompose-top3 refactor (FR-001, 500L module cap).
//! Owns the per-platform `InterceptFs` (FUSE Filesystem impl) and the
//! thread-shared `SharedInterceptFs` wrapper.
//!
//! The crate-root `mod platform;` declaration is gated to
//! `target_os = "linux" | target_os = "macos"`; Windows / unsupported
//! platforms see no `InterceptFs` symbol.

    use std::{
        ffi::OsStr,
        fs::{self, OpenOptions},
        io::{Seek, SeekFrom, Write as IoWrite},
        os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        path::{Path, PathBuf},
        sync::Mutex,
        time::{Duration, SystemTime},
    };

    use fuser::{
        BsdFileFlags, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation,
        INodeNo, OpenFlags, RenameFlags, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory,
        ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request, TimeOrNow, WriteFlags,
    };
    use tracing::{debug, trace};

    use crate::agent_cow::AgentCowStore;
    use crate::agents_conf::AgentsConf;
    use crate::inode_map::{abs_under, InodeMap, ROOT_INO};
    use crate::neg_dentry::{NegDentryMeters, NegativeDentryCache, DEFAULT_NEG_TTL};
    use crate::provenance::{annotate_write, default_session_id};
    use crate::read_cache::{ReadCacheMeters, ReadContentCache};
    use crate::write_serialize_meters::record_passthrough_write;
    use crate::InterceptFsOptions;

    const TTL: Duration = Duration::from_secs(1);

    /// Passthrough FUSE filesystem with read coalesce + write-serialize hooks.
    pub struct InterceptFs {
        /// Root of the real filesystem subtree being mirrored.
        backing: PathBuf,
        /// Session id stamped onto every write / CoW commit (`user.sharecli.session`).
        session_id: String,
        /// When true, CoW overlays are per-agent under [`AgentCowStore::cow_root`].
        cow_enabled: bool,
        /// Optional Feb `agents.conf` patterns (informational / CLI validation).
        agents_conf: Option<AgentsConf>,
        inodes: Mutex<InodeMap>,
        read_cache: Mutex<ReadContentCache>,
        neg_dentry: Mutex<NegativeDentryCache>,
        cow: AgentCowStore,
    }

    impl InterceptFs {
        /// Create a new [`InterceptFs`] rooted at `backing` with a default session id.
        pub fn new(backing: &Path) -> Self {
            Self::with_session(backing, default_session_id())
        }

        /// Create a new [`InterceptFs`] rooted at `backing` with an explicit session id.
        pub fn with_session(backing: &Path, session_id: impl Into<String>) -> Self {
            Self::with_options(
                backing,
                InterceptFsOptions {
                    session_id: session_id.into(),
                    ..InterceptFsOptions::default()
                },
            )
        }

        /// Create with full Feb-parity mount options (`--cow`, `--cow-dir`, …).
        pub fn with_options(backing: &Path, opts: InterceptFsOptions) -> Self {
            let session_id =
                if opts.session_id.is_empty() { default_session_id() } else { opts.session_id };
            let default_agent = opts.agent.clone().unwrap_or_else(|| session_id.clone());
            let cow_root = opts.cow_dir.unwrap_or_else(|| {
                if opts.cow {
                    backing.join(".sharecli-cow")
                } else {
                    backing.join(".sharecli-cow-staging")
                }
            });
            let agents_conf = opts.agents_conf.as_ref().and_then(|p| AgentsConf::load(p).ok());
            Self {
                backing: backing.to_path_buf(),
                session_id,
                cow_enabled: opts.cow,
                agents_conf,
                inodes: Mutex::new(InodeMap::new()),
                read_cache: Mutex::new(ReadContentCache::new()),
                neg_dentry: Mutex::new(NegativeDentryCache::with_ttl(DEFAULT_NEG_TTL)),
                cow: AgentCowStore::new(cow_root, default_agent, opts.serialize),
            }
        }

        /// Backing root path.
        pub fn backing(&self) -> &Path {
            &self.backing
        }

        /// Session id used for write provenance xattrs.
        pub fn session_id(&self) -> &str {
            &self.session_id
        }

        /// Whether per-agent CoW mode is enabled.
        pub fn cow_enabled(&self) -> bool {
            self.cow_enabled
        }

        /// CoW overlay root.
        pub fn cow_root(&self) -> &Path {
            self.cow.cow_root()
        }

        /// Default agent id for unscoped CoW ops.
        pub fn default_agent(&self) -> &str {
            self.cow.default_agent()
        }

        /// Loaded `agents.conf`, if any.
        pub fn agents_conf(&self) -> Option<&AgentsConf> {
            self.agents_conf.as_ref()
        }

        /// Whether write serialization locks are enabled.
        pub fn serialize_writes(&self) -> bool {
            self.cow.serialize()
        }

        /// Read-coalesce meters (hits / misses) without mounting.
        pub fn cache_meters(&self) -> ReadCacheMeters {
            self.read_cache.lock().expect("read cache lock").meters()
        }

        /// Negative-dentry meters (hits / misses) without mounting.
        pub fn neg_dentry_meters(&self) -> NegDentryMeters {
            self.neg_dentry.lock().expect("neg dentry lock").meters()
        }

        /// Probe whether relative `rel` exists under the backing root.
        ///
        /// Uses the negative dentry cache: a prior ENOENT within TTL returns
        /// `Ok(false)` without re-statting. A positive result invalidates any
        /// stale negative entry for `rel`.
        pub fn exists_rel(&self, rel: &Path) -> std::io::Result<bool> {
            {
                let mut neg = self.neg_dentry.lock().expect("neg dentry lock");
                if neg.is_negative(rel) {
                    crate::neg_dentry::record_global_neg_hit();
                    return Ok(false);
                }
            }
            let abs = abs_under(&self.backing, rel);
            match fs::metadata(&abs) {
                Ok(_) => {
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.invalidate(rel);
                    }
                    Ok(true)
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.remember_miss(rel.to_path_buf());
                        crate::neg_dentry::record_global_neg_miss();
                    }
                    Ok(false)
                }
                Err(err) => Err(err),
            }
        }

        /// Drop a negative-dentry entry for `rel` (create / mkdir / rename-into).
        pub fn invalidate_neg_rel(&self, rel: &Path) {
            if let Ok(mut neg) = self.neg_dentry.lock() {
                neg.invalidate(rel);
            }
        }

        /// Read a relative path through the in-process coalesce cache (no mount).
        pub fn read_coalesced_rel(&self, rel: &Path) -> std::io::Result<Vec<u8>> {
            let abs = abs_under(&self.backing, rel);
            self.read_cache.lock().expect("read cache lock").read_coalesced(&abs)
        }

        /// Stage CoW bytes for a relative path (no mount; FR-009 helpers).
        pub fn stage_rel(
            &self,
            rel: &Path,
            contents: &[u8],
        ) -> Result<(), crate::WriteSerializeError> {
            self.stage_rel_for_agent(None, rel, contents)
        }

        /// Stage CoW bytes for `agent` (default agent when `None`).
        pub fn stage_rel_for_agent(
            &self,
            agent: Option<&str>,
            rel: &Path,
            contents: &[u8],
        ) -> Result<(), crate::WriteSerializeError> {
            let abs = abs_under(&self.backing, rel);
            self.cow.stage_bytes(agent, &abs, contents)
        }

        /// Commit pending CoW staging for a relative path; invalidates read cache
        /// and stamps write provenance xattrs on the promoted backing file.
        pub fn commit_rel(&self, rel: &Path) -> Result<(), crate::WriteSerializeError> {
            self.commit_rel_for_agent(None, rel)
        }

        /// Commit pending CoW for `agent` (default when `None`).
        pub fn commit_rel_for_agent(
            &self,
            agent: Option<&str>,
            rel: &Path,
        ) -> Result<(), crate::WriteSerializeError> {
            let abs = abs_under(&self.backing, rel);
            self.cow.commit_pending(agent, &abs)?;
            annotate_write(&abs, &self.session_id).map_err(crate::WriteSerializeError::Io)?;
            if let Ok(mut cache) = self.read_cache.lock() {
                cache.invalidate(&abs);
            }
            Ok(())
        }

        /// Commit all pending CoW paths for `agent` (default when `None`).
        pub fn commit_all_for_agent(
            &self,
            agent: Option<&str>,
        ) -> Result<Vec<PathBuf>, crate::WriteSerializeError> {
            let abs_paths = self.cow.commit_all_for_agent(agent)?;
            let mut rels = Vec::new();
            for abs in abs_paths {
                annotate_write(&abs, &self.session_id).map_err(crate::WriteSerializeError::Io)?;
                if let Ok(mut cache) = self.read_cache.lock() {
                    cache.invalidate(&abs);
                }
                if let Ok(rel) = abs.strip_prefix(&self.backing) {
                    rels.push(rel.to_path_buf());
                }
            }
            Ok(rels)
        }

        /// Discard pending CoW staging for a relative path (backing unchanged).
        pub fn discard_rel(&self, rel: &Path) -> Result<(), crate::WriteSerializeError> {
            self.discard_rel_for_agent(None, rel)
        }

        /// Discard pending CoW for `agent` (default when `None`).
        pub fn discard_rel_for_agent(
            &self,
            agent: Option<&str>,
            rel: &Path,
        ) -> Result<(), crate::WriteSerializeError> {
            let abs = abs_under(&self.backing, rel);
            self.cow.discard_pending(agent, &abs)
        }

        /// Discard all pending CoW paths for `agent` (default when `None`).
        pub fn discard_all_for_agent(
            &self,
            agent: Option<&str>,
        ) -> Result<Vec<PathBuf>, crate::WriteSerializeError> {
            let abs_paths = self.cow.discard_all_for_agent(agent)?;
            Ok(abs_paths
                .into_iter()
                .filter_map(|abs| abs.strip_prefix(&self.backing).ok().map(Path::to_path_buf))
                .collect())
        }

        /// Passthrough write at `offset` for relative `rel`, serialized per path.
        ///
        /// On success, stamps [`crate::ATTR_SESSION`] / [`crate::ATTR_WRITTEN_AT`]
        /// on the backing file (write provenance).
        pub fn write_rel(&self, rel: &Path, offset: u64, data: &[u8]) -> std::io::Result<u32> {
            let abs = abs_under(&self.backing, rel);
            let data = data.to_vec();
            let session = self.session_id.clone();
            let n = self
                .cow
                .with_locked_path(None, &abs, || {
                    let mut file = OpenOptions::new().write(true).open(&abs)?;
                    file.seek(SeekFrom::Start(offset))?;
                    file.write_all(&data)?;
                    annotate_write(&abs, &session)?;
                    Ok::<u32, std::io::Error>(data.len() as u32)
                })
                .map_err(|e| std::io::Error::other(e.to_string()))??;
            record_passthrough_write();
            if let Ok(mut cache) = self.read_cache.lock() {
                cache.invalidate(&abs);
            }
            Ok(n)
        }

        /// Create a new regular file at relative `rel` (no mount; FR-009 helper).
        ///
        /// Invalidates negative dentry + read cache and stamps write provenance.
        pub fn create_rel(&self, rel: &Path, mode: u32) -> std::io::Result<()> {
            let abs = abs_under(&self.backing, rel);
            if let Some(parent) = abs.parent() {
                fs::create_dir_all(parent)?;
            }
            OpenOptions::new().write(true).create_new(true).mode(mode).open(&abs)?;
            self.after_create_at(rel, &abs)
        }

        /// Relative paths with pending CoW staging on the default agent.
        pub fn pending_rel_paths(&self) -> Result<Vec<PathBuf>, crate::WriteSerializeError> {
            Ok(self
                .cow
                .pending_for_agent(None)?
                .into_iter()
                .filter_map(|abs| abs.strip_prefix(&self.backing).ok().map(Path::to_path_buf))
                .collect())
        }

        /// Pending CoW paths grouped by agent (absolute backing paths stripped to rel).
        pub fn pending_by_agent(
            &self,
        ) -> Result<Vec<(String, Vec<PathBuf>)>, crate::WriteSerializeError> {
            let mut out = Vec::new();
            for entry in self.cow.list_agent_pending()? {
                let rels = entry
                    .backing_paths
                    .into_iter()
                    .filter_map(|abs| abs.strip_prefix(&self.backing).ok().map(Path::to_path_buf))
                    .collect::<Vec<_>>();
                if !rels.is_empty() {
                    out.push((entry.agent, rels));
                }
            }
            Ok(out)
        }

        fn after_create_at(&self, rel: &Path, abs: &Path) -> std::io::Result<()> {
            annotate_write(abs, &self.session_id)?;
            if let Ok(mut neg) = self.neg_dentry.lock() {
                neg.invalidate(rel);
            }
            if let Ok(mut cache) = self.read_cache.lock() {
                cache.invalidate(abs);
            }
            Ok(())
        }

        fn install_created_entry(&self, rel: PathBuf, path: PathBuf, reply: ReplyCreate) {
            let mut map = self.inodes.lock().expect("inode map");
            let ino = map.alloc_or_get(rel);
            match fs::metadata(&path) {
                Ok(meta) => {
                    let attr = Self::metadata_to_attr(ino, &meta);
                    reply.created(&TTL, &attr, Generation(0), FileHandle(ino), FopenFlags::empty());
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn install_created_entry_plain(&self, rel: PathBuf, path: PathBuf, reply: ReplyEntry) {
            let mut map = self.inodes.lock().expect("inode map");
            let ino = map.alloc_or_get(rel);
            match fs::metadata(&path) {
                Ok(meta) => reply.entry(&TTL, &Self::metadata_to_attr(ino, &meta), Generation(0)),
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn io_errno(err: std::io::Error) -> Errno {
            Errno::from(err)
        }

        fn meta_kind(meta: &fs::Metadata) -> FileType {
            if meta.is_dir() {
                FileType::Directory
            } else if meta.is_symlink() {
                FileType::Symlink
            } else {
                FileType::RegularFile
            }
        }

        fn metadata_to_attr(ino: u64, meta: &fs::Metadata) -> FileAttr {
            let kind = Self::meta_kind(meta);
            let now = SystemTime::now();
            FileAttr {
                ino: INodeNo(ino),
                size: meta.len(),
                blocks: meta.blocks(),
                atime: meta.accessed().unwrap_or(now),
                mtime: meta.modified().unwrap_or(now),
                ctime: now,
                crtime: now,
                kind,
                perm: meta.mode() as u16,
                nlink: meta.nlink() as u32,
                uid: meta.uid(),
                gid: meta.gid(),
                rdev: meta.rdev() as u32,
                blksize: 512,
                flags: 0,
            }
        }
    }

    impl Filesystem for InterceptFs {
        fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
            trace!(?parent, ?name, "lookup");
            let mut map = self.inodes.lock().expect("inode map");
            let Some(rel) = map.child_rel(parent.0, name) else {
                reply.error(Errno::ENOENT);
                return;
            };
            {
                let mut neg = self.neg_dentry.lock().expect("neg dentry lock");
                if neg.is_negative(&rel) {
                    crate::neg_dentry::record_global_neg_hit();
                    reply.error(Errno::ENOENT);
                    return;
                }
            }
            let Some((ino, rel)) = map.lookup_or_alloc(parent.0, name) else {
                reply.error(Errno::ENOENT);
                return;
            };
            let path = abs_under(&self.backing, &rel);
            match fs::metadata(&path) {
                Ok(meta) => {
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.invalidate(&rel);
                    }
                    let attr = Self::metadata_to_attr(ino, &meta);
                    reply.entry(&TTL, &attr, Generation(0));
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::NotFound {
                        if let Ok(mut neg) = self.neg_dentry.lock() {
                            neg.remember_miss(rel.clone());
                            crate::neg_dentry::record_global_neg_miss();
                        }
                    }
                    map.remove_rel(&rel);
                    reply.error(Self::io_errno(err));
                }
            }
        }

        fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
            trace!(?ino, "getattr");
            let map = self.inodes.lock().expect("inode map");
            let Some(path) = map.abs_path(&self.backing, ino.0) else {
                reply.error(Errno::ENOENT);
                return;
            };
            match fs::metadata(&path) {
                Ok(meta) => reply.attr(&TTL, &Self::metadata_to_attr(ino.0, &meta)),
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn setattr(
            &self,
            _req: &Request,
            ino: INodeNo,
            mode: Option<u32>,
            _uid: Option<u32>,
            _gid: Option<u32>,
            size: Option<u64>,
            _atime: Option<TimeOrNow>,
            _mtime: Option<TimeOrNow>,
            _ctime: Option<SystemTime>,
            _fh: Option<FileHandle>,
            _crtime: Option<SystemTime>,
            _chgtime: Option<SystemTime>,
            _bkuptime: Option<SystemTime>,
            _flags: Option<BsdFileFlags>,
            reply: ReplyAttr,
        ) {
            // Required for std::fs::write (open+truncate) through the mount — default
            // fuser setattr is ENOSYS and breaks privileged mount smoke (AC-009.8).
            let path = {
                let map = self.inodes.lock().expect("inode map");
                match map.abs_path(&self.backing, ino.0) {
                    Some(p) => p,
                    None => {
                        reply.error(Errno::ENOENT);
                        return;
                    }
                }
            };
            if let Some(new_size) = size {
                if let Err(err) =
                    OpenOptions::new().write(true).open(&path).and_then(|f| f.set_len(new_size))
                {
                    reply.error(Self::io_errno(err));
                    return;
                }
            }
            if let Some(mode) = mode {
                let perms = fs::Permissions::from_mode(mode);
                if let Err(err) = fs::set_permissions(&path, perms) {
                    reply.error(Self::io_errno(err));
                    return;
                }
            }
            match fs::metadata(&path) {
                Ok(meta) => {
                    if let Ok(mut cache) = self.read_cache.lock() {
                        cache.invalidate(&path);
                    }
                    reply.attr(&TTL, &Self::metadata_to_attr(ino.0, &meta));
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn open(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
            let map = self.inodes.lock().expect("inode map");
            let Some(path) = map.abs_path(&self.backing, ino.0) else {
                reply.error(Errno::ENOENT);
                return;
            };
            match fs::metadata(&path) {
                Ok(meta) if meta.is_file() => {
                    reply.opened(FileHandle(ino.0), FopenFlags::empty());
                }
                Ok(_) => reply.error(Errno::EISDIR),
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn read(
            &self,
            _req: &Request,
            ino: INodeNo,
            _fh: FileHandle,
            offset: u64,
            size: u32,
            _flags: OpenFlags,
            _lock: Option<fuser::LockOwner>,
            reply: ReplyData,
        ) {
            debug!(?ino, offset, size, "read");
            let path = {
                let map = self.inodes.lock().expect("inode map");
                match map.abs_path(&self.backing, ino.0) {
                    Some(p) => p,
                    None => {
                        reply.error(Errno::ENOENT);
                        return;
                    }
                }
            };
            match self.read_cache.lock().expect("read cache").read_slice(&path, offset, size) {
                Ok(buf) => reply.data(&buf),
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn write(
            &self,
            _req: &Request,
            ino: INodeNo,
            _fh: FileHandle,
            offset: u64,
            data: &[u8],
            _write_flags: WriteFlags,
            _flags: OpenFlags,
            _lock: Option<fuser::LockOwner>,
            reply: ReplyWrite,
        ) {
            debug!(?ino, offset, len = data.len(), "write");
            let path = {
                let map = self.inodes.lock().expect("inode map");
                match map.abs_path(&self.backing, ino.0) {
                    Some(p) => p,
                    None => {
                        reply.error(Errno::ENOENT);
                        return;
                    }
                }
            };
            let payload = data.to_vec();
            let session = self.session_id.clone();
            let result = self.cow.with_locked_path(None, &path, || {
                let mut file = OpenOptions::new().write(true).open(&path)?;
                file.seek(SeekFrom::Start(offset))?;
                file.write_all(&payload)?;
                annotate_write(&path, &session)?;
                Ok::<u32, std::io::Error>(payload.len() as u32)
            });
            match result {
                Ok(Ok(n)) => {
                    record_passthrough_write();
                    if let Ok(mut cache) = self.read_cache.lock() {
                        cache.invalidate(&path);
                    }
                    reply.written(n);
                }
                Ok(Err(err)) => reply.error(Self::io_errno(err)),
                Err(err) => reply.error(Errno::from(std::io::Error::other(err.to_string()))),
            }
        }

        fn readdir(
            &self,
            _req: &Request,
            ino: INodeNo,
            _fh: FileHandle,
            offset: u64,
            mut reply: ReplyDirectory,
        ) {
            debug!(?ino, offset, "readdir");
            let entries = {
                let mut map = self.inodes.lock().expect("inode map");
                let Some(rel) = map.resolve(ino.0).map(Path::to_path_buf) else {
                    reply.error(Errno::ENOENT);
                    return;
                };
                let parent_ino = if ino.0 == ROOT_INO {
                    ROOT_INO
                } else if rel.components().count() <= 1 {
                    map.alloc_or_get(PathBuf::new())
                } else if let Some(parent_rel) = rel.parent() {
                    map.alloc_or_get(parent_rel.to_path_buf())
                } else {
                    ROOT_INO
                };
                let dir_path = abs_under(&self.backing, &rel);
                let read_dir = match fs::read_dir(&dir_path) {
                    Ok(rd) => rd,
                    Err(err) => {
                        reply.error(Self::io_errno(err));
                        return;
                    }
                };
                let mut entries: Vec<(u64, FileType, std::ffi::OsString)> = Vec::new();
                entries.push((ino.0, FileType::Directory, std::ffi::OsString::from(".")));
                entries.push((parent_ino, FileType::Directory, std::ffi::OsString::from("..")));
                for ent in read_dir.flatten() {
                    let name = ent.file_name();
                    let child_rel = crate::inode_map::join_rel(&rel, &name);
                    let child_ino = map.alloc_or_get(child_rel);
                    let kind = ent
                        .metadata()
                        .map(|m| Self::meta_kind(&m))
                        .unwrap_or(FileType::RegularFile);
                    entries.push((child_ino, kind, name));
                }
                entries
            };
            for (i, (child_ino, kind, name)) in
                entries.into_iter().enumerate().skip(offset as usize)
            {
                let next_offset = (i + 1) as u64;
                if reply.add(INodeNo(child_ino), next_offset, kind, &name) {
                    break;
                }
            }
            reply.ok();
        }

        fn mkdir(
            &self,
            _req: &Request,
            parent: INodeNo,
            name: &OsStr,
            mode: u32,
            _umask: u32,
            reply: ReplyEntry,
        ) {
            let mut map = self.inodes.lock().expect("inode map");
            let Some(rel) = map.child_rel(parent.0, name) else {
                reply.error(Errno::ENOENT);
                return;
            };
            let path = abs_under(&self.backing, &rel);
            match fs::create_dir(&path) {
                Ok(()) => {
                    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(mode));
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.invalidate(&rel);
                    }
                    let ino = map.alloc_or_get(rel.clone());
                    match fs::metadata(&path) {
                        Ok(meta) => {
                            reply.entry(&TTL, &Self::metadata_to_attr(ino, &meta), Generation(0))
                        }
                        Err(err) => reply.error(Self::io_errno(err)),
                    }
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn create(
            &self,
            _req: &Request,
            parent: INodeNo,
            name: &OsStr,
            mode: u32,
            umask: u32,
            _flags: i32,
            reply: ReplyCreate,
        ) {
            debug!(?parent, ?name, mode, "create");
            let rel = {
                let map = self.inodes.lock().expect("inode map");
                let Some(rel) = map.child_rel(parent.0, name) else {
                    reply.error(Errno::ENOENT);
                    return;
                };
                rel
            };
            let path = abs_under(&self.backing, &rel);
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    reply.error(Errno::EIO);
                    return;
                }
            }
            let file_mode = mode & !umask;
            match OpenOptions::new().write(true).create_new(true).mode(file_mode).open(&path) {
                Ok(_file) => {
                    if let Err(err) = self.after_create_at(&rel, &path) {
                        let _ = fs::remove_file(&path);
                        reply.error(Self::io_errno(err));
                        return;
                    }
                    self.install_created_entry(rel, path, reply);
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn mknod(
            &self,
            _req: &Request,
            parent: INodeNo,
            name: &OsStr,
            mode: u32,
            umask: u32,
            _rdev: u32,
            reply: ReplyEntry,
        ) {
            const S_IFREG: u32 = 0o100_000;
            if mode & 0o170_000 != S_IFREG {
                reply.error(Errno::EPERM);
                return;
            }
            let rel = {
                let map = self.inodes.lock().expect("inode map");
                let Some(rel) = map.child_rel(parent.0, name) else {
                    reply.error(Errno::ENOENT);
                    return;
                };
                rel
            };
            let path = abs_under(&self.backing, &rel);
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_err() {
                    reply.error(Errno::EIO);
                    return;
                }
            }
            let file_mode = mode & !umask;
            match OpenOptions::new().write(true).create_new(true).mode(file_mode).open(&path) {
                Ok(_file) => {
                    if let Err(err) = self.after_create_at(&rel, &path) {
                        let _ = fs::remove_file(&path);
                        reply.error(Self::io_errno(err));
                        return;
                    }
                    self.install_created_entry_plain(rel, path, reply);
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
            let mut map = self.inodes.lock().expect("inode map");
            let Some(rel) = map.child_rel(parent.0, name) else {
                reply.error(Errno::ENOENT);
                return;
            };
            let path = abs_under(&self.backing, &rel);
            match fs::remove_file(&path) {
                Ok(()) => {
                    if let Ok(mut cache) = self.read_cache.lock() {
                        cache.invalidate(&path);
                    }
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.remember_miss(rel.clone());
                        crate::neg_dentry::record_global_neg_miss();
                    }
                    map.remove_rel(&rel);
                    reply.ok();
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
            let mut map = self.inodes.lock().expect("inode map");
            let Some(rel) = map.child_rel(parent.0, name) else {
                reply.error(Errno::ENOENT);
                return;
            };
            let path = abs_under(&self.backing, &rel);
            match fs::remove_dir(&path) {
                Ok(()) => {
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.remember_miss(rel.clone());
                        crate::neg_dentry::record_global_neg_miss();
                    }
                    map.remove_rel(&rel);
                    reply.ok();
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }

        fn rename(
            &self,
            _req: &Request,
            parent: INodeNo,
            name: &OsStr,
            newparent: INodeNo,
            newname: &OsStr,
            _flags: RenameFlags,
            reply: ReplyEmpty,
        ) {
            let mut map = self.inodes.lock().expect("inode map");
            let Some(old_rel) = map.child_rel(parent.0, name) else {
                reply.error(Errno::ENOENT);
                return;
            };
            let Some(new_rel) = map.child_rel(newparent.0, newname) else {
                reply.error(Errno::ENOENT);
                return;
            };
            let old_path = abs_under(&self.backing, &old_rel);
            let new_path = abs_under(&self.backing, &new_rel);
            match fs::rename(&old_path, &new_path) {
                Ok(()) => {
                    if let Ok(mut cache) = self.read_cache.lock() {
                        cache.invalidate(&old_path);
                        cache.invalidate(&new_path);
                    }
                    if let Ok(mut neg) = self.neg_dentry.lock() {
                        neg.remember_miss(old_rel.clone());
                        crate::neg_dentry::record_global_neg_miss();
                        neg.invalidate(&new_rel);
                    }
                    map.rename_rel(&old_rel, new_rel);
                    reply.ok();
                }
                Err(err) => reply.error(Self::io_errno(err)),
            }
        }
    }

    /// Mount with an explicit write-provenance session id (Hypervisor coalesce key).
    pub fn mount_with_session(
        mountpoint: &Path,
        backing: &Path,
        session_id: &str,
    ) -> anyhow::Result<()> {
        // Smoke/ephemeral mounts: no AutoUnmount (avoids allow_other / user_allow_other).
        // Callers and FuseGuard Drop force-unmount explicitly.
        #[cfg(target_os = "macos")]
        {
            use crate::{select_backend, FuseBackend};

            let attempt = |backend: Option<FuseBackend>| {
                let config = crate::session_registry::smoke_fuser_config_for_backend(backend);
                fuser::mount(InterceptFs::with_session(backing, session_id), mountpoint, &config)
            };

            match select_backend() {
                FuseBackend::Kernel => match attempt(Some(FuseBackend::Kernel)) {
                    Ok(()) => Ok(()),
                    Err(kernel_err) => {
                        // macFUSE may leave a transient mount registration after a
                        // failed backend negotiation (reported as EEXIST on retry).
                        // Only recycle an empty mountpoint; never remove user data.
                        let _ = crate::mount_smoke::force_unmount(mountpoint);
                        if let Ok(mut entries) = std::fs::read_dir(mountpoint) {
                            if entries.next().is_none() {
                                let _ = std::fs::remove_dir(mountpoint);
                                let _ = std::fs::create_dir(mountpoint);
                            }
                        }
                        attempt(Some(FuseBackend::Fskit))
                            .map_err(|fskit_err| anyhow::anyhow!(
                                "kernel backend failed: {kernel_err}; FSKit fallback failed: {fskit_err}; {}",
                                crate::backend::runtime_diagnostics()
                            ))
                    }
                },
                FuseBackend::Fskit => attempt(Some(FuseBackend::Fskit)).map_err(|err| {
                    anyhow::anyhow!(
                        "FSKit backend mount failed at {}: {err}; {}",
                        mountpoint.display(),
                        crate::backend::runtime_diagnostics()
                    )
                }),
                FuseBackend::Unavailable => attempt(None).map_err(|err| {
                    anyhow::anyhow!(
                        "FUSE backend unavailable; mount failed at {}: {err}; {}",
                        mountpoint.display(),
                        crate::backend::runtime_diagnostics()
                    )
                }),
            }?;
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let fs = InterceptFs::with_session(backing, session_id);
            let config = crate::session_registry::smoke_fuser_config();
            fuser::mount(fs, mountpoint, &config)?;
            Ok(())
        }
    }

    /// Share [`InterceptFs`] across FUSE session threads and the session registry.
    pub(crate) struct SharedInterceptFs(pub std::sync::Arc<InterceptFs>);

    impl Filesystem for SharedInterceptFs {
        fn lookup(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
            self.0.lookup(req, parent, name, reply);
        }
        fn getattr(&self, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr) {
            self.0.getattr(req, ino, fh, reply);
        }
        fn setattr(
            &self,
            req: &Request,
            ino: INodeNo,
            mode: Option<u32>,
            uid: Option<u32>,
            gid: Option<u32>,
            size: Option<u64>,
            atime: Option<TimeOrNow>,
            mtime: Option<TimeOrNow>,
            ctime: Option<SystemTime>,
            fh: Option<FileHandle>,
            crtime: Option<SystemTime>,
            chgtime: Option<SystemTime>,
            bkuptime: Option<SystemTime>,
            flags: Option<BsdFileFlags>,
            reply: ReplyAttr,
        ) {
            self.0.setattr(
                req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime,
                flags, reply,
            );
        }
        fn open(&self, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
            self.0.open(req, ino, flags, reply);
        }
        fn read(
            &self,
            req: &Request,
            ino: INodeNo,
            fh: FileHandle,
            offset: u64,
            size: u32,
            flags: OpenFlags,
            lock: Option<fuser::LockOwner>,
            reply: ReplyData,
        ) {
            self.0.read(req, ino, fh, offset, size, flags, lock, reply);
        }
        fn write(
            &self,
            req: &Request,
            ino: INodeNo,
            fh: FileHandle,
            offset: u64,
            data: &[u8],
            write_flags: WriteFlags,
            flags: OpenFlags,
            lock: Option<fuser::LockOwner>,
            reply: ReplyWrite,
        ) {
            self.0.write(req, ino, fh, offset, data, write_flags, flags, lock, reply);
        }
        fn readdir(
            &self,
            req: &Request,
            ino: INodeNo,
            fh: FileHandle,
            offset: u64,
            reply: ReplyDirectory,
        ) {
            self.0.readdir(req, ino, fh, offset, reply);
        }
        fn mkdir(
            &self,
            req: &Request,
            parent: INodeNo,
            name: &OsStr,
            mode: u32,
            umask: u32,
            reply: ReplyEntry,
        ) {
            self.0.mkdir(req, parent, name, mode, umask, reply);
        }
        fn create(
            &self,
            req: &Request,
            parent: INodeNo,
            name: &OsStr,
            mode: u32,
            umask: u32,
            flags: i32,
            reply: ReplyCreate,
        ) {
            self.0.create(req, parent, name, mode, umask, flags, reply);
        }
        fn mknod(
            &self,
            req: &Request,
            parent: INodeNo,
            name: &OsStr,
            mode: u32,
            umask: u32,
            rdev: u32,
            reply: ReplyEntry,
        ) {
            self.0.mknod(req, parent, name, mode, umask, rdev, reply);
        }
        fn unlink(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
            self.0.unlink(req, parent, name, reply);
        }
        fn rmdir(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
            self.0.rmdir(req, parent, name, reply);
        }
        fn rename(
            &self,
            req: &Request,
            parent: INodeNo,
            name: &OsStr,
            newparent: INodeNo,
            newname: &OsStr,
            flags: RenameFlags,
            reply: ReplyEmpty,
        ) {
            self.0.rename(req, parent, name, newparent, newname, flags, reply);
        }
    }
