# Suite 8: Failure Recovery

## Objective

Test what happens when things go wrong:
- Interrupted operations
- Corrupt state
- Missing files
- Permission errors
- Disk full scenarios
- Recovery mechanisms

## Working Directory

`.tmp/suite8`

## Setup

1. Build all binaries: `make build` (builds both `agt` and vendored `gix`)
2. Create directory: `mkdir -p .tmp/suite8 && cd .tmp/suite8`
3. Initialize an agt-managed repo

## Reference

Read `docs/agt.1.txt`:
- EXIT STATUS section
- Error handling behavior

## Scenarios

### Scenario 8.1: Interrupted Autocommit

Test killing autocommit mid-operation.

Steps:
1. Fork session, create many files
2. Start autocommit in background
3. Kill the process mid-way (SIGKILL)
4. Examine state:
   - Is worktree intact?
   - Is index intact?
   - Is timestamp file correct?
5. Try running autocommit again
6. Verify recovery works

Success: System recoverable after interrupt

### Scenario 8.2: Interrupted Fork

Test killing fork mid-operation.

Steps:
1. Start fork operation
2. Kill mid-way
3. Examine state:
   - Partial branch created?
   - Partial worktree created?
4. Clean up manually or retry fork
5. Verify can reach good state

Success: Interrupt leaves cleanable state

### Scenario 8.3: Missing Timestamp File

Test behavior with missing state files.

Steps:
1. Fork session normally
2. Delete the timestamp file: `rm .git/agt/timestamps/session-id`
3. Try autocommit
4. Verify error message is helpful
5. Recreate timestamp file manually
6. Verify autocommit works again

Success: Clear error, recoverable state

### Scenario 8.4: Corrupt Timestamp File

Test with invalid state files.

Steps:
1. Fork session
2. Write garbage to timestamp file
3. Try autocommit
4. Verify error message is helpful
5. Fix timestamp file
6. Verify recovery

Success: Graceful error handling

### Scenario 8.5: Missing Worktree

Test when worktree is deleted externally.

Steps:
1. Fork session
2. Delete worktree directory: `rm -rf sessions/session-id`
3. Try various operations:
   - autocommit (should fail gracefully)
   - list-sessions (should show issue)
   - prune (should handle gracefully)

Success: Missing worktree detected, not crashed

### Scenario 8.6: Detached Worktree

Test when .git file is removed from worktree.

Steps:
1. Fork session
2. Delete the `.git` file inside worktree
3. Try autocommit
4. Verify error handling

Success: Graceful error

### Scenario 8.7: Branch Already Exists

Test fork when branch name taken.

Steps:
1. Fork session "test-session"
2. Try to fork another session with same ID
3. Verify error message
4. Verify original session unaffected

Success: Clear error, no corruption

### Scenario 8.8: Permission Denied

Test with permission issues.

Steps:
1. Fork session
2. Create file in session
3. Make file read-only: `chmod 000 file`
4. Try autocommit
5. Verify behavior (may succeed reading, or fail gracefully)

Success: Graceful handling of permission issues

### Scenario 8.9: Disk Full Simulation

Test behavior when disk is "full".

Steps:
1. Create a small tmpfs or use quota if available
2. Fill it near capacity
3. Try autocommit with large files
4. Verify error handling
5. Free space, retry

Note: This may be hard to test without root. Document approach.

Success: Disk full errors handled gracefully

### Scenario 8.10: Corrupt Git Objects

Test with corrupt object store.

Steps:
1. Fork session, make commits
2. Identify an object file in `.git/objects`
3. Corrupt it (truncate or modify bytes)
4. Try various git operations
5. Document error messages
6. Use `git fsck` to diagnose
7. May need to restore from backup or re-clone

Success: Corruption detected, not silently ignored

### Scenario 8.11: Concurrent Modification Conflict

Test when same file modified during autocommit.

Steps:
1. Create a large file in session
2. Start autocommit
3. While autocommit is running, modify the file
4. Verify behavior:
   - Does it use old or new version?
   - Is there corruption?
   - Is there an error?

Success: Consistent behavior, no corruption

### Scenario 8.12: Invalid Session ID

Test with problematic session IDs.

Steps:
1. Try to fork with empty session ID
2. Try with spaces: "my session"
3. Try with slashes: "my/session"
4. Try with special chars: "session<>|"
5. Verify each case is handled

Success: Invalid IDs rejected with clear errors

### Scenario 8.13: Recovery After Crash

Full crash recovery test.

Steps:
1. Fork session
2. Make changes
3. Autocommit
4. Make more changes (uncommitted)
5. Simulate system crash (just leave state as-is)
6. "Restart" (pretend fresh process)
7. Examine state
8. Continue working

Success: Can resume after simulated crash

### Scenario 8.14: Git Lock Files

Test when git lock files are present.

Steps:
1. Create a lock file: `touch .git/index.lock`
2. Try git operations
3. Verify proper error message
4. Remove lock
5. Verify operations work

Success: Lock files handled per standard git

### Scenario 8.15: Orphaned Worktrees

Test cleanup of orphaned state.

Steps:
1. Fork session
2. Manually delete branch (but leave worktree)
3. Try operations
4. Clean up properly
5. Verify no residual state

Success: Orphaned state detectable and cleanable

## Success Criteria

- Interrupted operations don't corrupt state
- Missing/corrupt files produce helpful errors
- State is always recoverable (even if manually)
- No silent data loss
- Error messages actionable

## Failure Modes

- Corruption after interrupt
- Unhelpful error messages
- Unrecoverable state
- Silent data loss
- Crashes instead of errors

## Recovery Checklist

When things go wrong, try:
1. `git status` - see git's view
2. `git fsck` - check object integrity
3. Check `.git/agt/` state files
4. Check `git worktree list`
5. Check branch references: `git branch -a`
6. Check reflog: `git reflog`

## Notes

Some failure modes may be hard to reproduce reliably. Document any that require specific timing or system conditions.
