#!/bin/bash
# Script 4: Originate RISC-V Rollup
# Run this in a new terminal after the baker has produced a few blocks

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}=== Originating RISC-V Rollup ===${NC}"

# Load environment
source /tmp/jstz-debug-env.sh

# Get the RISC-V kernel path and checksum
KERNEL_PATH="/Users/alanmarko/projects/jstz_attempt2/jstz/crates/jstzd/resources/jstz_rollup/lightweight-kernel-executable.elf"

if [ ! -f "$KERNEL_PATH" ]; then
  echo -e "${RED}Error: Kernel not found at $KERNEL_PATH${NC}"
  exit 1
fi

echo -e "\n${BLUE}Computing kernel checksum...${NC}"
# macOS uses shasum, Linux uses sha256sum
if command -v sha256sum &>/dev/null; then
  KERNEL_CHECKSUM=$(sha256sum "$KERNEL_PATH" | awk '{print $1}')
elif command -v shasum &>/dev/null; then
  KERNEL_CHECKSUM=$(shasum -a 256 "$KERNEL_PATH" | awk '{print $1}')
else
  echo -e "${RED}Error: Neither sha256sum nor shasum found${NC}"
  exit 1
fi
echo "Checksum: $KERNEL_CHECKSUM"

# Format kernel parameter
KERNEL_PARAM="kernel:${KERNEL_PATH}:${KERNEL_CHECKSUM}"

echo -e "\n${BLUE}Kernel parameter:${NC}"
echo "$KERNEL_PARAM"

# Get rollup operator address for whitelist
echo -e "\n${BLUE}Getting rollup operator address...${NC}"
OPERATOR_ADDR=$(octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  show address rollup_operator | grep Hash: | awk '{print $2}')
echo "Operator address: $OPERATOR_ADDR"

# Transfer funds to rollup_operator (it's not a bootstrap account)
echo -e "\n${BLUE}Transferring funds to rollup_operator...${NC}"
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  transfer 1000000 from injector to rollup_operator --burn-cap 1 2>&1 | grep -v "Warning:"
echo -e "${GREEN}✓ Funds transferred${NC}"

# Wait for a few blocks
echo -e "\n${YELLOW}Waiting for block level 3...${NC}"
for i in {1..30}; do
  LEVEL=$(octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    rpc get /chains/main/blocks/head/header 2>/dev/null | grep -o '"level":[0-9]*' | cut -d':' -f2)
  if [ -n "$LEVEL" ] && [ "$LEVEL" -ge 3 ]; then
    echo -e "${GREEN}✓ At block level $LEVEL${NC}"
    break
  fi
  echo -n "."
  sleep 1
done
echo ""

# Originate the rollup
echo -e "\n${BLUE}Originating RISC-V smart rollup...${NC}"
echo "This may take a moment..."

set +e # Don't exit on error so we can see what happened
ORIGINATION_OUTPUT=$(octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  originate smart rollup jstz_rollup from rollup_operator \
  of kind riscv \
  of type string \
  with kernel "$KERNEL_PARAM" \
  --burn-cap 999999 \
  --force \
  --whitelist "[\"$OPERATOR_ADDR\"]" 2>&1)
ORIGINATION_EXIT_CODE=$?
set -e

echo "$ORIGINATION_OUTPUT"

if [ $ORIGINATION_EXIT_CODE -ne 0 ]; then
  echo -e "\n${RED}Error: Origination failed with exit code $ORIGINATION_EXIT_CODE${NC}"
  exit 1
fi

# Extract rollup address
ROLLUP_ADDR=$(echo "$ORIGINATION_OUTPUT" | grep -o 'sr1[a-zA-Z0-9]*' | head -1)

if [ -z "$ROLLUP_ADDR" ]; then
  echo -e "${RED}Error: Could not extract rollup address${NC}"
  echo "Output was:"
  echo "$ORIGINATION_OUTPUT"
  exit 1
fi

echo -e "\n${GREEN}✓ Rollup originated successfully!${NC}"
echo -e "${GREEN}  Address: $ROLLUP_ADDR${NC}"

# Save rollup address
echo "export ROLLUP_ADDR=$ROLLUP_ADDR" >>/tmp/jstz-debug-env.sh

echo -e "\n${YELLOW}Waiting for origination to be included in a block...${NC}"
sleep 10

echo -e "\n${GREEN}Ready to start rollup node (script 5)${NC}"
