ARG OCTEZ_TAG
FROM tezos/tezos-bare:${OCTEZ_TAG} AS octez

FROM rust:1.73.0-alpine AS builder
# Install build deps
RUN apk --no-cache add musl-dev libcrypto3 openssl-dev clang
ENV OPENSSL_DIR=/usr
# Copy srcs
WORKDIR /
COPY . .
# Build jstz-rollup
# RUSTFLAGS is needed to avoid linking to the static version of the C runtime (provided by musl)
# https://github.com/rust-lang/rust/pull/58575#issuecomment-496026747
RUN RUSTFLAGS='-C target-feature=-crt-static' cargo build --package jstz_rollup --release

FROM alpine AS node
# libcrypto3, openssl, and musl are needed for jstz-rollup binary
# hidapi, gmp, and libev are needed for octez-client binary
# libffi, libgmpxx are needed for octez-smart-rollup-node binary
RUN apk --no-cache add binutils bash libcrypto3 openssl musl hidapi gmp libev libffi libgmpxx
ENV RUST_BACKTRACE=1
ENV RUST_LOG=debug
COPY --from=octez /usr/local/bin/octez-smart-rollup-node /usr/bin/octez-smart-rollup-node
COPY --from=octez /usr/local/bin/octez-client /usr/bin/octez-client
COPY --from=builder /target/release/jstz-rollup /usr/bin/jstz-rollup
ARG KERNEL_PATH
COPY $KERNEL_PATH /root/jstz_kernel.wasm
COPY ./crates/jstz_rollup/entrypoint.sh ./entrypoint.sh
RUN chmod +x entrypoint.sh
ENTRYPOINT [ "./entrypoint.sh" ]