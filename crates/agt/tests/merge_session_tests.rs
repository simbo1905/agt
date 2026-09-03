#![cfg(unix)]
//! Integration tests for `agt session merge` (issue #11).
//!
//! Self-contained suite: builds its own bare repo + linked main worktree +
//! agent session sandbox, then exercises fast-forward merges, non-fast-forward
//! merges authored with the user identity, refusal paths, conflict aborts, and
//! `--dry-run` reporting.

use assert_cmd::Command as AgtCommand;
use gix::bstr::ByteSlice;
use gix::commit::NO_PARENT_IDS;
use gix::object::tree::EntryKind;
use gix_object::Tree;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const SESSION_ID: &str = "merge-session";
const USER_EMAIL: &str = "user@example.com";
const AGENT_EMAIL: &str = "agent@local";

// `tmp` is never read directly but must be kept alive: dropping the TempDir
// would delete the whole test repository while tests still use it.
#[allow(dead_code)]
struct TestRepo {
    tmp: TempDir,
    worktree: PathBuf,
    bare: PathBuf,
    root: PathBuf,
    git: PathBuf,
}

impl TestRepo {
    fn worktree(&self) -> &Path {
        &self.worktree
    }

    fn sandbox(&self) -> PathBuf {
        self.root.join("sessions").join(SESSION_ID).join("sandbox")
    }

    fn git(&self) -> Command {
        let mut cmd = Command::new(&self.git);
        cmd.current_dir(&self.worktree);
        cmd
    }

    fn git_stdout(&self, args: &[&str]) -> String {
        let output = self.git().args(args).output().expect("git should run");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn head_commit(&self) -> gix::ObjectId {
        let sha = self.git_stdout(&["rev-parse", "HEAD"]);
        sha.parse().expect("valid object id")
    }
}

fn agt_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("agt").to_path_buf()
}

fn agt_cmd(repo: &TestRepo) -> AgtCommand {
    let mut cmd = AgtCommand::new(agt_bin());
    cmd.env("AGT_GIT_PATH", &repo.git);
    cmd.env("AGT_WORKTREE_PATH", ensure_worktree_tool());
    // Pin the commit identity for host git so the tests never depend on the
    // developer's global git config nor on GIT_AUTHOR_*/GIT_COMMITTER_*
    // exported by an enclosing `git commit` running this repo's pre-commit
    // hook (environment beats repo config in git's precedence).
    cmd.env("GIT_AUTHOR_NAME", "Test User");
    cmd.env("GIT_AUTHOR_EMAIL", USER_EMAIL);
    cmd.env("GIT_COMMITTER_NAME", "Test User");
    cmd.env("GIT_COMMITTER_EMAIL", USER_EMAIL);
    cmd.env_remove("EMAIL");
    cmd
}

fn find_real_git() -> PathBuf {
    for candidate in [
        std::env::var("AGT_TEST_REAL_GIT").ok(),
        std::env::var("AGT_GIT_PATH").ok(),
    ]
    .into_iter()
    .flatten()
    {
        let candidate = PathBuf::from(&candidate);
        if candidate.exists() {
            return candidate;
        }
    }
    let output = Command::new("which")
        .arg("git")
        .output()
        .expect("which git");
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    panic!("could not find a real git binary for tests");
}

fn ensure_worktree_tool() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("failed to resolve repo root");
    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let name = format!("agt-worktree{exe_suffix}");
    let release = root.join("target/release").join(&name);
    if release.exists() {
        return release;
    }
    let debug = root.join("target/debug").join(&name);
    if debug.exists() {
        return debug;
    }
    let status = Command::new("cargo")
        .args(["build", "-p", "agt-worktree"])
        .status()
        .expect("failed to run cargo build");
    assert!(status.success(), "failed to build agt-worktree");
    debug
}

fn setup_repo() -> TestRepo {
    let tmp = TempDir::new().expect("tempdir");
    let bare = tmp.path().join("repo.git");
    init_bare_repo_with_commit(&bare);

    let root = tmp.path().to_path_buf();
    let worktree = root.join("main");
    let status = Command::new(ensure_worktree_tool())
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
        .status()
        .expect("agt-worktree add should run");
    assert!(status.success(), "agt-worktree add failed");

    let repo = TestRepo {
        tmp,
        worktree,
        bare,
        root,
        git: find_real_git(),
    };
    write_repo_config(&repo.bare);
    repo
}

