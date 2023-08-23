#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=./abstract.sh
commands_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${commands_dir}/abstract.sh"

# Parse arguments
while [[ $# -gt 0 ]]; do
    key="$1"

    case $key in
        --address)
            sr_address="$2"
            shift 2
            ;;
        *)
            cat <<EOF
Unknown option: $1

Options:
    --address <sr1>: rollup address, required to configure bridge contract
EOF
            exit 1
            ;;
    esac
done

# Contract source
src="${root_dir}/jstz_bridge/jstz_bridge.tz"
ctez_src="${root_dir}/jstz_bridge/jstz_ctez.tz"

# Originate ctez contract
bootstrap3_address="tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU"
init_ctez_storage="(Pair \"$bootstrap3_address\" { Elt \"$bootstrap3_address\" 10000000000 } )"
ctez_address=$(
    client originate contract "jstz_ctez" \
        transferring 0 from bootstrap3 \
        running "$ctez_src" \
        --init "$init_ctez_storage" \
        --burn-cap 999 \
        --force |
    grep "New contract" | 
    awk '{ print $3}'
)

# Originate bridge contract
init_storage="(Pair \"$ctez_address\" None)"
bridge_address=$(
    client originate contract "jstz_bridge" \
        transferring 0 from bootstrap3 \
        running "$src" \
        --init "$init_storage" \
        --burn-cap 999 \
        --force |
    grep "New contract" | 
    awk '{ print $3}'
)

# Set ticketer
# FIXME: This should (in future) be handled by smart-rollup-installer
set_ticketer_emsg=$(
    echo "{ \"SetTicketer\": \"${bridge_address}\" }" | xxd -ps | tr -d '\n' 
)

client send smart rollup message "hex:[ \"$set_ticketer_emsg\" ]" from bootstrap2\
    >/dev/null

# Set rollup address
client transfer 0 from bootstrap3 \
    to "jstz_bridge" \
    --entrypoint "set_rollup" \
    --arg "\"${sr_address}\"" \
    --burn-cap 999 \
    >/dev/null

cat <<EOF
The \`jstz_bridge\` contract has successfully been originated and configured. 
You may now run \`octez-client transfer 0 from .. to jstz_bridge ..\` to communicate
with \`jstz_rollup\` via the L1 layer.

To upgrade the bridge, run this command again after running \`make build-bridge\`.

EOF
