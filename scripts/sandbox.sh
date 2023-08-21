#!/usr/bin/env bash
set -euo pipefail

# Determine the root directory of jstz
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="${script_dir}/.."
log_dir="${root_dir}/logs"

port=19730
rpc=18730

# Create temporary directories for octez-node, octez-smart-rollup-node, and 
# octez-client
node_dir="$(mktemp -d -t octez_node.XXXXXXXX)"
rollup_node_dir="$(mktemp -d -t octez_smart_rollup_node.XXXXXXXX)"
client_dir="$(mktemp -d -t octez_client.XXXXXXXX)"

node_pids=()
rollup_pids=()

# Used to suppress warnings
export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER=Y

# Aliases for octez-client, octez-smart-rollup-node, and octez-node
client="${root_dir}/octez-client -base-dir $client_dir -endpoint http://127.0.0.1:$rpc"
rollup_node="${root_dir}/octez-smart-rollup-node -base-dir $client_dir -endpoint http://127.0.0.1:$rpc"
node="${root_dir}/octez-node"
jstz="${root_dir}/scripts/jstz.sh"

# Build artefacts produced by `make build-installer`
kernel="${root_dir}/target/kernel/jstz_kernel_installer.hex"
preimages="${root_dir}/target/kernel/preimages"

start_sandboxed_node() { 
    # Initialize node config
    $node config init \
        --network "sandbox" \
        --data-dir "$node_dir" \
        --net-addr "127.0.0.1:$port" \
        --rpc-addr "127.0.0.1:$rpc" \
        --connections 0

    # Generate an identity of the node we want to run
    $node identity generate \
        --data-dir "$node_dir"

    # Start newly configured node in the background
    # Record the pid to ensure when the parent process is terminated, the node is terminated
    $node run --synchronisation-threshold 0 --network "sandbox" --data-dir "$node_dir" --sandbox="${script_dir}/sandbox.json" &
    node_pids+=("$!")

    cleanup() {
        # shellcheck disable=SC2317
        kill "${node_pids[@]}"
    }
    trap cleanup EXIT SIGINT SIGTERM

    wait "${node_pids[@]}"
}

wait_for_node_to_initialize() {
    if $client rpc get /chains/main/blocks/head/hash >/dev/null 2>&1; then 
        return 
    fi
  
    printf "Waiting for node to initialize"
    local count=0
    while ! $client rpc get /chains/main/blocks/head/hash >/dev/null 2>&1; do
        count=$((count+1))
        printf "."
        sleep 1
    done

    echo " done."
}

wait_for_node_to_activate() {
    printf "Waiting for node to activate"
    while ! [ -f "${node_dir}/activated" ]; do
        printf "." 
        sleep 1
    done

    echo " done."
}

init_sandboxed_client() {
    wait_for_node_to_initialize

    $client bootstrapped

    # Add bootstrapped identities
    $client import secret key activator unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6

    # Activate alpha
    $client -block genesis \
        activate protocol ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK \
        with fitness 1 \
        and key activator \
        and parameters "${script_dir}/sandbox-params.json" 
    
    # Add more bootstrapped accounts
    $client import secret key bootstrap1 unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh
    $client import secret key bootstrap2 unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo
    $client import secret key bootstrap3 unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ
    $client import secret key bootstrap4 unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3
    $client import secret key bootstrap5 unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm

    # Create file to communicate to `wait_for_node_activation` process that the
    # node is activated
    touch "${node_dir}/activated"

    # Continuously bake
    while $client bake for --minimal-timestamp; do sleep 1; done
}

originate_rollup() {
    # Wait for node activation
    wait_for_node_to_activate

    # Originate the jstz rollup kernel
    $client originate smart rollup "jstz_rollup" \
        from "bootstrap1" \
        of kind wasm_2_0_0 \
        of type "(pair bytes (ticket unit))" \
        with kernel "file:$kernel" \
        --burn-cap 999

    # Copy kernel installer preimages to rollup node directory
    mkdir -p "${rollup_node_dir}/wasm_2_0_0"
    cp -r "${preimages}"/* "${rollup_node_dir}/wasm_2_0_0/"
}

start_rollup_node() {
    originate_rollup

    # Start newly originated rollup
    $rollup_node run operator for "jstz_rollup" with operators "bootstrap2" --data-dir "$rollup_node_dir" --log-kernel-debug --log-kernel-debug-file "${log_dir}/kernel.log" &
    rollup_pids+=("$!")

    cleanup() {
        # shellcheck disable=SC2317
        kill "${rollup_pids[@]}"
    }
    trap cleanup EXIT SIGINT SIGTERM

    wait "${rollup_pids[@]}"
}


main() {
    mkdir -p "$log_dir"

    start_sandboxed_node > "${log_dir}/node.log" 2>&1 &
    pids+=("$!")

    init_sandboxed_client > "${log_dir}/client.log" 2>&1 &
    pids+=("$!")

    start_rollup_node > "${log_dir}/rollup.log" 2>&1 &
    pids+=("$!")

    cat <<EOF
export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER=Y ;
export OCTEZ_CLIENT_DIR="$client_dir" ;
export OCTEZ_NODE_DIR="$node_dir" ;
export OCTEZ_ROLLUP_DIR="$rollup_node_dir" ;
alias octez-client="$client" ;
alias jstz="$jstz" ;
alias octez-reset="kill ${pids[@]}; rm -rf \"$node_dir\"; rm -rf \"$client_dir\"; rm -rf \"$rollup_node_dir\"; unalias octez-client octez-reset jstz" ;
EOF

    cat 1>&2 <<EOF
The node, baker and rollup node are now initialized. In the rest of this shell
session, you may now run \`octez-client\` to communicate the the launched node. 
For instance:

    octez-client rpc get /chains/main/blocks/head/metadata

You may observe the logs of the node, baker, rollup node and jstz kernel in \`logs\`. 
For instance:

    tail -f logs/kernel.log

To stop the node, baker and rollup node you may run \`octez-reset\`. 

Additionally, you may now use \`jstz\` to run jstz-specific commands. For instance:

    jstz deploy-bridge sr1..

Warning: All aliases will be removed when you close this shell. 

EOF

}

if [ "$0" == "${BASH_SOURCE[0]}" ]; then
    main
fi
