# AGT Monorepo

A polyglot monorepo for AI agent tooling, using [mise](https://mise.jdx.dev/) for tool version management.

## Overview

This repository contains tools for managing AI agent coding sessions with immutable filesystem snapshots and time-travel capabilities. The primary tool is `agt` (Agent Git Tool), a Git wrapper that enables:

- **Parallel agent workflows** - Multiple AI agents work concurrently in isolated worktrees
- **Immutable history** - Every file modification is captured in the Git object store
- **Sandboxing** - Designed to be compatible with sandboxing tools (e.g.  `bubblewrap` (bwrap) to jail agent processes)
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
│   └── agt/            # Rust implementation of agt
│       ├── Cargo.toml
│       └── src/
└── CODING_PROMPT.md    # Implementation prompt for AI agents
```

## Tools

### agt - Agent Git Tool

A dual-mode Git wrapper built on [gitoxide](https://github.com/Byron/gitoxide):

- **As `git`**: Drop-in replacement with filtered output (hides agent branches/commits)
- **As `agt`**: Full visibility plus agent session management commands

Key commands:
- `agt init <remote>` - Clone to bare repo with adjacent worktree
- `agt fork --session-id <id>` - Create new agent session
- `agt autocommit -C <path> --session-id <id>` - Snapshot all modified files

See [docs/agt.1.txt](docs/agt.1.txt) for the complete man page.
This repo vendors gitoxide and uses its `gix` CLI for git passthrough. Worktree add/remove is handled by the `agt-worktree` helper; system Git is not used.

## Quick Start

```bash
# Install mise if not present
curl https://mise.run | sh

# Clone and setup
git clone <this-repo>
cd agt
mise install

# Build agt
cd crates/agt
cargo build --release

# Initialize a project with agt
agt init https://github.com/user/project.git
cd project

# Configure
git config agt.agentEmail "agt.opencode@local"
git config agt.branchPrefix "agtsessions/"

# Create an agent session
agt fork --session-id agent-001

# After agent work, autocommit
agt autocommit -C sessions/agent-001 --session-id agent-001
```

## Development

This is a polyglot monorepo managed with mise. Currently includes:

- **Rust** - Core `agt` tool (crates/agt)

### Prerequisites

- [mise](https://mise.jdx.dev/) for tool management
- Rust toolchain (managed via mise)

### Building

```bash
mise install          # Install all tools
make build-gix        # Build vendored gix CLI
make build-worktree   # Build agt-worktree helper
cargo build           # Build all Rust crates
cargo test            # Run tests
```

## Design Philosophy

1. **Single object store** - One bare repo per project, all agents share it
2. **Worktree isolation** - Each agent session gets its own worktree and index
3. **Dual-parent commits** - Agent commits link to both agent history and user branch
4. **Local-only agent branches** - Never pushed to remotes, only user branches sync
5. **Timestamp-based scanning** - Fast file discovery without index manipulation

## Corner Cases

- Detached HEAD in agent worktrees is unsupported; autocommit expects a branch checkout.
- Symlink cycles are ignored during filesystem scans; symlinks are not followed.

## License

MIT
