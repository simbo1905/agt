use crate::config::AgtConfig;
use crate::gix_cli::{find_worktree_binary, repo_base_path};
use anyhow::{Context, Result};
use gix::Repository;
use gix_ref::transaction::PreviousValue;
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
        Some(ref_name) => match repo.rev_parse_single(ref_name) {
            Ok(obj) => obj.object()?.peel_to_commit()?,
            Err(_) => {
                let session_ref = format!("{}{}", config.branch_prefix, ref_name);
                repo.rev_parse_single(session_ref.as_str())?
                    .object()?
                    .peel_to_commit()?
            }
        },
        None => repo.head()?.peel_to_commit_in_place()?,
    };

    // 2. Create branch using gix
    repo.reference(
        format!("refs/heads/{branch_name}"),
        start_commit.id,
        PreviousValue::MustNotExist,
        "agt fork",
    )?;

    // 3. Create worktree
    let worktree_path = repo
        .work_dir()
        .context("No working directory found")?
        .join("sessions")
        .join(session_id);

    std::fs::create_dir_all(
        worktree_path
            .parent()
            .context("Failed to resolve sessions directory")?,
    )?;

    let status = StdCommand::new(find_worktree_binary(&repo_base_path(repo))?)
        .args([
            "add",
            "--git-dir",
            repo.common_dir().to_str().unwrap(),
            "--worktree",
            worktree_path.to_str().unwrap(),
            "--name",
            session_id,
            "--branch",
            &format!("refs/heads/{branch_name}"),
        ])
        .status()
        .context("Failed to create worktree")?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to create worktree for {session_id}"));
    }

    // 4. Initialize timestamp
    let agt_dir = repo.common_dir().join("agt");
    let timestamp_dir = agt_dir.join("timestamps");
    std::fs::create_dir_all(&timestamp_dir)?;
    let timestamp_file = timestamp_dir.join(session_id);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    let sessions_dir = agt_dir.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;
    let session_file = sessions_dir.join(format!("{session_id}.json"));
    let from_value = from.unwrap_or("HEAD");
    let session_json = format!(
        "{{\"session_id\":\"{session_id}\",\"branch\":\"{branch_name}\",\"worktree\":\"{}\",\"from\":\"{from_value}\",\"created_at\":{now}}}",
        worktree_path.display()
    );
    std::fs::write(&session_file, session_json)?;

    println!("Created agent session: {session_id}");
    println!("  Branch: {branch_name}");
    println!("  Worktree: {}", worktree_path.display());

    Ok(())
}
