#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=./abstract.sh
commands_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${commands_dir}/abstract.sh"

# Parse arguments
contract=""
while [[ $# -gt 0 ]]; do
    key="$1"

    case $key in
        --self-address)
            self_address="$2"
            shift 2
            ;;
        --contract)
            contract="$2"
            shift 2
            ;;
        *)
            cat <<EOF
Unknown option: $1

Options:
    --self-address <tz4>: contract address when executing \`contract\` 
    --contract <string>: contract code
EOF
            exit 1
            ;;
    esac
done

# Read from stdin if not provided
if [ -z "$contract" ]; then 
    contract=$(cat)
fi

# Create json message
jmsg=$(
    jq --null-input \
        --arg originating_address "$self_address" \
        --arg contract_code "$contract" \
        --arg initial_balance "$balance"
        '{ "Transaction": { "contract_address": { "Tz4": $originating_address }, "contract_code": $contract_code, "initial_balance": $balance  } }'
)

# Convert to external hex message
emsg=$(
    echo "$jmsg" | xxd -ps | tr -d '\n' 
)

# Send message
client send smart rollup message "hex:[ \"$emsg\" ]" from bootstrap2
