# Suite 4: Time Travel and Rollback

## Objective

Verify the ability to:
1. Roll back an agent session to any previous state
2. Fork from any point in agent history
3. Recover from bad agent decisions
4. Maintain complete history despite rollbacks

## Working Directory

`.tmp/suite4`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite4 && cd .tmp/suite4`
3. Initialize an agt-managed repo with content

## Reference

Read `docs/agt.1.txt`:
- "Roll back to any point in agent history"
- "Fork from any state"
- Commit graph section
- Examples section: "Roll back an agent session by 3 commits"

## Scenarios

### Scenario 4.1: Build a History to Roll Back

Create an agent with multiple autocommits.

Steps:
1. Fork agent session
2. Create file A, autocommit
3. Create file B, autocommit
4. Create file C, autocommit
5. Modify file A, autocommit
6. Delete file B, autocommit
7. Record all commit SHAs

Success: 5 commits on agent branch, each with different file states

### Scenario 4.2: Roll Back One Commit

Undo the last change.

Steps:
1. Note current state (no file B, modified A)
2. Roll back one commit: `git reset --hard HEAD~1`
3. Verify file B still deleted (wait, that's HEAD~2)
4. Verify file A is back to modified state
5. Check that working directory matches the commit

Success: Working directory reflects previous commit state

### Scenario 4.3: Roll Back Multiple Commits

Go back further in time.

Steps:
1. Reset to the commit where file B still exists
2. Verify files A, B, C all exist
3. Verify file A is in original state
4. Git log still shows full history (commits not deleted)

Success: Can jump to arbitrary past state

### Scenario 4.4: Continue from Rolled Back State

Resume work after rollback.

Steps:
1. From rolled-back state, make new changes
2. Autocommit
3. Verify new commit's parent is the rolled-back-to commit
4. History now has a fork (though old commits may become unreachable)

Success: Can continue working from any point

### Scenario 4.5: Fork from Historical Point

Create new agent from past state of another agent.

Steps:
1. Reset agent-alpha to historical commit
2. Fork agent-omega from current HEAD (historical point)
3. Agent-omega makes different changes
4. Autocommit
5. Verify agent-omega has independent history

Success: Can branch timelines

### Scenario 4.6: Recover Uncommitted Work After Rollback

What happens to uncommitted changes during reset?

Steps:
1. Agent makes changes but doesn't autocommit
2. Reset to earlier commit
3. Observe: uncommitted changes are LOST
4. Document this behavior - it's expected git behavior

Success: Test documents expected behavior (uncommitted = lost on reset)

### Scenario 4.7: Reflog Recovery

Use git reflog to find "lost" commits.

Steps:
1. Create commits
2. Reset back
3. Create new commits (branching history)
4. Use reflog to find the "abandoned" commits
5. Cherry-pick or checkout those commits

Success: Reflog preserves full history

### Scenario 4.8: Time Travel with User Branch Evolution

Rollback while user branch has advanced.

Steps:
1. Agent at commit A (parent: user@1)
2. Agent autocommit B (parent: user@2)
3. Agent autocommit C (parent: user@3)
4. Roll back to A
5. User makes commit user@4
6. Agent autocommit D from state A
7. Verify D has parents: A and user@4

Success: Time travel integrates with user branch tracking

### Scenario 4.9: Stash as Alternative to Rollback

Using stash instead of hard reset.

Steps:
1. Agent has uncommitted changes
2. Stash changes
3. Explore previous commits
4. Pop stash to restore

Success: Stash provides non-destructive rollback

### Scenario 4.10: Partial Rollback - Single File

Restore one file from history.

Steps:
1. Agent has modified multiple files across commits
2. Use `git checkout <commit> -- <file>` to restore single file
3. Autocommit
4. Verify only that file changed, others preserved

Success: Selective file-level time travel works

## Success Criteria

- Hard reset to any commit works
- Working directory accurately reflects historical state
- New work from historical point creates proper commits
- Dual-parent tracking continues correctly after rollback
- Reflog preserves abandoned commits

## Failure Modes

- Reset doesn't update working directory
- Files from "future" commits persist after reset
- Autocommit after rollback has wrong parent
- Dual-parent breaks after time travel
- Reflog doesn't show abandoned commits

## Critical Understanding

The key insight: Git's immutable object store means commits are NEVER truly deleted. Even after rollback, the "future" commits still exist in the object store and can be recovered via reflog or if you know the SHA.

This is what makes time travel safe - you can always get back to any state that was ever autocommitted.
