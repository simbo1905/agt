# Suite 1: Git Passthrough Kata - Basic Git Operations

## Objective

Verify that invoking `agt` as `git` reliably spawns the real git binary and filters its output to hide agent branches/commits. Verify that a set of Git commands works as expected.

This suite tests full `git(1)` compatibility since we now spawn the real git binary.

## Working Directory

`.tmp/suite1`

## Setup

1. Build all binaries: `make build`
2. Create working directory: `mkdir -p .tmp/suite1 && cd .tmp/suite1`
3. Create a symlink so `agt` can be invoked as `git`:
   - Option A: `ln -s $(pwd)/dist/agt ./git` and use `./git`
   - Option B: Just test with `agt` directly (it should pass through)
4. Configure `AGT_GIT_PATH` to point to the real git binary:
   - `export AGT_GIT_PATH=/usr/bin/git` (or wherever git is installed)

Note: When invoked as `git`, agt spawns the real git binary and filters its stdout.

## Reference

Read `docs/agt.1.txt` - specifically:
- "GIT COMMANDS (both modes)" section
- Environment variables (`AGT_GIT_PATH`, `AGT_WORKTREE_PATH`)

## Scenarios

### Scenario 1.0: Verify Git Passthrough

Verify that git-mode spawns the real git binary.

Steps:
1. Create symlink: `ln -s /path/to/dist/agt ./git`
2. Set `AGT_GIT_PATH=/usr/bin/git` (or system git location)
3. Run `./git --version`
4. Verify output shows the system git version (e.g., "git version 2.x.x")

Success: Passthrough uses real git binary

### Scenario 1.1: Repository Discovery + Status

Test that basic repo discovery and status works:
- Create a repository with `agt init` (or `git clone`)
- Run `git status` (via `./git status`)
- Verify it runs and exits successfully

Success: `git status` works as expected

### Scenario 1.2: Log / Branch / Tag Listing

- View the log (`git log`)
- List branches (`git branch`)
- List tags (`git tag`)

Success: Commands run successfully

### Scenario 1.3: Clone / Fetch (Local)

- Create a local bare repo as a "remote"
- Run `git clone` from it (via git passthrough)
- Run `git fetch` from it

Success: Clone/fetch succeed for local remotes

Note: `git log` filtering in git mode only supports the default format. Custom
formats (`--oneline`, `--pretty`, `--format`) require `--disable-agt`.

### Scenario 1.4: `git add -A` respects `.gitignore`

- Create a repo with a `.gitignore` that excludes `ignore_me.txt`
- Create both `ignore_me.txt` and `include_me.txt`
- Run `./git add -A` followed by `./git commit -m "snapshot"`
- Inspect the commit tree (e.g., `./git ls-tree HEAD`)

Success: `ignore_me.txt` is **absent** while `include_me.txt` and `.gitignore` are present.

### Scenario 1.5: `git commit` supports multiple `-m` flags

- Stage a file (`./git add file.txt`)
- Run `./git commit -m "Title" -m "Body paragraph"`
- Inspect the commit message (`./git log -1`)

Success: Commit summary equals `Title` and the body contains `Body paragraph`.

### Scenario 1.6: External helper passthrough (`git-xxx`)

Validate that git mode honours git's external helper mechanism.

Steps:
1. Ensure Scenario 1.0 setup (`./git` symlink and `AGT_GIT_PATH` pointing to real git) is still active.
2. Create a helper script:
   ```bash
   mkdir -p bin
   cat > bin/git-xxx <<'EOF'
   #!/usr/bin/env bash
   echo "helper: git-xxx invoked"
   EOF
   chmod +x bin/git-xxx
   ```
3. Prepend the helper directory to PATH:
   ```bash
   export PATH="$(pwd)/bin:$PATH"
   ```
4. Run the helper via agt-as-git:
   ```bash
   ./git xxx
   ```

Success:
- Output includes `helper: git-xxx invoked` (the helper ran).
- Exit status is 0.
- No "unknown command" error from agt or the underlying git.

This scenario verifies the contract documented in `docs/agt.1.txt` that agt, when
invoked as `git`, delegates unrecognised subcommands to `git-<name>` helpers on PATH.

## Success Criteria

All scenarios must pass. Failures indicate either:
- Git passthrough is not wired correctly, or
- Filtering is interfering with normal git operations

## Failure Modes

- Command not recognized
- Output format differs from expected
- Exit codes don't match git behavior
- Error messages differ significantly

## Notes

This suite does NOT test agt-specific commands like `agt autocommit`. It purely validates git passthrough behavior.
