//! Snapshot integration tests against the standalone `agt-snapshot` binary.
//! Unix-only: the snapshot subsystem bails at runtime on Windows and the
//! fixtures rely on POSIX permissions and symlinks.
#![cfg(unix)]

use assert_cmd::Command as AgtCommand;
use gix::commit::NO_PARENT_IDS;
use gix::object::tree::EntryKind;
use gix_object::Tree;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
fn log_test_start(test_name: &str) {
    if std::env::var("AGT_LOG").is_ok() {
        if let Some(log_path) = std::env::var_os("AGT_LOG_PATH") {
            let path = PathBuf::from(log_path);
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = writeln!(file, "[agt] test started: {}", test_name);
            }
        }
    }
}

fn snapshot_cmd() -> Result<AgtCommand, Box<dyn std::error::Error>> {
    let mut cmd = AgtCommand::new(env!("CARGO_BIN_EXE_agt-snapshot"));
    // Point to real git binary for any passthrough operations
    cmd.env("AGT_GIT_PATH", find_real_git()?);
    cmd.env("AGT_WORKTREE_PATH", ensure_worktree_tool()?);
    Ok(cmd)
}

// Helper functions
struct TestRepo {
    // Keeps the temp directory alive for the lifetime of the fixture.
    _tmp: TempDir,
    worktree: PathBuf,
    root: PathBuf,
}

impl TestRepo {
    fn worktree(&self) -> &Path {
        &self.worktree
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
        _tmp: tmp,
        worktree,
        root,
    })
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

    std::fs::write(path.join("HEAD"), "ref: refs/heads/main\n")?;

    Ok(())
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
    if let Ok(path) = std::env::var("AGT_TEST_REAL_GIT") {
        let candidate = PathBuf::from(&path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

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
fn test_snapshot_save_creates_store_and_includes_gitignored_files(
) -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_save_creates_store_and_includes_gitignored_files");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("visible.txt"), "visible")?;
    fs::write(repo.worktree().join("ignored.out"), "ignored")?;

    let output = snapshot_cmd()?
        .args(["save"])
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

#[test]
fn test_snapshot_save_honors_agt_snapshot_ignore() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_save_honors_agt_snapshot_ignore");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(
        repo.worktree().join(".gitignore"),
        ".agt-snapshots/
",
    )?;
    fs::write(
        repo.worktree().join(".agt-snapshot-ignore"),
        ".tmp/
*.sql.gz
",
    )?;
    fs::create_dir_all(repo.worktree().join(".tmp"))?;
    fs::write(repo.worktree().join(".tmp/skip.txt"), "skip")?;
    fs::write(repo.worktree().join("keep.txt"), "keep")?;
    fs::write(repo.worktree().join("dump.sql.gz"), "dump")?;

    let output = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("Ignored: 2"),
        "expected ignored count in output, got: {stdout}"
    );
    let tag = parse_snapshot_tag(&stdout);
    let store = repo.worktree().join(".agt-snapshots");
    let git_path = find_real_git()?;

    let keep = Command::new(&git_path)
        .args([
            "--git-dir",
            store.to_str().unwrap(),
            "show",
            &format!("{tag}:payload/keep.txt"),
        ])
        .output()?;
    assert!(keep.status.success());

    let skipped = Command::new(&git_path)
        .args([
            "--git-dir",
            store.to_str().unwrap(),
            "show",
            &format!("{tag}:payload/.tmp/skip.txt"),
        ])
        .output()?;
    assert!(!skipped.status.success());

    Ok(())
}

#[test]
fn test_snapshot_check_ignore_verbose_and_non_matching() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_check_ignore_verbose_and_non_matching");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(
        repo.worktree().join(".agt-snapshot-ignore"),
        ".tmp/
