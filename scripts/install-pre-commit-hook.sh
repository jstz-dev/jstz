#!/usr/bin/env bash
set -euo pipefail

cp ./scripts/pre-commit-hook.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
echo "Successfully install pre-commit hook (scripts/pre-commit-hook.sh) into .git/hooks."