# TODO

## PIVOT: Real Git Passthrough

This is a major architectural change. AGT now spawns the real git binary and filters its stdout, instead of using gix CLI for passthrough.

### Must Fix

- [ ] Fix `agt autocommit` parent2 semantics: parent2 must be the user branch head recorded at `agt fork` time, not the agent worktree `HEAD` (`crates/agt/src/commands/autocommit.rs`).
- [ ] Fix negative timestamp handling in scanners (avoid `i64 -> u64` wrap) (`crates/agt/src/scanner.rs`).

### Should Fix

- [ ] Improve `git log` filtering robustness: default "Author:" parsing only works for default pretty output; custom formats leak agent commits (`crates/agt/src/commands/passthrough.rs`).
- [ ] Clarify or implement `agt.userEmail` usage (currently unused, but documented).

### Must Fix (Code)

- [ ] Use consistent timestamp types (prefer `u64` epoch seconds in timestamp files; validate CLI inputs).
- [ ] `init`: ensure bare repo config explicitly keeps `core.bare = true` (and doesn't conflict with linked worktree metadata).
- [ ] `fork`: store a resolved `from` value in session metadata (commit id and/or branch), not a literal `"HEAD"` placeholder.

## Completed

- [x] Create config module to read `~/.agtconfig` and `.agt/config` (ini-style, like gitconfig)
- [x] Add `agt.gitPath` config setting - path to real git binary
- [x] Rewrite passthrough to spawn real git and filter stdout line-by-line
- [x] Remove gix CLI dependency from passthrough (keep gix library for object/tree operations)
- [x] Remove `gix` binary from build (no longer needed in dist/)
- [x] Update `agt init` to create `.agt/config` with default settings
- [x] Implement `AGT_GIT_PATH` environment variable override
- [x] Implement or remove documented `AGT_DEBUG` behavior (implemented in passthrough.rs)
- [x] Update docs/agt.1.txt for real git passthrough architecture
- [x] Update README.md with new config files (~/.agtconfig, .agt/config)
- [x] Update AGENTS.md with new architecture
- [x] Update CODING_PROMPT.md with new architecture
- [x] docs/agt.1.txt: explicitly state detached HEADs unsupported
- [x] docs/agt.1.txt: clarify symlink behavior in autocommit
- [x] docs/agt.1.txt: session metadata location documented
