#!/usr/bin/env bash
set -euo pipefail

# Run linting
make lint

# Check if linting passed
if [ $? -ne 0 ]; then
  echo "Linting failed. Aborting push."
  exit 1
fi

exit 0
