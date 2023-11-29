#!/usr/bin/env bash
set -eux

# Determine the root directory of jstz
scripts_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="${scripts_dir}/.."

operator_alias="operator"
export TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER=Y

if [ -z "$NETWORK" ]; then
  echo "NETWORK is not set. Please set it to one of the following: ghostnet, nairobinet, dailynet, weeklynet"
  exit 1
fi

endpoint=https://rpc.$NETWORK.teztnets.xyz/

operator_address() {
    octez-client --endpoint "$endpoint" show address "$operator_alias" 2>&1 | grep Hash | grep -oE "tz.*"
}

generate_keys() {
    echo "Generating keys..."
    octez-client --endpoint "$endpoint" gen keys "$operator_alias" --force
    address=$(operator_address)
    echo "Operator address: $address"
}

info() {
    octez-client --endpoint "$endpoint" show address "$operator_alias"
    octez-client --endpoint "$endpoint" get balance for "$operator_alias"
}

deploy_bridge() {
    operator_address=$(operator_address)
    
    echo "Deploying (mock) CTEZ contract..."

    ctez_src="${root_dir}/jstz_bridge/jstz_ctez.tz"
    init_ctez_storage="(Pair \"$operator_address\" { Elt \"$operator_address\" 10000000000 } )"
    ctez_address=$(
        octez-client --endpoint "$endpoint" originate contract "jstz_ctez" \
            transferring 0 from "$operator_alias" \
            running "$ctez_src" \
            --init "$init_ctez_storage" \
            --burn-cap 999 \
            --force |
        grep "New contract" | 
        awk '{ print $3}'
    )

    echo "Deployed CTEZ contract @ address: $ctez_address"

    echo "Deploying bridge..."

    src="${root_dir}/jstz_bridge/jstz_bridge.tz"
    init_storage="(Pair \"$ctez_address\" None)"
    bridge_address=$(
        octez-client --endpoint "$endpoint" originate contract "jstz_bridge" \
            transferring 0 from "$operator_alias" \
            running "$src" \
            --init "$init_storage" \
            --burn-cap 999 \
            --force | 
        grep "New contract" | 
        awk '{ print $3}'
    )

    echo "Deployed jstz bridge @ address: $bridge_address"
}

deploy_rollup() {
    echo "Deploying rollup..."

    kernel="${root_dir}/target/kernel/jstz_kernel_installer.hex"
    if [ ! -f "$kernel" ]; then
        echo "Kernel not found"
        exit 1
    fi
     
    octez-client --endpoint "$endpoint" originate smart rollup "jstz_rollup" \
        from "$operator_alias" \
        of kind wasm_2_0_0 \
        of type "(pair bytes (ticket unit))" \
        with kernel "file:$kernel" \
        --burn-cap 999 \
        --force
}

main() {
    command="$1"
    echo "Running command: $command"
    shift 1

    case $command in
        "generate-keys")
            generate_keys
            ;;
        "info")
            info
            ;;
        "deploy-bridge")
            deploy_bridge
            ;;
        "deploy-rollup")
            deploy_rollup
            ;;
        *)
            cat <<EOF
Usage: $0 <COMMAND>

Commands:
    generate-keys
    info
    deploy-bridge
    deploy-rollup
EOF
            exit 1
            ;;
    esac
}

if [ "$0" == "${BASH_SOURCE[0]}" ]; then
    main "$@"
fi