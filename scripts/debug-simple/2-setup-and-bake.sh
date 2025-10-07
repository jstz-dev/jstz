#!/bin/bash
set -e

source /tmp/jstz-debug-env.sh

for i in {1..30}; do
  if curl -s http://localhost:18731/health/ready >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key activator unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6 --force

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key injector unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh --force

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  import secret key rollup_operator unencrypted:edsk3D6aGWpSiMMiEfNZ7Jyi52S9AtjvLCutqnCi3qev65WShKLKW4 --force

cp /Users/alanmarko/projects/jstz_attempt2/jstz/crates/jstzd/tests/sandbox-params.json "$BASE_DIR/protocol_params.json"

# Use the same parameter file that jstzd uses by default
#cp crates/octez/resources/protocol_parameters/sandbox/ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK "$BASE_DIR/protocol_params.json"

# Add bootstrap accounts that jstzd adds (from crates/jstzd/resources/bootstrap_account/accounts.json)
#jq '.bootstrap_accounts += [
#  ["edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2", "4000000000000"],
#  ["edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav", "4000000000000"],
#  ["edpktoeTraS2WW9iaceu8hvHJ1sUJFSKavtzrMcp4jwMoJWWBts8AK", "4000000000000"],
#  ["edpkuumXzkHj1AhFmjEVLRq4z54iU2atLPUKt4fcu7ihqsEBiUT4wK", "4000000000000"],
#  ["edpktzNbDAUjUk697W7gYg2CRuBQjyPxbEg8dLccYYwKSKvkPvjtV9", "4000000000000"],
#  ["edpkuTXkJDGcFd5nh6VvMz8phXxU3Bi7h6hqgywNFi1vZTfQNnS1RV", "4000000000000"],
#  ["edpkuFrRoDSEbJYgxRtLx2ps82UdaYc1WwfS9sE11yhauZt5DgCHbU", "4000000000000"],
#  ["edpkv8EUUH68jmo3f7Um5PezmfGrRF24gnfLpH3sVNwJnV5bVCxL2n", "4000000000000"]
#]' "$BASE_DIR/protocol_params.json" > "$BASE_DIR/protocol_params_tmp.json"
#mv "$BASE_DIR/protocol_params_tmp.json" "$BASE_DIR/protocol_params.json"

octez-client --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  -block genesis activate protocol ProtoALphaALphaALphaALphaALphaALphaALphaALphaDdp3zK \
  with fitness 1 and key activator and parameters "$BASE_DIR/protocol_params.json"

octez-baker-alpha --base-dir "$CLIENT_DIR" --endpoint http://localhost:18731 \
  run with local node "$NODE_DIR" injector --liquidity-baking-toggle-vote pass --without-dal
