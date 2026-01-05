# Suite 10: Worktree Helper Tool

## Objective

Verify the `agt-worktree` sandbox helper can create and remove linked worktrees
for a bare repository in the vanilla case.

## Working Directory

`.tmp/suite10`

## Setup

1. Build binaries:
   - `make build`
2. Create directory: `mkdir -p .tmp/suite10 && cd .tmp/suite10`

## Scenario 10.1: Add Worktree (Bare Repo)

Steps:
1. Create a bare repo and add a commit (use system git for the spike if needed):
   - `git init --bare repo.git`
   - create a temp non-bare repo, add commit to `main`, push to `repo.git`
2. Run:
   - `../..//target/release/agt-worktree add --git-dir repo.git --worktree wt --name wt --branch refs/heads/main`
3. Inspect:
   - `wt/.git` contains `gitdir: <repo.git/worktrees/wt>`
   - `repo.git/worktrees/wt/HEAD` exists and points to `refs/heads/main`
   - `repo.git/worktrees/wt/commondir` is `../..`
   - `wt/README.md` exists (checked out)

Success: Worktree is created, metadata is correct, and files are checked out.

## Scenario 10.2: Remove Worktree

Steps:
1. Run:
   - `../..//target/release/agt-worktree remove --git-dir repo.git --worktree wt --name wt`
2. Verify:
   - `wt/` is removed
   - `repo.git/worktrees/wt/` is removed

Success: Worktree and metadata are removed.
