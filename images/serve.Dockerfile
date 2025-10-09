# Base image as the execution environment for built jstz components

FROM debian:bookworm-20250520-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    libssl-dev=3.0.17-1~deb12u3 curl=7.88.1-10+deb12u14 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

ENTRYPOINT ["/bin/sh"]
