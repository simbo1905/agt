use assert_cmd::Command as AgtCommand;
use gix::commit::NO_PARENT_IDS;
use gix::object::tree::EntryKind;
use gix_object::Tree;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use tempfile::TempDir;

fn agt_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("agt").to_path_buf()
}

fn agt_cmd_with_gix() -> Result<AgtCommand, Box<dyn std::error::Error>> {
    let mut cmd = AgtCommand::new(agt_bin());
    let gix_path = ensure_gix()?;
    cmd.env("AGT_GIX_PATH", gix_path);
    cmd.env("AGT_WORKTREE_PATH", ensure_worktree_tool()?);
    Ok(cmd)
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
    Ok(AgtCommand::new(git_path))
}

#[cfg(unix)]
#[test]
fn test_passthrough_uses_gix_path() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    gix::init(tmp.path())?;

    let gix_path = tmp.path().join("gix");
    fs::write(&gix_path, "#!/bin/sh\necho GIX-SENTINEL\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&gix_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&gix_path, perms)?;
    }

    let output = git_mode_cmd(&tmp)?
        .args(["branch"])
        .env("AGT_GIX_PATH", &gix_path)
        .current_dir(tmp.path())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("GIX-SENTINEL"));

    Ok(())
}

#[test]
fn test_init_creates_bare_repo() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    // Create a local bare repo to clone from
    let source = tmp.path().join("source");
    fs::create_dir_all(&source)?;
    init_bare_repo_with_commit(&source)?;

    let target = tmp.path().join("target");
    fs::create_dir_all(&target)?;

    // Test agt init
    agt_cmd_with_gix()?
        .args(["init", source.to_str().unwrap()])
        .current_dir(&target)
        .assert()
        .success();

    // Verify bare repo exists
    assert!(target.join("source.git").exists());
    assert!(target.join("source").exists());

    // Verify .git file points to admin directory (linked worktree pattern)
    let git_file = target.join("source/.git");
    assert!(git_file.exists());
    let git_content = fs::read_to_string(&git_file)?;
    // Should point to source.git/worktrees/source/
    assert!(git_content.contains("source.git/worktrees/source"));

    // Verify worktree admin directory exists with proper metadata
    let admin_dir = target.join("source.git/worktrees/source");
    assert!(admin_dir.exists());
    assert!(admin_dir.join("HEAD").exists());
    assert!(admin_dir.join("commondir").exists());
    assert!(admin_dir.join("gitdir").exists());
    assert!(admin_dir.join("index").exists());

    // Verify agt directory exists
    assert!(target.join("source.git/agt").exists());
    assert!(target.join("source.git/agt/timestamps").exists());
    assert!(target.join("source.git/agt/sessions").exists());

    Ok(())
}

#[test]
fn test_init_sets_default_config_and_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    let source = tmp.path().join("source");
    fs::create_dir_all(&source)?;
    init_bare_repo_with_commit(&source)?;

    let target = tmp.path().join("target");
    fs::create_dir_all(&target)?;

    agt_cmd_with_gix()?
        .args(["init", source.to_str().unwrap()])
        .current_dir(&target)
        .assert()
        .success();

    let config_path = target.join("source.git/config");
    let config_contents = fs::read_to_string(&config_path)?;
    // Verify agt config is set
    assert!(config_contents.contains("[agt]"));
    assert!(config_contents.contains("agentEmail = agt@local"));
    assert!(config_contents.contains("branchPrefix = agtsessions/"));
    // Repo should remain bare (no bare=false or worktree setting)
    assert!(config_contents.contains("bare = true"));
    assert!(!config_contents.contains("bare = false"));

    // Verify worktree is usable (gix can open it and sees work_dir)
    let worktree = target.join("source");
    let repo = gix::open(&worktree)?;
    assert!(repo.work_dir().is_some());

    Ok(())
}

#[test]
fn test_fork_creates_branch_and_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;

    // Test agt fork
    agt_cmd_with_gix()?
        .args(["fork", "--session-id", "test-session"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    // Verify branch exists
    let gix_repo = gix::open(repo.worktree())?;
    let branch_name = "agtsessions/test-session";
    assert!(gix_repo
        .find_reference(&format!("refs/heads/{branch_name}"))
        .is_ok());

    // Verify worktree exists
    assert!(repo.worktree().join("sessions/test-session").exists());

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

    // Create a test file
    let session_path = repo.worktree().join("sessions/test-session");
    fs::write(session_path.join("test.txt"), "test content")?;

    // Get current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    // Test agt autocommit with timestamp override
    agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            &(now - 3600).to_string(), // Force inclusion
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    // Verify commit has two parents and contains the file
    let repo = gix::open(repo.worktree())?;
    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    assert_eq!(commit.parent_ids().count(), 2);

    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("test.txt"))?;
    assert!(entry.is_some());

    Ok(())
}

