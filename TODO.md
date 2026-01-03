# TODO

## Must Fix

- Fix build plumbing: ensure `make build` builds the `agt-worktree` helper so `agt fork`/`agt prune-session` work in release installs (Makefile/README consistency).
- Fix `agt autocommit` parent2 semantics: parent2 must be the user branch head recorded at `agt fork` time, not the agent worktree `HEAD` (`crates/agt/src/commands/autocommit.rs`).
- Fix docs over-claiming “All standard Git commands” when passthrough is `gix` and some commands/subcommands are missing; align `docs/agt.1.txt` and examples accordingly.
- Implement or remove documented `AGT_DEBUG` behavior (docs mention debug output but code currently doesn’t emit it).
- Fix branch filtering for `+`-prefixed lines (e.g. `gix branch` output) (`crates/agt/src/commands/passthrough.rs`).
- Fix negative timestamp handling in scanners (avoid `i64 -> u64` wrap) (`crates/agt/src/scanner.rs`).

## Should Fix

- Improve `git log` filtering robustness: default “Author:” parsing only works for default pretty output; custom formats leak agent commits (`crates/agt/src/commands/passthrough.rs`).
- Clarify or implement `agt.userEmail` usage (currently unused, but documented).
- Make binary discovery more robust for non-monorepo installs (or document env var requirements clearly) (`crates/agt/src/gix_cli.rs`).

## Must Fix (Documentation)

- `docs/agt.1.txt`: explicitly state detached HEADs in agent worktrees are unsupported for `agt autocommit`.
- `docs/agt.1.txt`: clarify symlink behavior in autocommit (stored as symlink, not followed), including warning about symlinks targeting outside the worktree.
- `docs/agt.1.txt`: ensure session metadata location/creation matches implementation (`<common-dir>/agt/sessions/<id>.json`).

## Must Fix (Code)

- Use consistent timestamp types (prefer `u64` epoch seconds in timestamp files; validate CLI inputs).
- `init`: ensure bare repo config explicitly keeps `core.bare = true` (and doesn’t conflict with linked worktree metadata).
- `fork`: store a resolved `from` value in session metadata (commit id and/or branch), not a literal `"HEAD"` placeholder.
