FROM ghcr.io/jstz-dev/jstz/build AS builder

WORKDIR /jstz_build
COPY . .
RUN cargo build --bin jstz-oracle-node --release --features v2_runtime

FROM ghcr.io/jstz-dev/jstz/serve AS jstz_oracle_node
COPY --from=builder /jstz_build/target/release/jstz-oracle-node /usr/bin/jstz-oracle-node

ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug

ENTRYPOINT ["/usr/bin/jstz-oracle-node"]
