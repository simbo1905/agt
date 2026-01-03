# Issue 005: `git add` and `git commit` Implementation Report

## Summary
I have implemented the initial version of `git add` and `git commit` commands directly within `agt`, replacing the dependency on the system `git` binary for these operations. This allows `agt` to function in environments where `git` might not be configured or available, and provides tighter integration with the agent workspace.

## Implementation Details

### `git add`
The implementation supports:
- `git add <path>...`: Stages specific files.
- `git add -A` / `--all`: Stages all files in the worktree, handling additions, modifications, and deletions.
- `git add -u` / `--update`: Stages modifications and deletions of tracked files only.

The implementation manually manages the git index:
1.  Loads the current index.
2.  Walks the filesystem (using `jwalk`).
3.  Updates index entries with new stat/oid information.
4.  Writes the index back to disk.

### `git commit`
The implementation supports:
- `git commit -m "message"`: Creates a commit with the given message.
- Supports multiple `-m` flags (joined by newlines).
- Automatically detects author/committer from `agt` config or defaults.
- Updates the current HEAD reference.

## Why Issue 005 is "Hard"

While the basic functionality works, one significant challenge remains, explaining why this issue has been difficult to fully resolve:

### 1. `.gitignore` Support (The Missing Piece)
The current implementation of `git add -A` uses a naive directory walker that **does not respect `.gitignore` rules**. 

I have verified this behavior with a reproduction test case:
- **Test**: `test_git_add_all_respects_gitignore` in `crates/agt/tests/issue_005.rs`.
- **Result**: Fails because `ignore_me.txt` (listed in `.gitignore`) is incorrectly added to the index.

**Why it's complex**:
`gitoxide` (gix) provides powerful low-level tools (plumbing), but high-level "porcelain" features like "add directory respecting ignores" are not yet available as simple one-liners. To implement this correctly, we must:
- Instantiate a `gix::worktree::Stack` or use `gix::dir::walk`.
- Load and parse `.gitignore` files from all directories in the path hierarchy.
- efficient query the ignore status for every file encountered during the walk.
- Wiring this up with the current `jwalk` implementation or switching to `gix`'s directory walker is a non-trivial architectural change involving the `gix-ignore` and `gix-dir` crates.

### 2. Path Canonicalization
We encountered and fixed issues with path resolution on macOS, where `/var` is a symlink to `/private/var`. The implementation now attempts to canonicalize paths to ensure relative path calculations against the worktree root are correct.

## Verification
I created a new test suite `crates/agt/tests/issue_005.rs` containing:
- `test_git_commit_multiple_messages`: **PASS** (Verifies commit creation and message parsing).
- `test_git_add_all_respects_gitignore`: **FAIL** (Verifies the missing ignore support).

## Recommendation
To fully close Issue 005, the next step is to integrate `gix::ignore` into the file walking logic. This is the primary blocker for a production-ready `git add` replacement.
