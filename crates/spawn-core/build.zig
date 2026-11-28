const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // --- Static library consumed by Rust via build.rs / extern "C" ---
    const lib_module = b.createModule(.{
        .root_source_file = b.path("src/spawn_core.zig"),
        .target = target,
        .optimize = optimize,
    });
    lib_module.link_libc = true;
    // Keep stack probes out of the .a so consumers do not need Zig compiler-rt
    // when we skip bundling it (Darwin path uses build-obj from build.rs).
    lib_module.stack_check = false;
    // A static library embedded in an arbitrary consumer must be position
    // independent: Rust's musl targets link the binary as -static-pie, where a
    // non-PIC object fails with "relocation R_X86_64_32S ... recompile with -fPIE".
    lib_module.pic = true;

    const lib = b.addLibrary(.{
        .name = "spawn_core",
        .root_module = lib_module,
        .linkage = .static,
    });
    lib.linkLibC();
    // Required on Linux so the final rustc link finds __zig_probe_stack from
    // any remaining probe sites. On macOS, `zig build` of this static lib
    // fails resolving libSystem — build.rs uses `zig build-obj` + `ar` instead.
    if (target.result.os.tag != .macos) {
        lib.bundle_compiler_rt = true;
    }
    b.installArtifact(lib);

    // --- Test step: `zig build test` ---
    const test_module = b.createModule(.{
        .root_source_file = b.path("src/spawn_core.zig"),
        .target = target,
        .optimize = optimize,
    });
    test_module.link_libc = true;
    test_module.stack_check = false;

    const unit_tests = b.addTest(.{
        .root_module = test_module,
    });
    unit_tests.linkLibC();

    const run_tests = b.addRunArtifact(unit_tests);
    const test_step = b.step("test", "Run spawn-core unit tests");
    test_step.dependOn(&run_tests.step);
}
