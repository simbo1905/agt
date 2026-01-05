use crate::cli::SessionCommands;
use crate::config::AgtConfig;
use crate::gix_cli::{find_worktree_binary, repo_base_path};
use crate::isolation::SessionPaths;
use anyhow::{bail, Context, Result};
use gix::Repository;
use gix_ref::transaction::PreviousValue;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    isolation: Option<String>,
}

pub fn run(repo: &Repository, command: SessionCommands, config: &AgtConfig) -> Result<()> {
    match command {
        SessionCommands::New { id, from, profile } => {
            let session_id = id.unwrap_or_else(generate_session_id);
            create_session(repo, config, &session_id, from.as_deref(), &profile)
        }
        SessionCommands::Fork { from, id } => {
            let session_id = id.unwrap_or_else(generate_session_id);
            create_session(repo, config, &session_id, Some(&from), "default")
        }
        SessionCommands::Export { session_id } => export_session(repo, config, session_id),
        SessionCommands::Remove { id, delete_branch } => {
            super::prune_session::run(repo, &id, delete_branch, config)
        }
        SessionCommands::List => super::list_sessions::run(repo, config),
    }
}

fn create_session(
    repo: &Repository,
    config: &AgtConfig,
    session_id: &str,
    from: Option<&str>,
    profile: &str,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    let user_branch = resolve_user_branch(repo, from)?;

    // 1. Resolve starting point
    let start_commit = match from {
        Some(spec) => resolve_start_commit(repo, config, spec)?,
        None => repo.head()?.peel_to_commit_in_place()?,
    };

    // 2. Create shadow branch
    repo.reference(
        format!("refs/heads/{branch_name}"),
        start_commit.id,
        PreviousValue::MustNotExist,
        "agt session new",
    )?;

    // 3. Create session folder structure
    let repo_root = repo_root(repo)?;
    let sessions_dir = repo_root.join("sessions");
    std::fs::create_dir_all(&sessions_dir)?;
    let session_root = sessions_dir.join(session_id);

    let paths = SessionPaths::new(session_root.clone());
    paths.ensure_dirs()?;

    // 4. Create git worktree in sandbox
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
        bail!("Failed to create sandbox for {session_id}");
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

    let sessions_meta_dir = agt_dir.join("sessions");
    std::fs::create_dir_all(&sessions_meta_dir)?;
    let session_file = sessions_meta_dir.join(format!("{session_id}.json"));

    let metadata = SessionMetadata {
        session_id: session_id.to_string(),
        branch: branch_name.clone(),
        sandbox: std::fs::canonicalize(&paths.sandbox)
            .unwrap_or_else(|_| paths.sandbox.clone())
            .display()
            .to_string(),
        from: start_commit.id.to_string(),
        from_spec: from.map(str::to_string),
        from_commit: start_commit.id.to_string(),
        user_branch,
        created_at: now,
        profile: Some(profile.to_string()),
        isolation: None,
    };
    std::fs::write(&session_file, serde_json::to_string_pretty(&metadata)?)?;

    println!("Created session: {session_id}");
    println!("  Shadow branch: {branch_name}");
    println!("  Session folder: {}", paths.root.display());
    println!("  Sandbox: {}", paths.sandbox.display());
    println!("  Profile: {profile}");

    Ok(())
}

fn export_session(
    repo: &Repository,
    config: &AgtConfig,
    explicit_session_id: Option<String>,
) -> Result<()> {
    let (session_id, metadata) = match explicit_session_id {
        Some(id) => {
            let meta = load_metadata(repo, &id)?;
            (id, meta)
        }
        None => infer_session_from_cwd(repo)?,
    };

    let sandbox_path = PathBuf::from(&metadata.sandbox);
    if !sandbox_path.exists() {
        bail!("Sandbox not found: {}", sandbox_path.display());
    }

    ensure_clean_worktree(&sandbox_path, config)?;

    let branch_ref = current_branch(&sandbox_path)?;
    let short_branch = branch_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(branch_ref.as_str());

    println!(
        "Pushing session {session_id} branch {} to origin",
        short_branch
    );

    let status = StdCommand::new(&config.git_path)
        .current_dir(&sandbox_path)
        .arg("push")
        .arg("origin")
        .arg(short_branch)
        .status()
        .context("Failed to execute git push")?;

    if !status.success() {
        bail!("git push failed for session {session_id}");
    }

    println!("Export complete for session {session_id}");
    Ok(())
}

