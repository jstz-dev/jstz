ARG OCTEZ_TAG
FROM tezos/tezos:octez-v${OCTEZ_TAG} AS octez

FROM ghcr.io/jstz-dev/jstz/build_cli:7f08c1b AS builder
ENV OPENSSL_DIR=/usr
WORKDIR /jstz_build
COPY . .
RUN RUSTFLAGS='-C target-feature=-crt-static' cargo build --package jstz_cli

FROM alpine AS cli
# libcrypto3, openssl, and musl are needed for jstz-rollup binary
# hidapi, gmp, and libev are needed for octez-client binary
# libffi, libgmpxx are needed for octez-smart-rollup-node binary
RUN apk --no-cache add binutils bash libcrypto3 openssl musl hidapi gmp libev libffi libgmpxx
COPY --from=octez /usr/local/bin/octez-smart-rollup-node /usr/bin/octez-smart-rollup-node
COPY --from=octez /usr/local/bin/octez-node /usr/bin/octez-node
COPY --from=octez /usr/local/bin/octez-client /usr/bin/octez-client
COPY --from=octez /usr/share/zcash-params /root/.zcash-params
# Copy the jstz binary & dependencies
COPY --from=builder /jstz_build/target/debug/jstz /usr/bin/jstz
ENTRYPOINT [ "/usr/bin/jstz" ]
