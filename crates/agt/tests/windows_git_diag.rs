use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn configured_git() -> Option<PathBuf> {
    std::env::var("AGT_TEST_REAL_GIT")
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

#[test]
fn test_windows_git_runner_resolution_and_passthrough() -> Result<(), Box<dyn std::error::Error>> {
    let Some(git) = configured_git() else {
        eprintln!("skipping windows git diagnostic: AGT_TEST_REAL_GIT not set");
        return Ok(());
    };

    let version = Command::new(&git).arg("--version").output()?;
    assert!(version.status.success(), "git --version failed");

    let exec_path = Command::new(&git).arg("--exec-path").output()?;
    assert!(exec_path.status.success(), "git --exec-path failed");
    assert!(!String::from_utf8(exec_path.stdout)?.trim().is_empty());

    let tmp = TempDir::new()?;
    let repo = tmp.path().join("diag-repo");

    let init = Command::new(&git)
        .args(["init", repo.to_str().unwrap()])
        .output()?;
    assert!(init.status.success(), "git init failed: {:?}", init);

    let git_dir = Command::new(&git)
        .args(["-C", repo.to_str().unwrap(), "rev-parse", "--git-dir"])
        .output()?;
    assert!(
        git_dir.status.success(),
        "git rev-parse failed: {:?}",
        git_dir
    );
    assert_eq!(String::from_utf8(git_dir.stdout)?.trim(), ".git");

    fs::write(repo.join("README.md"), "diagnostic\n")?;

    let add = Command::new(&git)
        .args(["-C", repo.to_str().unwrap(), "add", "README.md"])
        .output()?;
    assert!(add.status.success(), "git add failed: {:?}", add);

    let status = Command::new(&git)
        .args(["-C", repo.to_str().unwrap(), "status", "--short"])
        .output()?;
    assert!(status.status.success(), "git status failed: {:?}", status);
    assert!(String::from_utf8(status.stdout)?.contains("README.md"));

    Ok(())
}
