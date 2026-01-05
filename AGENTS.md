# AGENTS.md - Guidance for AI Coding Agents

This document provides guidance for AI coding agents working on the AGT monorepo.

## Repository Overview

This is a polyglot monorepo containing tools for AI agent session management. Tool versions are managed with [mise](https://mise.jdx.dev/).

**Primary deliverable**: The `agt` binary - a Rust tool that wraps the host git binary.

## Documentation as Target State

- The documentation (especially `README.md` and `docs/agt.1.txt`) must always describe the target/final state, not the previous state.
- During reviews, do not rely on or keep any documentation other than the final state; the PR must be merged with the README that served as the specification for the work done.

## Project Structure

```
agt/
├── mise.toml           # Tool versions (rust, etc.)
├── vendor/
│   └── toybox/         # toybox submodule for chroot jails
├── crates/
│   ├── agt/            # Main Rust crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── cli.rs        # Command-line parsing (clap)
│   │       ├── config.rs     # AGT config reading (~/.agtconfig, .agt/config)
│   │       ├── filter.rs     # Branch/commit filtering logic
│   │       ├── commands/
│   │       │   ├── mod.rs
│   │       │   ├── clone.rs       # agt clone
│   │       │   ├── session.rs     # agt session {new,export,remove,fork}
│   │       │   ├── autocommit.rs  # agt autocommit
│   │       │   └── passthrough.rs # git command passthrough
│   │       └── scanner.rs    # Timestamp-based file scanning
│   └── agt-worktree/   # Sandbox helper binary
└── docs/
    └── agt.1.txt       # Man page (specification)
```

## Key Design Decisions

### Dual-Mode Binary

The binary checks `argv[0]` to determine mode:
- Invoked as `git` → filter mode (spawn host git, filter stdout)
- Invoked as `agt` → full mode (show everything + extra commands)

This is similar to how busybox works.

### Host Git Passthrough

When invoked as `git`, AGT:
1. Reads `agt.gitPath` from config to find the host git binary
2. Spawns the host git with all command-line arguments
3. Filters stdout line-by-line to hide agent branches/commits
4. Passes stderr through unmodified

This provides **full git compatibility** - every git command works while AGT filters agent state.

### Configuration

AGT uses its own configuration files (separate from git's):

**Global config**: `~/.agtconfig`
**Local config**: `.agt/config` (in repository root)

```ini
[agt]
    gitPath = /opt/git/bin/git
    agentEmail = agt.opencode@local
    branchPrefix = agtsessions/
    userEmail = simon@example.com
```

**Required settings:**
- `agt.gitPath` - Path to the host git binary (should NOT be on PATH)
- `agt.agentEmail` - Email for agent commits (filtered in git mode)
- `agt.branchPrefix` - Prefix for agent branches (default: `agtsessions/`)

### Why Separate Config Files?

- Host git is not on PATH to prevent users from accidentally bypassing AGT
- AGT needs to know where the host git is located
- Clean separation: git's config is for git, AGT's config is for AGT

### Terminology

See `DESIGN_20260104.md` for the full terminology table. Key terms:

| Term | World | Meaning |
|------|-------|---------|
| **Session** | Disk | An agent run with a unique ID and folder on disk |
| **Session folder** | Disk | `sessions/<id>/` - contains sandbox and tool state |
| **Sandbox** | Disk | `sessions/<id>/sandbox/` - where the agent runs (jailed) |
| **Shadow branch** | Git | `agtsessions/<id>` - where autocommits are stored |
| **Shadow tree** | Git | The tree in a shadow commit (mirrors session folder) |
| **Profile** | Config | Tool-specific folder requirements (opencode, cursor, etc.) |

### Sandboxing Strategy

We prioritize robust isolation for agents to enable "YOLO mode" where agents can run untrusted code safely.

**Key Components:**
1.  **Infrastructure**:
    -   **Linux VM/VPS**: The primary hosting target. We assume the ability to run a VM (e.g., via Lima) or provision a cheap VPS.
    -   **Chroot Jails**: We use a custom fork of [toybox](https://github.com/simbo1905/toybox) (branch `agt-agent-sandbox`, vendored in `vendor/toybox`) to construct lightweight chroot jails.
    -   **Isolation**: Agents are jailed into their sandbox folder, protecting the host from malicious/accidental damage.

2.  **Session Layout**:
    ```
    sessions/<id>/              # Session folder
    ├── sandbox/                # Agent runs here (jailed)
    │   ├── src/
    │   └── .git
    ├── xdg/                    # Tool state (bind-mounted to ~/.local/share)
    ├── config/                 # Tool config (bind-mounted to ~/.config)
    └── _/                      # AGT system folder
        └── index               # Captured git index
    ```

3.  **Implementation**:
    -   **Agent Spawner**: A process manager running on the host sets up the jail.
    -   **Jail Construction**: Uses `toybox` to create a minimal rootfs.
    -   **Bind Mounts**: Mounts the sandbox and `agt` binary (as `/usr/bin/git`) into the jail. Profile-specific folders (xdg, config) are bind-mounted to expected locations.
    -   **Execution**: The agent process runs inside the jail, perceiving a clean environment.

4.  **Profiles**: Different tools require different folders. A profile defines what folders exist in the session folder and where they're mounted in the jail.

5.  **Why this approach?**
    -   **Power Developer Focus**: We target users who are comfortable with VMs/VPS, avoiding the "black box" limitations of container-only solutions.
    -   **Flexibility**: Allows running any tool chain that can be installed in the jail/VM.
    -   **Scalability**: Large bare repos and aggressive agent activity are better handled in dedicated VMs than on a user's main desktop file system.

### Filtering Logic (git mode)

When spawning git and reading its stdout:
1. Exclude lines showing refs matching `agt.branchPrefix` (e.g., `agtsessions/*`)
2. Exclude commits where author email matches `agt.agentEmail`
3. The `--disable-agt` flag bypasses all filtering (spawns git directly without filtering)

### Commands

See [docs/agt.1.txt](docs/agt.1.txt) for the complete CLI reference.

Key commands:
- `agt clone <url>` - Clone remote repo into agt-managed structure
- `agt session new` - Create new session for a ticket
- `agt session export` - Push user branch to remote
- `agt session remove` - Remove a session
- `agt autocommit` - Create shadow commit

### Testing Requirements

- Create temporary bare repos for testing
- Test filtering by creating commits with the agent email
- Test autocommit with controllable timestamps (`--timestamp` flag)
- Integration tests that exercise full workflows

### Exploratory Test Suites

The `tests/exploratory/` directory contains test suites designed for parallel execution by AI agents. Key requirements:
1. **Isolation**: Each suite runs in `.tmp/suiteN` with a dedicated git config (see `ORCHESTRATION.md`).
2. **Determinism**: Use `--timestamp` for autocommit tests to override mtime.
3. **Documentation as Spec**: **Suite 9** verifies every claim in `docs/agt.1.txt`. Mismatches are bugs.
4. **Critical Path**: Suites 1, 2, 6, and 9 must pass before others run.

Agents should:
- Run `setup.sh` before starting a suite.
- Use `check.sh` to validate pass/fail criteria.
- Report mismatches between tool behavior and `docs/agt.1.txt` in **Suite 9**.

## Dependencies

Primary Rust crates:
- `gix` (gitoxide) - Git object/tree operations
- `clap` - CLI parsing
- `jwalk` or `walkdir` - Fast filesystem traversal
- `chrono` - Timestamp handling
- `tempfile` - Test fixtures

## Build Commands

```bash
# Build all binaries
make build

# Test
cargo test

# Binaries are in dist/
ls dist/
# agt  agt-worktree
```

## Important Notes

1. **Never use the regular Git index for autocommit** - build shadow trees directly from session folder scanning
2. **Timestamps must be overridable for testing** - use `--timestamp` flag
3. **Git worktrees share the object store** - this is Git's built-in concurrency handling (implementation detail)
4. **Shadow branches are local-only** - configure refspecs to prevent pushing
5. **Host git passthrough** - AGT spawns the configured host git for full compatibility
6. **Shadow tree = session folder** - the shadow tree mirrors the session folder structure exactly

## Reference Documentation

- Man page: [docs/agt.1.txt](docs/agt.1.txt)
- Bare repo layout: https://gist.github.com/simbo1905/22accc8dc39583672aa6f0483a800429
- gitoxide docs: https://docs.rs/gix/latest/gix/
