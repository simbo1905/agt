#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
	echo "usage: check-no-error-in-logs.sh <log-path>" >&2
	exit 2
fi

log_path="$1"

if [[ ! -f "$log_path" ]]; then
	exit 0
fi

if grep -E '\[agt\].*(delegated command failed|delegated command spawn failed|timed out|panic|cannot write log file)' "$log_path" >/dev/null; then
	exit 1
fi

exit 0
