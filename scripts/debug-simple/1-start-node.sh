#!/bin/bash
set -e

BASE_DIR="/tmp/jstz-debug-$(date +%s)"
mkdir -p "$BASE_DIR/node" "$BASE_DIR/client"

cat > /tmp/jstz-debug-env.sh << EOF
export BASE_DIR=$BASE_DIR
export NODE_DIR=$BASE_DIR/node
export CLIENT_DIR=$BASE_DIR/client
export NODE_RPC=http://localhost:18731
EOF

octez-node identity generate --data-dir "$BASE_DIR/node"

octez-node config init \
    --data-dir "$BASE_DIR/node" \
    --network sandbox \
    --net-addr "127.0.0.1:19732" \
    --rpc-addr "127.0.0.1:18731" \
    --expected-pow 0

octez-node run \
    --data-dir "$BASE_DIR/node" \
    --network sandbox \
    --synchronisation-threshold 0 \
    --connections 1
