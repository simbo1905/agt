# AGENTS.md - Guidance for AI Coding Agents

This document provides guidance for AI coding agents working on the AGT monorepo.

## Repository Overview

This is a polyglot monorepo containing tools for AI agent session management. Tool versions are managed with [mise](https://mise.jdx.dev/).

**Primary deliverable**: The `agt` binary - a Rust tool built on gitoxide.

## Documentation as Target State

- The documentation (especially `README.md` and `docs/agt.1.txt`) must always describe the target/final state, not the previous state.
- During reviews, do not rely on or keep any documentation other than the final state; the PR must be merged with the README that served as the specification for the work done.

## Project Structure

```
agt/
├── mise.toml           # Tool versions (rust, etc.)
├── crates/
│   └── agt/            # Main Rust crate
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── cli.rs        # Command-line parsing (clap)
│           ├── config.rs     # Git config reading ([agt] section)
│           ├── filter.rs     # Branch/commit filtering logic
│           ├── commands/
│           │   ├── mod.rs
│           │   ├── init.rs       # agt init
│           │   ├── fork.rs       # agt fork
│           │   ├── autocommit.rs # agt autocommit
│           │   └── passthrough.rs # git command passthrough
│           └── scanner.rs    # Timestamp-based file scanning
└── docs/
    └── agt.1.txt       # Man page (specification)
```

## Key Design Decisions

### Dual-Mode Binary

The binary checks `argv[0]` to determine mode:
- Invoked as `git` → filter mode (hide agent branches/commits)
- Invoked as `agt` → full mode (show everything + extra commands)

This is similar to how busybox works.

### Configuration

Read from git config files (global `~/.gitconfig` or local `.git/config`):

```ini
[agt]
    agentEmail = agt.opencode@local
    branchPrefix = agtsessions/
    userEmail = simon@example.com
```

Use gitoxide's config APIs to read these.

### Sandboxing Strategy (Optional)

Rather than total security, we would like to provide guardrails, like with tools like **bubblewrap (bwrap)** or other OS level tools where available (e.g. `sandbox-exec` is deprecated on Apple but they have `chroot` for isolation):

1.  **Agent Spawner**: A process manager running on the host.
    - Creates the session worktree (`agt fork`).
    - Configures the `bwrap`/`chroot` jail.
    - Bind-mounts `agt` as `/usr/bin/git` inside the jail.
2.  **Inside the Jail**:
    - The agent sees `argv[0] == "git"`.
    - `agt` applies filtering (hides `agtsessions/` etc.).
    - The agent works safely without seeing implementation details.
3.  **Outside the Jail**:
    - The spawner runs `agt autocommit` to checkpoint the session.
    - `agt` (full mode) captures all files, bypassing `.gitignore`.

We want to make the actual jail script extendable (perhaps by having `git` run `agt jail` by finding an `agt-jail` on the path to invoke it). 

### Filtering Logic (git mode)

When output would show branches, tags, or commits:
1. Exclude refs matching `agt.branchPrefix` (e.g., `agtsessions/*`)
2. Exclude commits where author email matches `agt.agentEmail`
3. The `--disable-agt` flag bypasses all filtering

### Commands

**`agt init <remote-url>`**
1. Clone remote as bare repo: `<name>.git`
2. Create adjacent worktree: `<name>/`
3. The worktree's `.git` is a file pointing to `../<name>.git`
4. Initialise `.git/agt/` directory structure
5. See: https://gist.github.com/simbo1905/22accc8dc39583672aa6f0483a800429

**`agt fork --session-id <id>`**
1. Create branch `agtsessions/<id>` from current HEAD (or `--from`)
2. Create worktree at `sessions/<id>/`
3. Initialise timestamp file at `.git/agt/timestamps/<id>`

**`agt autocommit -C <path> --session-id <id>`**
1. Read last timestamp from `.git/agt/timestamps/<id>`
2. Scan `<path>` for files with mtime >= timestamp (use jwalk or similar)
3. Build tree object from found files (ignore .gitignore)
4. Create a commit on `agtsessions/<id>` with:
   - Parent 1: tip of `agtsessions/<id>`
   - Parent 2: HEAD of worktree's tracked branch
5. Update timestamp file

### Testing Requirements

- Create temporary bare repos for testing
- Test filtering by creating commits with the agent email
- Test autocommit with controllable timestamps (`--timestamp` flag)
- Integration tests that exercise full workflows

### Exploratory Test Suites

The `tests/exploratory/` directory contains 9 core test suites designed for parallel execution by AI agents. Key requirements:
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
- `gix` (gitoxide) - Git operations
- `clap` - CLI parsing
- `jwalk` or `walkdir` - Fast filesystem traversal
- `chrono` - Timestamp handling
- `tempfile` - Test fixtures

## Build Commands

```bash
# Build
cargo build --release

# Test
cargo test

# Install locally
cargo install --path crates/agt
```

## Important Notes

1. **Never use the regular Git index for autocommit** - build trees directly from file scanning
2. **Timestamps must be overridable for testing** - use `--timestamp` flag
3. **Worktrees share the object store** - this is Git's built-in concurrency handling
4. **Agent branches are local-only** - configure refspecs to prevent pushing

## Reference Documentation

- Man page: [docs/agt.1.txt](docs/agt.1.txt)
- Bare repo layout: https://gist.github.com/simbo1905/22accc8dc39583672aa6f0483a800429
- gitoxide docs: https://docs.rs/gix/latest/gix/
