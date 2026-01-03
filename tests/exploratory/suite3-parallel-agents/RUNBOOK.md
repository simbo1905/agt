# Suite 3: Parallel Agent Workflows

## Objective

Verify that multiple agents can work concurrently without interfering with each other, and that selective merging works correctly.

The MVP scenario: "Fork three agents, delete two, merge one"

## Working Directory

`.tmp/suite3`

## Setup

1. Build all binaries: `make build` (builds both `agt` and vendored `gix`)
2. Create directory: `mkdir -p .tmp/suite3 && cd .tmp/suite3`
3. Initialize an agt-managed repo with some starting content

## Reference

Read `docs/agt.1.txt`:
- Multiple worktrees sharing object store
- `agt fork --from` option
- Session isolation
- Worktree independence

## Scenarios

### Scenario 3.1: Fork Three Agents from Same Point

Create three parallel agent sessions from the same starting commit.

Steps:
1. Initialize repo with some base content
2. Fork agent-alpha from HEAD
3. Fork agent-beta from HEAD
4. Fork agent-gamma from HEAD
5. Verify all three have independent worktrees
6. Verify all three branches exist
7. Verify they all start from the same commit

Success: Three independent sessions created from same base

### Scenario 3.2: Parallel Independent Work

Each agent works on different things simultaneously.

Steps:
1. In agent-alpha worktree: create `feature-a/` files, modify existing files
2. In agent-beta worktree: create `feature-b/` files, modify DIFFERENT existing files
3. In agent-gamma worktree: create `feature-c/` files
4. Verify changes are isolated (each worktree only sees its own changes)
5. Run git status in each - should only show local changes

Success: Worktrees are truly isolated

### Scenario 3.3: Concurrent Autocommits

Autocommit each agent's work.

Steps:
1. Autocommit agent-alpha
2. Autocommit agent-beta
3. Autocommit agent-gamma
4. Verify each branch has its own commit with its own changes
5. Verify no cross-contamination of files

Success: Each autocommit captures only that agent's changes

### Scenario 3.4: User Commits During Agent Work

User makes commits while all agents are working.

Steps:
1. In main worktree, make a commit
2. Autocommit all three agents
3. Verify each agent's new commit has:
   - Parent 1: their previous commit
   - Parent 2: the new user commit

Success: All agents track user branch evolution

### Scenario 3.5: Discard Two Agents

Abandon agent-beta and agent-gamma.

Steps:
1. Remove agent-beta worktree
2. Remove agent-gamma worktree
3. Optionally delete their branches
4. Verify agent-alpha is unaffected
5. Verify main worktree is unaffected

Success: Discarding agents doesn't affect others

### Scenario 3.6: Merge Surviving Agent

Merge agent-alpha into main.

Steps:
1. From main worktree, merge agent-alpha branch
2. Verify agent-alpha's changes appear on main
3. Verify no trace of beta/gamma work appears

Success: Selective merge works

### Scenario 3.7: Conflicting Work

Test agents that modify the same files.

Steps:
1. Fork two new agents
2. Both modify the same file differently
3. Autocommit both
4. Try to merge first agent - succeeds
5. Try to merge second agent - conflict expected
6. Resolve conflict or abandon

Success: Git conflict resolution works normally with agent branches

### Scenario 3.8: Fork from Another Agent

One agent forks from another agent's session.

Steps:
1. Agent-delta forks from agent-alpha (not from main)
2. Agent-delta makes changes
3. Autocommit agent-delta
4. Verify agent-delta's history includes agent-alpha's commits

Success: Forking from agent branches works

### Scenario 3.9: Object Store Efficiency

Verify that parallel agents share the object store.

Steps:
1. Create a large file in one agent
2. Autocommit
3. Create the SAME file in another agent
4. Autocommit
5. Check that only one blob exists in the object store (same SHA)

Success: Deduplication works across agents

### Scenario 3.10: Many Parallel Agents

Scale test with more agents.

Steps:
1. Fork 10 agents
2. Each makes unique changes
3. Autocommit all
4. List all session branches
5. Merge 3 of them selectively
6. Prune the rest

Success: System handles many concurrent agents

## Success Criteria

- No cross-contamination between agents
- Selective merge works
- Discard doesn't affect others
- Object store deduplication works
- Conflict handling works normally

## Failure Modes

- Changes leak between worktrees
- Autocommit picks up wrong files
- Merge brings in wrong changes
- Deleting one agent breaks another
- Object store bloat from duplication

## Notes

This suite validates the core parallel workflow that makes agt useful for multi-agent AI systems.
