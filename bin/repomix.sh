#!/bin/bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${REPO_ROOT}/.tmp"

mkdir -p "$OUTPUT_DIR"

repomix \
  --style markdown \
  --include "src/**,tests/**,docs/**,bin/**,**/*.md" \
  --ignore ".worktrees/**,bin/.venv/**,bin/node_modules/**,*.dist-info/**" \
  --output "${OUTPUT_DIR}/repomix-output.md"

echo "✓ Repomix output generated at: ${OUTPUT_DIR}/repomix-output.md"
