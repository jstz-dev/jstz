FROM rust:1.88.0-slim-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev libsqlite3-dev

WORKDIR /jstz_build
COPY . .
RUN cargo build --bin jstz-node --release --features oracle,v2_runtime

FROM debian:bookworm-20250520-slim AS jstz_node
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev libsqlite3-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /jstz_build/target/release/jstz-node /usr/bin/jstz-node

ENTRYPOINT ["/usr/bin/jstz-node"]