fn init_bare_repo_with_commit(path: &Path) {
    let repo = gix::ThreadSafeRepository::init(
        path,
        gix::create::Kind::Bare,
        gix::create::Options::default(),
    )
    .expect("init bare")
    .to_thread_local();

    let blob_id = repo.write_blob(b"# Test Repo").expect("blob").detach();
    let tree = Tree {
        entries: vec![gix_object::tree::Entry {
            mode: EntryKind::Blob.into(),
            filename: gix_object::bstr::BString::from("README.md"),
            oid: blob_id,
        }],
    };
    let tree_id = repo.write_object(tree).expect("tree").detach();

    let signature = gix::actor::SignatureRef {
        name: gix::bstr::BStr::new("Test User"),
        email: gix::bstr::BStr::new(USER_EMAIL),
        time: gix::date::Time::now_local_or_utc(),
    };
    repo.commit_as(
        signature,
        signature,
        "refs/heads/main",
        "Initial commit",
        tree_id,
        NO_PARENT_IDS,
    )
    .expect("initial commit");

    fs::write(path.join("HEAD"), "ref: refs/heads/main\n").expect("HEAD");
}

/// Writes git identity + agt settings into the shared repo config so merges
/// commit with a deterministic user identity. The worktree repo has its own
/// config file, so both sides get the identity (the merge commit is created
/// by host git running in the worktree, which otherwise falls back to the
/// developer's global git identity and makes the assertion environment-
/// dependent).
fn write_repo_config(bare: &Path) {
    let fragment = format!(
        "[user]\n\tname = Test User\n\temail = {USER_EMAIL}\n[agt]\n\tagentEmail = {AGENT_EMAIL}\n\tbranchPrefix = agtsessions/\n\n"
    );
    let config_path = bare.join("config");
    let mut contents = fs::read_to_string(&config_path).unwrap_or_default();
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&fragment);
    fs::write(&config_path, contents).expect("write repo config");
    if let Some(worktree_config_parent) = bare.parent().map(|p| p.join("main").join(".git")) {
        let worktree_config = worktree_config_parent.join("config");
        if worktree_config_parent.is_dir() {
            let mut wt_contents = fs::read_to_string(&worktree_config).unwrap_or_default();
            if !wt_contents.ends_with('\n') {
                wt_contents.push('\n');
            }
            wt_contents.push_str(&fragment);
            fs::write(&worktree_config, wt_contents).expect("write worktree repo config");
        }
    }
}

fn create_session(repo: &TestRepo) {
    agt_cmd(repo)
        .args(["session", "new", "--id", SESSION_ID])
        .current_dir(repo.worktree())
        .assert()
        .success();
}

fn autocommit(repo: &TestRepo) {
    agt_cmd(repo)
        .args(["autocommit", "--session-id", SESSION_ID, "--timestamp", "0"])
        .current_dir(repo.sandbox())
        .assert()
        .success();
}