*.sql.gz
!important.sql.gz
",
    )?;
    fs::create_dir_all(repo.worktree().join(".tmp"))?;
    fs::write(repo.worktree().join(".tmp/skip.txt"), "skip")?;
    fs::write(repo.worktree().join("dump.sql.gz"), "dump")?;
    fs::write(repo.worktree().join("important.sql.gz"), "keep")?;
    fs::write(repo.worktree().join("keep.txt"), "keep")?;

    snapshot_cmd()?
        .args([
            "check-ignore",
            "-v",
            "-n",
            ".tmp/skip.txt",
            "dump.sql.gz",
            "important.sql.gz",
            "keep.txt",
        ])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            ".agt-snapshot-ignore:1:.tmp/	.tmp/skip.txt",
        ))
        .stdout(predicate::str::contains(
            ".agt-snapshot-ignore:2:*.sql.gz	dump.sql.gz",
        ))
        .stdout(predicate::str::contains(
            ".agt-snapshot-ignore:3:!important.sql.gz	important.sql.gz",
        ))
        .stdout(predicate::str::contains("::	keep.txt"));

    snapshot_cmd()?
        .args(["check-ignore", "keep.txt"])
        .current_dir(repo.worktree())
        .assert()
        .failure();

    snapshot_cmd()?
        .args(["check-ignore", "-n", "keep.txt"])
        .current_dir(repo.worktree())
        .assert()
        .code(128)
        .stderr(predicate::str::contains(
            "fatal: --non-matching is only valid with --verbose",
        ));

    Ok(())
}

#[test]
fn test_snapshot_status_ignored_lists_paths() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_status_ignored_lists_paths");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(
        repo.worktree().join(".agt-snapshot-ignore"),
        ".tmp/
*.sql.gz
",
    )?;
    fs::create_dir_all(repo.worktree().join(".tmp"))?;
    fs::write(repo.worktree().join(".tmp/skip.txt"), "skip")?;
    fs::write(repo.worktree().join("dump.sql.gz"), "dump")?;
    fs::write(repo.worktree().join("keep.txt"), "keep")?;

    snapshot_cmd()?
        .args(["status", "--ignored", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::contains(".tmp"))
        .stdout(predicate::str::contains("dump.sql.gz"));

    Ok(())
}

#[test]
fn test_snapshot_save_warns_when_store_is_not_gitignored() -> Result<(), Box<dyn std::error::Error>>
{
    log_test_start("test_snapshot_save_warns_when_store_is_not_gitignored");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join("generated.txt"), "hello")?;

    snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stderr(predicate::str::contains("not ignored by Git"));

    Ok(())
}

