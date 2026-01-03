use crate::config::AgtConfig;
use anyhow::{Context, Result};
use gix::object::tree::EntryKind;
use gix::Repository;
use gix_object::TreeRefIter;
use gix_path::from_byte_slice;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct SessionMetadata {
    user_branch: String,
    worktree: String,
}

pub fn run(
    repo: &Repository,
    worktree_path: &Path,
    session_id: &str,
    override_timestamp: Option<i64>,
    dry_run: bool,
    config: &AgtConfig,
) -> Result<()> {
    let branch_name = format!("{}{}", config.branch_prefix, session_id);

    // Resolve session metadata up front so autocommit is always scoped to the
    // session worktree (avoids cross-contamination when invoked from repo root).
    let session_meta_path = repo
        .common_dir()
        .join("agt/sessions")
        .join(format!("{session_id}.json"));
    let session_meta_raw = std::fs::read_to_string(&session_meta_path)
        .with_context(|| format!("Failed to read {}", session_meta_path.display()))?;
    let session: SessionMetadata = serde_json::from_str(&session_meta_raw)
        .with_context(|| format!("Failed to parse {}", session_meta_path.display()))?;

    let mut session_worktree = PathBuf::from(&session.worktree);
    if !session_worktree.exists() {
        // Back-compat/migration: older session metadata may contain a worktree path
        // relative to the main worktree, but autocommit is often invoked from
        // inside the session worktree via `-C`. In that case, prefer the current
        // directory if it matches the session id.
        if worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == session_id)
            && worktree_path.exists()
        {
            session_worktree = worktree_path.to_path_buf();

            if let Ok(abs) = std::fs::canonicalize(&session_worktree) {
                if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&session_meta_raw) {
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert(
                            "worktree".to_string(),
                            serde_json::Value::String(abs.display().to_string()),
                        );
                        let _ = std::fs::write(&session_meta_path, serde_json::to_string(&v)?);
                    }
                }
            }
        } else {
            anyhow::bail!(
                "Session worktree does not exist: {}",
                session_worktree.display()
            );
        }
    }

    // 1. Read last timestamp (use common_dir for worktree support)
    let timestamp_file = repo.common_dir().join("agt/timestamps").join(session_id);
    let last_timestamp: u64 = std::fs::read_to_string(&timestamp_file)?.trim().parse()?;

    let scan_timestamp: u64 = match override_timestamp {
        Some(t) if t < 0 => anyhow::bail!("timestamp override must be >= 0"),
        Some(t) => t as u64,
        None => last_timestamp,
    };

    // 3. Build tree from files (not using index)
    let agent_branch_ref = format!("refs/heads/{branch_name}");
    let parent1 = repo
        .find_reference(&agent_branch_ref)?
        .peel_to_commit()
        .context("Failed to resolve agent session branch")?;

    // 2. Scan for modified files
    let snapshot_delta =
        compute_snapshot_delta(repo, &session_worktree, &parent1.tree()?, scan_timestamp)?;

    if snapshot_delta.changed.is_empty() && snapshot_delta.deleted.is_empty() {
        println!("No modified files since last autocommit");
        return Ok(());
    }

    if dry_run {
        let mut changed: Vec<_> = snapshot_delta.changed.iter().collect();
        changed.sort();
        let mut deleted: Vec<_> = snapshot_delta.deleted.iter().collect();
        deleted.sort();

        println!(
            "Would commit {} files, delete {} files:",
            snapshot_delta.changed.len(),
            snapshot_delta.deleted.len()
        );
        for f in changed {
            println!("  M {}", f.display());
        }
        for f in deleted {
            println!("  D {}", f.display());
        }
        return Ok(());
    }

    // Parent2 is the user branch head recorded at fork time (not the agent worktree HEAD).
    let parent2_id = repo
        .find_reference(&session.user_branch)?
        .peel_to_commit()
        .context("Failed to resolve user branch for parent2")?
        .id;

    // Explicitly reject detached/unborn HEAD inside the agent worktree.
    let worktree_repo = gix::open(&session_worktree)?;
    let head = worktree_repo.head()?;
    if head.is_detached() {
        anyhow::bail!("Detached HEAD in agent worktree is not supported");
    }
    if head.is_unborn() {
        anyhow::bail!("Unborn HEAD in agent worktree is not supported");
    }

    let tree_id = build_tree_from_delta(repo, &parent1, &session_worktree, &snapshot_delta)?;

    let signature = gix::actor::SignatureRef {
        name: gix::bstr::BStr::new("agt"),
        email: gix::bstr::BStr::new(&config.agent_email),
        time: gix::date::Time::now_local_or_utc(),
    };

    let commit_id = repo.commit_as(
        signature,
        signature,
        agent_branch_ref.as_str(),
        "agt autocommit",
        tree_id,
        [parent1.id, parent2_id],
    )?;

    // 6. Update timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    std::fs::write(&timestamp_file, now.to_string())?;

    println!(
        "Created commit {} with {} files",
        commit_id,
        snapshot_delta.changed.len()
    );

    Ok(())
}

