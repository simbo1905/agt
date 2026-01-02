use crate::config::AgtConfig;
use anyhow::{Context, Result};
use gix::Repository;
use std::process::Command as StdCommand;

pub fn run(
    repo: &Repository,
    session_id: &str,
    from: Option<&str>,
    config: &AgtConfig,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    // 1. Resolve starting point
    let start_commit = match from {
        Some(ref_name) => repo
            .rev_parse_single(ref_name)?
            .object()?
            .peel_to_commit()?,
        None => repo.head()?.peel_to_commit_in_place()?,
    };

    // 2. Create branch using git command
    StdCommand::new("git")
        .args(["branch", &branch_name, &start_commit.id.to_string()])
        .current_dir(repo.work_dir().unwrap())
        .status()?;

    // 3. Create worktree
    let worktree_path = repo
        .work_dir()
        .context("No working directory found")?
        .join("sessions")
        .join(session_id);

    // Use git worktree add equivalent
    StdCommand::new("git")
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            &branch_name,
        ])
        .current_dir(repo.work_dir().unwrap())
        .status()
        .context("Failed to create worktree")?;

    // 4. Initialize timestamp
    let timestamp_dir = repo.git_dir().join("agt/timestamps");
    std::fs::create_dir_all(&timestamp_dir)?;
    let timestamp_file = timestamp_dir.join(session_id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    println!("Created agent session: {session_id}");
    println!("  Branch: {branch_name}");
    println!("  Worktree: {}", worktree_path.display());

    Ok(())
}
