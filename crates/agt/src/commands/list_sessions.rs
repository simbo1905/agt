use crate::config::AgtConfig;
use anyhow::Result;
use gix::Repository;
use std::fs;
use std::path::Path;

pub fn run(repo: &Repository, config: &AgtConfig) -> Result<()> {
    let sessions_dir = repo.git_dir().join("agt/sessions");

    if !sessions_dir.exists() {
        println!("No agent sessions found");
        return Ok(());
    }

    let mut sessions = Vec::new();

    for entry in fs::read_dir(&sessions_dir)? {
        let entry = entry?;
        let session_id = entry.file_name().to_string_lossy().to_string();

        let branch_name = format!("{}{}", config.branch_prefix, session_id);
        let worktree_path = repo
            .work_dir().map_or_else(|| Path::new("<unknown>").to_path_buf(), |wd| wd.join("sessions").join(&session_id));

        sessions.push((session_id, branch_name, worktree_path));
    }

    if sessions.is_empty() {
        println!("No agent sessions found");
        return Ok(());
    }

    println!("Agent Sessions:");
    for (session_id, branch_name, worktree_path) in sessions {
        println!("  {session_id}:");
        println!("    Branch: {branch_name}");
        println!("    Worktree: {}", worktree_path.display());
    }

    Ok(())
}
