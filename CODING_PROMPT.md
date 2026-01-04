# Coding Prompt: Implement AGT (Agent Git Tool)

## Objective

Create a Rust binary called `agt` that wraps the real git binary to provide dual-mode Git operation for AI agent session management with immutable filesystem snapshots.

## Documentation as Target State

- The documentation (especially `README.md` and `docs/agt.1.txt`) must always describe the target/final state, not the previous state.
- During reviews, do not rely on or keep any documentation other than the final state; the PR must be merged with the README that served as the specification for the work done.

## Repository Setup

Create this structure:

```
crates/agt/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── cli.rs
    ├── config.rs      # Reads ~/.agtconfig and .agt/config
    ├── filter.rs
    ├── scanner.rs
    └── commands/
        ├── mod.rs
        ├── init.rs
        ├── fork.rs
        ├── autocommit.rs
        └── passthrough.rs
```

Also create at project root:
- `mise.toml` with rust = "1.83"
- `Cargo.toml` workspace file

## Dependencies (Cargo.toml)

```toml
[package]
name = "agt"
version = "0.1.0"
edition = "2021"

[dependencies]
gix = { version = "0.68", features = ["blocking-network-client"] }
clap = { version = "4", features = ["derive"] }
jwalk = "0.8"
chrono = "0.4"
anyhow = "1"
thiserror = "1"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

## Specification

### 1. Dual-Mode Detection (main.rs)

Check `std::env::args().next()` (argv[0]) to determine invocation name:
- Contains "git" → git mode (spawn real git, filter output)
- Contains "agt" → agt mode (full + extra commands)

```rust
fn main() -> anyhow::Result<()> {
    let invoked_as = std::env::args().next().unwrap_or_default();
    let is_git_mode = invoked_as.contains("git") && !invoked_as.contains("agt");
    // ...
}
```

### 2. Configuration (config.rs)

AGT uses its own config files, separate from git's:

- `~/.agtconfig` - Global configuration
- `.agt/config` - Local repository configuration

```rust
pub struct AgtConfig {
    pub git_path: PathBuf,         // agt.gitPath - path to real git binary
    pub agent_email: String,       // agt.agentEmail
    pub branch_prefix: String,     // agt.branchPrefix  
    pub user_email: Option<String>, // agt.userEmail
}

impl AgtConfig {
    pub fn load() -> anyhow::Result<Self> {
        // Read ~/.agtconfig first
        let global_config = dirs::home_dir()
            .map(|h| h.join(".agtconfig"))
            .filter(|p| p.exists());
        
        // Then read .agt/config if in a repo
        let local_config = find_repo_root()
            .map(|r| r.join(".agt/config"))
            .filter(|p| p.exists());
        
        // Parse ini-style config, local overrides global
        // ...
        
        // Check AGT_GIT_PATH env var override
        if let Ok(path) = std::env::var("AGT_GIT_PATH") {
            config.git_path = PathBuf::from(path);
        }
        
        Ok(config)
    }
}
```

### 3. CLI Structure (cli.rs)

Use clap derive:

```rust
#[derive(Parser)]
#[command(name = "agt")]
pub struct Cli {
    /// Disable agt filtering (git mode only)
    #[arg(long, global = true)]
    pub disable_agt: bool,

