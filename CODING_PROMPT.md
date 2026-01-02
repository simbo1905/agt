# Coding Prompt: Implement AGT (Agent Git Tool)

## Objective

Create a Rust binary called `agt` that wraps gitoxide to provide dual-mode Git operation for AI agent session management with immutable filesystem snapshots.

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
    ├── config.rs
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
- Contains "git" → git mode (filtered)
- Contains "agt" → agt mode (full + extra commands)

```rust
fn main() -> anyhow::Result<()> {
    let invoked_as = std::env::args().next().unwrap_or_default();
    let is_git_mode = invoked_as.contains("git") && !invoked_as.contains("agt");
    // ...
}
```

### 2. Configuration (config.rs)

Read from git config files using gitoxide:

```rust
pub struct AgtConfig {
    pub agent_email: String,      // agt.agentEmail
    pub branch_prefix: String,    // agt.branchPrefix  
    pub user_email: Option<String>, // agt.userEmail
}

impl AgtConfig {
    pub fn load(repo: &gix::Repository) -> anyhow::Result<Self> {
        let config = repo.config_snapshot();
        Ok(Self {
            agent_email: config
                .string("agt.agentEmail")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "agt@local".to_string()),
            branch_prefix: config
                .string("agt.branchPrefix")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "agtsessions/".to_string()),
            user_email: config.string("agt.userEmail").map(|s| s.to_string()),
        })
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

Filter branches and commits in git mode:

```rust
pub fn should_hide_ref(ref_name: &str, config: &AgtConfig) -> bool {
    ref_name.contains(&config.branch_prefix)
}

pub fn should_hide_commit(commit: &gix::Commit, config: &AgtConfig) -> bool {
    commit.author()
        .map(|a| a.email.to_string() == config.agent_email)
        .unwrap_or(false)
}
```

For git passthrough commands that list branches/tags/logs, intercept output and filter.

### 5. Init Command (commands/init.rs)

```rust
pub fn run(remote_url: &str, target_path: Option<&Path>) -> anyhow::Result<()> {
    // 1. Determine paths
    let repo_name = extract_repo_name(remote_url)?;
    let base = target_path.unwrap_or(Path::new("."));
    let bare_path = base.join(format!("{}.git", repo_name));
    let work_path = base.join(&repo_name);

    // 2. Clone as bare
    gix::clone::PrepareFetch::new(
        remote_url,
        &bare_path,
        gix::create::Kind::Bare,
        gix::create::Options::default(),
        gix::open::Options::isolated(),
    )?
    .fetch_then_checkout(gix::progress::Discard, &std::sync::atomic::AtomicBool::new(false))?;

    // 3. Create worktree directory
    std::fs::create_dir_all(&work_path)?;

    // 4. Create .git file pointing to bare repo
    let git_file = work_path.join(".git");
    std::fs::write(&git_file, format!("gitdir: ../{}.git", repo_name))?;

    // 5. Checkout HEAD
    // Use gix to checkout files to work_path

    // 6. Create agt state directory
    let agt_dir = bare_path.join("agt");
    std::fs::create_dir_all(agt_dir.join("timestamps"))?;
    std::fs::create_dir_all(agt_dir.join("sessions"))?;

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

    // 3. Create worktree
    let worktree_path = repo.work_dir()
        .ok_or_else(|| anyhow::anyhow!("No work dir"))?
        .join("sessions")
        .join(session_id);
    
    // Use git worktree add equivalent
    std::process::Command::new("git")
        .args(["worktree", "add", worktree_path.to_str().unwrap(), &branch_name])
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

For unrecognized commands, pass through to git but filter output in git mode:

```rust
pub fn run(
    args: &[String],
    is_git_mode: bool,
    disable_filter: bool,
    config: &AgtConfig,
) -> anyhow::Result<()> {
    let output = std::process::Command::new("git")
        .args(args)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    if is_git_mode && !disable_filter {
        // Filter output based on command
        let filtered = filter_output(&stdout, args, config);
        print!("{}", filtered);
    } else {
        print!("{}", stdout);
    }

    std::io::stderr().write_all(&output.stderr)?;
    std::process::exit(output.status.code().unwrap_or(1));
}

fn filter_output(output: &str, args: &[String], config: &AgtConfig) -> String {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("");
    
    match cmd {
        "branch" => filter_branch_output(output, config),
        "tag" => filter_tag_output(output, config),
        "log" => filter_log_output(output, config),
        _ => output.to_string(),
    }
}

fn filter_branch_output(output: &str, config: &AgtConfig) -> String {
    output
        .lines()
        .filter(|line| !line.contains(&config.branch_prefix))
        .collect::<Vec<_>>()
        .join("\n")
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
3. Configuration reading from git config
4. Branch/tag/commit filtering in git mode
5. `--disable-agt` flag to bypass filtering
6. `agt init` command creating bare repo layout
7. `agt fork` command creating sessions
8. `agt autocommit` with timestamp override for testing
9. Comprehensive test suite
10. All tests passing

## Build Verification

```bash
cargo build --release
cargo test
./target/release/agt --help
ln -s ./target/release/agt ./target/release/git
./target/release/git --help  # Should work as filtered git
```
