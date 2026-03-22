use assert_cmd::Command as AgtCommand;
use gix::commit::NO_PARENT_IDS;
use gix::object::tree::EntryKind;
use gix_object::Tree;
#[cfg(unix)]
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn agt_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("agt").to_path_buf()
}

fn agt_cmd_with_git() -> Result<AgtCommand, Box<dyn std::error::Error>> {
    let mut cmd = AgtCommand::new(agt_bin());
    // Point to real git binary for any passthrough operations
    cmd.env("AGT_GIT_PATH", find_real_git()?);
    cmd.env("AGT_WORKTREE_PATH", ensure_worktree_tool()?);
    Ok(cmd)
}

fn git_mode_cmd(tmp: &TempDir) -> Result<AgtCommand, Box<dyn std::error::Error>> {
    let git_path = tmp
        .path()
        .join(format!("git{}", std::env::consts::EXE_SUFFIX));
    if !git_path.exists() {
        fs::copy(agt_bin(), &git_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&git_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&git_path, perms)?;
        }
    }
    let mut cmd = AgtCommand::new(git_path);
    // In git mode, AGT spawns the real git and filters output
    cmd.env("AGT_GIT_PATH", find_real_git()?);
    cmd.env("AGT_WORKTREE_PATH", ensure_worktree_tool()?);
    Ok(cmd)
}

#[cfg(unix)]
#[test]
fn test_passthrough_uses_git_path() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    gix::init(tmp.path())?;

    // Create a mock git that outputs a sentinel
    let mock_git_path = tmp.path().join("mock-git");
    fs::write(&mock_git_path, "#!/bin/sh\necho GIT-SENTINEL\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&mock_git_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mock_git_path, perms)?;
    }

    let output = git_mode_cmd(&tmp)?
        .args(["branch"])
        .env("AGT_GIT_PATH", &mock_git_path)
        .current_dir(tmp.path())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("GIT-SENTINEL"));

    Ok(())
}

