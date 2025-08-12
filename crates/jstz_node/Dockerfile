FROM rust:1.88.0-slim-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    curl=7.88.1-10+deb12u12 pkg-config=1.8.1-1 \
    libssl-dev=3.0.17-1~deb12u2 libsqlite3-dev=3.40.1-2+deb12u1

WORKDIR /jstz_build
COPY . .
RUN cargo build --bin jstz-node --release --features oracle,v2_runtime

FROM debian:bookworm-20250520-slim AS jstz_node
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    curl=7.88.1-10+deb12u12 pkg-config=1.8.1-1 \
    libssl-dev=3.0.17-1~deb12u2 libsqlite3-dev=3.40.1-2+deb12u1 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /jstz_build/target/release/jstz-node /usr/bin/jstz-node
COPY --from=builder /jstz_build/run.sh /run.sh
COPY --from=builder /jstz_build/crates/jstz_node/kernel-executable /jstz_build/riscv_kernel

ENV RUST_BACKTRACE=1

ENTRYPOINT ["sh", "/run.sh"]