fn ensure_clean_worktree(sandbox: &Path, config: &AgtConfig) -> Result<()> {
    let output = StdCommand::new(&config.git_path)
        .current_dir(sandbox)
        .args(["status", "--porcelain"])
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        bail!("git status failed");
    }

    if !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        bail!("Sandbox has uncommitted changes; commit or stash before export");
    }

    Ok(())
}

fn current_branch(sandbox: &Path) -> Result<String> {
    let repo = gix::open(sandbox).context("Failed to open sandbox repository")?;
    let head = repo.head()?;
    if head.is_detached() {
        bail!("Detached HEAD in sandbox is not supported for export");
    }
    if head.is_unborn() {
        bail!("Sandbox has unborn HEAD; create an initial commit first");
    }

    let referent = head
        .referent_name()
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve sandbox branch"))?;
    Ok(referent.as_bstr().to_string())
}

fn resolve_start_commit<'repo>(
    repo: &'repo Repository,
    config: &AgtConfig,
    spec: &str,
) -> Result<gix::Commit<'repo>> {
    match repo.rev_parse_single(spec) {
        Ok(obj) => Ok(obj.object()?.peel_to_commit()?),
        Err(_) => {
            let session_ref = format!("{}{}", config.branch_prefix, spec);
            Ok(repo.rev_parse_single(session_ref.as_str())?
                .object()?
                .peel_to_commit()?)
        }
    }
}

fn resolve_user_branch(repo: &Repository, from: Option<&str>) -> Result<String> {
    if let Some(spec) = from {
        if let Some(branch) = user_branch_from_session(repo, spec)? {
            return Ok(branch);
        }

        let candidate = if spec.starts_with("refs/") {
            spec.to_string()
        } else {
            format!("refs/heads/{spec}")
        };
        if repo.find_reference(&candidate).is_ok() {
            return Ok(candidate);
        }
    }

    let head = repo.head()?;
    if head.is_unborn() {
        bail!("Unborn HEAD is not supported for session creation");
    }
    let referent = head
        .referent_name()
        .ok_or_else(|| anyhow::anyhow!("Detached HEAD is not supported for session creation"))?;
    Ok(referent.as_bstr().to_string())
}

fn user_branch_from_session(repo: &Repository, session_id: &str) -> Result<Option<String>> {
    let meta = load_metadata(repo, session_id).ok();
    Ok(meta.map(|m| m.user_branch))
}

fn load_metadata(repo: &Repository, session_id: &str) -> Result<SessionMetadata> {
    let path = repo
        .common_dir()
        .join("agt/sessions")
        .join(format!("{session_id}.json"));
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(
        serde_json::from_str(&raw)
            .with_context(|| format!("Failed to parse {}", path.display()))?,
    )
}

fn infer_session_from_cwd(repo: &Repository) -> Result<(String, SessionMetadata)> {
    let cwd = std::env::current_dir()
        .context("Failed to determine current directory")?
        .canonicalize()
        .context("Failed to canonicalize current directory")?;

    let sessions_dir = repo.common_dir().join("agt/sessions");
    for entry in std::fs::read_dir(&sessions_dir)
        .with_context(|| format!("Failed to read {}", sessions_dir.display()))?
    {
        let entry = entry?;
        if entry.path().extension() != Some(OsStr::new("json")) {
            continue;
        }
        let session_id = entry
            .path()
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_string();
        if let Ok(meta) = load_metadata(repo, &session_id) {
            let sandbox = PathBuf::from(&meta.sandbox);
            if sandbox.exists() {
                let sandbox = sandbox.canonicalize().unwrap_or_else(|_| sandbox.clone());
                if cwd == sandbox || cwd.starts_with(&sandbox) {
                    return Ok((session_id, meta));
                }
            }
        }
    }

    bail!("Unable to determine session from current directory; specify --session-id")
}

fn repo_root(repo: &Repository) -> Result<PathBuf> {
    // For our layout, the main worktree is directly inside the project root
    // So work_dir()'s parent is the project root where sessions/ lives
    let work_dir = repo
        .work_dir()
        .context("Repository has no working directory")?;

    // Canonicalize to handle relative paths
    let work_dir = std::fs::canonicalize(work_dir)
        .with_context(|| format!("Failed to canonicalize work dir: {}", work_dir.display()))?;

    work_dir
        .parent()
        .map(Path::to_path_buf)
        .context("Failed to resolve repository root")
}

fn generate_session_id() -> String {
    use chrono::Utc;
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    format!("session-{ts}")
}
