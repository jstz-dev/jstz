FROM ghcr.io/jstz-dev/jstz/build AS builder

WORKDIR /jstz_build
COPY . .
RUN cargo build --bin jstz-node --release --features oracle,v2_runtime

FROM ghcr.io/jstz-dev/jstz/serve AS jstz_node
COPY --from=builder /jstz_build/target/release/jstz-node /usr/bin/jstz-node

ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug

ENTRYPOINT ["/usr/bin/jstz-node"]
