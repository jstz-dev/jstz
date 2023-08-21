#!/usr/bin/env bash
set -euo pipefail

# Determine the root directory of jstz
# shellcheck disable=2154
root_dir="${commands_dir}/../.."

# TODO: Expose this from `sandbox.sh`
rpc=18730

client() {
    "${root_dir}/octez-client" -base-dir "${OCTEZ_CLIENT_DIR}" -endpoint http://127.0.0.1:$rpc "$@"
}
