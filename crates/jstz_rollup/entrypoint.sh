#!/usr/bin/env bash
set -e

export JSTZ_ROLLUP_OCTEZ_CLIENT_DIR="/root/.octez-client"
mkdir -p "$JSTZ_ROLLUP_OCTEZ_CLIENT_DIR"

export JSTZ_ROLLUP_OCTEZ_ROLLUP_NODE_DIR="/root/.octez-smart-rollup-node"
mkdir -p "$JSTZ_ROLLUP_OCTEZ_ROLLUP_NODE_DIR"

# shellcheck disable=SC2034
# JSTZ_ROLLUP_OCTEZ_NODE_ENDPOINT is used in the jstz-rollup command
export JSTZ_ROLLUP_OCTEZ_NODE_ENDPOINT="https://rpc.$NETWORK.teztnets.com/"

kernel_path="root/jstz_kernel.wasm"
installer_dir="root/installer"
logs_dir="root/logs"

if [ ! -f "$JSTZ_ROLLUP_OCTEZ_CLIENT_DIR/secret_keys" ]; then
  echo "Importing operator secret key..."
  if [ -z "$OPERATOR_SK" ]; then
    echo "OPERATOR_SK is not set"
    exit 1
  fi
  jstz-rollup operator import-keys --secret-key "$OPERATOR_SK"
fi

make-installer() {
  jstz-rollup make-installer \
    --kernel "$kernel_path" \
    --bridge "$JSTZ_ROLLUP_BRIDGE_ADDRESS" \
    --output "$installer_dir"

  # Check the exit status of the last command
  if [ $? -eq 0 ]; then
    echo "Installer created successfully in $installer_dir."
  else
    echo "Failed to create installer. Please check the parameters and try again."
    exit 1
  fi
}

deploy-bridge() {
  jstz-rollup deploy-bridge \
    --operator "$OPERATOR_ADDRESS"
}

deploy-installer() {
  jstz-rollup deploy-installer \
    --installer "$installer_dir/installer.wasm" \
    --bridge "$JSTZ_ROLLUP_BRIDGE_ADDRESS"
}

run() {
  if [ -z "$(ls -A $installer_dir)" ]; then
    make-installer
  fi

  mkdir -p "$logs_dir"

  jstz-rollup run \
    --preimages "$installer_dir/preimages" \
    --rollup "$JSTZ_ROLLUP_ADDRESS" \
    --logs "$logs_dir" \
    --addr "0.0.0.0"

  exit_status=$?

  if [ $exit_status -eq 0 ]; then
    echo "jstz-rollup node started successfully."
  else
    echo "Failed to start jstz-rollup node. Exit status: $exit_status"
    exit $exit_status
  fi
}

deploy() {
  JSTZ_ROLLUP_BRIDGE_ADDRESS=$(deploy-bridge | grep -oE 'KT1[a-zA-Z0-9]{33}' | uniq | tr -d '\n')
  echo "Bridge address: $JSTZ_ROLLUP_BRIDGE_ADDRESS"

  jstz-rollup deploy \
    --kernel "$kernel_path" \
    --bridge "$JSTZ_ROLLUP_BRIDGE_ADDRESS" \
    --output "$installer_dir" \
    --operator "$OPERATOR_ADDRESS"
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
  "deploy-bridge")
    deploy-bridge
    ;;
  "deploy-installer")
    deploy-installer
    ;;
  "make-installer")
    make-installer
    ;;
  *)
    cat <<EOF
Usage: $0 <COMMAND>

Commands: 
    run 
    deploy
    deploy-bridge
    deploy-installer
    make-installer
EOF
    exit 1
    ;;
  esac
}

if [ "$0" == "${BASH_SOURCE[0]}" ]; then
  main "$@"
fi
