#!/usr/bin/env bash
set -eux

client_dir="/root/.octez-client"
rollup_dir="/root/.octez-smart-rollup-node"
operator_alias="operator"

if [ -z "$NETWORK" ]; then
  echo "NETWORK is not set. Please set it to one of the following: nairobi"
  exit 1
fi

endpoint=https://rpc.$NETWORK.teztnets.xyz/

import_secret_key() {
    if [ ! -f "$client_dir/secret_keys" ]; then
        echo "Importing operator secret key..."
        if [ -z "$OPERATOR_SK" ]; then
            echo "OPERATOR_SK is not set"
            exit 1
        fi
        octez-client --endpoint "$endpoint" --base-dir "$client_dir" import secret key "$operator_alias" "$OPERATOR_SK"
    fi
}

run_node() {
    import_secret_key

    if [ ! -f "$rollup_dir/config.json" ]; then
        echo "Generating operator config..."
        if [ -z "$ROLLUP_ADDRESS" ]; then
            echo "ROLLUP_ADDRESS is not set"
            exit 1
        fi

        operator_address=$(octez-client --endpoint "$endpoint" --base-dir "$client_dir"  show address "$operator_alias" 2>&1 | grep Hash | grep -oE "tz.*")
        octez-smart-rollup-node --base-dir "$client_dir" init operator config \
            for "$ROLLUP_ADDRESS" \
            with operators "$operator_address" \
            --data-dir "$rollup_dir"
    fi
    
    if [ ! -d "$rollup_dir/wasm_2_0_0" ]; then
        echo "Initializing preimages folder..."
        cp -R /root/preimages "$rollup_dir/wasm_2_0_0"
    fi

    echo "Starting node..."

    mkdir -p /tmp/logs
    octez-smart-rollup-node --endpoint "$endpoint" --base-dir "$client_dir" \
        run --data-dir "$rollup_dir" --rpc-addr "0.0.0.0" \
        --log-kernel-debug --log-kernel-debug-file /tmp/logs/kernel.log
}


main() {
    command="$1"
    shift 1

    case $command in
        run-node)
            mkdir -p $client_dir
            mkdir -p $rollup_dir
            run_node
            ;;
        *)
            echo "Unknown command: $command"
            exit 1
            ;;
    esac

}

if [ "$0" == "${BASH_SOURCE[0]}" ]; then
    main "$@"
fi