    /// Run in directory
    #[arg(short = 'C', global = true)]
    pub directory: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Passthrough args for git commands
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    Init {
        remote_url: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    Fork {
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        from: Option<String>,
    },
    Autocommit {
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        timestamp: Option<i64>,
        #[arg(long)]
        dry_run: bool,
    },
    ListSessions,
    PruneSession {
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        delete_branch: bool,
    },
    Status,
}
```

### 4. Filtering Logic (filter.rs)

Filter git stdout in git mode:

```rust
pub fn should_hide_ref(ref_name: &str, config: &AgtConfig) -> bool {
    ref_name.contains(&config.branch_prefix)
}

pub fn should_hide_commit_line(line: &str, config: &AgtConfig) -> bool {
    // Check if line contains author email
    line.contains(&config.agent_email)
}
```

### 5. Init Command (commands/init.rs)

```rust
pub fn run(remote_url: &str, target_path: Option<&Path>, config: &AgtConfig) -> anyhow::Result<()> {
    // 1. Determine paths
    let repo_name = extract_repo_name(remote_url)?;
    let base = target_path.unwrap_or(Path::new("."));
    let bare_path = base.join(format!("{}.git", repo_name));
    let work_path = base.join(&repo_name);

    // 2. Clone as bare using real git
    std::process::Command::new(&config.git_path)
        .args(["clone", "--bare", remote_url, bare_path.to_str().unwrap()])
        .status()?;

    // 3. Create worktree directory
    std::fs::create_dir_all(&work_path)?;

    // 4. Create .git file pointing to bare repo
    let git_file = work_path.join(".git");
    std::fs::write(&git_file, format!("gitdir: ../{}.git", repo_name))?;

    // 5. Create .agt/config with default settings
    let agt_dir = work_path.join(".agt");
    std::fs::create_dir_all(&agt_dir)?;
    std::fs::write(agt_dir.join("config"), "[agt]\n    branchPrefix = agtsessions/\n")?;

    // 6. Checkout HEAD using real git
    std::process::Command::new(&config.git_path)
        .args(["checkout", "HEAD"])
        .current_dir(&work_path)
        .status()?;

    // 7. Create agt state directory
    let agt_state_dir = bare_path.join("agt");
    std::fs::create_dir_all(agt_state_dir.join("timestamps"))?;
    std::fs::create_dir_all(agt_state_dir.join("sessions"))?;

    Ok(())
}
```

### 6. Fork Command (commands/fork.rs)

```rust
pub fn run(
    repo: &gix::Repository,
    session_id: &str,
    from: Option<&str>,
    config: &AgtConfig,
) -> anyhow::Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);
    
    // 1. Resolve starting point
    let start_commit = match from {
        Some(ref_name) => repo.rev_parse_single(ref_name)?.object()?.peel_to_commit()?,
        None => repo.head()?.peel_to_commit()?,
    };

    // 2. Create branch
    repo.reference(
        format!("refs/heads/{}", branch_name),
        start_commit.id,
        gix::refs::transaction::PreviousValue::MustNotExist,
        format!("agt fork: create session {}", session_id),
    )?;

    // 3. Create worktree using agt-worktree helper
    let worktree_path = repo.work_dir()
        .ok_or_else(|| anyhow::anyhow!("No work dir"))?
        .join("sessions")
        .join(session_id);
    
    let worktree_bin = find_worktree_binary()?;
    std::process::Command::new(worktree_bin)
        .args(["add", worktree_path.to_str().unwrap(), &branch_name])
        .current_dir(repo.work_dir().unwrap())
        .status()?;

    // 4. Initialize timestamp
    let timestamp_file = repo.git_dir().join("agt/timestamps").join(session_id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    Ok(())
}
```

### 7. Autocommit Command (commands/autocommit.rs)

```rust
pub fn run(
    repo: &gix::Repository,
    worktree_path: &Path,
    session_id: &str,
    override_timestamp: Option<i64>,
    dry_run: bool,
    config: &AgtConfig,
) -> anyhow::Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);
    
    // 1. Read last timestamp
    let timestamp_file = repo.git_dir().join("agt/timestamps").join(session_id);
    let last_timestamp: i64 = std::fs::read_to_string(&timestamp_file)?
        .trim()
        .parse()?;
    
    let scan_timestamp = override_timestamp.unwrap_or(last_timestamp);

    // 2. Scan for modified files
    let modified_files = scan_modified_files(worktree_path, scan_timestamp)?;
    
    if modified_files.is_empty() {
        println!("No modified files since last autocommit");
        return Ok(());
    }

    if dry_run {
        println!("Would commit {} files:", modified_files.len());
        for f in &modified_files {
            println!("  {}", f.display());
        }
        return Ok(());
    }

    // 3. Build tree from files (not using index)
    let tree_id = build_tree_from_files(repo, worktree_path, &modified_files)?;

    // 4. Get parents
    let agent_branch_ref = format!("refs/heads/{}", branch_name);
    let parent1 = repo.find_reference(&agent_branch_ref)?
        .peel_to_commit()?;
    
    // Get worktree's tracked branch HEAD as parent2
    let worktree_head = gix::open(worktree_path)?.head()?.peel_to_commit()?;
    
    // 5. Create commit
    let signature = gix::actor::SignatureRef {
        name: "agt".into(),
        email: config.agent_email.as_str().into(),
        time: gix::date::Time::now_local_or_utc(),
    };

    let commit_id = repo.commit(
        &agent_branch_ref,
        &signature,
        &signature,
        "agt autocommit",
        tree_id,
        [parent1.id, worktree_head.id],
    )?;

    // 6. Update timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    println!("Created commit {}", commit_id);
    Ok(())
}

