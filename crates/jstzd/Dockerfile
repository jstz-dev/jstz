ARG OCTEZ_TAG=octez-v22.0
FROM tezos/tezos:${OCTEZ_TAG} AS octez
USER root
RUN mkdir /octez-bin && mv /usr/local/bin/octez-client /usr/local/bin/octez-node /usr/local/bin/octez-smart-rollup-node /usr/local/bin/octez-baker-* /octez-bin

FROM rust:1.82.0-alpine AS builder
RUN apk --no-cache add musl-dev libcrypto3 openssl-dev clang make
ENV OPENSSL_DIR=/usr
WORKDIR /
ADD . .
ARG KERNEL_PATH
COPY $KERNEL_PATH crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
# release build is required for rust-embed to pack the resource files into the executable
RUN KERNEL_DEST_DIR=/jstzd_kernel_files RUSTFLAGS='-C target-feature=-crt-static' cargo build --bin jstzd --release --features build-image

FROM alpine AS jstzd
# libcrypto3, openssl, and musl are needed for jstz-rollup binary
# hidapi, gmp, and libev are needed for octez-client binary
# libffi, libgmpxx are needed for octez-smart-rollup-node binary
# sqlite-dev is needed for octez-node binary
RUN apk --no-cache add binutils bash libcrypto3 openssl musl hidapi gmp libev libffi libgmpxx sqlite-dev
COPY --from=octez /octez-bin /usr/local/bin
COPY --from=octez /usr/share/zcash-params /root/.zcash-params
# Copy the jstzd binary & dependencies
COPY --from=builder /target/release/jstzd /usr/bin/jstzd
COPY --from=builder /jstzd_kernel_files /usr/share/jstzd

ENTRYPOINT [ "/usr/bin/jstzd" ]
