#!/bin/bash
# Script 2: Setup Protocol, Accounts, and Baker
# Run this in Terminal 2 after the node is running

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}=== Setting up Protocol and Accounts ===${NC}"

# Load environment
source /tmp/jstz-debug-env.sh

# Wait for node to be ready
echo -e "\n${YELLOW}Waiting for node to be ready...${NC}"
for i in {1..30}; do
    if curl -s http://localhost:18731/health/ready > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Node is ready!${NC}"
        break
    fi
    echo -n "."
    sleep 1
done
echo ""

# Import bootstrap accounts
echo -e "\n${BLUE}Importing bootstrap accounts...${NC}"

# Activator
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    import secret key activator unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6 --force

# Injector (bootstrap1)
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    import secret key injector unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh --force

# Rollup operator
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    import secret key rollup_operator unencrypted:edsk3D6aGWpSiMMiEfNZ7Jyi52S9AtjvLCutqnCi3qev65WShKLKW4 --force

echo -e "${GREEN}✓ Bootstrap accounts imported${NC}"

# Show addresses
echo -e "\n${BLUE}Account addresses:${NC}"
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 show address activator
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 show address injector
octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 show address rollup_operator

# Activate protocol
echo -e "\n${BLUE}Activating protocol...${NC}"

# Get protocol parameters - copy from jstzd's sandbox params
PARAMS_FILE="$BASE_DIR/protocol_params.json"
JSTZD_PARAMS="/Users/alanmarko/projects/jstz_attempt2/jstz/crates/jstzd/tests/sandbox-params.json"

if [ -f "$JSTZD_PARAMS" ]; then
    echo "Using parameters from: $JSTZD_PARAMS"
    cp "$JSTZD_PARAMS" "$PARAMS_FILE"
else
    echo "Warning: Could not find jstzd params, using minimal params"
    # Create minimal protocol parameters as fallback
    cat > "$PARAMS_FILE" << 'EOF'
{
  "bootstrap_accounts": [
    ["edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav", "4000000000000"],
    ["edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9", "4000000000000"],
    ["edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV", "4000000000000"],
    ["edpktoeTraS2WW9iaceu8hvHJ1sUJFSKavtzrMcp4jwMoJWWBts8AK", "4000000000000"]
  ],
  "consensus_rights_delay": 2,
  "blocks_per_cycle": 8,
  "blocks_per_commitment": 4,
  "nonce_revelation_threshold": 4,
  "cycles_per_voting_period": 8,
  "hard_gas_limit_per_operation": "1040000",
  "hard_gas_limit_per_block": "5200000",
  "proof_of_work_threshold": "-1",
  "minimal_stake": "6000000000",
  "vdf_difficulty": "50000",
  "seed_nonce_revelation_tip": "125000",
  "origination_size": 257,
  "issuance_weights": {
    "base_total_issued_per_minute": "85007812"
  },
  "cost_per_byte": "250",
  "hard_storage_limit_per_operation": "60000",
  "quorum_min": 2000,
  "quorum_max": 7000,
  "min_proposal_quorum": 500,
  "liquidity_baking_toggle_ew_period": 262144,
  "max_operations_time_to_live": 120,
  "minimal_block_delay": "1",
  "delay_increment_per_round": "1",
  "consensus_committee_size": 7000,
  "consensus_threshold": 4667,
  "minimal_participation_ratio": {
    "numerator": 2,
    "denominator": 3
  },
  "limit_of_delegation_over_baking": 9,
  "percentage_of_frozen_deposits_slashed_per_double_baking": 700,
  "percentage_of_frozen_deposits_slashed_per_double_attestation": 5000,
  "cache_script_size": 100000000,
  "cache_stake_distribution_cycles": 8,
  "cache_sampler_state_cycles": 8,
  "dal_parametric": {
    "feature_enable": false,
    "number_of_slots": 256,
    "attestation_lag": 4,
    "attestation_threshold": 50,
    "blocks_per_epoch": 1
  },
  "smart_rollup_arith_pvm_enable": false,
  "smart_rollup_origination_size": 6314,
  "smart_rollup_challenge_window_in_blocks": 20,
  "smart_rollup_stake_amount": "10000000000",
  "smart_rollup_commitment_period_in_blocks": 10,
  "smart_rollup_max_lookahead_in_blocks": 30000,
  "smart_rollup_max_active_outbox_levels": 20,
  "smart_rollup_max_outbox_messages_per_level": 100,
  "smart_rollup_number_of_sections_in_dissection": 32,
  "smart_rollup_timeout_period_in_blocks": 20,
  "smart_rollup_max_number_of_cemented_commitments": 5,
  "smart_rollup_max_number_of_parallel_games": 32,
  "smart_rollup_reveal_activation_level": {
    "raw_data": {
      "Blake2B": 0
    },
    "metadata": 0,
    "dal_page": 0,
    "dal_parameters": 0,
    "dal_attested_slots_validity_lag": 0
  },
  "smart_rollup_private_enable": true,
  "smart_rollup_riscv_pvm_enable": true
}
EOF
fi

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
    -block genesis activate protocol ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK \
    with fitness 1 and key activator and parameters "$PARAMS_FILE"

echo -e "${GREEN}✓ Protocol activated${NC}"

echo -e "\n${YELLOW}You can now proceed to script 3 to start the baker${NC}"

