#!/usr/bin/env bash
set -euo pipefail

cp ./scripts/pre-push-hook.sh .git/hooks/pre-push
cp ./scripts/pre-commit-hook.sh .git/hooks/pre-commit
cp ./scripts/commit-msg-hook.sh .git/hooks/commit-msg
chmod +x .git/hooks/pre-commit .git/hooks/commit-msg .git/hooks/pre-push
echo "Successfully installed git hooks (scripts/*-hook.sh) into .git/hooks/*"
