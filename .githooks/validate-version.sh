#!/bin/sh
set -eu

git_bin=${AGT_GIT_PATH:-$(command -v git)}
repo_root=$($git_bin rev-parse --show-toplevel)
cd "$repo_root"

latest_tag=$($git_bin describe --tags --match "release/*" --abbrev=0 2>/dev/null || true)

if [ -z "$latest_tag" ]; then
	exit 0
fi

case "$latest_tag" in
release/*)
	latest_version=${latest_tag#release/}
	;;
*)
	exit 0
	;;
esac

latest_base=${latest_version%%-*}

read_version() {
	awk -F '"' '/^version = "/ { print $2; exit }' "$1"
}

check_version_file() {
	file=$1
	current_version=$(read_version "$file")
	if [ -z "$current_version" ]; then
		printf '%s\n' "ERROR: could not read version from $file" >&2
		exit 1
	fi

	if [ "$current_version" != "$latest_base" ]; then
		printf '%s\n' "ERROR: $file version $current_version does not match latest reachable release base $latest_base ($latest_tag)" >&2
		exit 1
	fi
}

check_version_file "Cargo.toml"
check_version_file "crates/agt/Cargo.toml"
check_version_file "crates/agt-worktree/Cargo.toml"
