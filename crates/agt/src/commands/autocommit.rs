use crate::config::AgtConfig;
use anyhow::{Context, Result};
use gix::object::tree::EntryKind;
use gix::Repository;
use gix_object::TreeRefIter;
use gix_path::from_byte_slice;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct SessionMetadata {
    user_branch: String,
    sandbox: String,
}

struct SnapshotDelta {
    changed: HashMap<PathBuf, PathBuf>,
    deleted: HashSet<PathBuf>,
}

impl SnapshotDelta {
    fn new() -> Self {
        Self {
            changed: HashMap::new(),
            deleted: HashSet::new(),
        }
    }
}

pub fn run(
    repo: &Repository,
    cwd: &Path,
    session_id: &str,
    override_timestamp: Option<i64>,
    dry_run: bool,
    _siblings: Option<Vec<String>>,
    config: &AgtConfig,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    // Resolve session metadata
    let session_meta_path = repo
        .common_dir()
        .join("agt/sessions")
        .join(format!("{session_id}.json"));
    let session_meta_raw = std::fs::read_to_string(&session_meta_path)
        .with_context(|| format!("Failed to read {}", session_meta_path.display()))?;
    let session: SessionMetadata = serde_json::from_str(&session_meta_raw)
        .with_context(|| format!("Failed to parse {}", session_meta_path.display()))?;

    // Determine sandbox path
    let mut sandbox_path = PathBuf::from(&session.sandbox);
    if !sandbox_path.exists() {
        if cwd.ends_with("sandbox") && cwd.exists() {
            sandbox_path = cwd.to_path_buf();
        } else {
            anyhow::bail!("Sandbox does not exist: {}", sandbox_path.display());
        }
    }
    sandbox_path = std::fs::canonicalize(&sandbox_path)?;

    // Session folder is parent of sandbox
    let session_folder = sandbox_path
        .parent()
        .context("Sandbox has no parent")?
        .to_path_buf();

    // 1. Read last timestamp
    let timestamp_file = repo.common_dir().join("agt/timestamps").join(session_id);
    let last_timestamp: u64 = match std::fs::read_to_string(&timestamp_file) {
        Ok(s) => s.trim().parse()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e.into()),
    };

    let scan_timestamp: u64 = match override_timestamp {
        Some(t) if t < 0 => anyhow::bail!("timestamp override must be >= 0"),
        Some(t) => t as u64,
        None => last_timestamp,
    };

    // 2. Resolve parent shadow commit
    let shadow_branch_ref = format!("refs/heads/{branch_name}");
    let parent1 = repo
        .find_reference(&shadow_branch_ref)?
        .peel_to_commit()
        .context("Failed to resolve shadow branch")?;

    // 3. Compute Delta - scan entire session folder
    let mut base_paths = HashSet::new();
    collect_tree_paths(
        repo,
        parent1.tree_id()?.detach(),
        PathBuf::new(),
        &mut base_paths,
    )?;

    let delta = scan_changes(&session_folder, &base_paths, scan_timestamp)?;

    if delta.changed.is_empty() && delta.deleted.is_empty() {
        println!("No modified files since last autocommit");
        return Ok(());
    }

    if dry_run {
        let mut changed: Vec<_> = delta.changed.keys().collect();
        changed.sort();
        let mut deleted: Vec<_> = delta.deleted.iter().collect();
        deleted.sort();

        println!("Dry run: session {session_id}");
        println!(
            "  Would commit {} files, delete {} files:",
            delta.changed.len(),
            delta.deleted.len()
        );
        for f in changed {
            println!("  M {}", f.display());
        }
        for f in deleted {
            println!("  D {}", f.display());
        }
        return Ok(());
    }

    // 4. Create shadow commit
    let parent2_id = repo
        .find_reference(&session.user_branch)?
        .peel_to_commit()
        .context("Failed to resolve user branch for parent2")?
        .id;

    // Reject detached/unborn HEAD in sandbox
    let sandbox_repo = gix::open(&sandbox_path)?;
    let head = sandbox_repo.head()?;
    if head.is_detached() {
        anyhow::bail!("Detached HEAD in sandbox is not supported");
    }
    if head.is_unborn() {
        anyhow::bail!("Unborn HEAD in sandbox is not supported");
    }

    let tree_id = build_tree_from_delta(repo, &parent1, &delta)?;

    let signature = gix::actor::SignatureRef {
        name: gix::bstr::BStr::new("agt"),
        email: gix::bstr::BStr::new(&config.agent_email),
        time: gix::date::Time::now_local_or_utc(),
    };

    let commit_id = repo.commit_as(
        signature,
        signature,
        shadow_branch_ref.as_str(),
        "agt autocommit",
        tree_id,
        [parent1.id, parent2_id],
    )?;

    // Update timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    println!(
        "Created shadow commit {} with {} files",
        commit_id,
        delta.changed.len()
    );

    Ok(())
}

