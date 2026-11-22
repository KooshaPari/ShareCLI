FROM rust:1.87-slim AS builder

# `--features jemalloc` pulls in tikv-jemalloc-sys, which compiles the vendored
# jemalloc from source: its build.rs shells out to the autoconf-generated
# Makefile (`make`, `cc`). rust:*-slim ships a C compiler but has NO `make`, so
# the build aborted with:
#   thread 'main' panicked at tikv-jemalloc-sys-*/build.rs:
#   failed to execute command: No such file or directory (os error 2)
# Keep these packages in the builder only; the runtime stage stays minimal.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        make \
        autoconf \
        automake \
        perl \
        pkg-config \
        curl \
        xz-utils \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Zig 0.14.1 (pinned). crates/spawn-core-sys/build.rs shells out to `zig build`
# to produce crates/spawn-core's static lib on Linux, and spawn-core-sys is an
# unconditional dependency of the `sharecli` package. Without it the build stops at
#   Error: failed to run `zig build`: No such file or directory (os error 2)
# Unlike the smoke Dockerfile, this stage fails hard: a missing Zig cannot yield
# a working release binary, so it must not degrade into a warning.
ARG ZIG_VERSION=0.14.1
RUN curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-x86_64-${ZIG_VERSION}.tar.xz" \
      | tar -xJ -C /opt \
    && ln -s "/opt/zig-linux-x86_64-${ZIG_VERSION}/zig" /usr/local/bin/zig \
    && zig version

WORKDIR /app
COPY . .
RUN cargo build --release --locked --features jemalloc

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 --shell /usr/sbin/nologin sharecli

COPY --from=builder /app/target/release/sharecli /usr/local/bin/sharecli

USER sharecli
EXPOSE 9000
# See docs/ops/container-hardening.md for --read-only / no-new-privileges / cap-drop.
# Liveness probe matches `GET /healthz` in src/commands/serve.rs
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:9000/healthz || exit 1

ENTRYPOINT ["sharecli"]
CMD ["serve", "--bind", "0.0.0.0:9000"]