#[test]
fn test_snapshot_list_shows_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_list_shows_snapshots");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join("file.txt"), "v1")?;

    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let first_tag = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("file.txt"), "v2")?;
    let second = snapshot_cmd()?
        .args(["save", "-m", "second snapshot"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(second.status.success());
    let second_tag = parse_snapshot_tag(&String::from_utf8(second.stdout)?);

    let output = snapshot_cmd()?
        .args(["list"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;

    let second_message = "second snapshot";
    let first_line = stdout
        .lines()
        .find(|line| line.starts_with(&first_tag))
        .expect("expected first snapshot line in list output");
    assert!(
        first_line.starts_with(&format!("{first_tag} snapshot save for ")),
        "expected default message prefix in list output, got: {stdout}"
    );
    assert!(
        stdout.contains(&format!("{second_tag} {second_message}")),
        "expected second tag and message in list output, got: {stdout}"
    );
    assert!(
        stdout.contains("2 snapshot(s)"),
        "expected count in list output, got: {stdout}"
    );

    Ok(())
}

#[test]
fn test_snapshot_list_truncates_to_terminal_width() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_list_truncates_to_terminal_width");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join("file.txt"), "v1")?;

    let message = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-long-message";
    let save = snapshot_cmd()?
        .args(["save", "-m", message])
        .current_dir(repo.worktree())
        .output()?;
    assert!(save.status.success());
    let tag = parse_snapshot_tag(&String::from_utf8(save.stdout)?);

    let output = snapshot_cmd()?
        .args(["list"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let line = stdout
        .lines()
        .find(|line| line.starts_with(&tag))
        .expect("expected snapshot line in list output");

    assert_eq!(line.len(), 80, "expected 80-char line, got: {line}");
    assert!(line.starts_with(&format!("{tag} ")));
    assert!(
        line.ends_with(".."),
        "expected truncated suffix, got: {line}"
    );

    Ok(())
}

#[test]
fn test_snapshot_list_quiet_shows_only_tags() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_list_quiet_shows_only_tags");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join("file.txt"), "v1")?;

    let first = snapshot_cmd()?
        .args(["save", "-m", "first snapshot"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let first_tag = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("file.txt"), "v2")?;
    let second = snapshot_cmd()?
        .args(["save", "-m", "second snapshot"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(second.status.success());
    let second_tag = parse_snapshot_tag(&String::from_utf8(second.stdout)?);

    let output = snapshot_cmd()?
        .args(["list", "-q"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains(&format!("{first_tag}\n")),
        "expected first tag in quiet output, got: {stdout}"
    );
    assert!(
        stdout.contains(&format!("{second_tag}\n")),
        "expected second tag in quiet output, got: {stdout}"
    );
    assert!(
        !stdout.contains("first snapshot"),
        "quiet output should omit messages, got: {stdout}"
    );
    assert!(
        !stdout.contains("second snapshot"),
        "quiet output should omit messages, got: {stdout}"
    );
    assert!(
        stdout.contains("2 snapshot(s)"),
        "expected count in quiet output, got: {stdout}"
    );

    Ok(())
}

#[test]
fn test_snapshot_list_handles_lightweight_tags() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_list_handles_lightweight_tags");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join("file.txt"), "v1")?;

    let save = snapshot_cmd()?
        .args(["save", "-m", "annotated snapshot"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(save.status.success());
    let annotated_tag = parse_snapshot_tag(&String::from_utf8(save.stdout)?);
    let snapshot_store = repo.worktree().join(".agt-snapshots");
    let target = Command::new("git")
        .args([
            "-C",
            &snapshot_store.display().to_string(),
            "rev-parse",
            "refs/heads/agt-snapshots",
        ])
        .output()?;
    assert!(target.status.success());
    let target = String::from_utf8(target.stdout)?.trim().to_string();

    let lightweight = Command::new("git")
        .args([
            "-C",
            &snapshot_store.display().to_string(),
            "tag",
            "lightweight-only",
            &target,
        ])
        .status()?;
    assert!(lightweight.success());

    let output = snapshot_cmd()?
        .args(["list"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;

    assert!(
        stdout.contains(&format!("{annotated_tag} annotated snapshot")),
        "expected annotated tag output, got: {stdout}"
    );
    assert!(
        stdout.contains("lightweight-only"),
        "expected lightweight tag output, got: {stdout}"
    );
    assert!(
        !stdout.contains("lightweight-only "),
        "lightweight tag should not have a forced trailing message column, got: {stdout}"
    );

    Ok(())
}

#[test]
fn test_snapshot_check_reports_changes_between_snapshots() -> Result<(), Box<dyn std::error::Error>>
{
    log_test_start("test_snapshot_check_reports_changes_between_snapshots");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let before = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;
    fs::write(repo.worktree().join("added.txt"), "added")?;
    fs::remove_file(repo.worktree().join("README.md"))?;

    let second = snapshot_cmd()?
        .args(["save", "-m", "second snapshot"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(second.status.success());
    let after = parse_snapshot_tag(&String::from_utf8(second.stdout)?);

    let output = snapshot_cmd()?
        .args(["diff", &before, &after])
        .current_dir(repo.worktree())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("A added.txt"));
    assert!(stdout.contains("D README.md"));
    assert!(stdout.contains("M tracked.txt"));

    Ok(())
}

#[test]
fn test_snapshot_check_reports_deletions_ignoring_gitignore(
) -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_check_reports_deletions_ignoring_gitignore");
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()?;
    fs::write(repo_path.join(".gitignore"), "_generated/\n")?;
    fs::write(repo_path.join("tracked.txt"), "tracked")?;
    fs::create_dir_all(repo_path.join("_generated"))?;
    fs::write(repo_path.join("_generated/artifact.js"), "console.log(1);")?;
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(repo_path)
        .output()?;
    write_agt_config(repo_path, "agt@local", "agtsessions/")?;
    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo_path)
        .output()?;
    assert!(first.status.success());
    let before = parse_snapshot_tag(&String::from_utf8(first.stdout)?);
    fs::remove_file(repo_path.join("_generated/artifact.js"))?;
    let second = snapshot_cmd()?
        .args(["save", "-m", "after"])
        .current_dir(repo_path)
        .output()?;
    assert!(second.status.success());
    let after = parse_snapshot_tag(&String::from_utf8(second.stdout)?);
    let output = snapshot_cmd()?
        .args(["diff", &before, &after])
        .current_dir(repo_path)
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("D _generated/artifact.js"),
        "expected deletion of gitignored file, got: {stdout}"
    );
    Ok(())
}

#[test]
fn test_snapshot_diff_auto_sorts_timestamp_tags() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_diff_auto_sorts_timestamp_tags");
    let temp_dir = tempfile::tempdir()?;
    let repo_path = temp_dir.path();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()?;
    fs::write(repo_path.join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo_path.join("file.txt"), "v1")?;
    write_agt_config(repo_path, "agt@local", "agtsessions/")?;
    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo_path)
        .output()?;
    assert!(first.status.success());
    let older = parse_snapshot_tag(&String::from_utf8(first.stdout)?);
    fs::write(repo_path.join("file.txt"), "v2")?;
    fs::write(repo_path.join("new.txt"), "new")?;
    let second = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo_path)
        .output()?;
    assert!(second.status.success());
    let newer = parse_snapshot_tag(&String::from_utf8(second.stdout)?);
    let output = snapshot_cmd()?
        .args(["diff", &newer, &older])
        .current_dir(repo_path)
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("A new.txt"),
        "reversed order should still show additions correctly, got: {stdout}"
    );
    assert!(
        !stdout.contains("D new.txt"),
        "should not show deletions when newer is before, got: {stdout}"
    );
    Ok(())
}

#[test]
fn test_snapshot_restore_restores_prior_state() -> Result<(), Box<dyn std::error::Error>> {
    log_test_start("test_snapshot_restore_restores_prior_state");
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;
    fs::write(repo.worktree().join("extra.txt"), "extra")?;

    snapshot_cmd()?
        .args(["save", "-m", "backup current state"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    snapshot_cmd()?
        .args(["restore", "--snapshot", &snapshot])
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

#[test]
fn test_snapshot_restore_requires_clean_latest_snapshot_backup(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    snapshot_cmd()?
        .args(["restore", "--snapshot", &snapshot])
        .current_dir(repo.worktree())
        .assert()
        .failure()
        .stderr(predicate::str::contains("latest snapshot"));

    Ok(())
}

#[test]
fn test_snapshot_restore_can_restore_multiple_paths_without_fresh_backup(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::create_dir_all(repo.worktree().join("dist/cache"))?;
    fs::write(repo.worktree().join("lost-a.txt"), "a")?;
    fs::write(repo.worktree().join("dist/cache/output.bin"), "bin")?;

    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::remove_file(repo.worktree().join("lost-a.txt"))?;
    fs::remove_file(repo.worktree().join("dist/cache/output.bin"))?;

    snapshot_cmd()?
        .args([
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

#[test]
fn test_snapshot_targeted_restore_prompts_before_clobbering(
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    let first = snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .output()?;
    assert!(first.status.success());
    let snapshot = parse_snapshot_tag(&String::from_utf8(first.stdout)?);

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    snapshot_cmd()?
        .args(["restore", "--snapshot", &snapshot, "--path", "tracked.txt"])
        .write_stdin("n\n")
        .current_dir(repo.worktree())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Overwrite"));

    assert_eq!(
        fs::read_to_string(repo.worktree().join("tracked.txt"))?,
        "two"
    );

    snapshot_cmd()?
        .args(["restore", "--snapshot", &snapshot, "--path", "tracked.txt"])
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

#[test]
fn test_snapshot_status_reports_clean_and_changed() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    snapshot_cmd()?
        .args(["status", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::contains("clean"));

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    snapshot_cmd()?
        .args(["status", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::contains("changed"));

    Ok(())
}

#[test]
fn test_snapshot_status_double_quiet_uses_exit_code() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    fs::write(repo.worktree().join(".gitignore"), ".agt-snapshots/\n")?;
    fs::write(repo.worktree().join("tracked.txt"), "one")?;

    snapshot_cmd()?
        .args(["save"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    snapshot_cmd()?
        .args(["status", "-q", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    fs::write(repo.worktree().join("tracked.txt"), "two")?;

    snapshot_cmd()?
        .args(["status", "-q", "-q"])
        .current_dir(repo.worktree())
        .assert()
        .failure()
        .stdout(predicate::str::is_empty());

    Ok(())
}

#[test]
fn test_snapshot_save_honors_env_store_override() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;
    let custom_store = repo.repo_root().join("custom-snapshots.git");
    fs::write(
        repo.worktree().join(".gitignore"),
        "../custom-snapshots.git\n",
    )?;

    snapshot_cmd()?
        .args(["save"])
        .env("AGT_SNAPSHOT_STORE", &custom_store)
        .current_dir(repo.worktree())
        .assert()
        .success();

    assert!(custom_store.exists());
    Ok(())
}
