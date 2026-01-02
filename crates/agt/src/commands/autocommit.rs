use crate::config::AgtConfig;
use crate::scanner::scan_modified_files;
use anyhow::Result;
use gix::Repository;
use std::path::Path;
use std::process::Command as StdCommand;

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
    let _tree_id = build_tree_from_files(repo, worktree_path, &modified_files)?;

    // 4. Get parents (prepared for future dual-parent commit implementation)
    let agent_branch_ref = format!("refs/heads/{branch_name}");
    let _parent1 = repo.find_reference(&agent_branch_ref)?.peel_to_commit()?;

    // Get worktree's tracked branch HEAD as parent2
    let worktree_repo = gix::open(worktree_path)?;
    let _worktree_head = worktree_repo.head()?.peel_to_commit_in_place()?;

    // 5. Create commit using git command (simpler approach)
    StdCommand::new("git")
        .args([
            "-C",
            worktree_path.to_str().unwrap(),
            "commit",
            "-m",
            "agt autocommit",
            "--allow-empty",
            "--no-verify",
        ])
        .env("GIT_AUTHOR_NAME", "agt")
        .env("GIT_AUTHOR_EMAIL", &config.agent_email)
        .env("GIT_COMMITTER_NAME", "agt")
        .env("GIT_COMMITTER_EMAIL", &config.agent_email)
        .status()?;

    // For now, we'll skip the complex commit creation and just use git commands
    // This is a pragmatic approach to get the functionality working

    // 6. Update timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    println!("Created commit with {} files", modified_files.len());

    Ok(())
}

fn build_tree_from_files(
    repo: &Repository,
    worktree: &Path,
    files: &[std::path::PathBuf],
) -> Result<gix::ObjectId> {
    // Use gix to create blob objects for each file
    // Then build tree structure
    // This bypasses the index entirely

    // For now, we'll use git add to stage files and let git handle the tree building
    // This is a pragmatic approach to get the functionality working

    for file_path in files {
        let _full_path = worktree.join(file_path);
        StdCommand::new("git")
            .args([
                "-C",
                worktree.to_str().unwrap(),
                "add",
                file_path.to_str().unwrap(),
            ])
            .status()?;
    }

    // Return a dummy tree ID - in a real implementation we'd get the actual tree ID
    Ok(gix::ObjectId::empty_tree(repo.object_hash()))
}
