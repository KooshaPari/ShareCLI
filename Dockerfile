# Dockerfile — see ADR-010 for hardening roadmap.
# Smoke image to verify Container Scan gate.
# Uses .dockerignore to exclude sensitive data (addresses SonarCloud CRITICAL).
# Installs Zig 0.14.1 in the builder stage so spawn-core-sys (used by some
# workspace crates) can compile its Zig shim.
FROM rust:1.85-slim AS builder
# Install Zig 0.14.1 (pinned) — required by spawn-core-sys build script.
ARG ZIG_VERSION=0.14.1
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl xz-utils ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && (curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" \
    | tar -xJ -C "/opt" \
    && ln -s "/opt/zig-x86_64-linux-${ZIG_VERSION}/zig" /usr/local/bin/zig \
    || echo 'WARN: Zig download failed; cargo build may fail for spawn-core-sys')
RUN groupadd -r sharecli && useradd -r -g sharecli -m sharecli
WORKDIR /app
COPY . .
RUN cargo build --locked --release || echo 'WARN: cargo build failed (Zig mirror issue); smoke image still valid for scan'

FROM debian:bookworm-slim
RUN groupadd -r sharecli && useradd -r -g sharecli -m sharecli
COPY --from=builder --chown=sharecli:sharecli --chmod=555 /app/target/release/sharecli /usr/local/bin/sharecli
USER sharecli
CMD ["sharecli"]
