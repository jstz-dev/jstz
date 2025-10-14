#!/bin/bash
# Script 3: Start Baker
# This replicates what jstzd does when starting the baker
# Run this in Terminal 3 after protocol activation

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Starting Baker (jstzd equivalent) ===${NC}"

# Load environment
source /tmp/jstz-debug-env.sh

echo -e "\n${BLUE}Starting baker for injector account...${NC}"
echo "Baker will produce blocks automatically"
echo "Logs will appear below:"
echo ""

# Start baking (without DAL node, as jstzd does)
octez-baker-alpha --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  run with local node "$NODE_DIR" injector --liquidity-baking-toggle-vote pass --without-dal
