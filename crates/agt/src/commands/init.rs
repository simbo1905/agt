use crate::config::AgtConfig;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command as StdCommand;

pub fn run(remote_url: &str, target_path: Option<&Path>, _config: &AgtConfig) -> Result<()> {
    // 1. Determine paths
    let repo_name = extract_repo_name(remote_url)?;
    let base = target_path.unwrap_or(Path::new("."));
    let bare_path = base.join(format!("{repo_name}.git"));
    let work_path = base.join(&repo_name);

    // 2. Clone as bare
    let (checkout, _outcome) = gix::clone::PrepareFetch::new(
        remote_url,
        &bare_path,
        gix::create::Kind::Bare,
        gix::create::Options::default(),
        gix::open::Options::isolated(),
    )?
    .fetch_then_checkout(
        gix::progress::Discard,
        &std::sync::atomic::AtomicBool::new(false),
    )?;
    // Persist the repo so it's not deleted on drop
    checkout.persist();

    // 3. Create worktree directory
    std::fs::create_dir_all(&work_path)?;

    // 4. Create .git file pointing to bare repo
    let git_file = work_path.join(".git");
    std::fs::write(&git_file, format!("gitdir: ../{repo_name}.git"))?;

    // 5. Checkout HEAD using git command (simpler approach)
    StdCommand::new("git")
        .args([
            "--git-dir",
            bare_path.to_str().unwrap(),
            "--work-tree",
            work_path.to_str().unwrap(),
            "checkout",
            "HEAD",
        ])
        .status()?;

    // 6. Create agt state directory
    let agt_dir = bare_path.join("agt");
    std::fs::create_dir_all(agt_dir.join("timestamps"))?;
    std::fs::create_dir_all(agt_dir.join("sessions"))?;

    println!("Initialized agt repository: {repo_name}");
    println!("  Bare repo: {}", bare_path.display());
    println!("  Worktree: {}", work_path.display());

    Ok(())
}

fn extract_repo_name(url: &str) -> Result<String> {
    // Simple extraction - remove .git suffix and get last path component
    let mut name = url
        .trim_end_matches(".git")
        .split('/')
        .next_back()
        .context("Failed to extract repository name from URL")?
        .to_string();

    // Remove any trailing slashes or invalid characters
    name = name.trim_end_matches('/').to_string();

    if name.is_empty() {
        return Err(anyhow::anyhow!("Invalid repository URL"));
    }

    Ok(name)
}
