# Suite 9: Documentation Audit

## Objective

Verify that the tool behaves EXACTLY as documented in `docs/agt.1.txt`. If there's a mismatch, determine whether the bug is in the tool or the documentation.

## Working Directory

`.tmp/suite9`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite9 && cd .tmp/suite9`
3. Have `docs/agt.1.txt` open for reference

## Philosophy

The documentation IS the specification. This suite systematically walks through every claim in the man page and verifies it.

## Prerequisite: --help Matches Man Page

Before running individual checks, verify that `--help` output matches `docs/agt.1.txt`:

```bash
# Capture help output for all commands
agt --help > agt-help.txt
agt clone --help > agt-clone-help.txt
agt session --help > agt-session-help.txt
agt session new --help > agt-session-new-help.txt
agt session export --help > agt-session-export-help.txt
agt session remove --help > agt-session-remove-help.txt
agt session fork --help > agt-session-fork-help.txt
agt session list --help > agt-session-list-help.txt
agt autocommit --help > agt-autocommit-help.txt
agt status --help > agt-status-help.txt
```

For each command, verify:
- [ ] All options listed in --help match docs/agt.1.txt
- [ ] Option descriptions are consistent
- [ ] Required vs optional arguments match

**FAIL if**: Any --help output describes options not in docs/agt.1.txt, or vice versa.

## Audit Sections

### Section: NAME

> agt - Agent Git Tool for immutable filesystem snapshots and session management

Verify:
- [ ] Binary is named `agt`
- [ ] It manages "sessions"
- [ ] It creates "snapshots" (shadow commits)

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
- [ ] Shadow branches hidden in git mode
- [ ] Verify with `git branch` output

> Tags matching the configured prefix are hidden from output

Verify:
- [ ] Create shadow-prefixed tag
- [ ] Verify hidden in git mode

> Commits authored by the agent email are hidden from logs

Verify:
- [ ] Shadow commits by agent email hidden
- [ ] Verify with `git log` output

**agt mode (unfiltered)**
> no filtering is applied and additional commands are available

Verify:
- [ ] All branches visible with `agt branch`
- [ ] All commits visible with `agt log`
- [ ] Extra commands available (clone, session, autocommit)

### Section: GIT COMMANDS

> When invoked as 'git', agt spawns the real git binary and filters its stdout.

Verify:
- [ ] `./git --version` (via symlink) shows system git version
- [ ] AGT_GIT_PATH environment variable can override git location
- [ ] Git commands work via passthrough with filtering

### Section: CONFIGURATION

> agt.agentEmail - Email address used for auto-commit operations

Verify:
- [ ] Setting this affects autocommit author
- [ ] Setting this affects filtering

> agt.branchPrefix - Prefix for agent session branches

Verify:
- [ ] Session creates shadow branch with this prefix
- [ ] Filtering uses this prefix

> agt.userEmail - The user's normal email for reference

Verify:
- [ ] Document what this is actually used for
- [ ] If not used, note the discrepancy

### Section: COMMANDS - agt clone

> Clones <remote-url> as a bare repository into <name>/.bare/

Verify:
- [ ] Creates `.bare/` directory
- [ ] `.bare/` is a valid bare git repo

> Creates .git file pointing to .bare

Verify:
- [ ] `.git` is a file (not directory)
- [ ] Contents are `gitdir: .bare`

> Creates main worktree: <name>/main/

Verify:
- [ ] `main/` directory exists
- [ ] `main/.git` points to `../.bare/worktrees/main`

> Initializes AGT metadata in .bare/agt/

Verify:
- [ ] `.bare/agt/` exists
- [ ] `.bare/agt/timestamps/` exists
- [ ] `.bare/agt/sessions/` exists

> --path <directory> - Target directory (default: current dir)

Verify:
- [ ] Option works as documented

### Section: COMMANDS - agt session new

> Generates session ID (or uses provided --id)

Verify:
- [ ] Auto-generates ID if not provided
- [ ] Uses provided --id

> Creates shadow branch agtsessions/<id>

Verify:
- [ ] Branch name matches pattern

> Creates session folder at sessions/<id>/

Verify:
- [ ] Folder exists at correct location

> Creates sandbox (git worktree at sessions/<id>/sandbox/)

Verify:
- [ ] `sessions/<id>/sandbox/` exists
- [ ] Is a valid git worktree
- [ ] `.git` file points to correct location

> Creates sibling folders based on profile (xdg/, config/)

Verify:
- [ ] `sessions/<id>/xdg/` exists
- [ ] `sessions/<id>/config/` exists

> Initializes timestamp tracking for autocommits

Verify:
- [ ] Timestamp file created at `.bare/agt/timestamps/<id>`

> --from <ref> - Starting point

Verify:
- [ ] Can create session from branch name
- [ ] Can create session from commit SHA
- [ ] Default is HEAD

### Section: COMMANDS - agt session export

> Verifies no uncommitted changes in sandbox (fails if dirty)

Verify:
- [ ] Fails if sandbox has uncommitted changes

> Pushes branch to origin

Verify:
- [ ] User branch pushed to remote

> Shadow branches are NEVER pushed to origin

Verify:
- [ ] Shadow branch NOT on remote after export

### Section: COMMANDS - agt session remove

> Removes session folder (sessions/<id>/)

Verify:
- [ ] Folder removed

> Removes git worktree

Verify:
- [ ] Worktree entry removed from `.bare/worktrees/`

> Deletes shadow branch if --delete-branch

Verify:
- [ ] With flag: branch deleted
- [ ] Without flag: branch preserved

### Section: COMMANDS - agt session fork

> Fork an existing session to create a parallel session

Verify:
- [ ] Creates new session from existing session state
- [ ] New session has its own shadow branch

### Section: COMMANDS - agt session list

> List all agent sessions with their status

Verify:
- [ ] Command exists
- [ ] Shows session ID
- [ ] Shows shadow branch name
- [ ] Shows sandbox path

### Section: COMMANDS - agt autocommit

> Reads last autocommit timestamp from .bare/agt/timestamps/<id>

Verify:
- [ ] Timestamp file read correctly

> Scans the session folder for files with mtime >= last timestamp

Verify:
- [ ] Only modified files captured
- [ ] Scans entire session folder (not just sandbox)

> Builds shadow tree from session folder contents

Verify:
- [ ] Shadow tree includes sandbox/
- [ ] Shadow tree includes xdg/
- [ ] Shadow tree includes config/
- [ ] Shadow tree includes _/

> Creates shadow commit with two parents

Verify:
- [ ] Parent 1: previous shadow commit
- [ ] Parent 2: user branch HEAD

> Updates the timestamp file

Verify:
- [ ] Timestamp updated after commit

> --timestamp <epoch> - Override scan timestamp

Verify:
- [ ] Override works for testing

> --dry-run - Show what would be committed

Verify:
- [ ] Dry run shows files without committing

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

> -C <path> - Run as if started in <path>

Verify:
- [ ] Works for agt commands
- [ ] Works for git passthrough

### Section: FILES

> .bare/agt/timestamps/<session-id>

Verify:
- [ ] Location matches

> .bare/agt/sessions/<session-id>.json

Verify:
- [ ] File exists with session metadata

> sessions/<session-id>/ - Session folder

Verify:
- [ ] Layout matches: sandbox/, xdg/, config/, _/

### Section: ENVIRONMENT

> AGT_DISABLE_FILTER - If set to "1", disables filtering

Verify:
- [ ] Environment variable works

> AGT_DEBUG - If set to "1", enables debug output

Verify:
- [ ] Does debug output exist? Verify.

### Section: EXAMPLES

Walk through each example in the EXAMPLES section and verify it works.

- [ ] `agt clone https://github.com/user/project.git`
- [ ] `cd project/main`
- [ ] `agt session new --id agent-001`
- [ ] `cd sessions/agent-001/sandbox`
- [ ] `agt autocommit -C sessions/agent-001 --session-id agent-001`
- [ ] `agt session export --session-id agent-001`
- [ ] `agt session fork --from agent-001 --id agent-002`
- [ ] `agt session list`
- [ ] `agt session remove --id agent-001 --delete-branch`