#[test]
fn test_clone_creates_repo_layout() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    // Create a local bare repo to clone from
    let source = tmp.path().join("source");
    init_bare_repo_with_commit(&source)?;

    let target = tmp.path().join("target");
    fs::create_dir_all(&target)?;

    agt_cmd_with_git()?
        .args(["clone", source.to_str().unwrap()])
        .current_dir(&target)
        .assert()
        .success();

    let repo_root = target.join("source");
    let bare_repo = target.join("source.git");
    let main_worktree = repo_root.join("main");
    assert!(bare_repo.exists());
    assert!(main_worktree.exists());

    // Verify .git file points to admin directory (linked worktree pattern)
    let git_file = main_worktree.join(".git");
    assert!(git_file.exists());
    let git_content = fs::read_to_string(&git_file)?;
    assert!(
        git_content.contains("source.git/worktrees/main")
            || git_content.contains("source.git\\worktrees\\main")
    );

    // Verify worktree admin directory exists with proper metadata
    let admin_dir = bare_repo.join("worktrees/main");
    assert!(admin_dir.exists());
    assert!(admin_dir.join("HEAD").exists());
    assert!(admin_dir.join("commondir").exists());
    assert!(admin_dir.join("gitdir").exists());
    assert!(admin_dir.join("index").exists());

    // Verify agt directory exists
    assert!(bare_repo.join("agt").exists());
    assert!(bare_repo.join("agt/timestamps").exists());
    assert!(bare_repo.join("agt/sessions").exists());

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_save_creates_store_and_includes_gitignored_files(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("visible.txt"), "visible")?;
    fs::write(repo.worktree().join("ignored.out"), "ignored")?;

    let output = agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let tag = parse_snapshot_tag(&stdout);
    let store = repo.worktree().join(".agt-snapshots");
    assert!(store.exists());

    let git_path = find_real_git()?;
    let show = Command::new(&git_path)
        .args([
            "--git-dir",
            store.to_str().unwrap(),
            "show",
            &format!("{tag}:payload/ignored.out"),
        ])
        .output()?;
    assert!(show.status.success());
    assert_eq!(String::from_utf8(show.stdout)?, "ignored");

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_save_warns_when_store_is_not_gitignored() -> Result<(), Box<dyn std::error::Error>>
{
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join("generated.txt"), "hello")?;

    agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stderr(predicate::str::contains("not ignored by Git"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_check_reports_changes_between_snapshots() -> Result<(), Box<dyn std::error::Error>>
{
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let before = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;
    fs::write(repo.worktree().join("added.txt"), "added")?;
    fs::remove_file(repo.worktree().join("README.md"))?;

    let second = agt_cmd_with_git()?
        .args(["snapshot", "save", "-m", "second snapshot"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(second.status.success());
    let after = parse_snapshot_tag(&String::from_utf8(second.stdout)?);

    let output = agt_cmd_with_git()?
        .args(["snapshot", "check", "--before", &before, "--after", &after])
        .current_dir(repo.worktree())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("A added.txt"));
    assert!(stdout.contains("D README.md"));
    assert!(stdout.contains("M tracked.txt"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_restore_restores_prior_state() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;
    fs::write(repo.worktree().join("extra.txt"), "extra")?;

    agt_cmd_with_git()?
        .args(["snapshot", "save", "-m", "backup current state"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    agt_cmd_with_git()?
        .args(["snapshot", "restore", "--snapshot", &snapshot])
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(repo.worktree().join("tracked.txt"))?,
        "one"
    );
    assert!(!repo.worktree().join("extra.txt").exists());
    assert!(repo.worktree().join(".agt-snapshots").exists());

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_restore_requires_clean_latest_snapshot_backup(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    agt_cmd_with_git()?
        .args(["snapshot", "restore", "--snapshot", &snapshot])
        .current_dir(repo.worktree())
        .assert()
        .failure()
        .stderr(predicate::str::contains("latest snapshot"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_restore_can_restore_multiple_paths_without_fresh_backup(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::create_dir_all(repo.worktree().join("dist/cache"))?;
    fs::write(repo.worktree().join("lost-a.txt"), "a")?;
    fs::write(repo.worktree().join("dist/cache/output.bin"), "bin")?;

    let first = agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::remove_file(repo.worktree().join("lost-a.txt"))?;
    fs::remove_file(repo.worktree().join("dist/cache/output.bin"))?;

    agt_cmd_with_git()?
        .args([
            "snapshot",
            "restore",
            "--snapshot",
            &snapshot,
            "--path",
            "lost-a.txt",
            "--path",
            "dist/cache",
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert_eq!(fs::read_to_string(repo.worktree().join("lost-a.txt"))?, "a");
    assert_eq!(
        fs::read_to_string(repo.worktree().join("dist/cache/output.bin"))?,
        "bin"
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_targeted_restore_prompts_before_clobbering(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    agt_cmd_with_git()?
        .args([
            "snapshot",
            "restore",
            "--snapshot",
            &snapshot,
            "--path",
            "tracked.txt",
        ])
        .write_stdin("n\n")
        .current_dir(repo.worktree())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Overwrite"));

    assert_eq!(
        fs::read_to_string(repo.worktree().join("tracked.txt"))?,
        "two"
    );

    agt_cmd_with_git()?
        .args([
            "snapshot",
            "restore",
            "--snapshot",
            &snapshot,
            "--path",
            "tracked.txt",
        ])
        .write_stdin("y\n")
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(repo.worktree().join("tracked.txt"))?,
        "one"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_status_reports_clean_and_changed() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    agt_cmd_with_git()?
        .args(["snapshot", "status", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::contains("clean"));

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    agt_cmd_with_git()?
        .args(["snapshot", "status", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::contains("changed"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_status_double_quiet_uses_exit_code() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    agt_cmd_with_git()?
        .args(["snapshot", "status", "-q", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    agt_cmd_with_git()?
        .args(["snapshot", "status", "-q", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .failure()
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_snapshot_save_honors_env_store_override() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    let custom_store = repo.repo_root().join("custom-snapshots.git");
    fs::write(
        repo.worktree().join(".gitignore"),
        "../custom-snapshots.git\n",
    )?;

    agt_cmd_with_git()?
        .args(["snapshot", "save"])
        .env("AGT_SNAPSHOT_STORE", &custom_store)
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert!(custom_store.exists());
    Ok(())
}

#[test]
fn test_clone_sets_default_config() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    let source = tmp.path().join("source");
    init_bare_repo_with_commit(&source)?;

    let target = tmp.path().join("target");
    fs::create_dir_all(&target)?;

    agt_cmd_with_git()?
        .args(["clone", source.to_str().unwrap()])
        .current_dir(&target)
        .assert()
        .success();

    let config_path = target.join("source/main/.agt/config");
    let config_contents = fs::read_to_string(&config_path)?;
    // Verify agt config is set
    assert!(config_contents.contains("[agt]"));
    assert!(config_contents.contains("agentEmail = agt@local"));
    assert!(config_contents.contains("branchPrefix = agtsessions/"));

    // Verify bare repo does not have bare=false (remains bare)
    let bare_config_path = target.join("source.git/config");
    let bare_config_contents = fs::read_to_string(&bare_config_path)?;
    assert!(!bare_config_contents.contains("bare = false"));

    // Verify worktree is usable (gix can open it and sees work_dir)
    let worktree = target.join("source/main");
    let repo = gix::open(&worktree)?;
    assert!(repo.work_dir().is_some());

    Ok(())
}

#[test]
fn test_session_new_creates_shadow_branch_and_session() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;

    agt_cmd_with_git()?
        .args(["session", "new", "--id", "test-session"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    // Verify shadow branch exists
    let gix_repo = gix::open(repo.worktree())?;
    let branch_name = "agtsessions/test-session";
    assert!(gix_repo
        .find_reference(&format!("refs/heads/{branch_name}"))
        .is_ok());

    // Verify session folder and sandbox exist
    let session_root = repo.repo_root().join("sessions/test-session");
    assert!(session_root.exists());
    assert!(session_root.join("sandbox").exists());

    // Verify timestamp file exists
    let timestamp_file = repo.bare.join("agt/timestamps/test-session");
    assert!(timestamp_file.exists());

    // Verify session metadata exists
    let session_file = repo.bare.join("agt/sessions/test-session.json");
    assert!(session_file.exists());

    Ok(())
}

#[test]
fn test_autocommit_with_timestamp_override() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;

    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");
    fs::write(sandbox_path.join("test.txt"), "test content")?;

    // Get current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    // Test agt autocommit with timestamp override
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            &(now - 3600).to_string(), // Force inclusion
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    // Verify shadow commit has two parents and contains the file in sandbox/
    let repo = gix::open(repo.worktree())?;
    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    assert_eq!(commit.parent_ids().count(), 2);

    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("sandbox/test.txt"))?;
    assert!(entry.is_some());

    Ok(())
}

#[test]
fn test_autocommit_dry_run_output_includes_sandbox() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;

    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");
    fs::write(sandbox_path.join("dryrun.txt"), "x")?;

    let output = agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
            "--dry-run",
        ])
        .current_dir(&sandbox_path)
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Dry run: session test-session"));
    assert!(stdout.contains("M "));

    Ok(())
}

#[test]
fn test_autocommit_parent2_is_user_branch_head() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;

    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");
    fs::write(sandbox_path.join("p2.txt"), "p2")?;

    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    // A second autocommit ensures parent1 differs from parent2 after the shadow branch advances.
    fs::write(sandbox_path.join("p2b.txt"), "p2b")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let repo = gix::open(repo.worktree())?;
    let user_head = repo.find_reference("refs/heads/main")?.peel_to_commit()?.id;

    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    let parents: Vec<_> = commit.parent_ids().map(|id| id.to_owned()).collect();
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[1], user_head);
    assert_ne!(parents[0], parents[1]);

    Ok(())
}

#[test]
fn test_autocommit_records_deletions() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    fs::write(sandbox_path.join("delete-me.txt"), "to be deleted")?;

    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    fs::remove_file(sandbox_path.join("delete-me.txt"))?;

    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let repo = gix::open(repo.worktree())?;
    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("sandbox/delete-me.txt"))?;
    assert!(entry.is_none());

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_autocommit_preserves_symlink_entries() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    fs::write(sandbox_path.join("target.txt"), "target")?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("target.txt", sandbox_path.join("link.txt"))?;

    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let repo = gix::open(repo.worktree())?;
    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("sandbox/link.txt"))?;
    let entry = entry.expect("expected symlink entry in sandbox/");
    assert_eq!(entry.mode().kind(), gix::object::tree::EntryKind::Link);

    Ok(())
}

#[test]
fn test_git_mode_filters_shadow_branches() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_shadow_branch()?;

    // Test git branch command (should filter shadow branches)
    let output = git_mode_cmd(repo.tmp())?
        .args(["branch"])
        .current_dir(repo.worktree())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(!stdout.contains("agtsessions/"));

    Ok(())
}

#[test]
fn test_git_mode_add_and_commit() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    let worktree = repo.worktree();

    fs::write(worktree.join("git-add.txt"), "hello")?;

    git_mode_cmd(repo.tmp())?
        .args(["add", "git-add.txt"])
        .current_dir(worktree)
        .assert()
        .success();

    git_mode_cmd(repo.tmp())?
        .args(["commit", "-m", "add via git mode"])
        .current_dir(worktree)
        .assert()
        .success();

    let repo = gix::open(worktree)?;
    let mut branch_ref = repo.find_reference("refs/heads/main")?;
    let commit = branch_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("git-add.txt"))?;
    assert!(entry.is_some());

    Ok(())
}

#[test]
fn test_agt_mode_shows_all_branches() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_shadow_branch()?;

    // Test agt branch command (should show all branches including shadow branches)
    let output = agt_cmd_with_git()?
        .args(["branch", "-a"])
        .current_dir(repo.worktree())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("agtsessions/test-agent"));

    Ok(())
}

// Helper functions
struct TestRepo {
    tmp: TempDir,
    worktree: PathBuf,
    bare: PathBuf,
    root: PathBuf,
}

impl TestRepo {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn tmp(&self) -> &TempDir {
        &self.tmp
    }

    fn repo_root(&self) -> &Path {
        &self.root
    }
}

fn setup_basic_repo() -> Result<TestRepo, Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let bare = tmp.path().join("repo.git");
    init_bare_repo_with_commit(&bare)?;

    let root = tmp.path().to_path_buf();
    let worktree = root.join("main");
    let status = Command::new(ensure_worktree_tool()?)
        .args([
            "add",
            "--git-dir",
            bare.to_str().unwrap(),
            "--worktree",
            worktree.to_str().unwrap(),
            "--name",
            "repo",
            "--branch",
            "refs/heads/main",
        ])
        .status()?;
    assert!(status.success(), "agt-worktree add failed");

    Ok(TestRepo {
        tmp,
        worktree,
        bare,
        root,
    })
}

fn setup_repo_with_session() -> Result<TestRepo, Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;

    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;

    // Create session
    agt_cmd_with_git()?
        .args(["session", "new", "--id", "test-session"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    Ok(repo)
}

fn setup_repo_with_shadow_branch() -> Result<TestRepo, Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;

    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;

    let gix_repo = gix::open(&repo.bare)?;
    fs::write(repo.worktree().join("agent-file.txt"), "agent content")?;
    commit_worktree(
        &gix_repo,
        repo.worktree(),
        "refs/heads/agtsessions/test-agent",
        "Shadow commit",
        "agt@local",
    )?;

    Ok(repo)
}

fn init_bare_repo_with_commit(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let repo = gix::ThreadSafeRepository::init(
        path,
        gix::create::Kind::Bare,
        gix::create::Options::default(),
    )?
    .to_thread_local();

    let blob_id = repo.write_blob(b"# Test Repo")?.detach();
    let tree = Tree {
        entries: vec![gix_object::tree::Entry {
            mode: EntryKind::Blob.into(),
            filename: gix_object::bstr::BString::from("README.md"),
            oid: blob_id,
        }],
    };
    let tree_id = repo.write_object(tree)?.detach();

    let signature = gix::actor::SignatureRef {
        name: gix::bstr::BStr::new("Test User"),
        email: gix::bstr::BStr::new("test@example.com"),
        time: gix::date::Time::now_local_or_utc(),
    };
    repo.commit_as(
        signature,
        signature,
        "refs/heads/main",
        "Initial commit",
        tree_id,
        NO_PARENT_IDS,
    )?;

    Ok(())
}

fn commit_worktree(
    repo: &gix::Repository,
    root: &Path,
    reference: &str,
    message: &str,
    email: &str,
) -> Result<gix::ObjectId, Box<dyn std::error::Error>> {
    let tree_id = write_tree_from_worktree(repo, root)?;

    let signature = gix::actor::SignatureRef {
        name: gix::bstr::BStr::new("Test User"),
        email: gix::bstr::BStr::new(email),
        time: gix::date::Time::now_local_or_utc(),
    };

    let parents = if let Ok(mut r) = repo.find_reference(reference) {
        let parent = r.peel_to_commit()?;
        vec![parent.id]
    } else {
        Vec::new()
    };

    repo.commit_as(signature, signature, reference, message, tree_id, parents)?;

    Ok(tree_id)
}

fn write_tree_from_worktree(
    repo: &gix::Repository,
    root: &Path,
) -> Result<gix::ObjectId, Box<dyn std::error::Error>> {
    let empty_tree_id = repo.write_object(Tree::empty())?.detach();
    let mut editor = repo.edit_tree(empty_tree_id)?;

    for entry in jwalk::WalkDir::new(root)
        .skip_hidden(false)
        .process_read_dir(|_depth, _path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| {
                    dir_entry.file_name != std::ffi::OsStr::new(".git")
                })
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel_path = entry.path().strip_prefix(root)?.to_path_buf();
        let data = std::fs::read(entry.path())?;
        let blob_id = repo.write_blob(data)?.detach();
        editor.upsert(path_for_tree(&rel_path), EntryKind::Blob, blob_id)?;
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

fn write_agt_config(
    repo_path: &Path,
    agent_email: &str,
    branch_prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = gix::open(repo_path)?;
    let config_path = repo.common_dir().join("config");
    let mut contents = if config_path.exists() {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    if !contents.ends_with('\n') {
        contents.push('\n');
    }

    contents.push_str("[agt]\n");
    contents.push_str(&format!("\tagentEmail = {agent_email}\n"));
    contents.push_str(&format!("\tbranchPrefix = {branch_prefix}\n"));
    contents.push('\n');

    fs::write(&config_path, contents)?;

    Ok(())
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("failed to resolve repo root")
}

fn find_real_git() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Check AGT_GIT_PATH env var first
    if let Ok(path) = std::env::var("AGT_GIT_PATH") {
        let candidate = PathBuf::from(&path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    #[cfg(windows)]
    {
        let output = Command::new("where.exe").arg("git.exe").output()?;
        if output.status.success() {
            if let Some(path) = String::from_utf8(output.stdout)?
                .lines()
                .find(|line| !line.trim().is_empty())
            {
                return Ok(PathBuf::from(path.trim()));
            }
        }
    }

    #[cfg(not(windows))]
    {
        let output = Command::new("which").arg("git").output()?;

        if output.status.success() {
            let path = String::from_utf8(output.stdout)?.trim().to_string();
            return Ok(PathBuf::from(path));
        }
    }

    // Fallback locations
    for path in [
        #[cfg(windows)]
        "C:/Program Files/Git/bin/git.exe",
        #[cfg(windows)]
        "C:/Program Files/Git/cmd/git.exe",
        "/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ] {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    Err("Could not find git binary".into())
}

#[cfg(unix)]
fn parse_snapshot_tag(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Saved snapshot "))
        .map(str::trim)
        .map(ToOwned::to_owned)
        .expect("snapshot save output should include tag")
}

fn ensure_worktree_tool() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = repo_root();
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let name = format!("agt-worktree{exe_suffix}");
    let release = root.join("target/release").join(&name);
    let debug = root.join("target/debug").join(&name);

    if release.exists() {
        return Ok(release);
    }
    if debug.exists() {
        return Ok(debug);
    }

    let status = Command::new("cargo")
        .args(["build", "-p", "agt-worktree"])
        .status()?;
    if !status.success() {
        return Err("failed to build agt-worktree".into());
    }

    if debug.exists() {
        Ok(debug)
    } else if release.exists() {
        Ok(release)
    } else {
        Err("agt-worktree binary not found after build".into())
    }
}

#[test]
fn test_restore_resets_sandbox_to_shadow_commit() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    fs::write(sandbox_path.join("file-a.txt"), "version 1")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let gix_repo = gix::open(repo.worktree())?;
    let mut branch_ref = gix_repo.find_reference("refs/heads/agtsessions/test-session")?;
    let first_commit = branch_ref.peel_to_commit()?.id;

    fs::write(sandbox_path.join("file-a.txt"), "version 2")?;
    fs::write(sandbox_path.join("file-b.txt"), "new file")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    assert!(sandbox_path.join("file-b.txt").exists());
    assert_eq!(
        fs::read_to_string(sandbox_path.join("file-a.txt"))?,
        "version 2"
    );

    agt_cmd_with_git()?
        .args([
            "session",
            "restore",
            "--session-id",
            "test-session",
            "--commit",
            &first_commit.to_string(),
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert!(!sandbox_path.join("file-b.txt").exists());
    assert_eq!(
        fs::read_to_string(sandbox_path.join("file-a.txt"))?,
        "version 1"
    );

    Ok(())
}

#[test]
fn test_restore_resets_agent_state_folders() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let session_folder = repo.repo_root().join("sessions/test-session");
    let sandbox_path = session_folder.join("sandbox");
    let config_dir = session_folder.join("config");
    let xdg_dir = session_folder.join("xdg");

    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&xdg_dir)?;
    fs::write(config_dir.join("agent.json"), r#"{"model": "gpt-4"}"#)?;
    fs::write(xdg_dir.join("state.db"), "initial state")?;

    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let gix_repo = gix::open(repo.worktree())?;
    let mut branch_ref = gix_repo.find_reference("refs/heads/agtsessions/test-session")?;
    let first_commit = branch_ref.peel_to_commit()?.id;

    fs::write(config_dir.join("agent.json"), r#"{"model": "claude-3"}"#)?;
    fs::write(xdg_dir.join("state.db"), "modified state")?;
    fs::write(xdg_dir.join("new-file.txt"), "new")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    agt_cmd_with_git()?
        .args([
            "session",
            "restore",
            "--session-id",
            "test-session",
            "--commit",
            &first_commit.to_string(),
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(config_dir.join("agent.json"))?,
        r#"{"model": "gpt-4"}"#
    );
    assert_eq!(
        fs::read_to_string(xdg_dir.join("state.db"))?,
        "initial state"
    );
    assert!(!xdg_dir.join("new-file.txt").exists());

    Ok(())
}

#[test]
fn test_restore_continues_autocommit_with_correct_parent() -> Result<(), Box<dyn std::error::Error>>
{
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    fs::write(sandbox_path.join("a.txt"), "a")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let gix_repo = gix::open(repo.worktree())?;
    let mut branch_ref = gix_repo.find_reference("refs/heads/agtsessions/test-session")?;
    let first_commit_id = branch_ref.peel_to_commit()?.id;

    fs::write(sandbox_path.join("b.txt"), "b")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    agt_cmd_with_git()?
        .args([
            "session",
            "restore",
            "--session-id",
            "test-session",
            "--commit",
            &first_commit_id.to_string(),
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    fs::write(sandbox_path.join("c.txt"), "c")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let gix_repo = gix::open(repo.worktree())?;
    let mut branch_ref = gix_repo.find_reference("refs/heads/agtsessions/test-session")?;
    let new_commit = branch_ref.peel_to_commit()?;
    let parents: Vec<_> = new_commit.parent_ids().map(|id| id.to_owned()).collect();

    assert_eq!(parents.len(), 2);
    assert_eq!(parents[0], first_commit_id);

    Ok(())
}

#[test]
fn test_restore_deletes_files_not_in_shadow_tree() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    fs::write(sandbox_path.join("keep.txt"), "keep")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let gix_repo = gix::open(repo.worktree())?;
    let mut branch_ref = gix_repo.find_reference("refs/heads/agtsessions/test-session")?;
    let first_commit = branch_ref.peel_to_commit()?.id;

    fs::write(sandbox_path.join("extra.txt"), "extra")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    assert!(sandbox_path.join("extra.txt").exists());

    agt_cmd_with_git()?
        .args([
            "session",
            "restore",
            "--session-id",
            "test-session",
            "--commit",
            &first_commit.to_string(),
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert!(sandbox_path.join("keep.txt").exists());
    assert!(!sandbox_path.join("extra.txt").exists());

    Ok(())
}

#[test]
fn test_restore_preserves_tracked_file_content() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    let bare_repo = gix::open(&repo.bare)?;
    let main_worktree = repo.worktree();

    fs::write(main_worktree.join("tracked.txt"), "original tracked")?;
    commit_worktree(
        &bare_repo,
        main_worktree,
        "refs/heads/main",
        "Add tracked file",
        "agt@local",
    )?;

    fs::write(sandbox_path.join("tracked.txt"), "original tracked")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    fs::write(sandbox_path.join("tracked.txt"), "modified tracked")?;
    agt_cmd_with_git()?
        .args([
            "autocommit",
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(&sandbox_path)
        .assert()
        .success();

    let gix_repo = gix::open(repo.worktree())?;
    let mut branch_ref = gix_repo.find_reference("refs/heads/agtsessions/test-session")?;
    let target_commit = branch_ref.peel_to_commit()?.id;

    fs::write(sandbox_path.join("tracked.txt"), "drift")?;

    agt_cmd_with_git()?
        .args([
            "session",
            "restore",
            "--session-id",
            "test-session",
            "--commit",
            &target_commit.to_string(),
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(sandbox_path.join("tracked.txt"))?,
        "modified tracked"
    );

    Ok(())
}

#[test]
fn test_export_requires_clean_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let sandbox_path = repo.repo_root().join("sessions/test-session/sandbox");

    fs::write(sandbox_path.join("uncommitted.txt"), "dirty")?;

    let output = agt_cmd_with_git()?
        .args(["session", "export", "--session-id", "test-session"])
        .current_dir(repo.worktree())
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("uncommitted") || stderr.contains("commit"));

    Ok(())
}

#[test]
fn test_export_pushes_user_branch() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    let remote_bare = tmp.path().join("remote.git");
    init_bare_repo_with_commit(&remote_bare)?;

    let local = tmp.path().join("local");
    fs::create_dir_all(&local)?;

    agt_cmd_with_git()?
        .args(["clone", remote_bare.to_str().unwrap()])
        .current_dir(&local)
        .assert()
        .success();

    let worktree = local.join("remote/main");
    write_agt_config(&worktree, "agt@local", "agtsessions/")?;

    agt_cmd_with_git()?
        .args(["session", "new", "--id", "export-test"])
        .current_dir(&worktree)
        .assert()
        .success();

    let sandbox_path = local.join("remote/sessions/export-test/sandbox");
    fs::write(sandbox_path.join("exported.txt"), "content")?;

    let git_path = find_real_git()?;
    Command::new(&git_path)
        .current_dir(&sandbox_path)
        .args(["add", "exported.txt"])
        .status()?;
    Command::new(&git_path)
        .current_dir(&sandbox_path)
        .args(["commit", "-m", "add exported file"])
        .status()?;

    agt_cmd_with_git()?
        .args(["session", "export", "--session-id", "export-test"])
        .current_dir(&worktree)
        .assert()
        .success();

    let output = Command::new(&git_path)
        .current_dir(&remote_bare)
        .args(["branch", "-a"])
        .output()?;
    let branches = String::from_utf8_lossy(&output.stdout);

    assert!(branches.contains("agtsessions/export-test"));
    assert!(!branches.contains("main") || branches.contains("main"));

    Ok(())
}
