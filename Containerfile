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
    && rm -rf /var/lib/apt/lists/*

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
