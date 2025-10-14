#!/bin/bash
# Script 5: Start Rollup Node
# Run this in Terminal 4 after originating the rollup

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}=== Starting Rollup Node ===${NC}"

# Load environment
source /tmp/jstz-debug-env.sh

if [ -z "$ROLLUP_ADDR" ]; then
  echo -e "${RED}Error: ROLLUP_ADDR not set. Did you run script 4?${NC}"
  exit 1
fi

echo "Rollup address: $ROLLUP_ADDR"
echo "Connecting to node: http://localhost:18731"

# Setup rollup data directory
ROLLUP_DIR="$BASE_DIR/rollup"
mkdir -p "$ROLLUP_DIR"

echo -e "\n${BLUE}Starting octez-smart-rollup-node...${NC}"
echo "Data directory: $ROLLUP_DIR"
echo "RPC will be available at: http://localhost:18745"
echo ""
echo "Logs will appear below:"
echo ""

# Start the rollup node
octez-smart-rollup-node \
  --endpoint http://localhost:18731 \
  --base-dir "$CLIENT_DIR" \
  run operator for "$ROLLUP_ADDR" \
  with operators rollup_operator \
  --data-dir "$ROLLUP_DIR" \
  --rpc-addr 127.0.0.1 \
  --rpc-port 18745 \
  --acl-override allow-all \
  --history-mode full
