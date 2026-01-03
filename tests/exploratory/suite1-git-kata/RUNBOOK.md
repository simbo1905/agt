# Suite 1: Git Passthrough Kata - Basic `gix` Operations

## Objective

Verify that invoking `agt` as `git` reliably delegates to the vendored `gix` CLI (not system git), and that a minimal set of Git-like commands works as documented.

Note: This suite does **not** assert full `git(1)` compatibility. `gix` is not feature-complete.

## Working Directory

`.tmp/suite1`

## Setup

1. Build all binaries: `make build` (builds both `agt` and vendored `gix`)
2. Create working directory: `mkdir -p .tmp/suite1 && cd .tmp/suite1`
3. Create a symlink or alias so `agt` can be invoked as `git`:
   - Option A: `ln -s $(pwd)/target/release/agt ./git` and use `./git`
   - Option B: Just test with `agt` directly (it should pass through)

Note: When invoked as `git`, agt delegates to the vendored `gix` CLI, not system Git.
This ensures consistent behavior. Verify with `./git --version` which should mention `gix`.

## Reference

Read `docs/agt.1.txt` - specifically:
- "GIT COMMANDS (both modes)" section
- Limitations and environment variables (`AGT_GIX_PATH`, `AGT_WORKTREE_PATH`)

## Scenarios

### Scenario 1.0: Verify Vendored gix Passthrough

Verify that git-mode uses the vendored `gix`, not system Git.

Steps:
1. Create symlink: `ln -s /path/to/target/release/agt ./git`
2. Run `./git --version`
3. Verify output mentions `gix` (e.g., "gix v0.49.0...")
4. Verify output does NOT mention "Apple Git" or system git version

Success: Passthrough uses vendored gix binary

### Scenario 1.1: Repository Discovery + Status

Test that basic repo discovery and status works:
- Create a repository with `agt init` (or `gix clone`)
- Run `git status` (via `./git status`)
- Verify it runs and exits successfully

Success: `git status` works as expected

### Scenario 1.2: Basic Add + Commit

- Create a file
- Stage it (`git add <file>`)
- Commit it (`git commit -m "message"`)

Success: Commit exists and contains the file

### Scenario 1.3: Log / Branch / Tag Listing

- View the log (`git log`)
- List branches (`git branch`)
- List tags (`git tag`)

Success: Commands run successfully

### Scenario 1.4: Clone / Fetch (Local)

- Create a local bare repo as a "remote"
- Run `git clone` from it (via gix passthrough)
- Run `git fetch` from it

Success: Clone/fetch succeed for local remotes

Note: `git log` filtering in git mode only supports the default format. Custom
formats (`--oneline`, `--pretty`, `--format`) require `--disable-agt`.

## Success Criteria

All scenarios must pass. Failures indicate either:
- Vendored `gix` passthrough is not wired correctly, or
- Documentation claims exceed current `gix` capabilities

## Failure Modes

- Command not recognized
- Output format differs from standard git
- Exit codes don't match git behavior
- Error messages differ significantly

## Notes

This suite does NOT test agt-specific commands like `agt autocommit`. It purely validates `gix` passthrough behavior.
