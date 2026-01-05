use anyhow::{Context, Result};
use gix::object::tree::EntryKind;
use gix::{bstr::BStr, bstr::BString, bstr::ByteSlice, Repository};
use gix_index::entry::{Flags, Mode, Stage, Stat};
use gix_index::File as IndexFile;
use gix_path::from_byte_slice;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

pub fn maybe_handle_git_command(args: &[String], repo: &Repository) -> Result<bool> {
    match args.first().map(String::as_str) {
        Some("add") => {
            git_add(args, repo)?;
            Ok(true)
        }
        Some("commit") => {
            git_commit(args, repo)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn git_add(args: &[String], repo: &Repository) -> Result<()> {
    let work_dir = repo
        .work_dir()
        .context("No working directory found for git add")?;

    let AddArgs { all, update, paths } = parse_add_args(args)?;

    let mut index = repo
        .index_or_load_from_head_or_empty()
        .context("Failed to load index")?
        .into_owned();

    if all {
        stage_all(&mut index, repo, work_dir)?;
    } else if update {
        stage_tracked(&mut index, repo, work_dir)?;
    } else {
        stage_paths(&mut index, repo, work_dir, &paths)?;
    }

    index.sort_entries();
    index.write(Default::default())?;

    Ok(())
}

fn git_commit(args: &[String], repo: &Repository) -> Result<()> {
    let message = parse_commit_message(args)?;
    let mut index = repo
        .open_index()
        .context("Failed to open index (nothing staged?)")?;

    let tree_id = write_tree_from_index(repo, &mut index)?;

    let mut head = repo.head()?;
    if head.is_detached() {
        anyhow::bail!("Detached HEAD is not supported for git commit");
    }
    let ref_name = head
        .referent_name()
        .ok_or_else(|| anyhow::anyhow!("No branch reference for git commit"))?
        .to_owned();
    let ref_name = ref_name
        .as_bstr()
        .to_str()
        .context("Non-utf8 branch name")?;

    let parents = if head.is_unborn() {
        Vec::new()
    } else {
        vec![head.peel_to_commit_in_place()?.id]
    };

    let (name, email) = signature_from_config(repo);
    let signature = gix::actor::SignatureRef {
        name: BStr::new(name.as_bytes()),
        email: BStr::new(email.as_bytes()),
        time: gix::date::Time::now_local_or_utc(),
    };
    let commit_id = repo.commit_as(signature, signature, ref_name, &message, tree_id, parents)?;

    println!("Created commit {commit_id}");
    Ok(())
}

struct AddArgs {
    all: bool,
    update: bool,
    paths: Vec<PathBuf>,
}

fn parse_add_args(args: &[String]) -> Result<AddArgs> {
    let mut all = false;
    let mut update = false;
    let mut paths = Vec::new();
    let mut after_dd = false;

    for arg in args.iter().skip(1) {
        if after_dd {
            paths.push(PathBuf::from(arg));
            continue;
        }

        match arg.as_str() {
            "-A" | "--all" => all = true,
            "-u" | "--update" => update = true,
            "--" => after_dd = true,
            _ if arg.starts_with('-') => {
                anyhow::bail!("Unsupported git add flag: {arg}");
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    if !all && !update && paths.is_empty() {
        anyhow::bail!("git add requires paths or -A/-u");
    }

    Ok(AddArgs { all, update, paths })
}

fn parse_commit_message(args: &[String]) -> Result<String> {
    let mut messages = Vec::new();
    let mut it = args.iter().skip(1).peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-m" | "--message" => {
                let msg = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected message after {arg}"))?;
                messages.push(msg.to_string());
            }
            _ if arg.starts_with('-') => {
                anyhow::bail!("Unsupported git commit flag: {arg}");
            }
            _ => {}
        }
    }

    if messages.is_empty() {
        anyhow::bail!("git commit requires -m/--message");
    }

    Ok(messages.join("\n\n"))
}

fn signature_from_config(repo: &Repository) -> (String, String) {
    let config = repo.config_snapshot();
    let name = config
        .string("user.name")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "agt".to_string());
    let email = config
        .string("user.email")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "agt@local".to_string());

    (name, email)
}

fn stage_all(index: &mut IndexFile, repo: &Repository, work_dir: &Path) -> Result<()> {
    // Stage all untracked files (respecting .gitignore)
    for rel_path in walk_worktree(work_dir, repo)? {
        stage_one(index, repo, work_dir, &rel_path)?;
    }

    // Re-stage all tracked files (to pick up modifications)
    let tracked: Vec<PathBuf> = index
        .entries()
        .iter()
        .map(|entry| from_byte_slice(entry.path(index)).to_path_buf())
        .collect();

    for rel_path in &tracked {
        stage_one(index, repo, work_dir, rel_path)?;
    }

    // Remove entries for deleted files
    let work_dir = work_dir.to_path_buf();
    index.remove_entries(|_idx, path, _entry| {
        let rel = from_byte_slice(path).to_path_buf();
        !work_dir.join(rel).exists()
    });

    Ok(())
}

fn stage_tracked(index: &mut IndexFile, repo: &Repository, work_dir: &Path) -> Result<()> {
    let tracked: Vec<PathBuf> = index
        .entries()
        .iter()
        .map(|entry| from_byte_slice(entry.path(index)).to_path_buf())
        .collect();

    for rel_path in tracked {
        stage_one(index, repo, work_dir, &rel_path)?;
    }

    Ok(())
}

fn stage_paths(
    index: &mut IndexFile,
    repo: &Repository,
    work_dir: &Path,
    paths: &[PathBuf],
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let work_dir_canon = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf());
    let mut seen = HashSet::new();

    for path in paths {
        let abs = if path.is_absolute() {
            path.clone()
        } else {
            cwd.join(path)
        };

        let rel = abs
            .strip_prefix(&work_dir_canon)
            .or_else(|_| abs.strip_prefix(work_dir))
            .with_context(|| format!("Path is outside repository: {}", abs.display()))?
            .to_path_buf();

        if seen.insert(rel.clone()) {
            stage_path_or_dir(index, repo, work_dir, &rel)?;
        }
    }

    Ok(())
}

