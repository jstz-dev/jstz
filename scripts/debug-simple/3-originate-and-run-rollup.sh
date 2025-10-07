#!/bin/bash
set -e

source /tmp/jstz-debug-env.sh

OPERATOR_ADDR=$(octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  show address rollup_operator | grep Hash: | awk '{print $2}')

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  transfer 1000000 from injector to rollup_operator --burn-cap 1 2>&1 | grep -v "Warning:"

for i in {1..30}; do
  LEVEL=$(octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    rpc get /chains/main/blocks/head/header 2>/dev/null | grep -o '"level":[0-9]*' | cut -d':' -f2)
  if [ -n "$LEVEL" ] && [ "$LEVEL" -ge 3 ]; then
    break
  fi
  sleep 1
done

ORIGINATION_OUTPUT=$(octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  originate smart rollup jstz_rollup from rollup_operator \
  of kind riscv \
  of type string \
  with kernel "kernel:crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable.elf:d9c179173a5b0f014853cabad53f6a0eab92287f85f5c07e32ece8f40c36caff" \
  --burn-cap 999999 \
  --force \
  --whitelist "[\"$OPERATOR_ADDR\"]" 2>&1)

ROLLUP_ADDR=$(echo "$ORIGINATION_OUTPUT" | grep -o 'sr1[a-zA-Z0-9]*' | head -1)

echo "export ROLLUP_ADDR=$ROLLUP_ADDR" >>/tmp/jstz-debug-env.sh

sleep 10

mkdir -p "$BASE_DIR/rollup"

octez-smart-rollup-node \
  --endpoint http://localhost:18731 \
  --base-dir "$CLIENT_DIR" \
  run operator for "$ROLLUP_ADDR" \
  with operators rollup_operator \
  --data-dir "$BASE_DIR/rollup" \
  --rpc-addr 127.0.0.1 \
  --rpc-port 18745 \
  --acl-override allow-all \
  --history-mode full
