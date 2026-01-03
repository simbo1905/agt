use crate::config::AgtConfig;
use anyhow::{Context, Result};
use gix_features::progress::Discard;
use gix_fs::Capabilities;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub fn run(remote_url: &str, target_path: Option<&Path>, config: &AgtConfig) -> Result<()> {
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

    // 3. Create main worktree as a proper linked worktree
    // This keeps the repo bare while having a working main worktree
    std::fs::create_dir_all(&work_path)?;
    setup_main_worktree(&bare_path, &work_path, &repo_name)?;

    // 4. Write default config (agt settings only, keep bare=true)
    write_default_config(&bare_path, config)?;

    // 5. Create agt state directory
    let agt_dir = bare_path.join("agt");
    std::fs::create_dir_all(agt_dir.join("timestamps"))?;
    std::fs::create_dir_all(agt_dir.join("sessions"))?;

    println!("Initialized agt repository: {repo_name}");
    println!("  Bare repo: {}", bare_path.display());
    println!("  Worktree: {}", work_path.display());

    Ok(())
}

/// Set up the main worktree as a proper linked worktree.
/// This creates the worktree admin directory under <bare>.git/worktrees/<name>/
/// and the .git file in the worktree, then checks out HEAD.
fn setup_main_worktree(bare_path: &Path, work_path: &Path, name: &str) -> Result<()> {
    let repo = gix::open(bare_path).context("Failed to open bare repository")?;

    // Create admin directory: <bare>.git/worktrees/<name>/
    let admin_dir = bare_path.join("worktrees").join(name);
    std::fs::create_dir_all(&admin_dir)?;

    // Resolve HEAD to get the branch and commit
    let head = repo.head()?;
    let branch_ref = if head.is_unborn() {
        "refs/heads/main".to_string()
    } else {
        head.referent_name()
            .map(|r| r.as_bstr().to_string())
            .unwrap_or_else(|| "refs/heads/main".to_string())
    };

    // Write worktree metadata files
    // 1. Create .git file in worktree pointing to admin dir
    let worktree_git = work_path.join(".git");
    let admin_dir_abs = std::fs::canonicalize(&admin_dir)?;
    std::fs::write(
        &worktree_git,
        format!("gitdir: {}\n", admin_dir_abs.display()),
    )?;

    // 2. Write gitdir file in admin dir pointing back to worktree's .git
    let worktree_git_abs = std::fs::canonicalize(&worktree_git)?;
    std::fs::write(
        admin_dir.join("gitdir"),
        format!("{}\n", worktree_git_abs.display()),
    )?;

    // 3. Write commondir (relative path to the main git dir)
    std::fs::write(admin_dir.join("commondir"), "../..\n")?;

    // 4. Write HEAD
    std::fs::write(admin_dir.join("HEAD"), format!("ref: {branch_ref}\n"))?;

    // 5. Checkout files if HEAD points to a valid commit
    if !head.is_unborn() {
        let commit = head
            .into_peeled_id()
            .context("Failed to resolve HEAD")?
            .object()?
            .peel_to_commit()?;
        let tree_id = commit.tree_id()?.detach();

        // Write ORIG_HEAD
        std::fs::write(admin_dir.join("ORIG_HEAD"), format!("{}\n", commit.id))?;

        // Build index and checkout
        checkout_to_worktree(&repo, work_path, &admin_dir, tree_id)?;
    }

    Ok(())
}

/// Checkout a tree to a worktree directory
fn checkout_to_worktree(
    repo: &gix::Repository,
    worktree: &Path,
    admin_dir: &Path,
    tree_id: gix::ObjectId,
) -> Result<()> {
    let mut index = repo
        .index_from_tree(&tree_id)
        .context("Failed to build index from tree")?;
    index.set_path(admin_dir.join("index"));

    let mut opts = gix_worktree_state::checkout::Options::default();
    opts.fs = Capabilities::probe(worktree);
    opts.destination_is_initially_empty = true;
    opts.overwrite_existing = true;

    let files = Discard;
    let bytes = Discard;
    let interrupt = AtomicBool::new(false);
    gix_worktree_state::checkout(
        &mut index,
        worktree,
        repo.objects.clone().into_arc()?,
        &files,
        &bytes,
        &interrupt,
        opts,
    )?;

    index.write(Default::default())?;
    Ok(())
}

/// Write default agt configuration to the git config file.
/// Only writes the [agt] section - does NOT change bare=true.
fn write_default_config(bare_path: &Path, config: &AgtConfig) -> Result<()> {
    let config_path = bare_path.join("config");
    let mut contents = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    if !contents.ends_with('\n') {
        contents.push('\n');
    }

    // Ensure bare=true remains explicitly set (linked worktree admin dirs exist under a bare repo).
    if contents.contains("bare = false") {
        contents = contents.replace("bare = false", "bare = true");
    }
    if !contents.contains("bare = true") {
        contents.push_str("[core]\n\tbare = true\n\n");
    }

    // Only write agt section - keep repo bare
    contents.push_str("[agt]\n");
    contents.push_str(&format!("\tagentEmail = {}\n", config.agent_email));
    contents.push_str(&format!("\tbranchPrefix = {}\n", config.branch_prefix));
    if let Some(user_email) = &config.user_email {
        contents.push_str(&format!("\tuserEmail = {}\n", user_email));
    }
    contents.push('\n');

    std::fs::write(&config_path, contents)?;

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
