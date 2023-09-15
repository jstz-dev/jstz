#!/usr/bin/env bash
set -euo pipefail

script_dir="$(dirname "$0")"

main() {
    command="$1"
    shift 1

    case $command in
        deploy-bridge)
            # shellcheck source=./commands/deploy-bridge.sh
            "${script_dir}/commands/deploy-bridge.sh" "$@"
            ;;
        run-contract)
            # shellcheck source=./commands/run-contract.sh
            "${script_dir}/commands/run-contract.sh" "$@"
            ;;
        deposit)
            # shellcheck source=./commands/deposit.sh
            "${script_dir}/commands/deposit.sh" "$@"
            ;;
        view-console)
            # shellcheck source=./commands/deposit.sh
            "${script_dir}/commands/view-console.sh" "$@"
            ;;
        *)
            cat <<EOF
Usage: $0 <COMMAND>

Commands:
    deploy-bridge
    run-contract
    deposit
EOF
            exit 1
            ;;
    esac
}

if [ "$0" == "${BASH_SOURCE[0]}" ]; then
    main "$@"
fi
