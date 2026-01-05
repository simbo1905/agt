# Advanced Testing Strategies

Beyond the core 9 suites, here are additional testing dimensions to consider.

## Real Agent Workflow Testing

### Suite 10: Agent Simulation

Have an AI agent perform a realistic coding task using agt.

**Scenario**: "Fix a bug in a codebase"
1. `agt init` a real repository
2. `agt fork` a session
3. Agent reads code, understands problem
4. Agent makes changes (multiple files)
5. Autocommit periodically
6. Agent completes task
7. User reviews and merges

**Success**: The entire workflow feels natural, no agt friction.

**Scenario**: "Implement a feature with multiple attempts"
1. Agent forks session
2. Tries approach A, makes progress
3. Realizes A won't work, time travels back
4. Tries approach B
5. Succeeds, merges

**Success**: Time travel enables exploration without fear.

### Suite 11: Multi-Agent Collaboration

Have multiple AI agents work on related tasks.

**Scenario**: "Parallel feature development"
1. 3 agents each work on different features
2. All autocommitting to their sessions
3. User orchestrates which to merge
4. Merged features work together

**Scenario**: "Agent code review"
1. Agent A implements feature
2. Agent B reviews Agent A's code (reads from A's branch)
3. Agent B suggests changes
4. Agent A incorporates feedback

## Environment Testing

### Suite 12: Cross-Platform

Test on different operating systems.

- macOS (Apple Silicon, Intel)
- Linux (Ubuntu, Alpine, Arch)
- Windows (WSL2, native if supported)

**Focus**:
- File path handling
- Timestamp handling
- Symlink behavior
- Permission model differences

### Suite 13: Git Version Compatibility

Test with different git versions.

- git 2.25 (Ubuntu 20.04 default)
- git 2.34 (Ubuntu 22.04 default)
- git 2.43+ (latest)
- Minimal git (Alpine)

**Focus**:
- Worktree command compatibility
- Config file format
- Object format

### Suite 14: Filesystem Variations

Test on different filesystems.

- ext4 (Linux default)
- APFS (macOS)
- tmpfs (memory-based)
- Network filesystems (NFS, SMB)
- Case-sensitive vs case-insensitive

**Focus**:
- Timestamp resolution differences
- Case sensitivity handling
- Hard link support

## Integration Testing

### Suite 15: Tool Interoperability

Test with common git tools.

- tig (ncurses git browser)
- gitk (GUI)
- VS Code (Source Control panel)
- JetBrains IDEs
- GitHub CLI (`gh`)
- git-lfs

**Focus**:
- Do these tools work when pointed at agt-managed repo?
- Do they see filtered or unfiltered view?
- Any compatibility issues?

### Suite 16: CI/CD Integration

Test in CI environments.

- GitHub Actions
- GitLab CI
- Jenkins

**Scenario**: Agent runs in CI, commits go to session branch
**Focus**: Does autocommit work in ephemeral environments?

### Suite 17: Hooks and Scripts

Test with git hooks.

- pre-commit hooks
- post-commit hooks
- pre-push hooks

**Focus**: Do hooks fire for autocommit? Should they?

## Security Testing

### Suite 18: Sandbox Integration

Test with toybox chroot jails.

**Scenario**:
1. Spawn agent in chroot jail (using toybox)
2. Bind-mount agt as `/usr/bin/git`
3. Bind-mount sandbox folder as agent's working directory
4. Agent works normally
5. Verify agent cannot see outside jail
6. Verify agent cannot see other agent sessions

**Focus**:
- Jail configuration that works
- What the agent CAN see (git mode)
- What the agent CANNOT see

### Suite 19: Privilege Escalation Prevention

Test that agent cannot escape intended boundaries.

**Scenario**:
1. Agent is given limited view
2. Try to access agent-only commands
3. Try to read/write outside sandbox
4. Try to corrupt shared state

**Focus**: Defense in depth

### Suite 20: Data Confidentiality

Test that sensitive data is handled properly.

**Scenario**:
1. User has secrets in main branch
2. Agent session should not see certain files
3. Test .gitignore behavior (it's bypassed)
4. Test if this is a problem

**Focus**: Understanding what agent can access

## Data Integrity Testing

### Suite 21: Long-term Storage

Test data persists correctly over time.

**Scenario**:
1. Create sessions, make commits
2. `git gc` runs
3. Time passes (simulate by touching timestamps)
4. Prune and gc run again
5. Historical data still accessible

**Focus**: Reflog expiry, gc behavior

### Suite 22: Corruption Detection

Test detection of corrupt state.

**Scenario**:
1. Create valid state
2. Corrupt various files:
   - Object files
   - Refs
   - agt state files
3. Verify detection
4. Verify recovery options

**Focus**: Fail-safe behavior

### Suite 23: Backup and Restore

Test backup/restore workflows.

**Scenario**:
1. Full state exists
2. Backup bare repo
3. Destroy original
4. Restore from backup
5. All sessions recoverable

**Focus**: What needs to be backed up?

## Performance Testing

### Suite 24: Profiling

Profile agt performance.

**Focus**:
- Time spent in file scanning (jwalk)
- Time spent in git operations (real git passthrough)
- Memory allocation patterns
- I/O patterns

### Suite 25: Comparison with git

Benchmark agt vs raw git.

**Scenarios**:
- Simple commands (status, log, branch)
- Complex commands (merge, rebase)

**Focus**: What's the overhead of agt layer?

## Upgrade and Migration

### Suite 26: Version Upgrade

Test upgrading agt versions.

**Scenario**:
1. Use agt v0.1 to create state
2. Upgrade to v0.2
3. Existing state still works
4. New features available

**Focus**: Forward compatibility

### Suite 27: Schema Migration

If state file formats change, test migration.

**Focus**: Automated or manual migration

## Regression Testing

### Suite 28: Bug Reproduction

For each bug found, create a minimal reproduction.

**Format**:
```
Bug #N: Description
Steps to reproduce:
1. ...
2. ...
Expected: ...
Actual: ...
Fixed in: commit SHA
Regression test: scenario added to suite X
```

## Chaos Testing

### Suite 29: Random Chaos

Randomly inject failures and verify resilience.

**Scenarios**:
- Kill processes randomly
- Inject I/O errors
- Corrupt random bytes
- Race conditions

**Focus**: System survives random failures

### Suite 30: Fuzzing

Fuzz inputs to agt commands.

**Focus**:
- Invalid arguments
- Malformed session IDs
- Malformed paths
- Invalid timestamps

## Testing Meta

### How to Add New Suites

1. Create `tests/exploratory/suiteNN-name/RUNBOOK.md`
2. Follow the established format:
   - Objective
   - Working Directory
   - Setup
   - Reference
   - Scenarios
   - Success Criteria
   - Failure Modes
3. Update `README.md` with new suite
4. Update `ORCHESTRATION.md` if parallelization changes

### Test Quality Metrics

Track:
- Coverage: What % of docs/agt.1.txt is tested?
- Stability: Flaky test rate
- Maintenance: Time to update tests for new features
- Value: Bugs caught by exploratory vs unit tests
