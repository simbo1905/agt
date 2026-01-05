# Suite 3: Parallel Agent Workflows

## Objective

Verify that multiple agents can work concurrently without interfering with each other, and that selective merging works correctly.

The MVP scenario: "Create three sessions, delete two, merge one"

## Terminology

- **Session folder**: `sessions/<id>/` - contains sandbox and tool state
- **Sandbox**: `sessions/<id>/sandbox/` - where the agent runs
- **Shadow branch**: `agtsessions/<id>` - where autocommits are stored

## Working Directory

`.tmp/suite3`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite3 && cd .tmp/suite3`
3. Clone an agt-managed repo: `agt clone <url>`

## Reference

Read `docs/agt.1.txt`:
- Multiple sessions sharing object store
- `agt session new --from` option
- `agt session fork` for parallel sessions
- Session isolation
- Sandbox independence

## Scenarios

### Scenario 3.1: Create Three Sessions from Same Point

Create three parallel agent sessions from the same starting commit.

Steps:
1. Clone repo with some base content: `agt clone <url>`
2. Create session alpha: `agt session new --id agent-alpha`
3. Create session beta: `agt session new --id agent-beta`
4. Create session gamma: `agt session new --id agent-gamma`
5. Verify all three have independent sandboxes
6. Verify all three shadow branches exist
7. Verify they all start from the same commit

Success: Three independent sessions created from same base

### Scenario 3.2: Parallel Independent Work

Each agent works on different things simultaneously.

Steps:
1. In agent-alpha sandbox: create `feature-a/` files, modify existing files
2. In agent-beta sandbox: create `feature-b/` files, modify DIFFERENT existing files
3. In agent-gamma sandbox: create `feature-c/` files
4. Verify changes are isolated (each sandbox only sees its own changes)
5. Run git status in each - should only show local changes

Success: Sandboxes are truly isolated

### Scenario 3.3: Concurrent Autocommits

Autocommit each agent's work.

Steps:
1. `agt autocommit -C sessions/agent-alpha --session-id agent-alpha`
2. `agt autocommit -C sessions/agent-beta --session-id agent-beta`
3. `agt autocommit -C sessions/agent-gamma --session-id agent-gamma`
4. Verify each shadow branch has its own commit with its own changes
5. Verify no cross-contamination of files

Success: Each autocommit captures only that agent's changes

### Scenario 3.4: User Commits During Agent Work

User makes commits while all agents are working.

Steps:
1. In main worktree (`main/`), make a commit
2. Autocommit all three agents
3. Verify each agent's new shadow commit has:
   - Parent 1: their previous shadow commit
   - Parent 2: the new user commit

Success: All agents track user branch evolution

### Scenario 3.5: Remove Two Sessions

Remove agent-beta and agent-gamma.

Steps:
1. `agt session remove --id agent-beta`
2. `agt session remove --id agent-gamma --delete-branch`
3. Verify agent-beta session folder is removed, but shadow branch preserved
4. Verify agent-gamma session folder AND shadow branch are removed
5. Verify agent-alpha is unaffected
6. Verify main worktree is unaffected

Success: Removing sessions doesn't affect others

### Scenario 3.6: Merge Surviving Agent

Merge agent-alpha into main.

Steps:
1. From main worktree, merge agent-alpha shadow branch:
   `cd main && git merge agtsessions/agent-alpha`
2. Verify agent-alpha's changes appear on main
3. Verify no trace of beta/gamma work appears

Success: Selective merge works

### Scenario 3.7: Conflicting Work

Test agents that modify the same files.

Steps:
1. Create two new sessions
2. Both modify the same file differently
3. Autocommit both
4. Try to merge first agent - succeeds
5. Try to merge second agent - conflict expected
6. Resolve conflict or abandon

Success: Git conflict resolution works normally with shadow branches

### Scenario 3.8: Fork Session for Parallel Work

One agent forks from another agent's session.

Steps:
1. `agt session fork --from agent-alpha --id agent-delta`
2. Agent-delta makes changes
3. Autocommit agent-delta
4. Verify agent-delta's shadow branch history includes agent-alpha's commits

Success: Session forking works for parallel work

### Scenario 3.9: Object Store Efficiency

Verify that parallel agents share the object store.

Steps:
1. Create a large file in one agent's sandbox
2. Autocommit
3. Create the SAME file in another agent's sandbox
4. Autocommit
5. Check that only one blob exists in the object store (same SHA)

Success: Deduplication works across agents

### Scenario 3.10: Many Parallel Agents

Scale test with more agents.

Steps:
1. Create 10 sessions: `for i in {1..10}; do agt session new --id agent-$i; done`
2. Each makes unique changes
3. Autocommit all
4. List all sessions: `agt session list`
5. Export 3 of them: `agt session export --session-id agent-1`
6. Remove the rest: `agt session remove --id agent-4 --delete-branch`

Success: System handles many concurrent agents

## Success Criteria

- No cross-contamination between agents
- Selective merge works
- Remove doesn't affect others
- Object store deduplication works
- Conflict handling works normally

## Failure Modes

- Changes leak between sandboxes
- Autocommit picks up wrong files
- Merge brings in wrong changes
- Removing one agent breaks another
- Object store bloat from duplication

## Notes

This suite validates the core parallel workflow that makes agt useful for multi-agent AI systems.
