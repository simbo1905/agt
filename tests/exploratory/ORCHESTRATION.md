# Test Orchestration Guide

## Running Tests with Parallel Agents

This document explains how to assign test suites to AI coding agents for parallel execution.

## Agent Assignment Matrix

| Agent | Suite | Directory | Focus |
|-------|-------|-----------|-------|
| Agent A | Suite 1 | `.tmp/suite1` | Git Kata (basic compatibility) |
| Agent B | Suite 2 | `.tmp/suite2` | Single agent lifecycle |
| Agent C | Suite 3 | `.tmp/suite3` | Parallel agents (fork 3, delete 2, merge 1) |
| Agent D | Suite 4 | `.tmp/suite4` | Time travel and rollback |
| Agent E | Suite 5 | `.tmp/suite5` | Edge cases (binary, symlinks, etc.) |
| Agent F | Suite 6 | `.tmp/suite6` | Dual-mode filtering |
| Agent G | Suite 7 | `.tmp/suite7` | Stress testing |
| Agent H | Suite 8 | `.tmp/suite8` | Failure recovery |
| Agent I | Suite 9 | `.tmp/suite9` | Documentation audit |
| Agent J | Suite 10 | `.tmp/suite10` | Worktree helper tool |

## Prerequisites

Before assigning agents:

1. **Build all binaries**:
   ```bash
   make build
   ```
   This builds `agt`, `agt-worktree`, and the vendored `gix` CLI.

2. **Create working directories**:
   ```bash
   mkdir -p .tmp/suite{1..10}
   ```

3. **Ensure each agent can access**:
   - The `agt` binary (at `target/release/agt`)
   - The vendored `gix` binary (at `vendor/gitoxide/target/release/gix`)
   - Their assigned `.tmp/suiteN` directory
   - The documentation (`docs/agt.1.txt`)
   - Their suite runbook (`tests/exploratory/suiteN-*/RUNBOOK.md`)

## Agent Instructions Template

Copy this prompt for each agent, replacing `N` with suite number:

```
You are testing the `agt` tool. Your task is to execute Suite N.

Working directory: .tmp/suiteN
Runbook: tests/exploratory/suiteN-*/RUNBOOK.md
Reference documentation: docs/agt.1.txt

Instructions:
1. Read your runbook thoroughly
2. Read the reference documentation (docs/agt.1.txt)
3. Create your working directory if it doesn't exist
4. Execute each scenario in the runbook
5. For each scenario, record:
   - PASS: Behavior matches documentation
   - FAIL: Behavior differs from documentation
   - BLOCKED: Could not test (explain why)
6. At the end, produce a summary report

The documentation is the specification. If the tool behaves differently
from the documentation, that's a bug (in either tool or docs).

Do not ask for clarification on HOW to do things - figure it out from
the documentation. The test is whether the docs are complete enough
for you to succeed.
```

## Parallelization Strategy

### Fully Parallel (No Dependencies)

All suites can run in parallel. They use isolated directories:
- Suite 1: `.tmp/suite1`
- Suite 2: `.tmp/suite2`
- etc.

### Suggested Groupings

If you have limited agents, group by priority:

**Critical Path (run first)**:
- Suite 1: Git Kata - if basic git fails, nothing else matters
- Suite 2: Single Agent - the core use case
- Suite 9: Doc Audit - verification that docs match reality

**Core Features**:
- Suite 3: Parallel Agents - the multi-agent story
- Suite 4: Time Travel - unique value proposition
- Suite 6: Filtering - the dual-mode story

**Edge Cases and Robustness**:
- Suite 5: Edge Cases
- Suite 7: Stress Test
- Suite 8: Failure Recovery

## Expected Outputs

Each agent should produce:

### 1. Summary Report
```
Suite N: [Name]
Total Scenarios: X
Passed: Y
Failed: Z
Blocked: W

Failed Scenarios:
- Scenario N.M: [Brief description of failure]
```

### 2. Detailed Log
For each scenario:
- Commands executed
- Output received
- Pass/fail determination
- Evidence (output snippets, file contents)

### 3. Issue List
Any bugs or documentation discrepancies found:
```
- [TOOL BUG] Description
- [DOC BUG] Description
- [MISSING] Feature described but not implemented
- [UNCLEAR] Documentation ambiguous about X
```

## Handling Failures

When a scenario fails:

1. **Document the failure precisely**
   - What was expected (from docs)
   - What happened (actual behavior)
   - Commands used
   - Full output

2. **Determine failure type**
   - Tool bug: Tool doesn't match docs
   - Doc bug: Docs describe wrong behavior
   - Test bug: Runbook scenario was flawed
   - Environment bug: Setup issue

3. **Continue testing**
   - Don't block on one failure
   - Other scenarios may still pass

## Cleanup

After all tests complete:

```bash
# Remove all test artifacts
rm -rf .tmp/suite*

# Or selectively clean failed suites for re-run
rm -rf .tmp/suite3
```

## Re-running Failed Suites

To re-run a specific suite:

```bash
# Clean just that suite
rm -rf .tmp/suite3

# Re-create
mkdir -p .tmp/suite3

# Assign agent again
```

## Integration with CI

These exploratory tests complement the unit tests in `crates/agt/tests/`.

For CI, consider:
1. Run unit tests first (`cargo test`)
2. Run exploratory suites in parallel
3. Aggregate results
4. Fail build if any suite fails

## Metrics to Track

Across all suites, track:
- Total scenarios: ~100+
- Pass rate
- Common failure patterns
- Documentation coverage gaps
- Performance baselines (Suite 7)

## Maintenance

When adding features:
1. Update `docs/agt.1.txt` first
2. Add scenarios to relevant suites
3. Run affected suites
4. Verify new features work as documented

When fixing bugs:
1. Identify which suite would have caught it
2. Add regression scenario if missing
3. Verify fix passes the scenario
