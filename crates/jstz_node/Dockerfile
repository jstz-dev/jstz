FROM ghcr.io/jstz-dev/jstz/build:7f08c1b AS builder

WORKDIR /jstz_build
COPY . .
RUN cargo build --bin jstz-node --release --features oracle,v2_runtime

FROM ghcr.io/jstz-dev/jstz/serve:7f08c1b AS jstz_node
COPY --from=builder /jstz_build/target/release/jstz-node /usr/bin/jstz-node

ENTRYPOINT ["/usr/bin/jstz-node"]
