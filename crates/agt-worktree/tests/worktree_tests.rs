use assert_cmd::Command;
use gix::commit::NO_PARENT_IDS;
use gix::object::tree::EntryKind;
use gix_object::Tree;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn agt_worktree_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("agt-worktree").to_path_buf()
}

fn init_bare_repo_with_commit(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

#[test]
fn add_and_remove_worktree_basic() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let bare = tmp.path().join("repo.git");
    fs::create_dir_all(&bare)?;
    init_bare_repo_with_commit(&bare)?;

    let worktree = tmp.path().join("wt");
    let name = "wt";

    Command::new(agt_worktree_bin())
        .args([
            "add",
            "--git-dir",
            bare.to_str().unwrap(),
            "--worktree",
            worktree.to_str().unwrap(),
            "--name",
            name,
            "--branch",
            "refs/heads/main",
        ])
        .assert()
        .success();

    assert!(worktree.join("README.md").exists());
    let git_file = fs::read_to_string(worktree.join(".git"))?;
    assert!(git_file.contains("gitdir:"));

    let admin_dir = bare.join("worktrees").join(name);
    assert!(admin_dir.join("HEAD").exists());
    assert!(admin_dir.join("commondir").exists());
    assert!(admin_dir.join("gitdir").exists());
    assert!(admin_dir.join("index").exists());

    Command::new(agt_worktree_bin())
        .args([
            "remove",
            "--git-dir",
            bare.to_str().unwrap(),
            "--worktree",
            worktree.to_str().unwrap(),
            "--name",
            name,
        ])
        .assert()
        .success();

    assert!(!worktree.exists());
    assert!(!admin_dir.exists());

    Ok(())
}