fn stage_path_or_dir(
    index: &mut IndexFile,
    repo: &Repository,
    work_dir: &Path,
    rel_path: &Path,
) -> Result<()> {
    let full_path = work_dir.join(rel_path);
    if !full_path.exists() {
        remove_entry(index, rel_path);
        return Ok(());
    }

    let meta = gix_index::fs::Metadata::from_path_no_follow(&full_path)?;
    if meta.is_dir() {
        let root = full_path.clone();
        for rel in walk_path(&root, work_dir, repo)? {
            stage_one(index, repo, work_dir, &rel)?;
        }
        return Ok(());
    }

    stage_one(index, repo, work_dir, rel_path)
}

fn stage_one(
    index: &mut IndexFile,
    repo: &Repository,
    work_dir: &Path,
    rel_path: &Path,
) -> Result<()> {
    let full_path = work_dir.join(rel_path);
    if !full_path.exists() {
        remove_entry(index, rel_path);
        return Ok(());
    }

    let meta = gix_index::fs::Metadata::from_path_no_follow(&full_path)?;
    let (mode, data) = if meta.is_symlink() {
        let target = std::fs::read_link(&full_path)?;
        (
            Mode::SYMLINK,
            target
                .as_os_str()
                .to_string_lossy()
                .into_owned()
                .into_bytes(),
        )
    } else {
        let kind = if meta.is_executable() {
            Mode::FILE_EXECUTABLE
        } else {
            Mode::FILE
        };
        let data = std::fs::read(&full_path)?;
        (kind, data)
    };

    let blob_id = repo.write_blob(data)?.detach();
    let stat = Stat::from_fs(&meta)?;
    let path = path_to_bstring(rel_path);

    if let Some(entry) = index.entry_mut_by_path_and_stage(path.as_bstr(), Stage::Unconflicted) {
        entry.stat = stat;
        entry.id = blob_id;
        entry.flags = Flags::empty();
        entry.mode = mode;
    } else {
        index.dangerously_push_entry(stat, blob_id, Flags::empty(), mode, path.as_bstr());
    }

    Ok(())
}

fn remove_entry(index: &mut IndexFile, rel_path: &Path) {
    let path = path_to_bstring(rel_path);
    index.remove_entries(|_idx, entry_path, _entry| entry_path == path.as_bstr());
}

fn walk_worktree(work_dir: &Path, repo: &Repository) -> Result<Vec<PathBuf>> {
    walk_path(work_dir, work_dir, repo)
}

fn walk_path(root: &Path, work_dir: &Path, repo: &Repository) -> Result<Vec<PathBuf>> {
    // Use gix's dirwalk to respect .gitignore
    let index = repo.index_or_load_from_head_or_empty()?;
    let should_interrupt = AtomicBool::new(false);

    let options = repo
        .dirwalk_options()?
        .emit_tracked(false) // We want untracked files only
        .emit_untracked(gix::dir::walk::EmissionMode::Matching)
        .emit_ignored(None) // Don't emit ignored files
        .emit_pruned(false)
        .emit_empty_directories(false);

    let mut collector = gix::dir::walk::delegate::Collect::default();

    // Compute patterns to limit walk to the specified root
    let patterns: Vec<BString> = if root == work_dir {
        vec![]
    } else {
        let rel = root.strip_prefix(work_dir).with_context(|| {
            format!(
                "Root {} is not within work_dir {}",
                root.display(),
                work_dir.display()
            )
        })?;
        vec![path_to_bstring(rel)]
    };

    repo.dirwalk(&index, patterns, &should_interrupt, options, &mut collector)?;

    let mut paths = Vec::new();
    for (entry, _dir_status) in collector.into_entries_by_path() {
        // Only include files that are untracked
        if matches!(entry.status, gix::dir::entry::Status::Untracked) {
            let path = PathBuf::from(entry.rela_path.to_str_lossy().as_ref());
            paths.push(path);
        }
    }

    Ok(paths)
}

fn write_tree_from_index(repo: &Repository, index: &mut IndexFile) -> Result<gix::ObjectId> {
    let empty_tree_id = repo.write_object(gix_object::Tree::empty())?.detach();
    let mut editor = repo.edit_tree(empty_tree_id)?;

    for entry in index.entries().iter() {
        if entry.stage() != Stage::Unconflicted {
            continue;
        }
        if entry.flags.contains(Flags::REMOVE) {
            continue;
        }

        let kind = match entry.mode {
            Mode::FILE => EntryKind::Blob,
            Mode::FILE_EXECUTABLE => EntryKind::BlobExecutable,
            Mode::SYMLINK => EntryKind::Link,
            Mode::COMMIT => EntryKind::Commit,
            Mode::DIR => continue,
            _ => continue,
        };

        let path = path_from_bstr(entry.path(index));
        editor.upsert(path, kind, entry.id)?;
    }

    Ok(editor.write()?.detach())
}

fn path_to_bstring(path: &Path) -> BString {
    let mut buf = String::new();
    for (idx, component) in path.components().enumerate() {
        if idx > 0 {
            buf.push('/');
        }
        buf.push_str(&component.as_os_str().to_string_lossy());
    }
    BString::from(buf)
}

fn path_from_bstr(path: &BStr) -> String {
    from_byte_slice(path).to_string_lossy().into_owned()
}