/// Commits every change in the main worktree with the host git binary so the
/// index stays in sync with the worktree.
fn commit_all(repo: &TestRepo, message: &str) {
    for args in [
        vec!["add", "-A"],
        vec!["commit", "-m", message, "--no-gpg-sign"],
    ] {
        let output = repo.git().args(&args).output().expect("git should run");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn merge(repo: &TestRepo, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["session", "merge", SESSION_ID];
    args.extend_from_slice(extra);
    agt_cmd(repo)
        .args(&args)
        .current_dir(repo.worktree())
        .output()
        .expect("agt session merge should run")
}

fn shadow_branch(repo: &TestRepo) -> gix::ObjectId {
    let sha = repo.git_stdout(&["rev-parse", &format!("refs/heads/agtsessions/{SESSION_ID}")]);
    sha.parse().expect("valid object id")
}

fn worktree_status(repo: &TestRepo) -> String {
    repo.git_stdout(&["status", "--porcelain"])
}

// --- Tests -----------------------------------------------------------------

#[test]
fn merge_fast_forwards_user_branch_to_shadow_head() {
    let repo = setup_repo();
    create_session(&repo);

    fs::write(repo.sandbox().join("agent-work.txt"), "agent work").unwrap();
    autocommit(&repo);

    let before = repo.head_commit();
    let shadow = shadow_branch(&repo);
    assert_ne!(
        before, shadow,
        "precondition: shadow head differs from user head"
    );

    let output = merge(&repo, &[]);
    assert!(
        output.status.success(),
        "merge failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Fast-forward: user branch head now equals the shadow branch head.
    let after = repo.head_commit();
    assert_eq!(after, shadow);

    // The agent's file landed in the user worktree.
    assert_eq!(
        fs::read_to_string(repo.worktree().join("sandbox/agent-work.txt")).unwrap(),
        "agent work"
    );

    // Summary reports the merge commit and session id.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(SESSION_ID), "stdout: {stdout}");
    assert!(stdout.contains(&after.to_string()), "stdout: {stdout}");

    // The shadow branch is retained after a successful merge.
    assert_eq!(shadow_branch(&repo), shadow);
    assert_eq!(worktree_status(&repo), "");
}

#[test]
fn merge_commit_parents_and_identity_are_correct() {
    let repo = setup_repo();
    create_session(&repo);

    fs::write(repo.sandbox().join("agent-file.txt"), "from agent").unwrap();
    autocommit(&repo);
    let shadow = shadow_branch(&repo);

    let user_head_before = repo.head_commit();
    fs::write(repo.worktree().join("user-file.txt"), "from user").unwrap();
    commit_all(&repo, "user work");
    let user_head_after = repo.head_commit();

    let output = merge(&repo, &[]);
    assert!(
        output.status.success(),
        "merge failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let gix_repo = gix::open(repo.worktree()).unwrap();
    let head = gix_repo
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap();

    let parents: Vec<_> = head.parent_ids().map(|id| id.to_owned()).collect();
    assert_eq!(parents.len(), 2, "merge commit must have two parents");
    assert_eq!(
        parents[0], user_head_after,
        "first parent is the user branch head"
    );
    assert_eq!(
        parents[1], shadow,
        "second parent is the shadow branch head"
    );
    assert_ne!(head.id, user_head_before);

    // Authored as the USER identity, never the agent identity.
    let author = head.author().unwrap();
    assert_eq!(author.email.to_str().unwrap(), USER_EMAIL);
    assert_ne!(author.email.to_str().unwrap(), AGENT_EMAIL);
    assert_eq!(
        head.message().unwrap().summary().to_str().unwrap(),
        format!("Merge session {SESSION_ID}")
    );

    // Both sides of the merge survive in the worktree.
    assert_eq!(
        fs::read_to_string(repo.worktree().join("user-file.txt")).unwrap(),
        "from user"
    );
    assert_eq!(
        fs::read_to_string(repo.worktree().join("sandbox/agent-file.txt")).unwrap(),
        "from agent"
    );

    // Shadow branch retained.
    assert_eq!(shadow_branch(&repo), shadow);
    assert_eq!(worktree_status(&repo), "");
}

#[test]
fn merge_refuses_dirty_worktree() {
    let repo = setup_repo();
    create_session(&repo);

    fs::write(repo.sandbox().join("dirty.txt"), "agent work").unwrap();
    autocommit(&repo);

    fs::write(repo.worktree().join("uncommitted.txt"), "dirty").unwrap();
    let before = repo.head_commit();

    let output = merge(&repo, &[]);
    assert_eq!(output.status.code(), Some(1), "must refuse with exit 1");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("uncommitted changes"), "stderr: {stderr}");

    // Nothing was merged.
    assert_eq!(repo.head_commit(), before);
    assert!(!repo.worktree().join("sandbox/dirty.txt").exists());
}

#[test]
fn merge_refuses_unknown_session_id() {
    let repo = setup_repo();

    let output = agt_cmd(&repo)
        .args(["session", "merge", "no-such-session"])
        .current_dir(repo.worktree())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "must refuse with exit 1");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown session"), "stderr: {stderr}");
}

#[test]
fn merge_conflict_aborts_cleanly_leaving_worktree_clean() {
    let repo = setup_repo();
    create_session(&repo);

    // Agent writes a file, it is merged (fast-forward) into the user branch.
    fs::write(repo.sandbox().join("note.txt"), "agent v1").unwrap();
    autocommit(&repo);
    let output = merge(&repo, &[]);
    assert!(output.status.success(), "first merge should fast-forward");

    // Agent edits the same file again and autocommits.
    fs::write(repo.sandbox().join("note.txt"), "agent v2").unwrap();
    autocommit(&repo);
    let shadow = shadow_branch(&repo);

    // The user edits the same path on the user branch after that autocommit.
    fs::write(repo.worktree().join("sandbox/note.txt"), "user edit").unwrap();
    commit_all(&repo, "user edits note");
    let user_head = repo.head_commit();

    let output = merge(&repo, &[]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "conflicting merge must exit 1"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("aborted"), "stderr: {stderr}");
    assert!(stderr.contains(SESSION_ID), "stderr: {stderr}");

    // The worktree is left clean: no conflict markers, no merge in progress.
    assert_eq!(worktree_status(&repo), "");
    let merge_head = repo
        .git()
        .args(["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .output()
        .unwrap();
    assert!(
        !merge_head.status.success(),
        "MERGE_HEAD must not exist after abort"
    );

    assert_eq!(
        repo.head_commit(),
        user_head,
        "user branch head must be untouched"
    );
    assert_eq!(
        fs::read_to_string(repo.worktree().join("sandbox/note.txt")).unwrap(),
        "user edit"
    );

    // The session still holds the work on the shadow branch.
    assert_eq!(shadow_branch(&repo), shadow);
}

#[test]
fn dry_run_reports_fast_forward_without_touching_worktree() {
    let repo = setup_repo();
    create_session(&repo);

    fs::write(repo.sandbox().join("ff.txt"), "ff work").unwrap();
    autocommit(&repo);

    let before = repo.head_commit();
    let output = merge(&repo, &["--dry-run"]);
    assert!(output.status.success(), "dry run should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fast-forward"), "stdout: {stdout}");
    assert!(stdout.contains(SESSION_ID), "stdout: {stdout}");

    // Nothing changed.
    assert_eq!(repo.head_commit(), before);
    assert!(!repo.worktree().join("sandbox/ff.txt").exists());
    assert_eq!(worktree_status(&repo), "");
}

#[test]
fn dry_run_reports_merge_commit_without_touching_worktree() {
    let repo = setup_repo();
    create_session(&repo);

    fs::write(repo.sandbox().join("mc.txt"), "agent work").unwrap();
    autocommit(&repo);

    fs::write(repo.worktree().join("user-file.txt"), "from user").unwrap();
    commit_all(&repo, "user work");

    let before = repo.head_commit();
    let output = merge(&repo, &["--dry-run"]);
    assert!(output.status.success(), "dry run should exit 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("merge commit"), "stdout: {stdout}");
    assert!(stdout.contains(SESSION_ID), "stdout: {stdout}");

    assert_eq!(repo.head_commit(), before);
    assert!(!repo.worktree().join("sandbox/mc.txt").exists());
    assert_eq!(worktree_status(&repo), "");
}

#[test]
fn dry_run_conflict_exits_nonzero_without_touching_worktree() {
    let repo = setup_repo();
    create_session(&repo);

    // Set up divergent edits to the same path (agent vs user).
    fs::write(repo.sandbox().join("note.txt"), "agent v1").unwrap();
    autocommit(&repo);
    let output = merge(&repo, &[]);
    assert!(output.status.success(), "first merge should fast-forward");

    fs::write(repo.sandbox().join("note.txt"), "agent v2").unwrap();
    autocommit(&repo);

    fs::write(repo.worktree().join("sandbox/note.txt"), "user edit").unwrap();
    commit_all(&repo, "user edits note");

    let before = repo.head_commit();
    let output = merge(&repo, &["--dry-run"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "conflicting dry run must exit 1"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("conflict"), "stderr: {stderr}");

    assert_eq!(repo.head_commit(), before);
    assert_eq!(
        fs::read_to_string(repo.worktree().join("sandbox/note.txt")).unwrap(),
        "user edit"
    );
    assert_eq!(worktree_status(&repo), "");
}
