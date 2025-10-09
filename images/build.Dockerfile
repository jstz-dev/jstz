# Base image as the build environment for jstz components

FROM rust:1.88.0-slim-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    curl=7.88.1-10+deb12u14 pkg-config=1.8.1-1 \
    libssl-dev=3.0.17-1~deb12u3 libsqlite3-dev=3.40.1-2+deb12u2 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

ENTRYPOINT ["/bin/sh"]
