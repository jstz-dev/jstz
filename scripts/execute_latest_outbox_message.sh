run() {
  # Get the current level of the chain.
  level="$(curl -s "http://127.0.0.1:18730/chains/main/blocks/head/metadata" | jq .level_info.level)"
  if [ -z "$level" ]; then
    echo 'Error: No "level_info" found from octez-client rpc get "/chains/main/blocks/head/metadata".\n'
    return 1 # Failure
  fi
  octez_client="$1"
  found=false
  counter=0
  max="200"
  # Search back a $max number of levels for a non-empty rollup outbox message.
  while [ "$found" = "false" ] && [ "$counter" -lt "$max" ]; do
    msg=$(curl -s "http://127.0.0.1:8932/global/block/head/outbox/${level}/messages")
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
    while true; do
      payload=$(curl -s "http://127.0.0.1:8932/global/block/head/helpers/proofs/outbox/${level}/messages?index=0")
      proof=$(echo $payload | jq -r ".proof" 2>/dev/null)
      if [ "$?" -ne 0 ]; then
        sleep 1
      else
        break
      fi
    done
    proof=$(echo $payload | jq -r ".proof")
    commitment_hash=$(echo $payload | jq -r ".commitment")
    $octez_client -E http://127.0.0.1:18730 -d "$(cat ~/.jstz/config.json | jq -r ".sandbox.octez_client_dir")" execute outbox message of smart rollup jstz_rollup from bootstrap1 for commitment hash $commitment_hash and output proof "0x$proof" --burn-cap 999
  else
    echo "No messages in the last $max levels.\n"
    return 1
  fi
}

run $1
