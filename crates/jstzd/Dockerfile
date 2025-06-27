ARG OCTEZ_VERSION=22.1
ARG FEATURES="build-image,v2_runtime"
FROM rust:1.82.0-slim-bookworm AS builder
RUN apt update && apt install -y curl pkg-config libssl-dev libsqlite3-dev

WORKDIR /
ADD . .
ARG KERNEL_PATH
# set default path to a non-existent path such that docker ignores the copy when the path is not given
ARG BOOTSTRAP_ACCOUNT_PATH="/nonexistent_path"
ARG FEATURES
COPY $BOOTSTRAP_ACCOUNT_PATH* crates/jstzd/resources/bootstrap_account/accounts.json
COPY $KERNEL_PATH crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
# release build is required for rust-embed to pack the resource files into the executable
RUN KERNEL_DEST_DIR=/jstzd_kernel_files cargo build --bin jstzd --release --features "$FEATURES"

FROM debian:bookworm-20250520-slim AS jstzd
ARG OCTEZ_VERSION
# It's possible to download static executables from github according to architecture, but that seems
# a bit too messy and error prone.
# The `echo $'\n\n\n\n'` command below is due to the octez packages prompting for options during
# installation and `echo` makes sure that installation does not hang there.
RUN apt update && apt install -y curl gpg && \
    curl -s "https://packages.nomadic-labs.com/debian/octez.asc" | gpg --dearmor -o /etc/apt/keyrings/octez.gpg && \
    echo "deb [signed-by=/etc/apt/keyrings/octez.gpg] https://packages.nomadic-labs.com/debian bookworm main" | tee /etc/apt/sources.list.d/octez.list && \
    apt-get update && echo $'\n\n\n\n' | apt -y install octez-node=${OCTEZ_VERSION} \
        octez-client=${OCTEZ_VERSION} octez-baker=${OCTEZ_VERSION} \
        octez-smart-rollup-node=${OCTEZ_VERSION} && \
        cd /usr/bin && rm octez-admin-client octez-dal-node octez-accuser-* octez-signer octez-codec

# Copy the jstzd binary & dependencies
COPY --from=builder /target/release/jstzd /usr/bin/jstzd
COPY --from=builder /jstzd_kernel_files /usr/share/jstzd

ENTRYPOINT [ "/usr/bin/jstzd" ]
