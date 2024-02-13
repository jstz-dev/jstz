#!/usr/bin/env bash
set -e

export JSTZ_ROLLUP_OCTEZ_CLIENT_DIR="/root/.octez-client"
mkdir -p "$JSTZ_ROLLUP_OCTEZ_CLIENT_DIR"

export JSTZ_ROLLUP_OCTEZ_ROLLUP_NODE_DIR="/root/.octez-smart-rollup-node"
mkdir -p "$JSTZ_ROLLUP_OCTEZ_ROLLUP_NODE_DIR"

# shellcheck disable=SC2034 
# JSTZ_ROLLUP_OCTEZ_NODE_ENDPOINT is used in the jstz-rollup command
export JSTZ_ROLLUP_OCTEZ_NODE_ENDPOINT="https://rpc.$NETWORK.teztnets.com/"

installer_dir="root/installer"

if [ ! -f "$JSTZ_ROLLUP_OCTEZ_CLIENT_DIR/secret_keys" ]; then
    echo "Importing operator secret key..."
    if [ -z "$OPERATOR_SK" ]; then
        echo "OPERATOR_SK is not set"
        exit 1
    fi
    jstz-rollup operator import-keys --secret-key "$OPERATOR_SK"
fi


run() {
    mkdir -p "$LOGS_DIR"
    jstz-rollup run \
        --preimages "$installer_dir/preimages" \
        --rollup "$JSTZ_ROLLUP_ADDRESS" \
        --logs "$LOGS_DIR" \
        --addr "0.0.0.0"
}

deploy() {
    jstz-rollup deploy-installer \
        --installer "$installer_dir/installer.wasm" \
        --bridge "$JSTZ_ROLLUP_BRIDGE_ADDRESS"
}

main() {
    command="$1"
    shift 1

    case $command in
        "run")
            run
            ;;
        "deploy")
            deploy
            ;;
        *)
            cat <<EOF
Usage: $0 <COMMAND>

Commands: 
    run 
    deploy
EOF
            exit 1
            ;;
    esac
}

if [ "$0" == "${BASH_SOURCE[0]}" ]; then
    main "$@"
fi
