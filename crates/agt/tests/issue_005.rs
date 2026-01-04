use assert_cmd::Command as AgtCommand;
use gix::bstr::ByteSlice;
use gix::commit::NO_PARENT_IDS;
use gix::object::tree::EntryKind;
use gix_object::Tree;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

// --- Copied Helpers ---

fn agt_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("agt").to_path_buf()
}

fn git_mode_cmd(tmp: &TempDir) -> Result<AgtCommand, Box<dyn std::error::Error>> {
    let git_path = tmp.path().join("git");
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

struct TestRepo {
    tmp: TempDir,
    worktree: PathBuf,
    #[allow(dead_code)]
    bare: PathBuf,
}

impl TestRepo {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn tmp(&self) -> &TempDir {
        &self.tmp
    }
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("failed to resolve repo root")
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

fn find_real_git() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Check AGT_GIT_PATH env var first
    if let Ok(path) = std::env::var("AGT_GIT_PATH") {
        let candidate = PathBuf::from(&path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // Find git in PATH
    let output = Command::new("which")
        .arg("git")
        .output()?;
    
    if output.status.success() {
        let path = String::from_utf8(output.stdout)?.trim().to_string();
        return Ok(PathBuf::from(path));
    }

    // Fallback locations
    for path in ["/usr/bin/git", "/usr/local/bin/git", "/opt/homebrew/bin/git"] {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    Err("Could not find git binary".into())
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

fn setup_basic_repo() -> Result<TestRepo, Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    let bare = tmp.path().join("repo.git");
    fs::create_dir_all(&bare)?;
    init_bare_repo_with_commit(&bare)?;

    let worktree = tmp.path().join("repo");
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
    })
}

// --- New Tests for Issue 005 ---

#[test]
fn test_git_add_all_respects_gitignore() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    let worktree = repo.worktree();

    fs::write(worktree.join(".gitignore"), "ignore_me.txt\n")?;
    fs::write(worktree.join("ignore_me.txt"), "secret")?;
    fs::write(worktree.join("include_me.txt"), "public")?;

    // Verify file exists
    assert!(worktree.join("ignore_me.txt").exists());

    // git add -A
    git_mode_cmd(repo.tmp())?
        .args(["add", "-A"])
        .current_dir(worktree)
        .assert()
        .success();

    // Commit
    git_mode_cmd(repo.tmp())?
        .args(["commit", "-m", "add all"])
        .current_dir(worktree)
        .assert()
        .success();

    // Inspect commit
    let repo_gix = gix::open(worktree)?;
    let head = repo_gix.head()?.peel_to_commit_in_place()?.id;
    let commit = repo_gix.find_object(head)?.into_commit();
    let tree = commit.tree()?;

    // Check include_me.txt is present
    assert!(tree.lookup_entry_by_path(Path::new("include_me.txt"))?.is_some());
    // Check .gitignore is present
    assert!(tree.lookup_entry_by_path(Path::new(".gitignore"))?.is_some());

    // Check ignore_me.txt is ABSENT
    // This assertion is expected to FAIL if git_porcelain.rs doesn't handle ignore
    let ignore_entry = tree.lookup_entry_by_path(Path::new("ignore_me.txt"))?;
    if ignore_entry.is_some() {
        // Failing explicitly to show the bug
        panic!("FIXME: ignore_me.txt was committed despite being in .gitignore!");
    }

    Ok(())
}

#[test]
fn test_git_add_all_stages_tracked_modifications() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    let worktree = repo.worktree();

    let readme_path = worktree.join("README.md");
    fs::write(&readme_path, "updated contents")?;

    git_mode_cmd(repo.tmp())?
        .args(["add", "-A"])
        .current_dir(worktree)
        .assert()
        .success();

    git_mode_cmd(repo.tmp())?
        .args(["commit", "-m", "update tracked file"])
        .current_dir(worktree)
        .assert()
        .success();

    let repo_gix = gix::open(worktree)?;
    let head = repo_gix.head()?.peel_to_commit_in_place()?.id;
    let commit = repo_gix.find_object(head)?.into_commit();
    let tree = commit.tree()?;
    let entry = tree
        .lookup_entry_by_path(Path::new("README.md"))?
        .expect("expected README in commit");
    let blob = entry.object()?.into_blob();
    assert_eq!(blob.data.as_slice(), b"updated contents");

    Ok(())
}

#[test]
fn test_git_commit_multiple_messages() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;
    let worktree = repo.worktree();
    fs::write(worktree.join("file.txt"), "content")?;

    git_mode_cmd(repo.tmp())?
        .args(["add", "file.txt"])
        .current_dir(worktree)
        .assert()
        .success();

    git_mode_cmd(repo.tmp())?
        .args(["commit", "-m", "Title", "-m", "Body paragraph"])
        .current_dir(worktree)
        .assert()
        .success();

    let repo_gix = gix::open(worktree)?;
    let head = repo_gix.head()?.peel_to_commit_in_place()?.id;
    let commit = repo_gix.find_object(head)?.into_commit();
    
    assert_eq!(commit.message()?.summary().as_bstr(), "Title");
    assert!(commit.message()?.body.unwrap().to_string().contains("Body paragraph"));

    Ok(())
}
