use crate::config::AgtConfig;
use crate::scanner::scan_modified_files;
use anyhow::{Context, Result};
use gix::Repository;
use std::path::{Path, PathBuf};

pub fn run(
    repo: &Repository,
    worktree_path: &Path,
    session_id: &str,
    override_timestamp: Option<i64>,
    dry_run: bool,
    config: &AgtConfig,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    // 1. Read last timestamp (use common_dir for worktree support)
    let timestamp_file = repo.common_dir().join("agt/timestamps").join(session_id);
    let last_timestamp: i64 = std::fs::read_to_string(&timestamp_file)?.trim().parse()?;

    let scan_timestamp = override_timestamp.unwrap_or(last_timestamp);

    // 2. Scan for modified files
    let modified_files = scan_modified_files(worktree_path, scan_timestamp)?;

    if modified_files.is_empty() {
        println!("No modified files since last autocommit");
        return Ok(());
    }

    if dry_run {
        println!("Would commit {} files:", modified_files.len());
        for f in &modified_files {
            println!("  {}", f.display());
        }
        return Ok(());
    }

    // 3. Build tree from files (not using index)
    let agent_branch_ref = format!("refs/heads/{branch_name}");
    let parent1 = repo
        .find_reference(&agent_branch_ref)?
        .peel_to_commit()
        .context("Failed to resolve agent session branch")?;

    let worktree_repo = gix::open(worktree_path)?;
    let parent2_id = worktree_repo
        .head()?
        .peel_to_commit_in_place()
        .context("Failed to resolve worktree HEAD")?
        .id;

    let tree_id = build_tree_from_files(repo, &parent1, worktree_path, &modified_files)?;

    let signature = gix::actor::SignatureRef {
        name: gix::bstr::BStr::new("agt"),
        email: gix::bstr::BStr::new(&config.agent_email),
        time: gix::date::Time::now_local_or_utc(),
    };

    let commit_id = repo.commit_as(
        signature,
        signature,
        agent_branch_ref.as_str(),
        "agt autocommit",
        tree_id,
        [parent1.id, parent2_id],
    )?;

    // 6. Update timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    println!("Created commit {} with {} files", commit_id, modified_files.len());

    Ok(())
}

fn build_tree_from_files(
    repo: &Repository,
    base_commit: &gix::Commit<'_>,
    worktree: &Path,
    files: &[PathBuf],
) -> Result<gix::ObjectId> {
    let base_tree_id = base_commit.tree_id()?.detach();
    let mut editor = repo.edit_tree(base_tree_id)?;

    for relative_path in files {
        let full_path = worktree.join(relative_path);
        if !full_path.exists() {
            editor.remove(path_for_tree(relative_path))?;
            continue;
        }

        let data = std::fs::read(&full_path)
            .with_context(|| format!("Failed to read {}", full_path.display()))?;
        let blob_id = repo.write_blob(data)?;
        editor.upsert(
            path_for_tree(relative_path),
            gix::object::tree::EntryKind::Blob,
            blob_id.detach(),
        )?;
    }

    Ok(editor.write()?.detach())
}

fn path_for_tree(path: &Path) -> String {
    let mut buf = String::new();
    for (idx, component) in path.components().enumerate() {
        if idx > 0 {
            buf.push('/');
        }
        buf.push_str(&component.as_os_str().to_string_lossy());
    }
    buf
}
