# Suite 1: Git Kata - Basic Git Operations

## Objective

Verify that `agt` works as a drop-in replacement for `git` for all standard Git operations. This is the foundation - if basic git doesn't work, nothing else matters.

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
- "All standard Git commands are supported"

## Scenarios

### Scenario 1.0: Verify Vendored gix Passthrough

Verify that git-mode uses the vendored `gix`, not system Git.

Steps:
1. Create symlink: `ln -s /path/to/target/release/agt ./git`
2. Run `./git --version`
3. Verify output mentions `gix` (e.g., "gix v0.49.0...")
4. Verify output does NOT mention "Apple Git" or system git version

Success: Passthrough uses vendored gix binary

### Scenario 1.1: Repository Initialization

Test that basic repo creation works:
- Initialize a new repository
- Check status
- Verify .git directory structure

Success: `git init`, `git status` work as expected

### Scenario 1.2: Basic Commit Workflow

- Create a file
- Stage it (`git add`)
- Commit it (`git commit`)
- View the log (`git log`)

Success: Commit appears in log with correct message and author

### Scenario 1.3: Branching

- Create a branch
- Switch to it
- Make commits
- Switch back to main
- List branches

Success: Branch operations work, `git branch` shows branches correctly

### Scenario 1.4: Merging

- Create a feature branch
- Make changes on feature branch
- Switch to main
- Merge feature branch
- Verify merge commit

Success: Merge completes, history shows merge

### Scenario 1.5: Remote Operations (Local)

- Create a bare repo as "remote"
- Add it as a remote
- Push to it
- Clone from it to a new directory
- Pull changes

Success: Push/pull/clone work with local bare repo

### Scenario 1.6: Diff and Status

- Make changes to tracked file
- Check `git diff`
- Stage changes
- Check `git diff --staged`
- Check `git status`

Success: Diff output shows changes correctly

### Scenario 1.7: Reset and Checkout

- Make changes
- Discard with `git checkout -- file`
- Make and stage changes
- Unstage with `git reset`
- Create commits, then reset HEAD

Success: Working directory manipulation works

### Scenario 1.8: Stash

- Make changes
- Stash them
- Verify clean working directory
- Apply stash
- Verify changes restored

Success: Stash push/pop works

### Scenario 1.9: Tags

- Create a commit
- Tag it (lightweight and annotated)
- List tags
- Checkout by tag

Success: Tag operations work

### Scenario 1.10: Log Formatting

- Create several commits
- Test `git log --oneline`
- Test `git log --graph`
- Test `git log -p`

Success: Various log formats display correctly

## Success Criteria

ALL scenarios must pass. Any git command that fails when invoked via `agt` (in non-filtered mode) is a critical bug.

## Failure Modes

- Command not recognized
- Output format differs from standard git
- Exit codes don't match git behavior
- Error messages differ significantly

## Notes

This suite does NOT test agt-specific features like filtering or autocommit. It purely validates git compatibility.
