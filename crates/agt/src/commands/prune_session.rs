use crate::config::AgtConfig;
use crate::gix_cli::{find_worktree_binary, repo_base_path};
use anyhow::{Context, Result};
use gix::Repository;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SessionMetadata {
    session_id: String,
    branch: String,
    sandbox: String,
}

pub fn run(
    repo: &Repository,
    session_id: &str,
    delete_branch: bool,
    config: &AgtConfig,
) -> Result<()> {
    // Try to read session metadata to get the exact sandbox path
    let session_file = repo
        .common_dir()
        .join("agt/sessions")
        .join(format!("{session_id}.json"));

    let (metadata, sandbox_path) = if session_file.exists() {
        let raw = fs::read_to_string(&session_file)?;
        let meta: SessionMetadata = serde_json::from_str(&raw)?;
        let sandbox = PathBuf::from(&meta.sandbox);
        (meta, sandbox)
    } else {
        // Fallback: assume new layout
        let sandbox = repo
            .work_dir()
            .context("No working directory found")?
            .join("sessions")
            .join(session_id)
            .join("sandbox");
        let branch = format!("{}{}", config.branch_prefix, session_id);
        (
            SessionMetadata {
                session_id: session_id.to_string(),
                branch,
                sandbox: sandbox.display().to_string(),
            },
            sandbox,
        )
    };
    let branch_name = metadata.branch;

    if sandbox_path.exists() {
        let status = StdCommand::new(find_worktree_binary(&repo_base_path(repo))?)
            .args([
                "remove",
                "--git-dir",
                repo.common_dir().to_str().unwrap(),
                "--worktree",
                sandbox_path.to_str().unwrap(),
                "--name",
                session_id,
            ])
            .current_dir(repo.work_dir().unwrap())
            .status()
            .context("Failed to remove sandbox")?;

        if !status.success() {
            eprintln!(
                "Warning: sandbox removal failed for {}",
                sandbox_path.display()
            );
        } else {
            println!("Removed sandbox: {}", sandbox_path.display());
        }
    }

    // Cleanup session folder (parent of sandbox)
    // sandbox_path is `.../sessions/id/sandbox`. We want to remove `.../sessions/id`.
    if sandbox_path.ends_with("sandbox") {
        if let Some(parent) = sandbox_path.parent() {
            if parent.exists() {
                // Only remove if it looks like our session folder
                if parent.file_name().and_then(|n| n.to_str()) == Some(session_id) {
                    fs::remove_dir_all(parent)?;
                    println!("Removed session folder: {}", parent.display());
                }
            }
        }
    }

    // 2. Optionally delete shadow branch
    if delete_branch {
        let branch_ref = format!("refs/heads/{branch_name}");
        if repo.find_reference(&branch_ref).is_ok() {
            let branch_ref = repo.find_reference(&format!("refs/heads/{branch_name}"))?;
            branch_ref.delete()?;
            println!("Deleted shadow branch: {branch_name}");
        }
    }

    // 3. Remove timestamp file
    let timestamp_file = repo.common_dir().join("agt/timestamps").join(session_id);
    if timestamp_file.exists() {
        fs::remove_file(&timestamp_file)?;
        println!("Removed timestamp file");
    }

    // 4. Remove session metadata
    if session_file.exists() {
        fs::remove_file(&session_file)?;
        println!("Removed session metadata");
    }

    println!("Pruned session: {session_id}");

    Ok(())
}
