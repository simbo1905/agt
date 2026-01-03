#!/bin/sh
# Suite Setup Script
# Creates isolated git config and unique agt settings per suite

set -eu

# Parse suite number from directory name
SUITE_DIR="$(basename "$PWD")"
SUITE_NUM="${SUITE_DIR#suite}"

# Validate suite number
case "$SUITE_NUM" in
[0-9]*) ;; # Valid: starts with digit
*)
	echo "Error: Not in a suite directory (expected .tmp/suiteN)" >&2
	exit 1
	;;
esac

# Create isolated git config
GIT_CONFIG="$PWD/gitconfig"
cat >"$GIT_CONFIG" <<EOF
[user]
    name = AGT Test Suite $SUITE_NUM
    email = agt.suite$SUITE_NUM@test.local
[core]
    autocrlf = false
    safecrlf = false
[init]
    defaultBranch = main
[agt]
    agentEmail = agt.suite$SUITE_NUM.agent@test.local
    branchPrefix = agtsessions/suite$SUITE_NUM/
    userEmail = user.suite$SUITE_NUM@test.local
EOF

# Export git config for this session
export GIT_CONFIG_GLOBAL="$GIT_CONFIG"

# Verify setup
echo "Suite $SUITE_NUM setup complete:"
echo "  - Git config: $GIT_CONFIG"
echo "  - Agent email: agt.suite$SUITE_NUM.agent@test.local"
echo "  - Branch prefix: agtsessions/suite$SUITE_NUM/"
echo "  - User email: user.suite$SUITE_NUM@test.local"
echo ""
echo "To use this configuration, run:"
echo "  export GIT_CONFIG_GLOBAL=\"$GIT_CONFIG\""
