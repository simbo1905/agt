# Suite 2: Single Agent Workflow

## Objective

Verify the complete lifecycle of a single agent session:
1. Initialize agt-managed repository
2. Fork a session
3. Agent makes changes
4. Autocommit captures changes
5. User makes concurrent commits
6. Agent changes merged back

## Working Directory

`.tmp/suite2`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite2 && cd .tmp/suite2`
3. The `agt` binary should be accessible (add to PATH or use full path)

## Reference

Read `docs/agt.1.txt` thoroughly, especially:
- `agt init` command
- `agt fork` command
- `agt autocommit` command
- Repository Layout section
- Commit Graph section

## Scenarios

### Scenario 2.1: Repository Initialization

Use `agt init` to clone a test repository.

Steps:
1. Create a source bare repo with some initial content
2. Run `agt init` pointing to that source
3. Verify the expected layout:
   - `<name>.git/` - bare repository
   - `<name>/` - main worktree
   - `<name>/.git` - file pointing to bare repo
   - `<name>.git/agt/timestamps/` - exists
   - `<name>.git/agt/sessions/` - exists

Success: Layout matches docs/agt.1.txt "Repository Layout" section

### Scenario 2.2: Fork a Session

Create an agent session.

Steps:
1. Navigate to the main worktree
2. Run `agt fork --session-id agent-001`
3. Verify:
   - Branch `agtsessions/agent-001` exists
   - Worktree at `sessions/agent-001/` exists
   - Timestamp file at `.git/agt/timestamps/agent-001` exists
   - Can `cd sessions/agent-001` and it's a valid git worktree

Success: Session infrastructure created correctly

### Scenario 2.3: Agent Work Without Autocommit

Simulate agent making changes without committing.

Steps:
1. Enter the session worktree: `cd sessions/agent-001`
2. Create/modify several files
3. Verify files exist in worktree
4. Check `git status` shows changes (but not committed)

Success: Normal file operations work in session worktree

### Scenario 2.4: Autocommit Captures Changes

Use autocommit to snapshot the agent's work.

Steps:
1. From main repo, run autocommit for the session
2. Verify a commit was created on `agtsessions/agent-001`
3. Verify the commit has the configured agent email as author
4. Verify all modified files are in the commit (including any that would be .gitignore'd)

Success: Autocommit creates commit with all modified files

### Scenario 2.5: Interleaved User Commits

While agent works, user makes commits on main.

Steps:
1. In main worktree (not session), make changes and commit
2. Run another autocommit for the agent session
3. Verify the agent commit has TWO parents:
   - Parent 1: previous agent commit
   - Parent 2: current user branch HEAD

Success: Dual-parent commit structure works

### Scenario 2.6: Multiple Autocommits

Test that sequential autocommits work correctly.

Steps:
1. Agent makes changes, autocommit
2. Agent makes more changes, autocommit
3. Agent makes more changes, autocommit
4. Verify linear history on agent branch
5. Verify each commit only contains files changed since last autocommit

Success: Timestamp-based scanning only captures new changes

### Scenario 2.7: Merge Agent Work

Bring agent work back to main branch.

Steps:
1. From main worktree, merge the agent branch
2. Verify the agent's changes appear on main
3. Verify merge commit is created

Success: Standard git merge works with agent branches

### Scenario 2.8: Session Cleanup

Remove the session after work is done.

Steps:
1. Run appropriate cleanup/prune command
2. Verify worktree is removed
3. Verify branch can optionally be kept or deleted

Success: Session cleanup works per docs

## Success Criteria

Complete lifecycle works:
init → fork → work → autocommit → user commits → more autocommit → merge → cleanup

## Failure Modes

- `agt init` doesn't create expected structure
- `agt fork` fails to create worktree
- Autocommit doesn't find modified files
- Autocommit creates wrong parent structure
- Timestamp not updated after autocommit
- Merge conflicts unexpectedly
- Cleanup fails

## Notes

This is the "happy path" for single agent. Multi-agent scenarios are in Suite 3.
