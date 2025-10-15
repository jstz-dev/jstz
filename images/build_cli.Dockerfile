# Base image as the build environment for jstz cli

FROM rust:1.88.0-alpine3.22 AS builder
RUN apk --no-cache add musl-dev=1.2.5-r10 \
    libcrypto3=3.5.4-r0 openssl-dev=3.5.4-r0 \
    clang20=20.1.8-r0 make=4.4.1-r3 openssl-libs-static=3.5.4-r0

ENTRYPOINT [ "/bin/sh" ]
