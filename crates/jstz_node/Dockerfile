FROM rust:1.73.0 AS builder
WORKDIR /
COPY . .
RUN cargo build --package jstz_node --release

FROM debian:bookworm-slim AS node
RUN apt-get update && apt-get install -y openssl sqlite3
ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug
COPY --from=builder /target/release/jstz-node /usr/bin/jstz-node
ENTRYPOINT [ "/usr/bin/jstz-node" ]