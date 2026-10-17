# Queue ownership repair: candidate, not acceptance

Reviewed branch: fd0fcb7eb86e068dc86949bd5bf67930bb46ce99.
Prior native failure: Mature Recovery Oracle 37053571718, queue job 110993205434. The failing PID-reuse test must remain a positive regression control. Prior default-checkout jobs exercised PR merge 34ea3d47b709fee034ba87561d6c335d207eef88, not the branch SHA.

The candidate uses a held exclusive file lock as waiter authority. PID, file existence and a priority filename cannot establish ownership. A uniquely named temporary file is locked before no-clobber publication. Shared peer probes avoid creating transient phantom owners. Per-lane ordering is assigned under a separate append-only sequence journal lock; numeric ordering does not use PID or wall-clock ties. Torn sequence tails fail closed.

The existing 28 queue test contracts are retained in queue_tests.rs, with positive synthetic waiter fixtures changed to hold actual leases. Ten ownership/journal tests are added. Existing PID-reuse and panic integration tests remain unchanged. A new integration test kills a real child while it is waiting, leaves its old file in place, and requires a new waiter to proceed.

Local evidence: seven Linux kernel-lock protocol checks passed. This is diagnostic protocol evidence, not compiled Rust. No rustc/cargo is available in the local work environment; native Rust result is NOT_RUN. The new exact-checkout CI job checks each named positive test and rejects zero/ignored/missing/duplicate tests, compiler errors and collector failure. Receipt includes candidate, configuration, tool versions, binary/source hashes, timestamps and raw logs.

Limits: cooperative local Linux filesystem only; mixed old/new queue clients, remote-filesystem lock behavior, orphan-file maintenance and power-loss persistence of the ordering journal are not qualified. Slot paths/public API remain compatible. Old unleased files are ignored, not attributed to a live PID or removed by peers. No default Hypervisor routing, main branch, release or authorization gate is changed.