fn build_tree_from_delta(
    repo: &Repository,
    base_commit: &gix::Commit<'_>,
    worktree: &Path,
    delta: &SnapshotDelta,
) -> Result<gix::ObjectId> {
    let base_tree_id = base_commit.tree_id()?.detach();
    let mut editor = repo.edit_tree(base_tree_id)?;

    for relative_path in &delta.deleted {
        editor.remove(path_for_tree(relative_path))?;
    }

    for relative_path in &delta.changed {
        let full_path = worktree.join(relative_path);
        let metadata = std::fs::symlink_metadata(&full_path)
            .with_context(|| format!("Failed to stat {}", full_path.display()))?;
        let file_type = metadata.file_type();

        let (entry_kind, data) = if file_type.is_symlink() {
            let target = std::fs::read_link(&full_path)
                .with_context(|| format!("Failed to read symlink {}", full_path.display()))?;
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
            let data = std::fs::read(&full_path)
                .with_context(|| format!("Failed to read {}", full_path.display()))?;
            (kind, data)
        };

        let blob_id = repo.write_blob(data)?;
        editor.upsert(path_for_tree(relative_path), entry_kind, blob_id.detach())?;
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

struct SnapshotDelta {
    changed: HashSet<PathBuf>,
    deleted: HashSet<PathBuf>,
}

fn compute_snapshot_delta(
    repo: &Repository,
    worktree: &Path,
    base_tree: &gix::Tree<'_>,
    since_timestamp: u64,
) -> Result<SnapshotDelta> {
    let threshold =
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(since_timestamp);

    let mut current_paths = HashSet::new();
    let mut changed = HashSet::new();

    for entry in jwalk::WalkDir::new(worktree)
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
        let rel_path = path.strip_prefix(worktree)?.to_path_buf();
        let file_type = entry.file_type();
        if file_type.is_dir() {
            continue;
        }

        current_paths.insert(rel_path.clone());
        let metadata = std::fs::symlink_metadata(&path)?;
        let mtime = metadata.modified()?;
        if mtime >= threshold {
            changed.insert(rel_path);
        }
    }

    let mut base_paths = HashSet::new();
    collect_tree_paths(repo, base_tree.id, PathBuf::new(), &mut base_paths)?;

    let deleted = base_paths
        .difference(&current_paths)
        .cloned()
        .collect::<HashSet<_>>();

    Ok(SnapshotDelta { changed, deleted })
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
    use super::{compute_snapshot_delta, path_for_tree, SnapshotDelta};
    use anyhow::Result;
    use gix::commit::NO_PARENT_IDS;
    use gix::object::tree::EntryKind;
    use gix_object::Tree;
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

    fn assert_contains(set: &std::collections::HashSet<PathBuf>, path: &Path) {
        assert!(
            set.contains(path),
            "expected set to contain {}",
            path.display()
        );
    }

    #[test]
    fn snapshot_delta_detects_add_modify_delete() -> Result<()> {
        let tmp = init_repo()?;
        let repo = gix::open(tmp.path())?;
        let commit = repo.head()?.peel_to_commit_in_place()?;
        let base_tree = commit.tree()?;

        fs::write(tmp.path().join("c.txt"), "c")?;
        fs::write(tmp.path().join("dir/b.txt"), "")?;
        fs::remove_file(tmp.path().join("a.txt"))?;

        let SnapshotDelta { changed, deleted } =
            compute_snapshot_delta(&repo, tmp.path(), &base_tree, 0)?;

        assert_contains(&changed, Path::new("c.txt"));
        assert_contains(&changed, Path::new("dir/b.txt"));
        assert_contains(&deleted, Path::new("a.txt"));

        Ok(())
    }

    #[test]
    fn snapshot_delta_reports_rename_as_delete_add() -> Result<()> {
        let tmp = init_repo()?;
        let repo = gix::open(tmp.path())?;
        let commit = repo.head()?.peel_to_commit_in_place()?;
        let base_tree = commit.tree()?;

        fs::rename(tmp.path().join("dir/b.txt"), tmp.path().join("dir/c.txt"))?;

        let SnapshotDelta { changed, deleted } =
            compute_snapshot_delta(&repo, tmp.path(), &base_tree, 0)?;

        assert_contains(&changed, Path::new("dir/c.txt"));
        assert_contains(&deleted, Path::new("dir/b.txt"));

        Ok(())
    }

    #[test]
    fn snapshot_delta_detects_nested_move_and_truncate() -> Result<()> {
        let tmp = init_repo()?;
        let repo = gix::open(tmp.path())?;
        let commit = repo.head()?.peel_to_commit_in_place()?;
        let base_tree = commit.tree()?;

        fs::create_dir_all(tmp.path().join("nested"))?;
        fs::rename(tmp.path().join("a.txt"), tmp.path().join("nested/a.txt"))?;
        fs::write(tmp.path().join("nested/a.txt"), "")?;

        let SnapshotDelta { changed, deleted } =
            compute_snapshot_delta(&repo, tmp.path(), &base_tree, 0)?;

        assert_contains(&changed, Path::new("nested/a.txt"));
        assert_contains(&deleted, Path::new("a.txt"));

        Ok(())
    }
}
