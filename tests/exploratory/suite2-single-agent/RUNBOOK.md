# Suite 2: Single Agent Workflow

## Objective

Verify the complete lifecycle of a single agent session:
1. Clone repository with agt
2. Create a new session
3. Agent makes changes
4. Autocommit captures changes
5. User makes concurrent commits
6. Export agent work to remote

## Terminology

- **Session folder**: `sessions/<id>/` - contains sandbox and tool state
- **Sandbox**: `sessions/<id>/sandbox/` - where the agent runs
- **Shadow branch**: `agtsessions/<id>` - where autocommits are stored

## Working Directory

`.tmp/suite2`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite2 && cd .tmp/suite2`
3. The `agt` binary should be accessible (add to PATH or use full path)

## Reference

Read `docs/agt.1.txt` thoroughly, especially:
- `agt clone` command
- `agt session new` command
- `agt session export` command
- `agt autocommit` command
- Repository Layout section

## Scenarios

### Scenario 2.1: Repository Clone

Use `agt clone` to set up an agt-managed repository.

Steps:
1. Create a source repo on GitHub/GitLab (or use a test repo)
2. Run `agt clone <url>`
3. Verify the expected layout:
   - `<name>/` - project directory
   - `<name>.git/` - bare repository
   - `<name>/.git` - file containing `gitdir: ../<name>.git`
   - `<name>/main/` - main worktree (user's working directory)
   - `<name>/main/.git` - file pointing to `../<name>.git/worktrees/main`
   - `<name>.git/agt/timestamps/` - exists
   - `<name>.git/agt/sessions/` - exists

Success: Layout matches docs/agt.1.txt ARCHITECTURE section

### Scenario 2.2: Create a New Session

Create an agent session.

Steps:
1. Navigate to project directory: `cd <name>`
2. Run `agt session new --id agent-001`
3. Verify:
   - Shadow branch `agtsessions/agent-001` exists
   - Session folder at `sessions/agent-001/` exists
   - Sandbox at `sessions/agent-001/sandbox/` exists
   - xdg folder at `sessions/agent-001/xdg/` exists
   - config folder at `sessions/agent-001/config/` exists
   - Timestamp file at `<name>.git/agt/timestamps/agent-001` exists
   - Can `cd sessions/agent-001/sandbox` and it's a valid git checkout

Success: Session infrastructure created correctly

### Scenario 2.3: Agent Work Without Autocommit

Simulate agent making changes without committing.

Steps:
1. Enter the sandbox: `cd sessions/agent-001/sandbox`
2. Create/modify several files
3. Verify files exist in sandbox
4. Check `git status` shows changes (but not committed)

Success: Normal file operations work in sandbox

### Scenario 2.4: Autocommit Captures Changes

Use autocommit to snapshot the agent's work.

Steps:
1. From project root, run autocommit for the session:
   `agt autocommit -C sessions/agent-001 --session-id agent-001`
2. Verify a shadow commit was created on `agtsessions/agent-001`
3. Verify the commit has the configured agent email as author
4. Verify shadow tree contains:
   - `sandbox/` - agent's code files
   - `xdg/` - tool state (may be empty)
   - `config/` - tool config (may be empty)
   - `_/index` - captured git index for the sandbox worktree

Success: Autocommit creates shadow commit with session folder contents

### Scenario 2.5: Interleaved User Commits

While agent works, user makes commits on main.

Steps:
1. In main worktree (`main/`), make changes and commit
2. Run another autocommit for the agent session
3. Verify the shadow commit has TWO parents:
   - Parent 1: previous shadow commit
   - Parent 2: current user branch HEAD

Success: Dual-parent shadow commit structure works

### Scenario 2.6: Multiple Autocommits

Test that sequential autocommits work correctly.

Steps:
1. Agent makes changes, autocommit
2. Agent makes more changes, autocommit
3. Agent makes more changes, autocommit
4. Verify linear history on shadow branch
5. Verify each commit only contains files changed since last autocommit

Success: Timestamp-based scanning only captures new changes

### Scenario 2.7: Export Session Work

Push user branch to remote.

Steps:
1. In sandbox, create a feature branch and make commits
2. Run `agt session export --session-id agent-001`
3. Verify:
   - User branch is pushed to origin
   - Shadow branch is NOT on remote
   - Command fails if sandbox has uncommitted changes

Success: Export pushes only user branches, never shadow branches

### Scenario 2.8: Session Cleanup

Remove the session after work is done.

Steps:
1. Run `agt session remove --id agent-001`
2. Verify session folder is removed
3. Shadow branch is preserved (not deleted)
4. Run `agt session remove --id agent-002 --delete-branch`
5. Verify shadow branch is also deleted

Success: Session cleanup works per docs

### Scenario 2.9: List Sessions

Verify session listing works.

Steps:
1. Create multiple sessions
2. Run `agt session list`
3. Verify output shows:
   - Session ID
   - Shadow branch name
   - Sandbox path

Success: Session listing shows expected information

## Success Criteria

Complete lifecycle works:
clone → session new → work → autocommit → user commits → more autocommit → export → remove

## Failure Modes

- `agt clone` doesn't create expected structure
- `agt session new` fails to create session folder/sandbox
- Autocommit doesn't find modified files
- Autocommit creates wrong parent structure
- Timestamp not updated after autocommit
- Export pushes shadow branches
- Cleanup fails

## Notes

This is the "happy path" for single agent. Multi-agent scenarios are in Suite 3.
