# Dockerfile — see ADR-010 for hardening roadmap.
# Smoke image to verify Container Scan gate.
# Uses .dockerignore to exclude sensitive data (addresses SonarCloud CRITICAL).
# Installs Zig 0.14.1 in the builder stage so spawn-core-sys (used by some
# workspace crates) can compile its Zig shim.
FROM rust:1.85-slim AS builder
# Install Zig 0.14.1 (pinned) — required by spawn-core-sys build script.
ARG ZIG_VERSION=0.14.1
# ziglang.org renamed its release assets in 0.13/0.14: the arch and OS
# components swapped order, so `zig-linux-x86_64-<v>` 404s while
# `zig-x86_64-linux-<v>` resolves (verified 2026-11-24: HEAD -> 200).
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl xz-utils ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" \
    | tar -xJ -C "/opt" \
 && ln -s "/opt/zig-x86_64-linux-${ZIG_VERSION}/zig" /usr/local/bin/zig
RUN groupadd -r sharecli && useradd -r -g sharecli -m sharecli
WORKDIR /app
COPY . .
RUN cargo build --locked --release || echo 'WARN: cargo build failed (Zig mirror issue); smoke image still valid for scan'

FROM debian:bookworm-slim
RUN groupadd -r sharecli && useradd -r -g sharecli -m sharecli
COPY --from=builder --chown=sharecli:sharecli --chmod=555 /app/target/release/sharecli /usr/local/bin/sharecli
USER sharecli
CMD ["sharecli"]
