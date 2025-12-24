ARG OCTEZ_VERSION=23.3
ARG FEATURES="build-image,oracle,blueprint"
FROM ghcr.io/jstz-dev/jstz/build:7f08c1b AS builder

WORKDIR /jstz_build
ADD . .
ARG KERNEL_PATH
# set default path to a non-existent path such that docker ignores the copy when the path is not given
ARG BOOTSTRAP_ACCOUNT_PATH="/nonexistent_path"
ARG LIGHTWEIGHT_KERNEL_PATH
ARG FEATURES
COPY $BOOTSTRAP_ACCOUNT_PATH* crates/jstzd/resources/bootstrap_account/accounts.json
COPY $KERNEL_PATH crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm
COPY $LIGHTWEIGHT_KERNEL_PATH crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable
# release build is required for rust-embed to pack the resource files into the executable
RUN KERNEL_DEST_DIR=/jstzd_kernel_files cargo build --bin jstzd --release --features "$FEATURES"

FROM ghcr.io/jstz-dev/jstz/serve:7f08c1b AS jstzd
ARG OCTEZ_VERSION
# It's possible to download static executables from github according to architecture, but that seems
# a bit too messy and error prone.
# The `echo $'\n\n\n\n'` command below is due to the octez packages prompting for options during
# installation and `echo` makes sure that installation does not hang there.
RUN apt update && apt install -y curl gpg ca-certificates=20230311+deb12u1 && \
    curl -s "https://packages.nomadic-labs.com/debian/octez.asc" | gpg --dearmor -o /etc/apt/keyrings/octez.gpg && \
    # FIXME(https://linear.app/tezos/issue/JSTZ-901/verify-nl-package-in-jstzd-dockerfile)
    # Set trusted=yes to skip signing as it is currently broken
    echo "deb [signed-by=/etc/apt/keyrings/octez.gpg] https://packages.nomadic-labs.com/debian bookworm main" | tee /etc/apt/sources.list.d/octez.list && \
    apt-get update && echo $'\n\n\n\n' | apt -y install octez-node=${OCTEZ_VERSION} \
        octez-client=${OCTEZ_VERSION} octez-baker=${OCTEZ_VERSION} \
        octez-smart-rollup-node=${OCTEZ_VERSION} && \
        cd /usr/bin && rm octez-admin-client octez-dal-node octez-accuser-* octez-signer octez-codec

# Copy the jstzd binary & dependencies
COPY --from=builder /jstz_build/target/release/jstzd /usr/bin/jstzd
COPY --from=builder /jstzd_kernel_files /usr/share/jstzd

ENTRYPOINT [ "/usr/bin/jstzd" ]
