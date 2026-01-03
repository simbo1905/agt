use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use gix_features::progress::Discard;
use gix_fs::Capabilities;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

#[derive(Parser)]
#[command(name = "agt-worktree")]
#[command(about = "Minimal worktree add/remove helper for agt")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a linked worktree and checkout the given branch
    Add {
        /// Path to the bare git directory
        #[arg(long)]
        git_dir: PathBuf,
        /// Path to create the worktree
        #[arg(long)]
        worktree: PathBuf,
        /// Worktree name (used under .git/worktrees/<name>)
        #[arg(long)]
        name: String,
        /// Branch ref to checkout (e.g. refs/heads/main)
        #[arg(long)]
        branch: String,
    },
    /// Remove a linked worktree and its metadata
    Remove {
        /// Path to the bare git directory
        #[arg(long)]
        git_dir: PathBuf,
        /// Path to the worktree to remove
        #[arg(long)]
        worktree: PathBuf,
        /// Worktree name (used under .git/worktrees/<name>)
        #[arg(long)]
        name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Add {
            git_dir,
            worktree,
            name,
            branch,
        } => add_worktree(&git_dir, &worktree, &name, &branch),
        Command::Remove {
            git_dir,
            worktree,
            name,
        } => remove_worktree(&git_dir, &worktree, &name),
    }
}

fn add_worktree(git_dir: &Path, worktree: &Path, name: &str, branch: &str) -> Result<()> {
    let repo = gix::open(git_dir).context("Failed to open bare repository")?;
    if !repo.is_bare() {
        anyhow::bail!("Repository must be bare");
    }
    validate_add_paths(git_dir, worktree, name)?;

    let admin_dir = git_dir.join("worktrees").join(name);
    std::fs::create_dir_all(&admin_dir)?;
    std::fs::create_dir_all(worktree)?;

    let head_commit = repo
        .rev_parse_single(branch)
        .context("Failed to resolve branch")?
        .object()?
        .peel_to_commit()?;

    write_metadata_files(worktree, &admin_dir, name, branch, head_commit.id)?;
    checkout_tree(&repo, worktree, &admin_dir, head_commit.tree_id()?.detach())?;

    Ok(())
}

fn remove_worktree(git_dir: &Path, worktree: &Path, name: &str) -> Result<()> {
    let repo = gix::open(git_dir).context("Failed to open bare repository")?;
    if !repo.is_bare() {
        anyhow::bail!("Repository must be bare");
    }
    validate_remove_paths(git_dir, worktree, name)?;

    let admin_dir = git_dir.join("worktrees").join(name);
    if admin_dir.exists() {
        std::fs::remove_dir_all(&admin_dir)?;
    }
    if worktree.exists() {
        std::fs::remove_dir_all(worktree)?;
    }

    Ok(())
}

fn validate_add_paths(git_dir: &Path, worktree: &Path, name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Worktree name cannot be empty");
    }
    if worktree.starts_with(git_dir) {
        anyhow::bail!("Worktree must not be inside the bare repository");
    }
    if worktree.exists() && worktree.read_dir()?.next().is_some() {
        anyhow::bail!("Worktree path must be empty");
    }
    Ok(())
}

fn validate_remove_paths(git_dir: &Path, worktree: &Path, name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Worktree name cannot be empty");
    }
    if worktree.starts_with(git_dir) {
        anyhow::bail!("Worktree must not be inside the bare repository");
    }
    Ok(())
}

fn write_metadata_files(
    worktree: &Path,
    admin_dir: &Path,
    _name: &str,
    branch: &str,
    head_id: gix::ObjectId,
) -> Result<()> {
    let worktree_git = worktree.join(".git");
    let admin_dir_abs = std::fs::canonicalize(admin_dir)?;
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
    std::fs::write(admin_dir.join("HEAD"), format!("ref: {branch}\n"))?;
    std::fs::write(admin_dir.join("ORIG_HEAD"), format!("{}\n", head_id))?;

    Ok(())
}

fn checkout_tree(
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
