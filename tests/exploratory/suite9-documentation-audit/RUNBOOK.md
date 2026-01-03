# Suite 9: Documentation Audit

## Objective

Verify that the tool behaves EXACTLY as documented in `docs/agt.1.txt`. If there's a mismatch, determine whether the bug is in the tool or the documentation.

## Working Directory

`.tmp/suite9`

## Setup

1. Build all binaries: `make build` (builds both `agt` and vendored `gix`)
2. Create directory: `mkdir -p .tmp/suite9 && cd .tmp/suite9`
3. Have `docs/agt.1.txt` open for reference

## Philosophy

The documentation IS the specification. This suite systematically walks through every claim in the man page and verifies it.

## Audit Sections

### Section: NAME

> agt - Agent Git Tool for immutable filesystem snapshots and session management

Verify:
- [ ] Binary is named `agt`
- [ ] It manages "sessions"
- [ ] It creates "snapshots" (commits)

### Section: SYNOPSIS

> agt <command> [options]
> git [--disable-agt] <command> [options]

Verify:
- [ ] `agt <command>` syntax works
- [ ] Can be invoked as `git`
- [ ] `--disable-agt` flag exists and works

### Section: DUAL-MODE OPERATION

**git mode (filtered)**
> Branches matching the configured prefix are hidden from output

Verify:
- [ ] Agent branches hidden in git mode
- [ ] Verify with `git branch` output

> Tags matching the configured prefix are hidden from output

Verify:
- [ ] Create agent-prefixed tag
- [ ] Verify hidden in git mode

> Commits authored by the agent email are hidden from logs

Verify:
- [ ] Commits by agent email hidden
- [ ] Verify with `git log` output

**agt mode (unfiltered)**
> no filtering is applied and additional commands are available

Verify:
- [ ] All branches visible with `agt branch`
- [ ] All commits visible with `agt log`
- [ ] Extra commands available (init, fork, autocommit)

### Section: GIT COMMANDS

> agt uses the vendored `gix` CLI for all Git operations; system Git is not used.

Verify:
- [ ] `./git --version` (via symlink) shows gix version, not system Git
- [ ] AGT_GIX_PATH environment variable can override gix location
- [ ] Git commands work without system Git installed (in isolated environment)

### Section: CONFIGURATION

> agt.agentEmail - Email address used for auto-commit operations

Verify:
- [ ] Setting this affects autocommit author
- [ ] Setting this affects filtering

> agt.branchPrefix - Prefix for agent session branches

Verify:
- [ ] Fork creates branch with this prefix
- [ ] Filtering uses this prefix

> agt.userEmail - The user's normal email for reference

Verify:
- [ ] Document what this is actually used for
- [ ] If not used, note the discrepancy

### Section: COMMANDS - agt init

> Clones <remote-url> as a bare repository (<name>.git)

Verify:
- [ ] Creates bare repo with `.git` suffix

> Creates an adjacent main checkout directory (<name>)

Verify:
- [ ] Creates worktree without `.git` suffix

> Sets up default agt configuration

Verify:
- [ ] What defaults are set? Verify they match docs.

> Creates the agt state directory (.git/agt/)

Verify:
- [ ] `.git/agt/` exists
- [ ] `.git/agt/timestamps/` exists
- [ ] `.git/agt/sessions/` exists

> --path <directory> - Target directory (default: current dir)

Verify:
- [ ] Option works as documented

### Section: COMMANDS - agt fork

> Creates branch agtsessions/<id> from specified starting point

Verify:
- [ ] Branch name matches pattern
- [ ] Default starting point is HEAD

> Creates worktree at sessions/<id>/

Verify:
- [ ] Worktree location matches

> Initializes timestamp tracking for auto-commits

Verify:
- [ ] Timestamp file created

> --session-id <id> - Unique identifier for the session (required)

Verify:
- [ ] Errors without session-id

> --from <ref> - Starting point

Verify:
- [ ] Can fork from branch name
- [ ] Can fork from commit SHA
- [ ] Can fork from another session ID