fn scan_changes(
    scan_root: &Path,
    base_paths: &HashSet<PathBuf>,
    since_timestamp: u64,
) -> Result<SnapshotDelta> {
    let threshold =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(since_timestamp);

    let mut current_paths = HashSet::new();
    let mut delta = SnapshotDelta::new();

    for entry in jwalk::WalkDir::new(scan_root)
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
        let path = entry.path();
        if entry.file_type().is_dir() {
            continue;
        }

        let rel_path = path.strip_prefix(scan_root)?.to_path_buf();
        current_paths.insert(rel_path.clone());

        let metadata = std::fs::symlink_metadata(&path)?;
        let mtime = metadata.modified()?;
        if mtime >= threshold {
            delta.changed.insert(rel_path, path);
        }
    }

    // Deletions: in base_paths but not in current scan
    for base_path in base_paths {
        if !current_paths.contains(base_path) {
            delta.deleted.insert(base_path.clone());
        }
    }

    Ok(delta)
}

fn build_tree_from_delta(
    repo: &Repository,
    base_commit: &gix::Commit<'_>,
    delta: &SnapshotDelta,
) -> Result<gix::ObjectId> {
    let base_tree_id = base_commit.tree_id()?.detach();
    let mut editor = repo.edit_tree(base_tree_id)?;

    for relative_path in &delta.deleted {
        editor.remove(path_for_tree(relative_path))?;
    }

    for (repo_path, fs_path) in &delta.changed {
        let metadata = std::fs::symlink_metadata(fs_path)
            .with_context(|| format!("Failed to stat {}", fs_path.display()))?;
        let file_type = metadata.file_type();

        let (entry_kind, data) = if file_type.is_symlink() {
            let target = std::fs::read_link(fs_path)
                .with_context(|| format!("Failed to read symlink {}", fs_path.display()))?;
            (
                EntryKind::Link,
                target
                    .as_os_str()
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes(),
            )
        } else {
            let mut kind = EntryKind::Blob;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o111 != 0 {
                    kind = EntryKind::BlobExecutable;
                }
            }
            let data = std::fs::read(fs_path)
                .with_context(|| format!("Failed to read {}", fs_path.display()))?;
            (kind, data)
        };

        let blob_id = repo.write_blob(data)?;
        editor.upsert(path_for_tree(repo_path), entry_kind, blob_id.detach())?;
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

fn collect_tree_paths(
    repo: &Repository,
    tree_id: gix::ObjectId,
    prefix: PathBuf,
    out: &mut HashSet<PathBuf>,
) -> Result<()> {
    let tree = repo.find_object(tree_id)?.try_into_tree()?;
    for entry in TreeRefIter::from_bytes(&tree.data).filter_map(Result::ok) {
        let name = from_byte_slice(entry.filename).to_owned();
        let mut path = prefix.clone();
        path.push(name);
        if entry.mode.kind() == EntryKind::Tree {
            collect_tree_paths(repo, entry.oid.to_owned(), path, out)?;
        } else {
            out.insert(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{path_for_tree, scan_changes, SnapshotDelta};
    use anyhow::Result;
    use gix::commit::NO_PARENT_IDS;
    use gix::object::tree::EntryKind;
    use gix_object::Tree;
    use std::collections::HashSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn init_repo() -> Result<TempDir> {
        let tmp = TempDir::new()?;
        let repo = gix::init(tmp.path())?;

        fs::write(tmp.path().join("a.txt"), "a")?;
        fs::create_dir_all(tmp.path().join("dir"))?;
        fs::write(tmp.path().join("dir/b.txt"), "b")?;

        let tree_id = write_tree_from_worktree(&repo, tmp.path())?;
        let signature = gix::actor::SignatureRef {
            name: gix::bstr::BStr::new("Test User"),
            email: gix::bstr::BStr::new("test@example.com"),
            time: gix::date::Time::now_local_or_utc(),
        };
        repo.commit_as(
            signature,
            signature,
            "refs/heads/main",
            "base",
            tree_id,
            NO_PARENT_IDS,
        )?;

        Ok(tmp)
    }

    fn write_tree_from_worktree(repo: &gix::Repository, root: &Path) -> Result<gix::ObjectId> {
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

    fn assert_changed_contains(delta: &SnapshotDelta, path: &Path) {
        assert!(
            delta.changed.contains_key(path),
            "expected changed to contain {}",
            path.display()
        );
    }

    fn assert_deleted_contains(delta: &SnapshotDelta, path: &Path) {
        assert!(
            delta.deleted.contains(path),
            "expected deleted to contain {}",
            path.display()
        );
    }

    #[test]
    fn scan_changes_detects_add_modify_delete() -> Result<()> {
        let tmp = init_repo()?;

        let mut base_paths = HashSet::new();
        base_paths.insert(PathBuf::from("a.txt"));
        base_paths.insert(PathBuf::from("dir/b.txt"));

        fs::write(tmp.path().join("c.txt"), "c")?;
        fs::write(tmp.path().join("dir/b.txt"), "")?;
        fs::remove_file(tmp.path().join("a.txt"))?;

        let delta = scan_changes(tmp.path(), &base_paths, 0)?;

        assert_changed_contains(&delta, Path::new("c.txt"));
        assert_changed_contains(&delta, Path::new("dir/b.txt"));
        assert_deleted_contains(&delta, Path::new("a.txt"));

        Ok(())
    }
}
