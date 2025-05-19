#!/bin/bash
set -e

run() {
  jstzd_base_url="http://127.0.0.1:54321"
  # Get the current level of the chain.
  octez_node_base_url="$(curl -s "$jstzd_base_url/config/octez_node" | jq -r .rpc_endpoint)"
  level="$(curl -s "$octez_node_base_url/chains/main/blocks/head/metadata" | jq .level_info.level)"
  if [ -z "$level" ]; then
    echo 'Error: No "level_info" found from octez-client rpc get "/chains/main/blocks/head/metadata".'
    return 1 # Failure
  fi
  octez_client="${1:-"octez-client"}"
  found=false
  counter=0
  max="200"
  rollup_node_base_url="$(curl -s "$jstzd_base_url/config/octez_rollup" | jq -r .rpc_endpoint)"
  # Search back a $max number of levels for a non-empty rollup outbox message.
  while [ "$found" = "false" ] && [ "$counter" -lt "$max" ]; do
    msg=$(curl -s "$rollup_node_base_url/global/block/head/outbox/${level}/messages")
    case "$msg" in
    *'[]'*)
      level=$((level - 1))
      ;;
    *)
      found=true
      ;;
    esac
    counter=$((counter + 1))
  done
  if [ "$found" = "true" ]; then
    echo "Found outbox message at $level"
    set +e # temporarily set +e because we want to check the status code below
    while true; do
      payload=$(curl -s "$rollup_node_base_url/global/block/head/helpers/proofs/outbox/${level}/messages?index=0")
      proof=$(echo "$payload" | jq -r ".proof" 2>/dev/null)
      # shellcheck disable=SC2181
      if [ "$?" -ne 0 ]; then
        sleep 1
      else
        break
      fi
    done
    set -e
    proof=$(echo "$payload" | jq -r ".proof")
    commitment_hash=$(echo "$payload" | jq -r ".commitment")
    octez_client_base_dir="$(curl -s "$jstzd_base_url/config/octez_client" | jq -r .base_dir)"
    $octez_client -E "$octez_node_base_url" -d "$octez_client_base_dir" execute outbox message of smart rollup sr1PuFMgaRUN12rKQ3J2ae5psNtwCxPNmGNK from bootstrap1 for commitment hash "$commitment_hash" and output proof "0x$proof" --burn-cap 999
  else
    echo "No messages in the last $max levels."
    return 1
  fi
}

run $1