### Section: ARCHITECTURE

> Repository Layout (after agt clone)

Verify layout matches:
```
project/
├── .bare/
├── .git
├── main/
│   └── .git
└── sessions/
    └── agent-001/
        ├── sandbox/
        ├── xdg/
        ├── config/
        └── _/
```

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
- Diagram audit passes (see below)

## Diagram Audit

Before running the documentation checks, generate and inspect diagrams:

1. Run `make docs` from the repository root
2. Verify `.tmp/DESIGN_*.pdf` files are generated without errors
3. Open `.tmp/*.svg` and verify:
   - All mermaid diagrams render (no error placeholders)
   - Sequence diagram participants match component names in `DESIGN_20260104.md`
   - Numbered steps in sequence diagrams have corresponding table entries
   - Flowchart components align with terminology in `docs/agt.1.txt`

Checklist:
- [ ] `make docs` completes successfully
- [ ] All `.tmp/diagram-*.svg` files are valid SVG (not error output)
- [ ] Component diagram shows: agt CLI, Host Git Binary, agent sandbox helper
- [ ] Sequence diagrams use consistent participant IDs (P1-P8)
- [ ] Step tables below each sequence diagram match the numbered arrows

**FAIL if**: Any diagram fails to render, or diagram content contradicts `docs/agt.1.txt`.

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
