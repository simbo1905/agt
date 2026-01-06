# Suite 4: Time Travel and Rollback

## Objective

Verify the ability to:
1. Roll back an agent session to any previous shadow commit state
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

Read `DESIGN_20260104.md`:
- Section 4: "Snapshot Restore Process"
- Section 3: "Shadow Branch Topology"

## Scenarios

### Scenario 4.1: Build a History to Roll Back

Create an agent with multiple autocommits.

Steps:
1. Create session: `agt session new --id agent-alpha`
2. In sandbox, create file A, run `agt autocommit --session-id agent-alpha`
3. Create file B, autocommit
4. Create file C, autocommit
5. Modify file A, autocommit
6. Delete file B, autocommit
7. Record all shadow commit SHAs: `git log agtsessions/agent-alpha --oneline`

Success: 5 commits on shadow branch, each with different file states

### Scenario 4.2: Restore to Previous Shadow Commit

Undo the last change using agt session restore.

Steps:
1. Note current state (no file B, modified A)
2. Find shadow commit SHA where file B still exists
3. Restore: `agt session restore --session-id agent-alpha --commit <SHA>`
4. Verify file B is restored
5. Verify file A is in state from that commit
6. Verify git index is restored from `_/index`
7. Verify agent config/state folders match that commit

Success: Complete session state (sandbox + agent state + index) reflects historical commit

### Scenario 4.3: Restore Multiple Commits Back

Go back further in time.

Steps:
1. Restore to the commit where file A was just created (first autocommit)
2. Verify only file A exists in sandbox
3. Verify agent state folders reflect that point in time
4. Shadow branch log still shows full history (commits not deleted)

Success: Can jump to arbitrary past state with full session restoration

### Scenario 4.4: Continue from Restored State

Resume work after restore.

Steps:
1. From restored state, make new changes in sandbox
2. Run `agt autocommit --session-id agent-alpha`
3. Verify new shadow commit's parent1 is the restored-to commit
4. Verify parent2 is current user branch tip
5. History now has a fork (though old commits may become unreachable)

Success: Can continue working from any restored point

### Scenario 4.5: Fork from Historical Point

Create new agent from past state of another agent.

Steps:
1. Restore agent-alpha to historical commit
2. Fork: `agt session fork --from agent-alpha --id agent-omega`
3. Agent-omega makes different changes
4. Autocommit both sessions
5. Verify agent-omega has independent history

Success: Can branch timelines

### Scenario 4.6: Uncommitted Work and Restore

What happens to uncommitted sandbox changes during restore?

Steps:
1. Agent makes changes in sandbox but doesn't autocommit
2. Restore to earlier commit
3. Observe: uncommitted changes are LOST (replaced by shadow tree)
4. Document this behavior - autocommit before restore to preserve work

Success: Test documents expected behavior (uncommitted = lost on restore)

### Scenario 4.7: Shadow Commit Recovery via Reflog

Use git reflog to find "abandoned" shadow commits after restore.

Steps:
1. Create multiple autocommits
2. Restore to earlier state
3. Create new autocommits (branching shadow history)
4. Use reflog: `git reflog agtsessions/agent-alpha`
5. Restore to one of the "abandoned" commits

Success: Reflog preserves full shadow branch history

### Scenario 4.8: Restore with Evolved User Branch

Restore while user branch has advanced.

Steps:
1. Agent at shadow commit SC1 (parent2 = user@1)
2. Agent autocommit SC2 (parent2 = user@2)
3. Agent autocommit SC3 (parent2 = user@3)
4. Restore to SC1
5. User makes commit user@4 on user branch
6. Agent autocommit SC4 from restored state
7. Verify SC4 has: parent1 = SC1, parent2 = user@4

Success: Restore preserves dual-parent tracking with current user branch

### Scenario 4.9: Verify Index Restoration

Confirm git index is properly restored.

Steps:
1. In sandbox, stage files: `git add file1.txt file2.txt`
2. Do NOT commit to user branch
3. Run autocommit (captures staged files in `_/index`)
4. Unstage files: `git reset HEAD`
5. Make other changes and autocommit
6. Restore to earlier commit with staged files
7. Verify `git status` shows file1.txt and file2.txt as staged

Success: Git index state is restored from `_/index`

### Scenario 4.10: Verify .gitignore Files Restored

Confirm files ignored by user branch are restored.

Steps:
1. Create `node_modules/` in sandbox (typically in .gitignore)
2. Autocommit (shadow tree includes node_modules)
3. Delete `node_modules/`
4. Autocommit
5. Restore to commit where node_modules existed
6. Verify `node_modules/` is restored

Success: Shadow commits capture and restore .gitignore'd files

## Success Criteria

- `agt session restore` restores complete session folder state
- Sandbox directory matches historical shadow tree
- Agent config/state folders (xdg/, config/) are restored
- Git index is restored from `_/index`
- .gitignore'd files are restored from shadow tree
- New autocommits from restored state have correct dual-parents
- Reflog preserves abandoned shadow commits

## Failure Modes

- Restore only affects sandbox, not agent state folders
- Git index not restored (staged files lost)
- .gitignore'd files not restored
- Autocommit after restore has wrong parent1
- Dual-parent tracking breaks after restore
- Reflog doesn't show abandoned shadow commits

## Critical Understanding

The key insight: AGT's shadow commits capture the COMPLETE session state, not just user code. Restore reconstructs:
1. The sandbox with user code AND .gitignore'd build artifacts
2. The agent config and state folders
3. The git index (staged but uncommitted changes)

This is fundamentally different from `git reset --hard` which only affects the user branch worktree. Use `agt session restore` for full time travel.
