#!/bin/bash
#
# Install rbak git hooks (pre-commit, etc.)
# Configures core.hooksPath to point at .githooks/
#
# Usage: ./scripts/install-git-hooks.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if ! git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "Not inside a git repository: $REPO_ROOT" >&2
  exit 1
fi

git -C "$REPO_ROOT" config core.hooksPath .githooks

echo "✅ Git hooks configured to use .githooks"
echo "   pre-commit now runs cargo fmt --all for staged Rust files"
