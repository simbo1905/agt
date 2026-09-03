//! Git-style PATH delegation tests:
//! - agt mode: `agt snapshot ...` / `agt setup ...` delegate to `agt-snapshot` on PATH
//! - git mode: unknown subcommands delegate to `git-<name>` helpers on PATH
//!
//! The delegation mechanism execs external scripts, so these tests are unix-only.

use assert_cmd::Command as AgtCommand;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn agt_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("agt").to_path_buf()
}

#[cfg(unix)]
fn write_shell_script(
    dir: &Path,
    name: &str,
    body: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
    let mut perms = fs::metadata(&path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms)?;
    Ok(path)
}

/// PATH with `dir` prepended, so delegation finds fakes but real tools stay reachable.
#[cfg(unix)]
fn prepended_path(dir: &Path) -> String {
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let joined = std::env::join_paths(
        std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&existing)),
    )
    .expect("joined PATH is valid");
    joined.to_string_lossy().into_owned()
}

#[cfg(unix)]
fn make_fake_git(tmp: &TempDir) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    // Keep the fake git out of the repo working directory and off the child's PATH.
    let bin_dir = tmp.path().join("_bin");
    fs::create_dir_all(&bin_dir)?;
    let fake_git = bin_dir.join("git");
    fs::copy(agt_bin(), &fake_git)?;
    let mut perms = fs::metadata(&fake_git)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_git, perms)?;
    Ok(fake_git)
}

#[cfg(unix)]
fn init_plain_repo(tmp: &TempDir) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let repo_dir = tmp.path().join("repo");
    fs::create_dir_all(&repo_dir)?;
    let status = StdCommand::new("git")
        .current_dir(&repo_dir)
        .args(["init", "-q"])
        .status()?;
    assert!(status.success(), "git init failed");
    Ok(repo_dir)
}

#[cfg(unix)]
fn find_real_git() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = StdCommand::new("which").arg("git").output()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(
        output.status.success() && !path.is_empty(),
        "git not on PATH"
    );
    Ok(PathBuf::from(path))
}

#[cfg(unix)]
#[test]
fn test_agt_snapshot_delegates_to_agt_snapshot_on_path() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let helper_dir = tmp.path().join("helpers");
    write_shell_script(
        &helper_dir,
        "agt-snapshot",
        "printf 'args:'; printf ' [%s]' \"$@\"; printf '\\n'; echo out-marker; echo err-marker 1>&2; exit 0",
    )?;

    AgtCommand::new(agt_bin())
        .env("PATH", prepended_path(&helper_dir))
        .current_dir(tmp.path())
        .args([
            "snapshot",
            "save",
            "--target",
            "/tmp/x",
            "-m",
            "hello world",
        ])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("[save]")
                .and(predicates::str::contains("[--target]"))
                .and(predicates::str::contains("[/tmp/x]"))
                .and(predicates::str::contains("[-m]"))
                .and(predicates::str::contains("[hello world]")),
        )
        .stdout(predicates::str::contains("out-marker"))
        .stderr(predicates::str::contains("err-marker"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_agt_snapshot_delegation_propagates_nonzero_exit_code(
) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let helper_dir = tmp.path().join("helpers");
    write_shell_script(&helper_dir, "agt-snapshot", "exit 7")?;

    AgtCommand::new(agt_bin())
        .env("PATH", prepended_path(&helper_dir))
        .current_dir(tmp.path())
        .args(["snapshot", "status"])
        .assert()
        .code(7);
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_agt_snapshot_without_agt_snapshot_on_path_errors() -> Result<(), Box<dyn std::error::Error>>
{
    let tmp = TempDir::new()?;
    let empty_path = TempDir::new()?;

    AgtCommand::new(agt_bin())
        .env("PATH", empty_path.path())
        .current_dir(tmp.path())
        .args(["snapshot", "save"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "agt: 'snapshot' is not an agt command. See 'agt --help'.",
        ))
        .stderr(predicates::str::contains(
            "'agt-snapshot' was not found on PATH",
        ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_agt_setup_without_agt_snapshot_on_path_errors() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let empty_path = TempDir::new()?;

    AgtCommand::new(agt_bin())
        .env("PATH", empty_path.path())
        .current_dir(tmp.path())
        .args(["setup"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "agt: 'setup' is not an agt command. See 'agt --help'.",
        ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_git_mode_unknown_subcommand_runs_git_name_helper() -> Result<(), Box<dyn std::error::Error>>
{
    let tmp = TempDir::new()?;
    let repo_dir = init_plain_repo(&tmp)?;
    let fake_git = make_fake_git(&tmp)?;
    let helper_dir = tmp.path().join("helpers");
    write_shell_script(
        &helper_dir,
        "git-xyzhelper",
        "printf 'helper:'; printf ' [%s]' \"$@\"; printf '\\n'; exit 0",
    )?;

    AgtCommand::new(&fake_git)
        .env("PATH", prepended_path(&helper_dir))
        .env("AGT_GIT_PATH", find_real_git()?)
        .current_dir(&repo_dir)
        .args(["xyzhelper", "--flag", "value"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("helper:")
                .and(predicates::str::contains("[--flag]"))
                .and(predicates::str::contains("[value]")),
        );
    Ok(())
}

#[cfg(unix)]
#[test]
fn test_git_mode_unknown_subcommand_without_helper_falls_back_to_host_git(
) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let repo_dir = init_plain_repo(&tmp)?;
    let fake_git = make_fake_git(&tmp)?;
    let empty_path = TempDir::new()?;

    // Nothing on PATH: the helper lookup misses and the host-git passthrough
    // fails exactly as before the delegation existed. Redirect AGT_LOG_PATH to
    // a temp file so the intentionally-failed delegation does not pollute a
    // CI-checked log inherited through the environment (see integration_tests).
    let log_path = tmp.path().join("agt-fallback.log");
    AgtCommand::new(&fake_git)
        .env("PATH", empty_path.path())
        .env("AGT_GIT_PATH", find_real_git()?)
        .env("AGT_LOG_PATH", &log_path)
        .current_dir(&repo_dir)
        .args(["xyz"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("is not a git command"));
    Ok(())
}
