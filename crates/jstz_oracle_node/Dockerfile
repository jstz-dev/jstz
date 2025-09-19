FROM rust:1.88.0-slim-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    curl libssl-dev=3.0.17-1~deb12u2 pkg-config=1.8.1-1

WORKDIR /jstz_build
COPY . .
RUN cargo build --bin jstz-oracle-node --release --features v2_runtime

FROM debian:bookworm-20250520-slim AS jstz_oracle_node
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    libssl-dev=3.0.17-1~deb12u2 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /jstz_build/target/release/jstz-oracle-node /usr/bin/jstz-oracle-node

ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug

ENTRYPOINT ["/usr/bin/jstz-oracle-node"]