fn scan_modified_files(root: &Path, since_timestamp: i64) -> anyhow::Result<Vec<PathBuf>> {
    let threshold = std::time::UNIX_EPOCH + std::time::Duration::from_secs(since_timestamp as u64);
    let mut files = Vec::new();
    
    for entry in jwalk::WalkDir::new(root)
        .skip_hidden(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let metadata = entry.metadata()?;
            let mtime = metadata.modified()?;
            if mtime >= threshold {
                files.push(entry.path().strip_prefix(root)?.to_path_buf());
            }
        }
    }
    
    Ok(files)
}

fn build_tree_from_files(
    repo: &gix::Repository,
    worktree: &Path,
    files: &[PathBuf],
) -> anyhow::Result<gix::ObjectId> {
    // Use gix to create blob objects for each file
    // Then build tree structure
    // This bypasses the index entirely
    todo!("Implement tree building from file list")
}
```

### 8. Git Passthrough (commands/passthrough.rs)

Spawn real git and filter stdout:

```rust
pub fn run(
    args: &[String],
    is_git_mode: bool,
    disable_filter: bool,
    config: &AgtConfig,
) -> anyhow::Result<()> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let mut child = Command::new(&config.git_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())  // Pass stderr through unmodified
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    
    for line in reader.lines() {
        let line = line?;
        
        if is_git_mode && !disable_filter {
            if should_filter_line(&line, cmd, config) {
                continue;  // Skip this line
            }
        }
        
        println!("{}", line);
    }

    let status = child.wait()?;
    std::process::exit(status.code().unwrap_or(1));
}

fn should_filter_line(line: &str, cmd: &str, config: &AgtConfig) -> bool {
    match cmd {
        "branch" => line.contains(&config.branch_prefix),
        "tag" => line.contains(&config.branch_prefix),
        "log" => {
            // Filter commits by agent email
            // This is approximate - works for default log format
            line.contains(&config.agent_email)
        },
        _ => false,
    }
}
```

### 9. Testing Requirements

Create comprehensive tests in `crates/agt/tests/`:

**integration_tests.rs:**
```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_init_creates_bare_repo() {
    let tmp = TempDir::new().unwrap();
    // Create a local git repo to clone from
    let source = tmp.path().join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&source)
        .status()
        .unwrap();
    
    let target = tmp.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    
    Command::cargo_bin("agt")
        .unwrap()
        .args(["init", source.to_str().unwrap()])
        .current_dir(&target)
        .assert()
        .success();
    
    // Verify bare repo exists
    assert!(target.join("source.git").exists());
    assert!(target.join("source").exists());
}

#[test]
fn test_git_mode_filters_branches() {
    // Setup repo with agent branch
    let tmp = setup_repo_with_agent_branch();
    
    // Run as "git" (symlink or rename binary)
    // Verify agent branch is hidden
}

#[test]
fn test_agt_mode_shows_all_branches() {
    // Setup repo with agent branch
    let tmp = setup_repo_with_agent_branch();
    
    // Run as "agt"
    // Verify agent branch is visible
}

#[test]
fn test_autocommit_with_timestamp_override() {
    // Setup repo with session
    let tmp = setup_repo_with_session();
    
    // Modify a file
    // Run autocommit with --timestamp to force inclusion
    // Verify commit was created with two parents
}

#[test]
fn test_fork_creates_branch_and_worktree() {
    let tmp = setup_basic_repo();
    
    Command::cargo_bin("agt")
        .unwrap()
        .args(["fork", "--session-id", "test-session"])
        .current_dir(tmp.path())
        .assert()
        .success();
    
    // Verify branch exists
    // Verify worktree exists
    // Verify timestamp file exists
}
```

## Deliverables

1. Working `agt` binary with all commands
2. Dual-mode operation (git vs agt invocation)
3. Configuration reading from `~/.agtconfig` and `.agt/config`
4. Branch/tag/commit filtering in git mode (via stdout filtering)
5. `--disable-agt` flag to bypass filtering
6. `agt init` command creating bare repo layout
7. `agt fork` command creating sessions
8. `agt autocommit` with timestamp override for testing
9. Comprehensive test suite
10. All tests passing

## Build Verification

```bash
make build
cargo test
./dist/agt --help
ln -s ./dist/agt ./dist/git
./dist/git --help  # Should work as filtered git
```
