use agt_core::config::AgtConfig;
use anyhow::{bail, Context, Result};
use gix::Repository;
use std::process::Command as StdCommand;

pub fn run(repo: &Repository, session_id: &str, dry_run: bool, config: &AgtConfig) -> Result<()> {
    // 1. The session must be known to agt
    let session_file = repo
        .common_dir()
        .join("agt/sessions")
        .join(format!("{session_id}.json"));
    if !session_file.exists() {
        bail!("Unknown session id: {session_id}");
    }

    // 2. The session must have a shadow branch to merge
    let shadow_branch = format!("{}{}", config.branch_prefix, session_id);
    let shadow_ref = format!("refs/heads/{shadow_branch}");
    let shadow_head = repo
        .find_reference(shadow_ref.as_str())
        .with_context(|| {
            format!("Shadow branch {shadow_branch} not found for session {session_id}")
        })?
        .peel_to_commit()
        .with_context(|| format!("Failed to resolve shadow branch {shadow_branch}"))?
        .id;

    // 3. The current branch must be a real user branch
    let mut head = repo.head()?;
    if head.is_unborn() {
        bail!("Unborn HEAD is not supported for session merge; create an initial commit first");
    }
    if head.is_detached() {
        bail!("Detached HEAD is not supported for session merge; checkout a branch first");
    }
    let user_branch = head
        .referent_name()
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve current branch"))?
        .as_bstr()
        .to_string();
    let user_head = head.peel_to_commit_in_place()?.id;

    // 4. The current worktree must be clean (same standard agt uses for session export)
    ensure_clean_worktree(config, session_id)?;

    // 5. Decide the merge shape: fast-forward when the user branch head is an
    //    ancestor of the shadow branch head, otherwise a real merge commit.
    let fast_forward = is_ancestor(config, &user_head.to_string(), &shadow_head.to_string())?;

    if dry_run {
        report_dry_run(
            config,
            session_id,
            &shadow_branch,
            &user_branch,
            &shadow_head.to_string(),
            fast_forward,
        )?;
        return Ok(());
    }

    if !fast_forward {
        ensure_user_identity(config)?;
    }

    // 6. Perform the merge with the host git binary; the merge commit (when one
    //    is created) is authored with the user's git identity, never the agent
    //    identity (which lives in agt's own config, not git's).
    let mut merge = StdCommand::new(&config.git_path);
    merge.arg("-c").arg("commit.gpgsign=false");
    merge.arg("merge");
    if fast_forward {
        merge.arg("--ff-only");
    } else {
        merge
            .arg("--no-ff")
            .arg("-m")
            .arg(format!("Merge session {session_id}"));
    }
    merge.arg(&shadow_branch);

    let status = merge.status().context("Failed to execute git merge")?;

    if !status.success() {
        // Abort the failed merge so the working tree is left clean. The session
        // still holds the work on its shadow branch.
        let _ = StdCommand::new(&config.git_path)
            .args(["merge", "--abort"])
            .status();
        bail!(
            "Merging session {session_id} failed (conflict or error); merge aborted cleanly. \
             Session {session_id} still holds the work on {shadow_branch}; resolve by merging \
             {shadow_branch} into {user_branch} manually."
        );
    }

    let merge_commit = rev_parse_head(config)?;
    println!("Merged session {session_id} into {user_branch}");
    println!("  Merge commit: {merge_commit}");
    print_diff_stat(config, &user_head.to_string());
    println!("  Shadow branch {shadow_branch} retained; remove it later with 'agt session remove --id {session_id} --delete-branch'");

    Ok(())
}

fn report_dry_run(
    config: &AgtConfig,
    session_id: &str,
    shadow_branch: &str,
    user_branch: &str,
    shadow_head: &str,
    fast_forward: bool,
) -> Result<()> {
    println!("Dry run: session {session_id}");
    if fast_forward {
        println!("  Would fast-forward {user_branch} to {shadow_head}");
    } else if predict_conflicts(config, user_branch, shadow_head)? {
        bail!(
            "Dry run: merging {shadow_branch} into {user_branch} would conflict; no changes made"
        );
    } else {
        println!("  Would create a merge commit on {user_branch} merging {shadow_branch}");
        println!("  The merge commit would be authored with your git user identity");
    }
    println!("  Shadow branch {shadow_branch} would be retained");
    Ok(())
}

fn ensure_clean_worktree(config: &AgtConfig, session_id: &str) -> Result<()> {
    let output = StdCommand::new(&config.git_path)
        .args(["status", "--porcelain"])
        .output()
        .context("Failed to run git status")?;

    if !output.status.success() {
        bail!("git status failed");
    }

    if !String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        bail!("Working tree has uncommitted changes; commit or stash before merging session {session_id}");
    }

    Ok(())
}

fn ensure_user_identity(config: &AgtConfig) -> Result<()> {
    let output = StdCommand::new(&config.git_path)
        .args(["config", "user.email"])
        .output()
        .context("Failed to run git config")?;

    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        bail!(
            "No git user identity configured; set user.name and user.email so the merge commit \
             is authored with your identity"
        );
    }

    Ok(())
}

fn is_ancestor(config: &AgtConfig, ancestor: &str, descendant: &str) -> Result<bool> {
    let status = StdCommand::new(&config.git_path)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()
        .context("Failed to run git merge-base")?;
    Ok(status.success())
}

fn predict_conflicts(config: &AgtConfig, user_head: &str, shadow_head: &str) -> Result<bool> {
    let output = StdCommand::new(&config.git_path)
        .args(["merge-tree", "--write-tree", user_head, shadow_head])
        .output()
        .context("Failed to run git merge-tree")?;

    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        code => bail!(
            "git merge-tree failed with status {:?}: {}",
            code,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
    }
}

fn rev_parse_head(config: &AgtConfig) -> Result<String> {
    let output = StdCommand::new(&config.git_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        bail!("git rev-parse HEAD failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn print_diff_stat(config: &AgtConfig, before: &str) {
    let output = StdCommand::new(&config.git_path)
        .args(["diff", "--stat", before, "HEAD"])
        .output();
    if let Ok(output) = output {
        if output.status.success() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
    }
}
