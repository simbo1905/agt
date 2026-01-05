use crate::config::AgtConfig;
use crate::gix_cli::{find_worktree_binary, repo_base_path};
use crate::isolation::SessionPaths;
use anyhow::{Context, Result};
use gix::Repository;
use gix_ref::transaction::PreviousValue;
use serde::{Deserialize, Serialize};
use std::process::Command as StdCommand;

#[derive(Debug, Serialize, Deserialize)]
struct SessionMetadata {
    session_id: String,
    branch: String,
    sandbox: String,
    from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_spec: Option<String>,
    from_commit: String,
    user_branch: String,
    created_at: u64,
    #[serde(default)]
    isolation: Option<String>,
}

pub fn run(
    repo: &Repository,
    session_id: &str,
    from: Option<&str>,
    isolation: &str,
    config: &AgtConfig,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    let user_branch = resolve_user_branch(repo, from)?;

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

    // 2. Create shadow branch using gix
    repo.reference(
        format!("refs/heads/{branch_name}"),
        start_commit.id,
        PreviousValue::MustNotExist,
        "agt fork",
    )?;

    // 3. Create session folder structure
    let repo_work_dir = repo
        .work_dir()
        .context("No working directory found")?;
    let session_root = repo_work_dir
        .join("sessions")
        .join(session_id);

    std::fs::create_dir_all(
        session_root
            .parent()
            .context("Failed to resolve sessions directory")?,
    )?;

    let paths = SessionPaths::new(session_root);
    paths.ensure_dirs()?;

    // 4. Create git worktree in sandbox (implementation detail)
    let status = StdCommand::new(find_worktree_binary(&repo_base_path(repo))?)
        .args([
            "add",
            "--git-dir",
            repo.common_dir().to_str().unwrap(),
            "--worktree",
            paths.sandbox.to_str().unwrap(),
            "--name",
            session_id,
            "--branch",
            &format!("refs/heads/{branch_name}"),
        ])
        .status()
        .context("Failed to create sandbox")?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "Failed to create sandbox for {session_id}"
        ));
    }

    // 5. Initialize timestamp and metadata
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
    
    let session = SessionMetadata {
        session_id: session_id.to_string(),
        branch: branch_name.clone(),
        sandbox: std::fs::canonicalize(&paths.sandbox)
            .unwrap_or(paths.sandbox.clone())
            .display()
            .to_string(),
        from: start_commit.id.to_string(),
        from_spec: from.map(str::to_string),
        from_commit: start_commit.id.to_string(),
        user_branch,
        created_at: now,
        isolation: Some(isolation.to_string()),
    };
    std::fs::write(&session_file, serde_json::to_string(&session)?)?;

    println!("Created session: {session_id}");
    println!("  Shadow branch: {branch_name}");
    println!("  Session folder: {}", paths.root.display());
    println!("  Sandbox: {}", paths.sandbox.display());
    println!("  Isolation: {isolation}");

    Ok(())
}

fn resolve_user_branch(repo: &Repository, from: Option<&str>) -> Result<String> {
    if let Some(spec) = from {
        // 1) If `--from` is another session id, inherit its user branch.
        let inherited = repo
            .common_dir()
            .join("agt/sessions")
            .join(format!("{spec}.json"));
        if inherited.exists() {
            let raw = std::fs::read_to_string(&inherited)?;
            let session: SessionMetadata = serde_json::from_str(&raw)?;
            return Ok(session.user_branch);
        }

        // 2) If `--from` names a local branch, use that as user branch.
        let candidate = if spec.starts_with("refs/") {
            spec.to_string()
        } else {
            format!("refs/heads/{spec}")
        };
        if repo.find_reference(&candidate).is_ok() {
            return Ok(candidate);
        }
    }

    // 3) Otherwise use current HEAD referent; we explicitly do not support detached/unborn.
    let head = repo.head()?;
    if head.is_unborn() {
        anyhow::bail!("Unborn HEAD is not supported for agt fork");
    }
    let referent = head
        .referent_name()
        .ok_or_else(|| anyhow::anyhow!("Detached HEAD is not supported for agt fork"))?;
    Ok(referent.as_bstr().to_string())
}
