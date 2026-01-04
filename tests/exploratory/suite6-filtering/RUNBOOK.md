# Suite 6: Dual-Mode Filtering

## Objective

Verify the core dual-mode behavior:
- When invoked as `git`: agent branches/commits are HIDDEN
- When invoked as `agt`: everything is VISIBLE
- The `--disable-agt` flag bypasses filtering

## Working Directory

`.tmp/suite6`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite6 && cd .tmp/suite6`
3. Create a symlink for git-mode testing:
   ```bash
   ln -s $(pwd)/../../dist/agt ./git
   ```
4. Configure `AGT_GIT_PATH` to point to the real git binary:
   ```bash
   export AGT_GIT_PATH=/usr/bin/git
   ```
5. Initialize an agt-managed repo

## Reference

Read `docs/agt.1.txt`:
- DUAL-MODE OPERATION section
- CONFIGURATION section (agt.agentEmail, agt.branchPrefix)
- --disable-agt option
- AGT_DISABLE_FILTER environment variable

## Scenarios

### Scenario 6.0: Passthrough Uses Real Git

Verify that git-mode passthrough is using the real git binary.

Steps:
1. Set `AGT_GIT_PATH` to the system git binary path
2. Run `./git --version`
3. Confirm output shows the system git version (e.g., "git version 2.x.x")

Success: `./git --version` reports real git version

### Scenario 6.0b: Sandbox Isolation (Linux/macOS)

Verify that isolation works with the real git binary.

Steps (Linux + bwrap):
1. Use bwrap to create a minimal root with `agt` and system `git` only
2. Set `AGT_GIT_PATH` to point to git inside the jail
3. Run `./git --version` inside the jail
4. Confirm output shows git version

Steps (macOS + chroot):
1. Create a minimal chroot with `agt` and `git` binaries
2. Set `AGT_GIT_PATH` appropriately
3. Run `./git --version` inside the chroot
4. Confirm output shows git version

Success: Passthrough works correctly in isolated environment

### Scenario 6.1: Configure Filtering

Set up the configuration that controls filtering.

Steps:
1. Configure `agt.agentEmail` (e.g., "agt@local")
2. Configure `agt.branchPrefix` (e.g., "agtsessions/")
3. Verify config is readable: `git config --list | grep agt`

Success: Configuration values set correctly

### Scenario 6.2: Create Content to Filter

Create agent branches and commits to be filtered.

Steps:
1. Fork an agent session
2. Make changes in the session
3. Autocommit (creates commit with agent email)
4. Create another agent session
5. Make and autocommit changes

Success: Multiple agent branches and commits exist

### Scenario 6.3: Branch Filtering (git mode)

Test that agent branches are hidden.

Steps:
1. Use the git symlink: `./git branch`
2. Verify NO branches starting with `agtsessions/` appear
3. User branches (like `main`) still appear

Success: Agent branches hidden in git mode

### Scenario 6.4: Branch Visibility (agt mode)

Test that agt shows all branches.

Steps:
1. Use agt directly: `agt branch` or `agt branch -a`
2. Verify agent branches ARE visible
3. Verify user branches also visible

Success: All branches visible in agt mode

### Scenario 6.5: Log Filtering (git mode)

Test that agent commits are hidden from log.

Steps:
1. Have both user and agent commits in history
2. Use git symlink: `./git log`
3. Verify commits by `agt.agentEmail` do NOT appear
4. User commits still appear

Success: Agent commits hidden in git mode log

### Scenario 6.6: Log Visibility (agt mode)

Test that agt shows all commits.

Steps:
1. Use agt: `agt log`
2. Verify agent commits ARE visible
3. Note the author email matches `agt.agentEmail`

Success: All commits visible in agt mode

### Scenario 6.7: Tag Filtering

Test that agent tags are filtered.

Steps:
1. Create a tag with agent prefix on an agent commit
2. Create a user tag
3. `./git tag` should hide agent tag
4. `agt tag` should show all tags

Success: Tags filtered by prefix

### Scenario 6.8: --disable-agt Flag

Test the bypass flag.

Steps:
1. Use: `./git --disable-agt branch`
2. Verify agent branches ARE visible despite git invocation
3. Test with other commands: `./git --disable-agt log`

Success: --disable-agt shows everything

### Scenario 6.9: AGT_DISABLE_FILTER Environment Variable

Test environment variable bypass.

Steps:
1. Set: `export AGT_DISABLE_FILTER=1`
2. Use: `./git branch`
3. Verify agent branches visible
4. Unset variable, verify filtering resumes

Success: Environment variable works as documented

### Scenario 6.10: Remote Operations Filtering

Test filtering with remote-related commands.

Steps:
1. Set up a local "remote" bare repo
2. Push user branch
3. Do NOT push agent branches (they should never go to remote)
4. `./git remote show origin` should not reference agent branches
5. `./git fetch` should not try to fetch agent branches

Success: Remote operations unaffected by local agent branches

### Scenario 6.11: Custom Branch Prefix

Test non-default branch prefix.

Steps:
1. Change `agt.branchPrefix` to something custom (e.g., "ai-sessions/")
2. Create session with new prefix
3. Verify filtering uses new prefix
4. Old prefix sessions (if any) should now be visible in git mode

Success: Custom prefix works correctly

### Scenario 6.12: Custom Agent Email

Test non-default agent email.

Steps:
1. Change `agt.agentEmail` to something custom
2. Create commits with new email (via autocommit)
3. Verify log filtering uses new email

Success: Custom email works correctly

### Scenario 6.13: Interactive Commands

Test filtering with interactive commands if possible.

Steps:
1. Test `./git log --oneline`
2. Test `./git log --graph --all`
3. Test `./git branch -v`
4. Verify all filtered appropriately

Success: Various output formats all filtered

### Scenario 6.14: Worktree Visibility

Test if worktrees are affected by mode.

Steps:
1. `./git worktree list` - should it hide agent worktrees?
2. `agt worktree list` - should show all
3. Document observed behavior

Success: Consistent behavior documented

### Scenario 6.15: From Inside Agent Worktree

Test filtering when pwd is in agent worktree.

Steps:
1. `cd sessions/agent-001`
2. Run `./git log` from there
3. Note: current branch is agent branch
4. What should be filtered? Document behavior.

Success: Behavior from inside agent worktree is sensible

## Success Criteria

- Clear separation between git and agt modes
- Filtering works for branches, commits, tags
- Bypass mechanisms work (flag and env var)
- Custom configuration respected
- Edge cases have sensible behavior

## Failure Modes

- Filtering doesn't work (agent branches visible in git mode)
- Over-filtering (user branches hidden)
- --disable-agt doesn't work
- Environment variable ignored
- Crash on filtered output

## Critical Security Note

Filtering is NOT a security mechanism. It's for UX - keeping agent internals out of the way. An adversary could easily use `--disable-agt` or `agt` to see everything.

The security boundary, if needed, is the bwrap sandbox documented in the architecture section.
