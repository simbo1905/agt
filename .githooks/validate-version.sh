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

parse_base_version() {
	case "$1" in
	[0-9]*.[0-9]*.[0-9]*-*)
		echo "${1%%-*}"
		;;
	*)
		echo "$1"
		;;
	esac
}

is_newer_or_equal() {
	current_base=$1
	latest_base=$2

	if [ "$(printf '%s\n%s\n' "$latest_base" "$current_base" | sort -V | head -n1)" != "$latest_base" ]; then
		echo "no"
	else
		echo "yes"
	fi
}

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

	current_base=$(parse_base_version "$current_version")

	if [ "$(is_newer_or_equal "$current_base" "$latest_base")" != "yes" ]; then
		printf '%s\n' "ERROR: $file version $current_version is older than latest reachable release base $latest_base ($latest_tag)" >&2
		exit 1
	fi
}

check_version_file "Cargo.toml"
check_version_file "crates/agt/Cargo.toml"
check_version_file "crates/agt-worktree/Cargo.toml"
