#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
	echo "usage: run-with-agt-log.sh <command> [args...]" >&2
	exit 2
fi

if [[ -n "${AGT_LOG_PATH:-}" ]]; then
	rm -f -- "$AGT_LOG_PATH"
fi

set +e
"$@"
status=$?
set -e

log_status=0

if [[ -n "${AGT_LOG_PATH:-}" ]]; then
	if [[ -f "$AGT_LOG_PATH" ]]; then
		echo "--- AGT log: $AGT_LOG_PATH ---"
		cat -- "$AGT_LOG_PATH"
		if ! bash "$(dirname "$0")/check-no-error-in-logs.sh" "$AGT_LOG_PATH"; then
			log_status=1
		fi
	else
		echo "No AGT log found at $AGT_LOG_PATH"
	fi
fi

if [[ $status -ne 0 ]]; then
	exit "$status"
fi

exit "$log_status"