### Section: COMMANDS - agt autocommit

> Reads last autocommit timestamp from .git/agt/timestamps/<id>

Verify:
- [ ] Timestamp file read correctly

> Scans <path> for files with mtime >= last timestamp

Verify:
- [ ] Only modified files captured
- [ ] Timestamp comparison works

> Creates a tree object from all modified files (ignores .gitignore)

Verify:
- [ ] .gitignore'd files ARE captured

> Creates a commit on agtsessions/<id> with:
> - Parent 1: last commit on agtsessions/<id>
> - Parent 2: current HEAD of the worktree's tracked branch

Verify:
- [ ] Commit has two parents
- [ ] Parent order is correct

> Updates the timestamp file

Verify:
- [ ] Timestamp updated after commit

> --timestamp <epoch> - Override scan timestamp (for testing)

Verify:
- [ ] Override works for testing

> --dry-run - Show what would be committed

Verify:
- [ ] Dry run shows files without committing

### Section: COMMANDS - agt list-sessions

> List all agent sessions with their status

Verify:
- [ ] Command exists
- [ ] Shows session ID
- [ ] Shows branch name
- [ ] Shows worktree path

### Section: COMMANDS - agt prune-session

> Remove an agent session's worktree

Verify:
- [ ] Worktree removed

> --delete-branch - Also delete the session branch

Verify:
- [ ] Flag works

### Section: COMMANDS - agt status

> Show agt-specific status including:
> - Active sessions
> - Pending autocommits
> - Configuration summary

Verify:
- [ ] Each item is shown

### Section: OPTIONS

> --disable-agt - disable all agt filtering

Verify:
- [ ] Works in git mode

> -C <path> - Run as if git was started in <path>

Verify:
- [ ] Works for agt commands
- [ ] Works for git passthrough

### Section: FILES

> .git/agt/timestamps/<session-id>

Verify:
- [ ] Location matches

> .git/agt/sessions/<session-id>.json

Verify:
- [ ] Does this file exist? If not, documentation error.

> sessions/<session-id>/ - Default location for agent session worktrees

Verify:
- [ ] Location matches

### Section: ENVIRONMENT

> AGT_DISABLE_FILTER - If set to "1", disables filtering

Verify:
- [ ] Environment variable works

> AGT_DEBUG - If set to "1", enables debug output

Verify:
- [ ] Does debug output exist? Verify.

### Section: EXAMPLES

Walk through each example in the EXAMPLES section and verify it works.

### Section: REPOSITORY LAYOUT

> project.git/           # Bare repository
> project/               # Main user worktree
> ├── .git               # File pointing to ../project.git
> └── sessions/          # Agent worktrees

Verify:
- [ ] Layout matches after `agt init`

### Section: EXIT STATUS

> 0 - Success
> 1 - General error
> 2 - Invalid command or options
> 128 - Git operation failed

Verify:
- [ ] Exit codes match documented behavior

## Audit Results Template

For each section, record:

| Claim | Status | Notes |
|-------|--------|-------|
| Description | PASS/FAIL/MISSING | Details |

## Discrepancy Handling

When documentation doesn't match behavior:

1. **Tool Bug**: Tool should match docs → file issue to fix tool
2. **Doc Bug**: Docs describe impossible/wrong behavior → file issue to fix docs
3. **Missing Feature**: Docs describe feature not implemented → file issue to implement
4. **Unclear Docs**: Behavior is correct but docs are ambiguous → file issue to clarify

## Success Criteria

- Every documented feature works as described
- All discrepancies identified and classified
- Recommendations made for fixes

## Failure Modes

- Feature completely missing
- Feature works differently than documented
- Documentation ambiguous/unclear
- Examples don't work

## Output Format

Create a report: `suite9-results.md` with:

1. Summary: X/Y checks passed
2. Full checklist with status
3. Discrepancies table
4. Recommendations

## Notes

This is the most important suite. If the documentation is wrong, users (including AI agents) will be confused. The documentation must be the source of truth.
