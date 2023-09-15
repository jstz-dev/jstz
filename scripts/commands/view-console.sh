#!/usr/bin/env bash
set -euo pipefail

commands_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
logs_dir=${commands_dir}'/../../logs'

ERROR=🔴
WARN=🟠
INFO=🟢
LOG=🪵

grep_for=""
function addSymbol () {
    if [ -z "$grep_for" ]
    then
        grep_for="$1"
    else
        grep_for="$grep_for"'\|'"$1"
    fi
}
# Parse arguments
while [[ $# -gt 0 ]]; do
    key="$1"

    case $key in
        --log)
            addSymbol "$LOG"
            shift 1
            ;;
        --info)
            addSymbol "$INFO"
            shift 1
            ;;
        --warn)
            addSymbol "$WARN"
            shift 1
            ;;
        --error)
            addSymbol "$ERROR"
            shift 1
            ;;
        --custom)
            addSymbol "$2"
            shift 2
            ;;
        *)
            cat <<EOF
Unknown option: $1

Options:
    --log: view console.log
    --info: view console.info
    --warn: view console.warn
    --error: view console.error
EOF
            exit 1
            ;;
    esac
done
if [ -z "$grep_for" ]
then
    grep_for="$LOG"'\|'"$INFO"'\|'"$WARN"'\|'"$ERROR"
fi

tail -f ${logs_dir}/kernel.log | grep "$grep_for"
