#!/bin/bash
# Script 1: Start Octez Node
# This replicates what jstzd does when starting the octez node
# Run this in Terminal 1

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Starting Octez Node (jstzd equivalent) ===${NC}"

# Setup directories (using temp dir like jstzd does)
export BASE_DIR="/tmp/jstz-debug-$(date +%s)"
export NODE_DIR="$BASE_DIR/octez-node"
export CLIENT_DIR="$BASE_DIR/octez-client"

mkdir -p "$NODE_DIR"
mkdir -p "$CLIENT_DIR"

echo "Base directory: $BASE_DIR"
echo "Node directory: $NODE_DIR"
echo "Client directory: $CLIENT_DIR"

# Save directories to file for other scripts
echo "export BASE_DIR=$BASE_DIR" >/tmp/jstz-debug-env.sh
echo "export NODE_DIR=$NODE_DIR" >>/tmp/jstz-debug-env.sh
echo "export CLIENT_DIR=$CLIENT_DIR" >>/tmp/jstz-debug-env.sh
echo "export NODE_RPC=http://localhost:18731" >>/tmp/jstz-debug-env.sh

echo -e "${GREEN}✓ Directories created${NC}"

# Generate identity
echo -e "\n${BLUE}Generating node identity...${NC}"
octez-node identity generate --data-dir "$NODE_DIR"
echo -e "${GREEN}✓ Identity generated${NC}"

# Initialize config
echo -e "\n${BLUE}Initializing node config...${NC}"
octez-node config init \
  --data-dir "$NODE_DIR" \
  --network sandbox \
  --net-addr "127.0.0.1:19732" \
  --rpc-addr "127.0.0.1:18731" \
  --expected-pow 0
echo -e "${GREEN}✓ Config initialized${NC}"

# Start the node
echo -e "\n${BLUE}Starting octez node...${NC}"
echo "RPC endpoint: http://localhost:18731"
echo "Logs will appear below:"
echo ""

octez-node run \
  --data-dir "$NODE_DIR" \
  --network sandbox \
  --synchronisation-threshold 0 \
  --connections 0
