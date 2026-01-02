use crate::config::AgtConfig;
use anyhow::{Context, Result};
use gix::Repository;
use std::fs;
use std::process::Command as StdCommand;

pub fn run(
    repo: &Repository,
    session_id: &str,
    delete_branch: bool,
    config: &AgtConfig,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    // 1. Remove worktree
    let worktree_path = repo
        .work_dir()
        .context("No working directory found")?
        .join("sessions")
        .join(session_id);

    if worktree_path.exists() {
        fs::remove_dir_all(&worktree_path)
            .with_context(|| format!("Failed to remove worktree: {}", worktree_path.display()))?;
        println!("Removed worktree: {}", worktree_path.display());
    }

    // 2. Optionally delete branch
    if delete_branch {
        let branch_ref = format!("refs/heads/{branch_name}");
        if repo.find_reference(&branch_ref).is_ok() {
            StdCommand::new("git")
                .args(["branch", "-D", &branch_name])
                .current_dir(repo.work_dir().unwrap())
                .status()?;
            println!("Deleted branch: {branch_name}");
        }
    }

    // 3. Remove timestamp file
    let timestamp_file = repo.git_dir().join("agt/timestamps").join(session_id);
    if timestamp_file.exists() {
        fs::remove_file(&timestamp_file)?;
        println!("Removed timestamp file");
    }

    // 4. Remove session metadata
    let session_file = repo
        .git_dir()
        .join("agt/sessions")
        .join(format!("{session_id}.json"));
    if session_file.exists() {
        fs::remove_file(&session_file)?;
        println!("Removed session metadata");
    }

    println!("Pruned session: {session_id}");

    Ok(())
}
