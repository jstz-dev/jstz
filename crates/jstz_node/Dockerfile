FROM rust:1.82.0-slim-bookworm AS builder
RUN apt update && apt install -y curl pkg-config libssl-dev libsqlite3-dev

WORKDIR /jstz_build
ADD . .
ARG KERNEL_PATH
COPY $KERNEL_PATH /jstz_build/riscv_kernel
RUN cargo build --bin jstz-node --release --features oracle,inject_inbox,v2_runtime

FROM debian:bookworm-20250520-slim AS jstzd
RUN apt update && apt install -y curl pkg-config libssl-dev libsqlite3-dev

# Copy the jstzd binary & dependencies
COPY --from=builder /jstz_build/target/release/jstz-node /usr/bin/jstz-node

RUN mkdir -p /jstz_build/preimages && touch /jstz_build/debug_log && echo "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh" > /jstz_build/injector
COPY --from=builder /jstz_build/riscv_kernel /jstz_build/riscv_kernel
COPY --from=builder /jstz_build/run.sh /jstz_build/run.sh

ENTRYPOINT ["sh", "/jstz_build/run.sh"]
