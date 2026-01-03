# Suite 7: Stress and Scale Testing

## Objective

Test agt's behavior under scale:
- Many files
- Large files
- Deep directories
- Many agents
- Rapid autocommits
- Large history

## Working Directory

`.tmp/suite7`

## Setup

1. Build all binaries: `make build` (builds both `agt` and vendored `gix`)
2. Create directory: `mkdir -p .tmp/suite7 && cd .tmp/suite7`
3. Initialize an agt-managed repo

## Reference

Read `docs/agt.1.txt`:
- Timestamp-based scanning (jwalk performance)
- Object store sharing

## Scenarios

### Scenario 7.1: 10,000 Files

Test with many small files.

Steps:
1. Fork session
2. Create 10,000 small files (e.g., 100 bytes each)
3. Time the autocommit
4. Verify all files captured
5. Modify 100 files
6. Time second autocommit
7. Verify only 100 in new commit

Success: Autocommit completes in reasonable time, correct file counts

### Scenario 7.2: 1GB Single File

Test with very large file.

Steps:
1. Fork session
2. Create 1GB file (use dd or head from /dev/urandom)
3. Time the autocommit
4. Note memory usage
5. Modify 1 byte
6. Time second autocommit
7. Note: entire file re-stored

Success: Handles large file without crash, documents performance

### Scenario 7.3: Deep Directory Tree

Test with deeply nested structure.

Steps:
1. Create directory 50 levels deep
2. Put files at various depths
3. Autocommit
4. Verify all depths captured
5. Modify file at deepest level
6. Autocommit
7. Verify only that file in commit

Success: Deep paths handled correctly

### Scenario 7.4: Wide Directory Tree

Test with many directories at same level.

Steps:
1. Create 1000 directories at top level
2. Put file in each
3. Autocommit
4. Verify all captured

Success: Wide trees handled correctly

### Scenario 7.5: 100 Parallel Agents

Test with many concurrent sessions.

Steps:
1. Fork 100 agent sessions
2. Verify all created successfully
3. Make change in each
4. Autocommit all (maybe in batches)
5. List all sessions
6. Verify no cross-contamination

Success: 100 agents work independently

### Scenario 7.6: Rapid Sequential Autocommits

Test high-frequency commits.

Steps:
1. Fork session
2. In a loop: modify file, autocommit (100 times)
3. Verify 100 commits created
4. Each commit has correct parent
5. Git log shows linear history

Success: Rapid autocommits work without race conditions

### Scenario 7.7: Concurrent Autocommits

Test simultaneous autocommits for different agents.

Steps:
1. Fork 10 agents
2. Start autocommit for all 10 simultaneously (background processes)
3. Wait for all to complete
4. Verify no corruption
5. Each agent has correct history

Success: Concurrent autocommits are safe

### Scenario 7.8: Large History

Test with many historical commits.

Steps:
1. Create 1000 autocommits (with small changes each)
2. Test git log performance
3. Test git blame performance
4. Test checkout of historical commits
5. Verify reflog doesn't become unwieldy

Success: Large history is navigable

### Scenario 7.9: Object Store Growth

Monitor object store size.

Steps:
1. Record initial `.git/objects` size
2. Create many commits
3. Record size after
4. Run `git gc`
5. Record size after gc
6. Compare efficiency

Success: Storage growth is reasonable, gc works

### Scenario 7.10: Memory Usage

Monitor memory during operations.

Steps:
1. Monitor process memory during various operations:
   - Large file autocommit
   - Many files autocommit
   - git log with many commits
2. Document memory patterns
3. Verify no memory leaks (memory releases after operation)

Success: Memory usage is bounded and reasonable

### Scenario 7.11: Timestamp Resolution

Test timestamp scanning edge cases.

Steps:
1. Create two files within same second
2. Modify one within same second as creation
3. Autocommit
4. Verify both captured correctly
5. Test with subsecond modifications if possible

Success: Timestamp scanning handles edge cases

### Scenario 7.12: Mixed Workload

Combined stress test.

Steps:
1. Fork 10 agents
2. Each agent:
   - Creates 1000 files
   - Includes some large files (10MB each)
   - Has deep directories
3. Autocommit all
4. User makes commits while this happens
5. Merge 3 agents
6. Prune rest

Success: Mixed workload completes correctly

## Success Criteria

- No crashes or data corruption under load
- Performance is documented (not necessarily fast, but understood)
- Memory usage is bounded
- Concurrent operations are safe

## Failure Modes

- OOM (out of memory)
- Timeout
- Corruption under concurrent access
- Missing files in large batches
- Performance degradation (non-linear scaling)

## Performance Baseline Documentation

Record these metrics during testing:

| Operation | File Count | Size | Time | Memory Peak |
|-----------|------------|------|------|-------------|
| Autocommit 100 files | 100 | 1KB each | ___ | ___ |
| Autocommit 10,000 files | 10,000 | 100B each | ___ | ___ |
| Autocommit 1 file | 1 | 1GB | ___ | ___ |
| Fork session | - | - | ___ | ___ |
| git log (1000 commits) | - | - | ___ | ___ |

## Notes

This is NOT acceptance criteria - it's to understand performance characteristics. The tool should work correctly at any scale; speed is secondary but should be documented.
