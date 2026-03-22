#!/usr/bin/env bash
set -euo pipefail

echo "=== Recreating Windows CI emulation environment in /tmp ==="

HARNESS_ROOT="/tmp/agt-test"
WORKSPACE="$HARNESS_ROOT/D/a/agt/agt"
RUNNER_TEMP="$HARNESS_ROOT/runner_temp"
GITHUB_ENV="$RUNNER_TEMP/github_env"
WORKSPACE_LOG="$WORKSPACE/agt.log"

echo "=== Cleaning up old harness ==="
rm -rf "$HARNESS_ROOT"
mkdir -p "$WORKSPACE" "$RUNNER_TEMP"

echo "=== Mirroring workspace into fake GitHub runner path ==="
rsync -a --delete \
	--exclude target \
	--exclude dist \
	--exclude .tmp \
	"$(git rev-parse --show-toplevel)/" "$WORKSPACE/"

echo "=== Building release binary ==="
cargo build --release --manifest-path "$WORKSPACE/Cargo.toml"

echo "=== Installing mock git.exe candidates with spaces in paths ==="
MOCK_GIT_ROOT="$HARNESS_ROOT/Program Files/Git"
mkdir -p "$MOCK_GIT_ROOT/bin" "$MOCK_GIT_ROOT/cmd" "$MOCK_GIT_ROOT/mingw64/bin"

if command -v git &>/dev/null; then
	REAL_GIT=$(which git)
	echo "Found real git at: $REAL_GIT"
	cp "$REAL_GIT" "$MOCK_GIT_ROOT/bin/git.exe" 2>/dev/null || cp "$REAL_GIT" "$MOCK_GIT_ROOT/bin/git" 2>/dev/null || true
	cp "$REAL_GIT" "$MOCK_GIT_ROOT/cmd/git.exe" 2>/dev/null || cp "$REAL_GIT" "$MOCK_GIT_ROOT/cmd/git" 2>/dev/null || true
	cp "$REAL_GIT" "$MOCK_GIT_ROOT/mingw64/bin/git.exe" 2>/dev/null || cp "$REAL_GIT" "$MOCK_GIT_ROOT/mingw64/bin/git" 2>/dev/null || true
fi

if [ -f "$WORKSPACE/target/release/agt" ]; then
	cp "$WORKSPACE/target/release/agt" "$WORKSPACE/target/release/agt.exe"
	ln -sf "$WORKSPACE/target/release/agt.exe" "$WORKSPACE/git.exe" 2>/dev/null || true
fi

echo "=== Setting up PATH with mock git candidates ==="
PATH_WITH_MOCKS="$MOCK_GIT_ROOT/bin:$MOCK_GIT_ROOT/cmd:$MOCK_GIT_ROOT/mingw64/bin:$PATH"

echo "=== Running Git resolution (PowerShell simulation) ==="
SELECTED_GIT=$(PATH="$PATH_WITH_MOCKS" which git)
echo "Selected git: $SELECTED_GIT"

echo "=== Setting up GitHub environment ==="
echo "AGT_TEST_REAL_GIT=$SELECTED_GIT" >"$GITHUB_ENV"
echo "AGT_LOG=1" >>"$GITHUB_ENV"
echo "AGT_LOG_PATH=$WORKSPACE_LOG" >>"$GITHUB_ENV"
echo "GITHUB_ENV=$GITHUB_ENV"

echo "=== Environment setup complete ==="
echo "HARNESS_ROOT: $HARNESS_ROOT"
echo "WORKSPACE: $WORKSPACE"
echo "GITHUB_ENV: $GITHUB_ENV"
echo "WORKSPACE_LOG: $WORKSPACE_LOG"
echo "PATH_WITH_MOCKS: $PATH_WITH_MOCKS"
echo ""
echo "To run tests:"
echo "  cd $WORKSPACE"
echo "  source $GITHUB_ENV"
echo "  PATH=\"$PATH_WITH_MOCKS\" cargo test --test integration_tests -- --nocapture"
echo ""
echo "To run full simulation:"
echo "  bash .tmp/run-windows-ci-emulation.sh"
