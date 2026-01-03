use crate::config::AgtConfig;
use anyhow::Result;
use gix::Repository;
use std::fs;

pub fn run(repo: &Repository, config: &AgtConfig) -> Result<()> {
    println!("AGT Status:");
    println!("  Configuration:");
    println!("    Agent Email: {}", config.agent_email);
    println!("    Branch Prefix: {}", config.branch_prefix);
    if let Some(user_email) = &config.user_email {
        println!("    User Email: {user_email}");
    }

    // Count active sessions
    let sessions_dir = repo.common_dir().join("agt/sessions");
    let session_count = if sessions_dir.exists() {
        fs::read_dir(&sessions_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".json"))
            .count()
    } else {
        0
    };

    println!("  Active Sessions: {session_count}");

    // Check for pending autocommits
    let timestamps_dir = repo.common_dir().join("agt/timestamps");
    if timestamps_dir.exists() {
        let mut pending = 0;
        for entry in fs::read_dir(&timestamps_dir)? {
            let entry = entry?;
            let session_id = entry.file_name().to_string_lossy().to_string();
            let timestamp_file = entry.path();

            let last_timestamp: i64 = fs::read_to_string(&timestamp_file)?.trim().parse()?;

            // Check if there are files modified since last autocommit
            let worktree_path = repo
                .work_dir().map_or_else(std::path::PathBuf::new, |wd| wd.join("sessions").join(&session_id));

            if worktree_path.exists() {
                let modified_files =
                    crate::scanner::scan_modified_files(&worktree_path, last_timestamp)?;
                if !modified_files.is_empty() {
                    pending += 1;
                }
            }
        }

        println!("  Sessions with pending changes: {pending}");
    }

    Ok(())
}
