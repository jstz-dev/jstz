#!/usr/bin/env bash
set -euo pipefail

# shellcheck source=./abstract.sh
commands_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${commands_dir}/abstract.sh"

# Parse arguments
while [[ $# -gt 0 ]]; do
    key="$1"

    case $key in
        --amount)
            amount="$2"
            shift 2
            ;;
        --to)
            to="$2"
            shift 2
            ;;
        --from)
            from="$2"
            shift 2
            ;;
        *)
            cat <<EOF
Unknown option: $1

Options:
    --from <tz1/alias>: L1 address that is depositing the tokens
    --to <tz4>: contract address that is recieving the tokens
    --amount <nat>: amount of L2 xtz (ctez) to transfer
EOF
            exit 1
            ;;
    esac
done

# Convert tz4 address to hexencoded bytes
# 1. Convert to bytes using `base58` command
# 2. `tail -c +4` outputs the bytes starting at byte 4 (skipping bytes 1-3, which is the tz4 prefix)
# 3. `head -c -4` removes the last 4 bytes (the cheksum)
# 4. xxd converts to hexcode
to_hex=$(echo -n "$to" | base58 -d - | tail -c +4 | head -c -4 | xxd -p -u) 

# Send deposit transfer
client transfer 0 \
    from "${from}" \
    to jstz_bridge \
    --entrypoint "deposit" \
    --arg "(Pair ${amount} 0x${to_hex})" \
    --burn-cap 999
