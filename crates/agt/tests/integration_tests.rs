use assert_cmd::Command as AgtCommand;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn agt_cmd() -> AgtCommand {
    assert_cmd::cargo::cargo_bin_cmd!("agt")
}

#[test]
fn test_init_creates_bare_repo() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    // Create a local git repo to clone from
    let source = tmp.path().join("source");
    fs::create_dir_all(&source)?;

    // Initialize source repo
    Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&source)
        .status()?;

    let target = tmp.path().join("target");
    fs::create_dir_all(&target)?;

    // Test agt init
    agt_cmd()
        .args(["init", source.to_str().unwrap()])
        .current_dir(&target)
        .assert()
        .success();

    // Verify bare repo exists
    assert!(target.join("source.git").exists());
    assert!(target.join("source").exists());

    // Verify .git file points to bare repo
    let git_file = target.join("source/.git");
    assert!(git_file.exists());
    let git_content = fs::read_to_string(git_file)?;
    assert!(git_content.contains("gitdir: ../source.git"));

    // Verify agt directory exists
    assert!(target.join("source.git/agt").exists());
    assert!(target.join("source.git/agt/timestamps").exists());
    assert!(target.join("source.git/agt/sessions").exists());

    Ok(())
}

#[test]
fn test_fork_creates_branch_and_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_basic_repo()?;

    // Test agt fork
    agt_cmd()
        .args(["fork", "--session-id", "test-session"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify branch exists
    let repo = gix::open(tmp.path())?;
    let branch_name = "agtsessions/test-session";
    assert!(repo
        .find_reference(&format!("refs/heads/{branch_name}"))
        .is_ok());

    // Verify worktree exists
    assert!(tmp.path().join("sessions/test-session").exists());

    // Verify timestamp file exists
    let timestamp_file = tmp.path().join(".git/agt/timestamps/test-session");
    assert!(timestamp_file.exists());

    Ok(())
}

#[test]
fn test_autocommit_with_timestamp_override() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_repo_with_session()?;

    // Create a test file
    let session_path = tmp.path().join("sessions/test-session");
    fs::write(session_path.join("test.txt"), "test content")?;

    // Get current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    // Test agt autocommit with timestamp override
    agt_cmd()
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            &(now - 3600).to_string(), // Force inclusion
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    Ok(())
}

#[test]
fn test_git_mode_filters_branches() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_repo_with_agent_branch()?;

    // Test git branch command (should filter agent branches)
    let output = agt_cmd()
        .args(["branch"])
        .current_dir(tmp.path())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(!stdout.contains("agtsessions/"));

    Ok(())
}

#[test]
fn test_agt_mode_shows_all_branches() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_repo_with_agent_branch()?;

    // Test agt branch command (should show all branches)
    let output = agt_cmd()
        .args(["branch", "-a"])
        .current_dir(tmp.path())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("agtsessions/test-agent"));

    Ok(())
}

// Helper functions
fn setup_basic_repo() -> Result<TempDir, Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(tmp.path())
        .status()?;

    // Configure git
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .status()?;

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(tmp.path())
        .status()?;

    // Create initial commit
    fs::write(tmp.path().join("README.md"), "# Test Repo")?;
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(tmp.path())
        .status()?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(tmp.path())
        .status()?;

    Ok(tmp)
}

fn setup_repo_with_session() -> Result<TempDir, Box<dyn std::error::Error>> {
    let tmp = setup_basic_repo()?;

    // Create agt config
    Command::new("git")
        .args(["config", "agt.agentEmail", "agt@local"])
        .current_dir(tmp.path())
        .status()?;

    Command::new("git")
        .args(["config", "agt.branchPrefix", "agtsessions/"])
        .current_dir(tmp.path())
        .status()?;

    // Create session
    agt_cmd()
        .args(["fork", "--session-id", "test-session"])
        .current_dir(tmp.path())
        .assert()
        .success();

    Ok(tmp)
}

fn setup_repo_with_agent_branch() -> Result<TempDir, Box<dyn std::error::Error>> {
    let tmp = setup_basic_repo()?;

    // Create agt config
    Command::new("git")
        .args(["config", "agt.agentEmail", "agt@local"])
        .current_dir(tmp.path())
        .status()?;

    Command::new("git")
        .args(["config", "agt.branchPrefix", "agtsessions/"])
        .current_dir(tmp.path())
        .status()?;

    // Create agent branch
    Command::new("git")
        .args(["checkout", "-b", "agtsessions/test-agent"])
        .current_dir(tmp.path())
        .status()?;

    // Create a commit on agent branch
    fs::write(tmp.path().join("agent-file.txt"), "agent content")?;
    Command::new("git")
        .args(["add", "agent-file.txt"])
        .current_dir(tmp.path())
        .status()?;

    Command::new("git")
        .args(["commit", "-m", "Agent commit"])
        .env("GIT_AUTHOR_EMAIL", "agt@local")
        .env("GIT_COMMITTER_EMAIL", "agt@local")
        .current_dir(tmp.path())
        .status()?;

    // Go back to main branch
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(tmp.path())
        .status()?;

    Ok(tmp)
}
