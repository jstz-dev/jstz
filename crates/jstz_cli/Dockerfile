ARG OCTEZ_TAG
FROM tezos/tezos-bare:${OCTEZ_TAG} AS octez

FROM rust:1.73.0-alpine AS builder
RUN rustup target add wasm32-unknown-unknown
RUN apk --no-cache add musl-dev libcrypto3 openssl-dev clang make
ENV OPENSSL_DIR=/usr
WORKDIR /
COPY . .
RUN make build-cli-kernel
RUN RUSTFLAGS='-C target-feature=-crt-static' cargo build --package jstz_cli --release

FROM alpine AS cli
# libcrypto3, openssl, and musl are needed for jstz-rollup binary
# hidapi, gmp, and libev are needed for octez-client binary
# libffi, libgmpxx are needed for octez-smart-rollup-node binary
RUN apk --no-cache add binutils bash libcrypto3 openssl musl hidapi gmp libev libffi libgmpxx
COPY --from=octez /usr/local/bin/octez-smart-rollup-node /usr/bin/octez-smart-rollup-node
COPY --from=octez /usr/local/bin/octez-node /usr/bin/octez-node
COPY --from=octez /usr/local/bin/octez-client /usr/bin/octez-client
COPY --from=builder /target/release/jstz /usr/bin/jstz
ENTRYPOINT [ "/usr/bin/jstz" ]