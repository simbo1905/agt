# AGT Monorepo

A polyglot monorepo for AI agent tooling, using [mise](https://mise.jdx.dev/) for tool version management.

## Overview

This repository contains tools for managing AI agent coding sessions with immutable filesystem snapshots and time-travel capabilities. The primary tool is `agt` (Agent Git Tool), a Git wrapper that enables:

- **Parallel agent workflows** - Multiple AI agents work concurrently in isolated worktrees
- **Immutable history** - Every file modification is captured in the Git object store
- **Sandboxing** - Designed to be compatible with sandboxing tools (e.g. `bubblewrap` (bwrap) to jail agent processes)
- **Time travel** - Roll back to any point in agent history, fork from any state
- **Transparent user experience** - When invoked as `git`, agent branches are hidden

## Repository Structure

```
agt/
├── README.md           # This file
├── AGENTS.md           # Guidance for AI coding agents
├── mise.toml           # Tool version management
├── docs/
│   └── agt.1.txt       # Man page for agt tool
├── crates/
│   ├── agt/            # Rust implementation of agt
│   │   ├── Cargo.toml
│   │   └── src/
│   └── agt-worktree/   # Worktree helper binary
└── CODING_PROMPT.md    # Implementation prompt for AI agents
```

## Tools

### agt - Agent Git Tool

A dual-mode Git wrapper that spawns the real git binary and filters its output:

- **As `git`**: Spawns real git, filters stdout to hide agent branches/commits
- **As `agt`**: Full visibility plus agent session management commands

Key commands:
- `agt init <remote>` - Clone to bare repo with adjacent worktree
- `agt fork --session-id <id>` - Create new agent session
- `agt autocommit -C <path> --session-id <id>` - Snapshot all modified files

See [docs/agt.1.txt](docs/agt.1.txt) for the complete man page.

## Configuration

AGT uses its own configuration files, separate from git's:

- `~/.agtconfig` - Global configuration
- `.agt/config` - Local repository configuration

Example `~/.agtconfig`:
```ini
[agt]
    gitPath = /opt/git/bin/git
    agentEmail = agt.opencode@local
    branchPrefix = agtsessions/
```

**Required settings:**
- `agt.gitPath` - Path to the real git binary (should NOT be on PATH)
- `agt.agentEmail` - Email for agent commits (filtered in git mode)
- `agt.branchPrefix` - Prefix for agent branches (default: `agtsessions/`)

## Quick Start

```bash
# Install mise if not present
curl https://mise.run | sh

# Clone and setup
git clone <this-repo>
cd agt
mise install

# Build
make build

# Configure agt (in ~/.agtconfig)
cat >> ~/.agtconfig << 'EOF'
[agt]
    gitPath = /usr/bin/git
    agentEmail = agt.opencode@local
    branchPrefix = agtsessions/
EOF

# Initialize a project with agt
dist/agt init https://github.com/user/project.git
cd project

# Create an agent session
agt fork --session-id agent-001

# After agent work, autocommit
agt autocommit -C sessions/agent-001 --session-id agent-001
```

## Development

This is a polyglot monorepo managed with mise. Currently includes:

- **Rust** - Core `agt` tool (crates/agt) and worktree helper (crates/agt-worktree)

### Prerequisites

- [mise](https://mise.jdx.dev/) for tool management
- Rust toolchain (managed via mise)

### Building

```bash
mise install          # Install all tools
make build            # Build all binaries to dist/
```

After building, binaries are in `dist/`:
- `dist/agt` - Main AGT tool
- `dist/agt-worktree` - Worktree helper

Run AGT from the `dist/` folder or add it to your PATH:

```bash
export PATH="/path/to/agt/dist:$PATH"
```

The `agt` and `agt-worktree` binaries should be in the same folder.

## Design Philosophy

1. **Single object store** - One bare repo per project, all agents share it
2. **Worktree isolation** - Each agent session gets its own worktree and index
3. **Dual-parent commits** - Agent commits link to both agent history and user branch
4. **Local-only agent branches** - Never pushed to remotes, only user branches sync
5. **Timestamp-based scanning** - Fast file discovery without index manipulation
6. **Real git passthrough** - Full git compatibility via spawning real git binary

## How It Works

When invoked as `git` (via symlink or rename):
1. AGT reads `agt.gitPath` from `~/.agtconfig` or `.agt/config`
2. Spawns the real git binary with all arguments
3. Filters stdout line-by-line to hide agent branches/commits
4. Passes stderr through unmodified

This gives full git compatibility while hiding agent implementation details from the user.

## Corner Cases

- Detached HEAD in agent worktrees is unsupported; autocommit expects a branch checkout.
- Symlink cycles are ignored during filesystem scans; symlinks are not followed.
- Symlinks are stored as symlinks; targets are captured as-is (external symlinks may be broken when checked out elsewhere).

## Known Limitations

- **Merging agent branches back** - Not yet implemented. Use manual `git merge` to integrate agent work into user branches.

## Environment Variables

- `AGT_GIT_PATH` - Override `agt.gitPath` configuration
- `AGT_WORKTREE_PATH` - Override location of `agt-worktree` binary
- `AGT_DISABLE_FILTER` - Set to "1" to disable filtering in git mode
- `AGT_DEBUG` - Set to "1" for debug output

## License

MIT
