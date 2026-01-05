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
    let repo_root = base.join(&repo_name);
    let bare_path = base.join(format!("{repo_name}.git"));
    let main_path = repo_root.join("main");

    std::fs::create_dir_all(&repo_root)?;
    std::fs::create_dir_all(&bare_path)?;

    // 2. Clone as bare into .bare/
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
    checkout.persist();

    // 3. Create main worktree linked to bare repo
    std::fs::create_dir_all(&main_path)?;
    setup_main_worktree(&bare_path, &main_path, "main")?;

    // 4. Create sessions/ folder (empty)
    std::fs::create_dir_all(repo_root.join("sessions"))?;

    // 5. Create .agt directory in main worktree with config
    let agt_dir = main_path.join(".agt");
    std::fs::create_dir_all(&agt_dir)?;
    write_agt_config(&agt_dir, config)?;

    // 7. Create agt state directory inside .bare
    let agt_state_dir = bare_path.join("agt");
    std::fs::create_dir_all(agt_state_dir.join("timestamps"))?;
    std::fs::create_dir_all(agt_state_dir.join("sessions"))?;

    println!("Cloned agt repository: {repo_name}");
    println!("  Bare repo: {}", bare_path.display());
    println!("  Main worktree: {}", main_path.display());

    Ok(())
}

fn setup_main_worktree(bare_path: &Path, work_path: &Path, name: &str) -> Result<()> {
    let repo = gix::open(bare_path).context("Failed to open bare repository")?;

    // Create admin directory: .bare/worktrees/<name>
    let admin_dir = bare_path.join("worktrees").join(name);
    std::fs::create_dir_all(&admin_dir)?;

    // Resolve HEAD to get branch
    let head = repo.head()?;
    let branch_ref = if head.is_unborn() {
        "refs/heads/main".to_string()
    } else {
        head.referent_name()
            .map(|r| r.as_bstr().to_string())
            .unwrap_or_else(|| "refs/heads/main".to_string())
    };

    // Write worktree metadata
    let admin_dir_abs = std::fs::canonicalize(&admin_dir)?;
    let worktree_git = work_path.join(".git");
    std::fs::write(
        &worktree_git,
        format!("gitdir: {}\n", admin_dir_abs.display()),
    )?;

    let worktree_git_abs = std::fs::canonicalize(&worktree_git)?;
    std::fs::write(
        admin_dir.join("gitdir"),
        format!("{}\n", worktree_git_abs.display()),
    )?;
    std::fs::write(admin_dir.join("commondir"), "../..\n")?;
    std::fs::write(admin_dir.join("HEAD"), format!("ref: {branch_ref}\n"))?;

    if !head.is_unborn() {
        let commit = head
            .into_peeled_id()
            .context("Failed to resolve HEAD")?
            .object()?
            .peel_to_commit()?;
        std::fs::write(admin_dir.join("ORIG_HEAD"), format!("{}\n", commit.id))?;
        checkout_to_worktree(&repo, work_path, &admin_dir, commit.tree_id()?.detach())?;
    }

    Ok(())
}

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

    let opts = gix_worktree_state::checkout::Options {
        fs: Capabilities::probe(worktree),
        destination_is_initially_empty: true,
        overwrite_existing: true,
        ..Default::default()
    };

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

fn write_agt_config(agt_dir: &Path, config: &AgtConfig) -> Result<()> {
    let config_path = agt_dir.join("config");

    let mut contents = String::new();
    contents.push_str("[agt]\n");
    contents.push_str(&format!("    gitPath = {}\n", config.git_path.display()));
    contents.push_str(&format!("    agentEmail = {}\n", config.agent_email));
    contents.push_str(&format!("    branchPrefix = {}\n", config.branch_prefix));
    if let Some(user_email) = &config.user_email {
        contents.push_str(&format!("    userEmail = {}\n", user_email));
    }

    std::fs::write(&config_path, contents)?;

    Ok(())
}

fn extract_repo_name(url: &str) -> Result<String> {
    let mut name = url
        .trim_end_matches(".git")
        .split('/')
        .next_back()
        .context("Failed to extract repository name from URL")?
        .to_string();

    name = name.trim_end_matches('/').to_string();

    if name.is_empty() {
        return Err(anyhow::anyhow!("Invalid repository URL"));
    }

    Ok(name)
}