#[test]
fn test_autocommit_dry_run_output_includes_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;

    let session_path = repo.worktree().join("sessions/test-session");
    fs::write(session_path.join("dryrun.txt"), "x")?;

    let output = agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
            "--dry-run",
        ])
        .current_dir(repo.worktree())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Dry run: session test-session"));
    assert!(stdout.contains("Worktree:"));
    assert!(stdout.contains(session_path.to_str().unwrap()));
    assert!(stdout.contains("M "));

    Ok(())
}

#[test]
fn test_autocommit_parent2_is_user_branch_head() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;

    let session_path = repo.worktree().join("sessions/test-session");
    fs::write(session_path.join("p2.txt"), "p2")?;

    agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    // A second autocommit ensures parent1 differs from parent2 after the agent branch advances.
    fs::write(session_path.join("p2b.txt"), "p2b")?;
    agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(repo.worktree())
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
    let session_path = repo.worktree().join("sessions/test-session");

    fs::write(session_path.join("delete-me.txt"), "to be deleted")?;

    agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    fs::remove_file(session_path.join("delete-me.txt"))?;

    agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    let repo = gix::open(repo.worktree())?;
    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("delete-me.txt"))?;
    assert!(entry.is_none());

    Ok(())
}

#[cfg(unix)]
#[test]
fn test_autocommit_preserves_symlink_entries() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_session()?;
    let session_path = repo.worktree().join("sessions/test-session");

    fs::write(session_path.join("target.txt"), "target")?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("target.txt", session_path.join("link.txt"))?;

    agt_cmd_with_gix()?
        .args([
            "autocommit",
            "-C",
            session_path.to_str().unwrap(),
            "--session-id",
            "test-session",
            "--timestamp",
            "0",
        ])
        .current_dir(repo.worktree())
        .assert()
        .success();

    let repo = gix::open(repo.worktree())?;
    let mut branch_ref = repo.find_reference("refs/heads/agtsessions/test-session")?;
    let commit = branch_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.lookup_entry_by_path(std::path::Path::new("link.txt"))?;
    let entry = entry.expect("expected symlink entry");
    assert_eq!(entry.mode().kind(), gix::object::tree::EntryKind::Link);

    Ok(())
}

#[test]
fn test_git_mode_filters_branches() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_agent_branch()?;

    // Test git branch command (should filter agent branches)
    let output = git_mode_cmd(repo.tmp())?
        .args(["branch"])
        .env("AGT_GIX_PATH", ensure_gix()?)
        .current_dir(repo.worktree())
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    assert!(!stdout.contains("agtsessions/"));

    Ok(())
}

#[test]
fn test_agt_mode_shows_all_branches() -> Result<(), Box<dyn std::error::Error>> {
    let repo = setup_repo_with_agent_branch()?;

    // Test agt branch command (should show all branches)
    let output = agt_cmd_with_gix()?
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
}

impl TestRepo {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn tmp(&self) -> &TempDir {
        &self.tmp
    }
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

fn setup_repo_with_session() -> Result<TestRepo, Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;

    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;

    // Create session
    agt_cmd_with_gix()?
        .args(["fork", "--session-id", "test-session"])
        .current_dir(repo.worktree())
        .assert()
        .success();

    Ok(repo)
}

fn setup_repo_with_agent_branch() -> Result<TestRepo, Box<dyn std::error::Error>> {
    let repo = setup_basic_repo()?;

    write_agt_config(repo.worktree(), "agt@local", "agtsessions/")?;

    let gix_repo = gix::open(&repo.bare)?;
    fs::write(repo.worktree().join("agent-file.txt"), "agent content")?;
    commit_worktree(
        &gix_repo,
        repo.worktree(),
        "refs/heads/agtsessions/test-agent",
        "Agent commit",
        "agt@local",
    )?;

    Ok(repo)
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

fn ensure_gix() -> Result<PathBuf, Box<dyn std::error::Error>> {
    static GIX_PATH: OnceLock<PathBuf> = OnceLock::new();

    if let Some(path) = GIX_PATH.get() {
        return Ok(path.to_path_buf());
    }

    let path = find_or_build_gix()?;
    let _ = GIX_PATH.set(path.clone());
    Ok(path)
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

fn find_or_build_gix() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("AGT_GIX_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let root = repo_root();
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let gix_name = format!("gix{exe_suffix}");
    let release = root.join("vendor/gitoxide/target/release").join(&gix_name);
    let debug = root.join("vendor/gitoxide/target/debug").join(&gix_name);

    if release.exists() {
        return Ok(release);
    }
    if debug.exists() {
        return Ok(debug);
    }

    let status = Command::new("cargo")
        .args([
            "build",
            "--manifest-path",
            root.join("vendor/gitoxide/Cargo.toml").to_str().unwrap(),
            "-p",
            "gix",
            "--release",
        ])
        .status()?;
    if !status.success() {
        return Err("failed to build vendored gix".into());
    }

    if release.exists() {
        Ok(release)
    } else {
        Err("vendored gix binary not found after build".into())
    }
}
