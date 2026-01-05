# Exploratory Test Suites for AGT

## Overview

These test suites are designed to be executed by AI coding agents. Each suite runs in an isolated `.tmp/suiteN` directory and validates different aspects of the `agt` tool.

## Philosophy

1. **Documentation is the spec** - If the tool behaves differently from `docs/agt.1.txt`, that's a bug (in either the tool or the docs)
2. **Agents discover the right way** - Runbooks describe WHAT to test, not exactly HOW
3. **Isolation** - Each suite uses its own `.tmp/suiteN` directory
4. **Parallel execution** - Suites can run concurrently in separate agent sessions

## Core Test Suites

| Suite | Directory | Focus |
|-------|-----------|-------|
| 1 | `.tmp/suite1` | Git Kata - basic git commands via agt |
| 2 | `.tmp/suite2` | Single Agent - one agent workflow with user commits |
| 3 | `.tmp/suite3` | Parallel Agents - multiple agents, merge one |
| 4 | `.tmp/suite4` | Time Travel - rollback and fork from history |
| 5 | `.tmp/suite5` | Edge Cases - binary files, symlinks, deletions |
| 6 | `.tmp/suite6` | Filtering - dual-mode git vs agt visibility |
| 7 | `.tmp/suite7` | Stress Test - many files, deep trees |
| 8 | `.tmp/suite8` | Failure Recovery - interrupted operations |
| 9 | `.tmp/suite9` | Documentation Audit - verify docs match behavior |
| 10 | `.tmp/suite10` | Sandbox Helper - agt-worktree add/remove |

## Additional Documentation

- **[ORCHESTRATION.md](ORCHESTRATION.md)** - How to assign suites to parallel agents
- **[ADVANCED_TESTING.md](ADVANCED_TESTING.md)** - Additional testing strategies beyond core suites

## Architecture Note

When invoked as `git` (via symlink), agt spawns the real git binary and filters its stdout
to hide shadow branches, tags, and commits. This provides full git compatibility while keeping
agent implementation details hidden from users.

The path to the real git binary is configured via `agt.gitPath` in `~/.agtconfig` or can be
overridden with the `AGT_GIT_PATH` environment variable.

Build binaries with: `make build`

## Running a Suite

Each suite has a `RUNBOOK.md` with:
- **Objective**: What we're testing
- **Setup**: Prerequisites and initialization
- **Scenarios**: Test cases to execute
- **Success Criteria**: How to know it passed
- **Failure Modes**: What could go wrong

## For Testing Agents

When assigned a suite:

1. Read the RUNBOOK.md for your suite
2. Read `docs/agt.1.txt` for reference (it's the specification)
3. Create your working directory: `.tmp/suiteN`
4. Execute the scenarios
5. Report: PASS/FAIL with evidence

## Creating a Test Repository

Most suites need a "source" repo to clone. Create one with:

```bash
mkdir -p .tmp/suiteN/source
cd .tmp/suiteN/source
git init --bare
```

Or use an existing test repo if the suite specifies one.

## Cleanup

To reset all test state:
```bash
rm -rf .tmp/suite*
```

## Adding New Suites

1. Create `tests/exploratory/suiteN-name/RUNBOOK.md`
2. Follow the template structure
3. Ensure it can run in isolation from other suites
