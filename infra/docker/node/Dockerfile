FROM rust:1.71.0 AS builder
WORKDIR /
COPY . .
RUN cargo build --package jstz_node --release

FROM debian:buster-slim AS node
COPY --from=builder /target/release/jstz-node /usr/bin/jstz-node
RUN apt-get update
RUN apt-get install -y openssl
ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug
ENTRYPOINT [ "/usr/bin/jstz-node" ]