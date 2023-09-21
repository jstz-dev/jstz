#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=./abstract.sh
commands_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${commands_dir}/abstract.sh"

# Parse arguments
while [[ $# -gt 0 ]]; do
    key="$1"
    if [[ "$key" == --referer ]]
    then
        referer="$2"
        shift 2
    else
        url="$1"
        shift 1
    fi
done

# Create json message
jmsg=$(
    jq --null-input \
        --arg referer "$referer" \
        --arg url "$url" \
        '{ "Transaction": { "referer": { "Tz4": $referer }, "url": $url } }'
)

# Convert to external hex message
emsg=$(
    echo "$jmsg" | xxd -ps | tr -d '\n' 
)

# Send message
client send smart rollup message "hex:[ \"$emsg\" ]" from bootstrap2
