#!/bin/bash
# Script 2: Setup Protocol and Accounts
# This replicates what jstzd does for protocol setup
# Run this in Terminal 2 after the node is running

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}=== Setting up Protocol and Accounts (jstzd equivalent) ===${NC}"

# Load environment
source /tmp/jstz-debug-env.sh

# Wait for node to be ready (jstzd waits 30 retries × 1 second)
echo -e "\n${YELLOW}Waiting for node to be ready...${NC}"
for i in {1..30}; do
  if curl -s http://localhost:18731/health/ready >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Node is ready!${NC}"
    break
  fi
  echo -n "."
  sleep 1
done
echo ""

# Import ALL 8 bootstrap accounts that jstzd uses
# These are from crates/jstzd/resources/bootstrap_account/accounts.json
echo -e "\n${BLUE}Importing 8 bootstrap accounts (jstzd uses all of these)...${NC}"

# Activator (1 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key activator unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6 --force

# Injector (100,000,000,000 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key injector unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh --force

# Rollup operator (100,000,000,000 mutez) - THIS IS A BOOTSTRAP ACCOUNT IN JSTZD!
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key rollup_operator unencrypted:edsk3D6aGWpSiMMiEfNZ7Jyi52S9AtjvLCutqnCi3qev65WShKLKW4 --force

# Bootstrap1 (100,000,000,000 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key bootstrap1 unencrypted:edsk3Admhyr2GZe5bz7LAx5KEjz6U8U56ouvdsgCuFRf2m6Wakhebz --force

# Bootstrap2 (100,000,000,000 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key bootstrap2 unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo --force

# Bootstrap3 (100,000,000,000 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key bootstrap3 unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ --force

# Bootstrap4 (100,000,000,000 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key bootstrap4 unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3 --force

# Bootstrap5 (100,000,000,000 mutez)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key bootstrap5 unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm --force

echo -e "${GREEN}✓ All 8 bootstrap accounts imported${NC}"

# Show addresses
echo -e "\n${BLUE}Account addresses:${NC}"
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 show address activator
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 show address injector
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 show address rollup_operator

# Activate protocol using octez protocol parameters (not jstzd/tests params!)
echo -e "\n${BLUE}Activating protocol...${NC}"

# Use the octez sandbox protocol parameters that jstzd uses
PARAMS_FILE="$BASE_DIR/protocol_params.json"
OCTEZ_PARAMS="/Users/alanmarko/projects/jstz_attempt2/jstz/crates/octez/resources/protocol_parameters/sandbox/ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK"

if [ -f "$OCTEZ_PARAMS" ]; then
  echo "Using octez parameters from: $OCTEZ_PARAMS"
  cp "$OCTEZ_PARAMS" "$PARAMS_FILE"

  # Add the bootstrap accounts to the parameters
  # jstzd adds these programmatically in config.rs:387-395
  echo -e "\n${BLUE}Adding bootstrap accounts to protocol parameters...${NC}"

  # Use jq to add bootstrap accounts if it's available, otherwise use python
  if command -v jq &>/dev/null; then
    jq '. + {
      "bootstrap_accounts": [
        ["edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2", "1"],
        ["edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav", "100000000000"],
        ["edpktoeTraS2WW9iaceu8hvHJ1sUJFSKavtzrMcp4jwMoJWWBts8AK", "100000000000"],
        ["edpkuumXzkHj1AhFmjEVLRq4z54iU2atLPUKt4fcu7ihqsEBiUT4wK", "100000000000"],
        ["edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9", "100000000000"],
        ["edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV", "100000000000"],
        ["edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU", "100000000000"],
        ["edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n", "100000000000"]
      ]
    }' "$PARAMS_FILE" >"$PARAMS_FILE.tmp" && mv "$PARAMS_FILE.tmp" "$PARAMS_FILE"
  else
    # Fallback to python if jq not available
    python3 -c "
import json
with open('$PARAMS_FILE', 'r') as f:
    params = json.load(f)
params['bootstrap_accounts'] = [
    ['edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2', '1'],
    ['edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav', '100000000000'],
    ['edpktoeTraS2WW9iaceu8hvHJ1sUJFSKavtzrMcp4jwMoJWWBts8AK', '100000000000'],
    ['edpkuumXzkHj1AhFmjEVLRq4z54iU2atLPUKt4fcu7ihqsEBiUT4wK', '100000000000'],
    ['edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9', '100000000000'],
    ['edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV', '100000000000'],
    ['edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU', '100000000000'],
    ['edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n', '100000000000']
]
with open('$PARAMS_FILE', 'w') as f:
    json.dump(params, f, indent=2)
"
  fi
else
  echo "Error: Could not find octez params at $OCTEZ_PARAMS"
  exit 1
fi

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  -block genesis activate protocol ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK \
  with fitness 1 and key activator and parameters "$PARAMS_FILE"

echo -e "${GREEN}✓ Protocol activated${NC}"

echo -e "\n${YELLOW}You can now proceed to script 3 to start the baker${NC}"
