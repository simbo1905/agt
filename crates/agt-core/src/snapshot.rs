use crate::config::AgtConfig;
use crate::progress::{Progress, ProgressGating};
use anyhow::{bail, Context, Result};
use gix::bstr::BStr;
use gix::bstr::ByteSlice;
use gix::object::tree::EntryKind;
use gix_object::{compute_hash, Kind, Tree};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io::{self, BufRead, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const DEFAULT_STORE_DIR: &str = ".agt-snapshots";
const SNAPSHOT_REF: &str = "refs/heads/agt-snapshots";
const MANIFEST_PATH: &str = "meta/manifest.bin";
const PAYLOAD_PREFIX: &str = "payload";
const MANIFEST_MAGIC: &[u8; 8] = b"AGTSNP01";
const MANIFEST_VERSION: u32 = 1;
const SNAPSHOT_LIST_WIDTH: usize = 80;
const SNAPSHOT_IGNORE_FILE: &str = ".agt-snapshot-ignore";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotManifest {
    target_root: String,
    created_at_ns: u128,
    records: Vec<SnapshotRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRecord {
    path: String,
    kind: RecordKind,
    object_id: String,
    file_id: Option<String>,
    parent_file_id: Option<String>,
    size: u64,
    create_ts_ns: Option<u128>,
    modified_ts_ns: Option<u128>,
    change_ts_ns: Option<i128>,
    mode: Option<u32>,
    uid: Option<u32>,
    gid: Option<u32>,
    flags: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    File,
    Executable,
    Symlink,
}

// Portions of `PlatformFileId` and `get_file_id()` are adapted from the
// `file-id` crate: <https://crates.io/crates/file-id>
// Repository: <https://github.com/notify-rs/notify>
// Original copyright: Copyright (c) 2023 Notify Contributors
// License used here: MIT
// Modified from original to inline the small cross-platform file identity logic.
// See the root LICENSE file for the full third-party notice.
#[cfg(target_family = "unix")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum PlatformFileId {
    Inode { device_id: u64, inode_number: u64 },
}

#[cfg(target_family = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PlatformFileId;

#[derive(Clone, Debug)]
struct SnapshotIgnoreRules {
    repo_root: PathBuf,
    source_path: PathBuf,
    search: gix::ignore::Search,
    case: gix::glob::pattern::Case,
}

#[derive(Clone, Debug)]
struct SnapshotIgnoreMatch {
    ignored: bool,
    source: String,
    line_number: usize,
    pattern: String,
}

#[derive(Default)]
struct SnapshotCapture {
    records: Vec<SnapshotRecord>,
    ignored_file_count: usize,
}

pub fn save(
    config: &AgtConfig,
    target: &Path,
    store: Option<&Path>,
    message: Option<&str>,
    verbose: bool,
) -> Result<()> {
    ensure_supported_platform()?;
    let current_dir = std::env::current_dir()?;
    let target_root = target
        .canonicalize()
        .with_context(|| format!("Failed to resolve target {}", target.display()))?;
    let store_path = resolve_store_path(store, &current_dir)?;
    if let Some(work_dir) = discover_repo_root(&current_dir)? {
        warn_if_store_not_ignored(&work_dir, config, &store_path)?;
    }
    let snapshot_repo = open_or_init_snapshot_repo(&store_path)?;
    let ignore_rules = load_snapshot_ignore_rules(&target_root)?;

    // Progress is decided once: TTY-gated, quiet-suppressed, AGT_NO_PORCELINE
    // forced off. `-v` upgrades the TTY line to an accurate percentage by
    // paying for a metadata-only pre-walk; without it there is no extra walk.
    let gating = ProgressGating::from_env(false, verbose);
    let mut progress = Progress::new(gating);
    if gating.accurate {
        let (files, bytes) = prewalk_totals(&target_root, &store_path, ignore_rules.as_ref())?;
        progress.set_totals(files, bytes);
    }

    let created_at_ns = now_ns();
    let mut capture = capture_records(
        &snapshot_repo,
        &target_root,
        &store_path,
        ignore_rules.as_ref(),
        true,
        &mut progress,
    )?;
    progress.finish();
    capture
        .records
        .sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = SnapshotManifest {
        target_root: normalize_path(&target_root),
        created_at_ns,
        records: capture.records,
    };

    let tag_name = next_tag_name(&snapshot_repo, created_at_ns)?;
    let message = message
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("snapshot save for {}", target_root.display()));
    let commit_id = write_snapshot_commit(&snapshot_repo, config, &manifest, &message)?;
    let signature = signature(config);
    snapshot_repo.tag(
        &tag_name,
        commit_id.as_ref(),
        Kind::Commit,
        Some(signature),
        &message,
        gix_ref::transaction::PreviousValue::MustNotExist,
    )?;

    println!("Saved snapshot {tag_name}");
    println!("Store: {}", store_path.display());
    println!("Files: {}", manifest.records.len());
    println!("Ignored: {}", capture.ignored_file_count);
    Ok(())
}

pub fn setup(store: Option<&Path>) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let store_path = resolve_store_path(store, &current_dir)?;

    ensure_store_directory(&store_path)?;

    if let Some(repo_root) = discover_repo_root(&current_dir)? {
        ensure_store_ignored(&repo_root, &store_path)?;
    }

    println!("Snapshot store ready at {}", store_path.display());
    Ok(())
}

pub fn is_timestamp_tag(tag: &str) -> Option<u64> {
    let digits: String = tag.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 17 {
        digits.parse::<u64>().ok()
    } else {
        None
    }
}

pub fn check(before: &str, after: &str, store: Option<&Path>) -> Result<()> {
    ensure_supported_platform()?;
    let current_dir = std::env::current_dir()?;
    let store_path = resolve_store_path(store, &current_dir)?;
    let snapshot_repo = open_snapshot_repo(&store_path)?;

    let (sorted_before, sorted_after) = match (is_timestamp_tag(before), is_timestamp_tag(after)) {
        (Some(before_ts), Some(after_ts)) if before_ts > after_ts => {
            (after.to_string(), before.to_string())
        }
        _ => (before.to_string(), after.to_string()),
    };

    let before_manifest = load_manifest_for_tag(&snapshot_repo, &sorted_before)?;
    let after_manifest = load_manifest_for_tag(&snapshot_repo, &sorted_after)?;
    let diff = diff_manifests(&before_manifest, &after_manifest);

    println!("Comparing {} -> {}", sorted_before, sorted_after);
    emit_diff(&diff);
    Ok(())
}

pub fn status(store: Option<&Path>, ignored: bool, quiet: u8) -> Result<()> {
    ensure_supported_platform()?;
    let current_dir = std::env::current_dir()?;
    let store_path = resolve_store_path(store, &current_dir)?;
    if ignored {
        return status_ignored(&current_dir, &store_path, quiet);
    }

    let snapshot_repo = open_snapshot_repo(&store_path)?;
    let latest_tag = latest_snapshot_tag(&snapshot_repo)?.context("No snapshots found in store")?;
    let manifest = load_manifest_for_tag(&snapshot_repo, &latest_tag)?;
    let target_root = PathBuf::from(&manifest.target_root);
    let ignore_rules = load_snapshot_ignore_rules(&target_root)?;

    if quiet > 0 {
        let changed = has_changes_against_manifest(
            &snapshot_repo,
            &manifest,
            &target_root,
            &store_path,
            ignore_rules.as_ref(),
        )?;
        if quiet > 1 {
            if changed {
                std::process::exit(1);
            }
            return Ok(());
        }

        println!("{}", if changed { "changed" } else { "clean" });
        return Ok(());
    }

    let current_manifest = SnapshotManifest {
        target_root: manifest.target_root.clone(),
        created_at_ns: now_ns(),
        records: capture_records(
            &snapshot_repo,
            &target_root,
            &store_path,
            ignore_rules.as_ref(),
            false,
            &mut Progress::disabled(),
        )?
        .records,
    };
    let diff = diff_manifests(&manifest, &current_manifest);
    println!("Latest snapshot {latest_tag}");
    emit_diff(&diff);
    if diff.is_empty() {
        println!("Clean");
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckIgnoreFormat {
    Plain,
    Verbose,
    VerboseWithNonMatching,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckIgnoreTerminator {
    Newline,
    Nul,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckIgnoreSource {
    Paths,
    Stdin,
}

pub struct CheckIgnoreOptions {
    pub format: CheckIgnoreFormat,
    pub terminator: CheckIgnoreTerminator,
    pub source: CheckIgnoreSource,
}

impl CheckIgnoreOptions {
    fn from_flags(verbose: bool, non_matching: bool, nul: bool, stdin: bool) -> Self {
        let format = if verbose {
            if non_matching {
                CheckIgnoreFormat::VerboseWithNonMatching
            } else {
                CheckIgnoreFormat::Verbose
            }
        } else {
            CheckIgnoreFormat::Plain
        };
        let terminator = if nul {
            CheckIgnoreTerminator::Nul
        } else {
            CheckIgnoreTerminator::Newline
        };
        let source = if stdin {
            CheckIgnoreSource::Stdin
        } else {
            CheckIgnoreSource::Paths
        };
        Self {
            format,
            terminator,
            source,
        }
    }
}

/// Dispatches `check-ignore` from CLI flags, preserving the `git check-ignore`
/// exit-code contract (0/1/128).
pub fn check_ignore_flags(
    verbose: bool,
    non_matching: bool,
    nul: bool,
    stdin: bool,
    paths: &[String],
) -> Result<()> {
    if non_matching && !verbose {
        eprintln!("fatal: --non-matching is only valid with --verbose");
        std::process::exit(128);
    }
    check_ignore(
        CheckIgnoreOptions::from_flags(verbose, non_matching, nul, stdin),
        paths,
    )
}

pub fn check_ignore(options: CheckIgnoreOptions, paths: &[String]) -> Result<()> {
    ensure_supported_platform()?;

    let current_dir = std::env::current_dir()?;
    let ignore_rules = load_snapshot_ignore_rules(&current_dir)?;
    let is_nul = options.terminator == CheckIgnoreTerminator::Nul;
    let inputs =
        collect_check_ignore_inputs(options.source == CheckIgnoreSource::Stdin, is_nul, paths)?;

    let mut saw_ignored = false;
    let mut saw_match = false;
    let mut stdout = io::stdout().lock();

    for input in inputs {
        let match_ = ignore_rules
            .as_ref()
            .and_then(|rules| rules.match_for_input(&input, &current_dir));
        if let Some(info) = &match_ {
            if info.ignored {
                saw_ignored = true;
            }
            saw_match = true;
        }

        match options.format {
            CheckIgnoreFormat::Verbose | CheckIgnoreFormat::VerboseWithNonMatching => {
                if let Some(info) = match_ {
                    write_check_ignore_verbose(&mut stdout, &input, &info, is_nul)?;
                } else if options.format == CheckIgnoreFormat::VerboseWithNonMatching {
                    write_check_ignore_non_matching(&mut stdout, &input, is_nul)?;
                }
            }
            CheckIgnoreFormat::Plain => {
                if let Some(info) = match_ {
                    if info.ignored {
                        write_check_ignore_plain(&mut stdout, &input, is_nul)?;
                    }
                }
            }
        }
    }

    match options.format {
        CheckIgnoreFormat::Verbose | CheckIgnoreFormat::VerboseWithNonMatching => {
            if saw_match {
                return Ok(());
            }
        }
        CheckIgnoreFormat::Plain => {
            if saw_ignored {
                return Ok(());
            }
        }
    }
    std::process::exit(1)
}

pub fn list(store: Option<&Path>, quiet: bool) -> Result<()> {
    ensure_supported_platform()?;
    let current_dir = std::env::current_dir()?;
    let store_path = resolve_store_path(store, &current_dir)?;
    let snapshot_repo = open_snapshot_repo(&store_path)?;

    let mut tags: Vec<(String, Option<String>)> = Vec::new();
    for reference in snapshot_repo.references()?.tags()? {
        let reference = reference.map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let full_name = reference.name().as_bstr().to_string();
        let Some(short_name) = full_name.strip_prefix("refs/tags/") else {
            continue;
        };
        let message = reference.try_id().and_then(|id| {
            let object = id.object().ok()?;
            let tag = object.try_to_tag_ref().ok()?;
            let message = normalize_snapshot_message(tag.message.as_bstr().to_string());
            if message.is_empty() {
                None
            } else {
                Some(message)
            }
        });
        tags.push((short_name.to_string(), message));
    }

    tags.sort_by(|left, right| left.0.cmp(&right.0));
    for (tag, message) in &tags {
        if quiet {
            println!("{tag}");
        } else if let Some(message) = message {
            println!("{}", format_snapshot_list_line(tag, message));
        } else {
            println!("{tag}");
        }
    }
    println!("\n{} snapshot(s)", tags.len());
    Ok(())
}

fn status_ignored(target_root: &Path, store_path: &Path, quiet: u8) -> Result<()> {
    let ignored_paths = collect_ignored_paths(
        target_root,
        store_path,
        load_snapshot_ignore_rules(target_root)?.as_ref(),
    )?;
    if quiet > 1 {
        if ignored_paths.is_empty() {
            std::process::exit(1);
        }
        return Ok(());
    }

    for path in ignored_paths {
        println!("{path}");
    }
    Ok(())
}

fn collect_check_ignore_inputs(stdin: bool, nul: bool, paths: &[String]) -> Result<Vec<String>> {
    let mut inputs = paths.to_vec();
    if !stdin {
        return Ok(inputs);
    }

    if nul {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        inputs.extend(
            buf.split(|byte| *byte == 0)
                .filter(|entry| !entry.is_empty())
                .map(|entry| String::from_utf8_lossy(entry).into_owned()),
        );
        return Ok(inputs);
    }

    let mut line = String::new();
    let mut handle = io::stdin().lock();
    loop {
        line.clear();
        let bytes = handle.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if let Some(stripped) = line.strip_suffix('\n') {
            let stripped = stripped.strip_suffix('\r').unwrap_or(stripped);
            inputs.push(stripped.to_string());
        } else {
            inputs.push(line.clone());
        }
    }
    Ok(inputs)
}

fn write_check_ignore_plain(out: &mut dyn Write, input: &str, nul: bool) -> Result<()> {
    if nul {
        out.write_all(input.as_bytes())?;
        out.write_all(&[0])?;
    } else {
        writeln!(out, "{input}")?;
    }
    Ok(())
}

fn write_check_ignore_verbose(
    out: &mut dyn Write,
    input: &str,
    info: &SnapshotIgnoreMatch,
    nul: bool,
) -> Result<()> {
    if nul {
        out.write_all(info.source.as_bytes())?;
        out.write_all(&[0])?;
        out.write_all(info.line_number.to_string().as_bytes())?;
        out.write_all(&[0])?;
        out.write_all(info.pattern.as_bytes())?;
        out.write_all(&[0])?;
        out.write_all(input.as_bytes())?;
        out.write_all(&[0])?;
    } else {
        writeln!(
            out,
            "{}:{}:{}	{}",
            info.source, info.line_number, info.pattern, input
        )?;
    }
    Ok(())
}

fn write_check_ignore_non_matching(out: &mut dyn Write, input: &str, nul: bool) -> Result<()> {
    if nul {
        out.write_all(&[0, 0, 0])?;
        out.write_all(input.as_bytes())?;
        out.write_all(&[0])?;
    } else {
        writeln!(out, "::	{input}")?;
    }
    Ok(())
}

fn normalize_snapshot_message(message: String) -> String {
    message.replace('\n', " ").trim().to_string()
}

fn format_snapshot_list_line(tag: &str, message: &str) -> String {
    let full_line = format!("{tag} {message}");
    if full_line.chars().count() <= SNAPSHOT_LIST_WIDTH {
        return full_line;
    }

    let prefix = format!("{tag} ");
    let available = SNAPSHOT_LIST_WIDTH.saturating_sub(prefix.chars().count());
    if available <= 2 {
        return prefix;
    }

    let truncated_message: String = message.chars().take(available - 2).collect();
    format!("{prefix}{truncated_message}..")
}

pub fn restore(
    snapshot: &str,
    target: &Path,
    paths: &[PathBuf],
    store: Option<&Path>,
) -> Result<()> {
    ensure_supported_platform()?;
    let current_dir = std::env::current_dir()?;
    let store_path = resolve_store_path(store, &current_dir)?;
    let snapshot_repo = open_snapshot_repo(&store_path)?;

    if paths.is_empty() {
        ensure_latest_snapshot_is_clean_backup(&snapshot_repo, &store_path)?;
    }

    let ref_name = format!("refs/tags/{snapshot}");
    let mut tag_ref = snapshot_repo.find_reference(ref_name.as_str())?;
    let commit = tag_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let target_root = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    let payload_tree = tree
        .lookup_entry_by_path(Path::new(PAYLOAD_PREFIX))?
        .context("Snapshot payload tree missing")?;

    if paths.is_empty() {
        let mut expected_paths = HashSet::new();
        collect_tree_paths(
            &snapshot_repo,
            payload_tree.object_id(),
            PathBuf::new(),
            &mut expected_paths,
        )?;
        for path in &expected_paths {
            contained_join(&target_root, path.as_os_str())?;
        }
        remove_paths_not_in_snapshot(&target_root, &expected_paths, Path::new(""), &store_path)?;
        restore_tree_to_disk(
            &snapshot_repo,
            payload_tree.object_id(),
            &PathBuf::new(),
            &target_root,
        )?;
    } else {
        let mut restore_entries = Vec::new();
        for path in paths {
            let relative = contained_relative(path.as_os_str())?;
            let scope = path_to_tree(path);
            let scoped_path = format!("{PAYLOAD_PREFIX}/{scope}");
            let entry = tree
                .lookup_entry_by_path(Path::new(&scoped_path))?
                .with_context(|| format!("Snapshot path not found: {}", path.display()))?;
            let destination = target_root.join(&relative);
            if restore_would_clobber(
                &snapshot_repo,
                entry.object_id(),
                entry.mode().kind(),
                &relative,
                &target_root,
            )? && !confirm_overwrite(&destination)?
            {
                bail!("Restore cancelled by user")
            }
            restore_entries.push((entry.object_id(), entry.mode().kind(), relative));
        }

        for (object_id, kind, destination) in restore_entries {
            restore_entry_to_disk(&snapshot_repo, object_id, kind, &destination, &target_root)?;
        }
    }

    println!(
        "Restored snapshot {snapshot} into {}",
        target_root.display()
    );
    Ok(())
}

fn ensure_supported_platform() -> Result<()> {
    #[cfg(unix)]
    {
        Ok(())
    }

    #[cfg(not(unix))]
    {
        bail!("snapshot commands are not supported on Windows yet")
    }
}

fn resolve_store_path(store: Option<&Path>, current_dir: &Path) -> Result<PathBuf> {
    let candidate = match store {
        Some(path) => absolutize(path, current_dir)?,
        None => match std::env::var("AGT_SNAPSHOT_STORE") {
            Ok(path) => absolutize(Path::new(&path), current_dir)?,
            Err(_) => absolutize(Path::new(DEFAULT_STORE_DIR), current_dir)?,
        },
    };
    canonicalize_store_path(&candidate)
}

/// Canonicalizes the store path so that symlinked or `..`-spelled locations
/// always compare equal to the canonical paths produced by walking the
/// canonicalized target. Falls back to a lexical resolution when the store
/// does not exist yet (nothing along a non-existent tail can be a symlink,
/// so lexical `..`/`.` handling is exact there).
fn canonicalize_store_path(path: &Path) -> Result<PathBuf> {
    let mut resolved = PathBuf::new();
    let mut tail: Vec<OsString> = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                if tail.is_empty() {
                    resolved.push(component.as_os_str());
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if tail.pop().is_none() {
                    resolved.pop();
                }
            }
            Component::Normal(part) => {
                let candidate = resolved.join(part);
                match candidate.canonicalize() {
                    Ok(canonical) if tail.is_empty() => resolved = canonical,
                    _ => tail.push(part.to_os_string()),
                }
            }
        }
    }
    if resolved.as_os_str().is_empty() {
        bail!("Failed to resolve snapshot store path {}", path.display());
    }
    for part in tail {
        resolved.push(part);
    }
    Ok(resolved)
}

fn absolutize(path: &Path, base: &Path) -> Result<PathBuf> {
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    })
}

/// Joins a store- or CLI-controlled relative path onto `root`, rejecting any
/// component that could escape the target (absolute paths, `..`, `.`, Windows
/// prefixes, NUL bytes). All restore-time joins (writes AND the deletion set)
/// must go through this function.
pub fn contained_join(root: &Path, relative: &OsStr) -> Result<PathBuf> {
    Ok(root.join(contained_relative(relative)?))
}

fn contained_relative(relative: &OsStr) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    let mut joined = PathBuf::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(part) => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStrExt;
                    if part.as_bytes().contains(&0) {
                        bail!(
                            "Snapshot path contains NUL bytes: {}",
                            relative_path.display()
                        );
                    }
                }
                joined.push(part);
            }
            Component::RootDir
            | Component::Prefix(_)
            | Component::ParentDir
            | Component::CurDir => {
                bail!(
                    "Snapshot path escapes the target root: {}",
                    relative_path.display()
                );
            }
        }
    }
    if joined.as_os_str().is_empty() {
        bail!("Snapshot path is empty");
    }
    Ok(joined)
}

fn open_or_init_snapshot_repo(path: &Path) -> Result<gix::Repository> {
    if path.exists() {
        if let Ok(repo) = gix::open(path) {
            return Ok(repo);
        }
        if path.read_dir()?.next().is_none() {
            return Ok(gix::init_bare(path)?);
        }
        bail!(
            "Snapshot store {} exists but is not a bare Git repository",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(gix::init_bare(path)?)
}

fn ensure_store_directory(path: &Path) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            return Ok(());
        }
        bail!(
            "Snapshot store {} exists but is not a directory",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn open_snapshot_repo(path: &Path) -> Result<gix::Repository> {
    gix::open(path).with_context(|| format!("Failed to open snapshot store {}", path.display()))
}

fn warn_if_store_not_ignored(work_dir: &Path, config: &AgtConfig, store_path: &Path) -> Result<()> {
    let Ok(repo) = gix::open(work_dir) else {
        return Ok(());
    };
    let work_dir = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf());
    let store_abs = store_path
        .canonicalize()
        .unwrap_or_else(|_| store_path.to_path_buf());
    if !store_abs.starts_with(&work_dir) {
        return Ok(());
    }
    let rel = match store_abs.strip_prefix(&work_dir) {
        Ok(path) => path,
        Err(_) => return Ok(()),
    };

    let git_dir = repo.git_dir();
    let status = Command::new(&config.git_path)
        .current_dir(&work_dir)
        .args([
            "--git-dir",
            &git_dir.to_string_lossy(),
            "--work-tree",
            &work_dir.to_string_lossy(),
            "check-ignore",
            "-q",
            "--",
            &rel.to_string_lossy(),
        ])
        .status();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) if status.code() == Some(1) => {
            eprintln!(
                "Warning: snapshot store {} is not ignored by Git",
                store_path.display()
            );
            Ok(())
        }
        _ => {
            eprintln!(
                "Warning: unable to determine whether snapshot store {} is ignored by Git",
                store_path.display()
            );
            Ok(())
        }
    }
}

fn discover_repo_root(current_dir: &Path) -> Result<Option<PathBuf>> {
    match gix::discover(current_dir) {
        Ok(repo) => Ok(repo
            .work_dir()
            .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))),
        Err(_) => Ok(None),
    }
}

fn ensure_store_ignored(repo_root: &Path, store_path: &Path) -> Result<()> {
    let store_abs = store_path
        .canonicalize()
        .unwrap_or_else(|_| store_path.to_path_buf());
    if !store_abs.starts_with(repo_root) {
        return Ok(());
    }

    let rel = store_abs
        .strip_prefix(repo_root)
        .with_context(|| format!("{} is outside {}", store_abs.display(), repo_root.display()))?;
    if rel.as_os_str().is_empty() {
        bail!("Snapshot store cannot be the repository root")
    }

    let ignore_entry = ignore_entry_for(rel);
    let gitignore_path = repo_root.join(".gitignore");
    let mut lines = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
            .lines()
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if lines.iter().any(|line| line.trim() == ignore_entry) {
        return Ok(());
    }

    lines.push(ignore_entry.to_string());
    let mut contents = lines.join("\n");
    contents.push('\n');
    fs::write(gitignore_path, contents)?;
    Ok(())
}

fn ignore_entry_for(path: &Path) -> String {
    let normalized = normalize_rel_path(path);
    format!("{normalized}/")
}

fn capture_records(
    repo: &gix::Repository,
    target_root: &Path,
    store_path: &Path,
    ignore_rules: Option<&SnapshotIgnoreRules>,
    write_blobs: bool,
    progress: &mut Progress,
) -> Result<SnapshotCapture> {
    let store_for_walk = store_path.to_path_buf();
    let mut capture = SnapshotCapture::default();

    for entry in jwalk::WalkDir::new(target_root)
        .skip_hidden(false)
        .process_read_dir(move |_depth, path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| {
                    dir_entry.file_name != OsStr::new(".git")
                        && dir_entry.file_name != OsStr::new(DEFAULT_STORE_DIR)
                        && path.join(&dir_entry.file_name) != store_for_walk
                })
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if path.starts_with(store_path) {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)?;
        let rel_path = path
            .strip_prefix(target_root)
            .with_context(|| format!("{} is outside {}", path.display(), target_root.display()))?;
        if ignore_rules
            .and_then(|rules| rules.match_relative_path(rel_path, metadata.file_type().is_dir()))
            .is_some_and(|match_| match_.ignored)
        {
            if !entry.file_type().is_dir() {
                capture.ignored_file_count += 1;
            }
            continue;
        }
        if !entry.file_type().is_dir() {
            let record = build_record(repo, target_root, rel_path, &path, &metadata, write_blobs)?;
            progress.tick(record.size);
            capture.records.push(record);
        }
    }

    Ok(capture)
}

/// Metadata-only pre-walk that counts the files (and bytes) a save would
/// capture, so `-v` progress can show an accurate percentage. Mirrors the
/// filtering of `capture_records` exactly.
fn prewalk_totals(
    target_root: &Path,
    store_path: &Path,
    ignore_rules: Option<&SnapshotIgnoreRules>,
) -> Result<(u64, u64)> {
    let store_for_walk = store_path.to_path_buf();
    let mut files = 0_u64;
    let mut bytes = 0_u64;

    for entry in jwalk::WalkDir::new(target_root)
        .skip_hidden(false)
        .process_read_dir(move |_depth, path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| {
                    dir_entry.file_name != OsStr::new(".git")
                        && dir_entry.file_name != OsStr::new(DEFAULT_STORE_DIR)
                        && path.join(&dir_entry.file_name) != store_for_walk
                })
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        if path.starts_with(store_path) {
            continue;
        }
        let rel_path = path
            .strip_prefix(target_root)
            .with_context(|| format!("{} is outside {}", path.display(), target_root.display()))?;
        if ignore_rules
            .and_then(|rules| rules.match_relative_path(rel_path, entry.file_type().is_dir()))
            .is_some_and(|match_| match_.ignored)
        {
            continue;
        }
        files += 1;
        bytes += fs::symlink_metadata(&path)?.len();
    }

    Ok((files, bytes))
}

fn build_record(
    repo: &gix::Repository,
    target_root: &Path,
    rel_path: &Path,
    abs_path: &Path,
    metadata: &Metadata,
    write_blobs: bool,
) -> Result<SnapshotRecord> {
    let parent = abs_path.parent().unwrap_or(target_root);
    let rel = normalize_rel_path(rel_path);
    let file_type = metadata.file_type();
    let (kind, object_id, size) = if file_type.is_symlink() {
        let target = fs::read_link(abs_path)?;
        let data = target
            .as_os_str()
            .to_string_lossy()
            .into_owned()
            .into_bytes();
        let object_id = if write_blobs {
            repo.write_blob(&data)?.to_string()
        } else {
            compute_hash(repo.object_hash(), Kind::Blob, &data).to_string()
        };
        (RecordKind::Symlink, object_id, data.len() as u64)
    } else {
        let data = fs::read(abs_path)?;
        let object_id = if write_blobs {
            repo.write_blob(&data)?.to_string()
        } else {
            compute_hash(repo.object_hash(), Kind::Blob, &data).to_string()
        };
        let kind = if is_executable(metadata) {
            RecordKind::Executable
        } else {
            RecordKind::File
        };
        (kind, object_id, metadata.len())
    };

    Ok(SnapshotRecord {
        path: rel,
        kind,
        object_id,
        file_id: get_file_id(abs_path).ok().map(|id| format!("{id:?}")),
        parent_file_id: get_file_id(parent).ok().map(|id| format!("{id:?}")),
        size,
        create_ts_ns: system_time_to_ns(metadata.created().ok()),
        modified_ts_ns: system_time_to_ns(metadata.modified().ok()),
        change_ts_ns: metadata_change_time_ns(metadata),
        mode: metadata_mode(metadata),
        uid: metadata_uid(metadata),
        gid: metadata_gid(metadata),
        flags: metadata_flags(metadata),
    })
}

fn load_snapshot_ignore_rules(target_root: &Path) -> Result<Option<SnapshotIgnoreRules>> {
    let Some(repo_root) = discover_repo_root(target_root)? else {
        return Ok(None);
    };
    if !target_root.starts_with(&repo_root) {
        return Ok(None);
    }

    let source_path = repo_root.join(SNAPSHOT_IGNORE_FILE);
    if !source_path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let mut search = gix::ignore::Search::default();
    search.add_patterns_buffer(&bytes, source_path.clone(), Some(repo_root.as_path()));

    let case = if gix::fs::Capabilities::probe(repo_root.as_path()).ignore_case {
        gix::glob::pattern::Case::Fold
    } else {
        gix::glob::pattern::Case::Sensitive
    };

    Ok(Some(SnapshotIgnoreRules {
        repo_root,
        source_path,
        search,
        case,
    }))
}

fn collect_ignored_paths(
    target_root: &Path,
    store_path: &Path,
    ignore_rules: Option<&SnapshotIgnoreRules>,
) -> Result<Vec<String>> {
    let Some(ignore_rules) = ignore_rules else {
        return Ok(Vec::new());
    };

    let mut ignored = BTreeSet::new();
    let store_for_walk = store_path.to_path_buf();
    for entry in jwalk::WalkDir::new(target_root)
        .skip_hidden(false)
        .process_read_dir(move |_depth, path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| {
                    dir_entry.file_name != OsStr::new(".git")
                        && dir_entry.file_name != OsStr::new(DEFAULT_STORE_DIR)
                        && path.join(&dir_entry.file_name) != store_for_walk
                })
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if path.starts_with(store_path) {
            continue;
        }
        let rel_path = path
            .strip_prefix(target_root)
            .with_context(|| format!("{} is outside {}", path.display(), target_root.display()))?;
        let is_dir = entry.file_type().is_dir();
        if ignore_rules
            .match_relative_path(rel_path, is_dir)
            .is_some_and(|match_| match_.ignored)
        {
            ignored.insert(normalize_rel_path(rel_path));
        }
    }

    Ok(ignored.into_iter().collect())
}

impl SnapshotIgnoreRules {
    fn match_relative_path(
        &self,
        relative_path: &Path,
        is_dir: bool,
    ) -> Option<SnapshotIgnoreMatch> {
        let direct = self.match_bstr(
            normalize_rel_path(relative_path).as_bytes().as_bstr(),
            Some(is_dir),
        );
        if let Some(ancestor) = self.match_ignored_ancestor(relative_path) {
            return Some(ancestor);
        }
        direct
    }

    fn match_for_input(&self, input: &str, current_dir: &Path) -> Option<SnapshotIgnoreMatch> {
        let path = Path::new(input);
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.join(path)
        };
        let canonical = candidate.canonicalize().unwrap_or(candidate);
        if !canonical.starts_with(&self.repo_root) {
            return None;
        }
        let rel = canonical.strip_prefix(&self.repo_root).ok()?;
        let is_dir = fs::symlink_metadata(&canonical)
            .map(|meta| meta.file_type().is_dir())
            .ok()
            .unwrap_or(false);
        self.match_relative_path(rel, is_dir)
    }

    fn match_ignored_ancestor(&self, relative_path: &Path) -> Option<SnapshotIgnoreMatch> {
        let mut best: Option<SnapshotIgnoreMatch> = None;
        for ancestor in relative_path.ancestors().skip(1) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            let candidate = self.match_bstr(
                normalize_rel_path(ancestor).as_bytes().as_bstr(),
                Some(true),
            );
            if let Some(candidate) = candidate.filter(|match_| match_.ignored) {
                if best
                    .as_ref()
                    .is_none_or(|current| candidate.line_number >= current.line_number)
                {
                    best = Some(candidate);
                }
            }
        }
        best
    }

    fn match_bstr(&self, relative: &BStr, is_dir: Option<bool>) -> Option<SnapshotIgnoreMatch> {
        let match_ = self
            .search
            .pattern_matching_relative_path(relative, is_dir, self.case)?;
        Some(SnapshotIgnoreMatch {
            ignored: !match_.pattern.is_negative(),
            source: match_
                .source
                .map(|path| {
                    path.strip_prefix(&self.repo_root)
                        .map(normalize_rel_path)
                        .unwrap_or_else(|_| normalize_path(path))
                })
                .unwrap_or_else(|| {
                    normalize_rel_path(
                        self.source_path
                            .strip_prefix(&self.repo_root)
                            .unwrap_or(&self.source_path),
                    )
                }),
            line_number: match_.sequence_number,
            pattern: match_.pattern.to_string(),
        })
    }
}

fn write_snapshot_commit(
    repo: &gix::Repository,
    config: &AgtConfig,
    manifest: &SnapshotManifest,
    message: &str,
) -> Result<gix::ObjectId> {
    let empty_tree = repo.write_object(Tree::empty())?.detach();
    let mut editor = repo.edit_tree(empty_tree)?;

    for record in &manifest.records {
        editor.upsert(
            format!("{PAYLOAD_PREFIX}/{}", record.path),
            record.kind.entry_kind(),
            gix::ObjectId::from_hex(record.object_id.as_bytes())?,
        )?;
    }

    let manifest_bytes = manifest.encode()?;
    let manifest_id = repo.write_blob(&manifest_bytes)?.detach();
    editor.upsert(MANIFEST_PATH, EntryKind::Blob, manifest_id)?;
    let tree_id = editor.write()?.detach();

    let parents = if let Ok(mut existing) = repo.find_reference(SNAPSHOT_REF) {
        vec![existing.peel_to_commit()?.id]
    } else {
        Vec::new()
    };

    let sig = signature(config);
    Ok(repo
        .commit_as(sig, sig, SNAPSHOT_REF, message, tree_id, parents)?
        .detach())
}

fn next_tag_name(repo: &gix::Repository, created_at_ns: u128) -> Result<String> {
    let mut candidate = created_at_ns;
    loop {
        let name = format!("{candidate:020}");
        let ref_name = format!("refs/tags/{name}");
        if repo.find_reference(ref_name.as_str()).is_err() {
            return Ok(name);
        }
        candidate += 1;
    }
}

fn latest_snapshot_tag(repo: &gix::Repository) -> Result<Option<String>> {
    let mut latest = None;
    for reference in repo.references()?.tags()? {
        let reference = reference.map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let full_name = reference.name().as_bstr().to_string();
        let Some(short_name) = full_name.strip_prefix("refs/tags/") else {
            continue;
        };
        if latest
            .as_ref()
            .is_none_or(|current: &String| short_name > current.as_str())
        {
            latest = Some(short_name.to_string());
        }
    }
    Ok(latest)
}

fn load_manifest_for_tag(repo: &gix::Repository, tag: &str) -> Result<SnapshotManifest> {
    let ref_name = format!("refs/tags/{tag}");
    let mut tag_ref = repo.find_reference(ref_name.as_str())?;
    let commit = tag_ref.peel_to_commit()?;
    let tree = commit.tree()?;
    let manifest_entry = tree
        .lookup_entry_by_path(Path::new(MANIFEST_PATH))?
        .context("Snapshot manifest missing")?;
    let blob = repo
        .find_object(manifest_entry.object_id())?
        .try_into_blob()?;
    SnapshotManifest::decode(&blob.data)
}

fn diff_manifests(before: &SnapshotManifest, after: &SnapshotManifest) -> SnapshotDiff {
    let before_map: HashMap<&str, &SnapshotRecord> = before
        .records
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect();
    let after_map: HashMap<&str, &SnapshotRecord> = after
        .records
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect();

    let mut added = BTreeSet::new();
    let mut deleted = BTreeSet::new();
    let mut modified = BTreeSet::new();

    for path in before_map.keys() {
        match after_map.get(path) {
            None => {
                deleted.insert((*path).to_string());
            }
            Some(after_record) if *before_map[path] != **after_record => {
                modified.insert((*path).to_string());
            }
            Some(_) => {}
        }
    }

    for path in after_map.keys() {
        if !before_map.contains_key(path) {
            added.insert((*path).to_string());
        }
    }

    SnapshotDiff {
        added: added.into_iter().collect(),
        deleted: deleted.into_iter().collect(),
        modified: modified.into_iter().collect(),
    }
}

fn emit_diff(diff: &SnapshotDiff) {
    for path in &diff.added {
        println!("A {path}");
    }
    for path in &diff.deleted {
        println!("D {path}");
    }
    for path in &diff.modified {
        println!("M {path}");
    }
}

fn has_changes_against_manifest(
    repo: &gix::Repository,
    manifest: &SnapshotManifest,
    target_root: &Path,
    store_path: &Path,
    ignore_rules: Option<&SnapshotIgnoreRules>,
) -> Result<bool> {
    let mut expected: HashMap<String, &SnapshotRecord> = manifest
        .records
        .iter()
        .map(|record| (record.path.clone(), record))
        .collect();
    let store_for_walk = store_path.to_path_buf();

    for entry in jwalk::WalkDir::new(target_root)
        .skip_hidden(false)
        .process_read_dir(move |_depth, path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| {
                    dir_entry.file_name != OsStr::new(".git")
                        && dir_entry.file_name != OsStr::new(DEFAULT_STORE_DIR)
                        && path.join(&dir_entry.file_name) != store_for_walk
                })
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        if path.starts_with(store_path) {
            continue;
        }

        let metadata = fs::symlink_metadata(&path)?;
        let rel_path = path.strip_prefix(target_root)?;
        if ignore_rules
            .and_then(|rules| rules.match_relative_path(rel_path, metadata.file_type().is_dir()))
            .is_some_and(|match_| match_.ignored)
        {
            continue;
        }

        let rel_path = normalize_rel_path(rel_path);
        let Some(expected_record) = expected.remove(&rel_path) else {
            return Ok(true);
        };
        let current = build_record(
            repo,
            target_root,
            Path::new(&rel_path),
            &path,
            &metadata,
            false,
        )?;
        if &current != expected_record {
            return Ok(true);
        }
    }

    Ok(!expected.is_empty())
}

fn ensure_latest_snapshot_is_clean_backup(repo: &gix::Repository, store_path: &Path) -> Result<()> {
    let latest_tag = latest_snapshot_tag(repo)?.context("No snapshots found in store")?;
    let manifest = load_manifest_for_tag(repo, &latest_tag)?;
    let target_root = PathBuf::from(&manifest.target_root);
    let ignore_rules = load_snapshot_ignore_rules(&target_root)?;
    if has_changes_against_manifest(
        repo,
        &manifest,
        &target_root,
        store_path,
        ignore_rules.as_ref(),
    )? {
        bail!(
            "Full restore requires the latest snapshot ({latest_tag}) to match the current filesystem; run `agt snapshot save` first"
        );
    }
    Ok(())
}

fn remove_paths_not_in_snapshot(
    target_root: &Path,
    expected_paths: &HashSet<PathBuf>,
    scope_prefix: &Path,
    store_path: &Path,
) -> Result<()> {
    let store_for_walk = store_path.to_path_buf();
    let scope_for_walk = scope_prefix.to_path_buf();

    for entry in jwalk::WalkDir::new(target_root)
        .skip_hidden(false)
        .process_read_dir(move |_depth, path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| {
                    dir_entry.file_name != OsStr::new(".git")
                        && dir_entry.file_name != OsStr::new(DEFAULT_STORE_DIR)
                        && path.join(&dir_entry.file_name) != store_for_walk
                })
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if entry.file_type().is_dir() || path.starts_with(store_path) {
            continue;
        }
        let rel = path.strip_prefix(target_root)?.to_path_buf();
        if !scope_for_walk.as_os_str().is_empty() && !rel.starts_with(&scope_for_walk) {
            continue;
        }
        if !expected_paths.contains(&rel) {
            fs::remove_file(&path)?;
        }
    }

    Ok(())
}

fn restore_tree_to_disk(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    prefix: &Path,
    disk_root: &Path,
) -> Result<()> {
    use gix_object::TreeRefIter;
    use gix_path::from_byte_slice;

    let tree = repo.find_object(tree_id)?.try_into_tree()?;
    for entry in TreeRefIter::from_bytes(&tree.data).filter_map(Result::ok) {
        let name = from_byte_slice(entry.filename);
        let entry_path = prefix.join(name);
        restore_entry_to_disk(
            repo,
            entry.oid.to_owned(),
            entry.mode.kind(),
            &entry_path,
            disk_root,
        )?;
    }
    Ok(())
}

fn restore_entry_to_disk(
    repo: &gix::Repository,
    object_id: gix::ObjectId,
    kind: EntryKind,
    relative_path: &Path,
    disk_root: &Path,
) -> Result<()> {
    let disk_path = contained_join(disk_root, relative_path.as_os_str())?;
    match kind {
        EntryKind::Tree => {
            fs::create_dir_all(&disk_path)?;
            restore_tree_to_disk(repo, object_id, relative_path, disk_root)
        }
        EntryKind::Link => {
            let blob = repo.find_object(object_id)?.try_into_blob()?;
            let target = String::from_utf8_lossy(&blob.data);
            if let Some(parent) = disk_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if disk_path.exists() || disk_path.is_symlink() {
                fs::remove_file(&disk_path)?;
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink(target.as_ref(), &disk_path)?;
            #[cfg(not(unix))]
            fs::write(&disk_path, target.as_bytes())?;
            Ok(())
        }
        EntryKind::Blob | EntryKind::BlobExecutable => {
            let blob = repo.find_object(object_id)?.try_into_blob()?;
            if let Some(parent) = disk_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&disk_path, &blob.data)?;
            #[cfg(unix)]
            if kind == EntryKind::BlobExecutable {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&disk_path)?.permissions();
                perms.set_mode(perms.mode() | 0o111);
                fs::set_permissions(&disk_path, perms)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn collect_tree_paths(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    prefix: PathBuf,
    out: &mut HashSet<PathBuf>,
) -> Result<()> {
    use gix_object::TreeRefIter;
    use gix_path::from_byte_slice;

    let tree = repo.find_object(tree_id)?.try_into_tree()?;
    for entry in TreeRefIter::from_bytes(&tree.data).filter_map(Result::ok) {
        let name = from_byte_slice(entry.filename).to_owned();
        let path = prefix.join(name);
        collect_entry_paths(repo, entry.oid.to_owned(), &path, out, entry.mode.kind())?;
    }
    Ok(())
}

fn collect_entry_paths(
    repo: &gix::Repository,
    object_id: gix::ObjectId,
    prefix: &Path,
    out: &mut HashSet<PathBuf>,
    kind: EntryKind,
) -> Result<()> {
    if kind == EntryKind::Tree {
        collect_tree_paths(repo, object_id, prefix.to_path_buf(), out)
    } else {
        out.insert(prefix.to_path_buf());
        Ok(())
    }
}

fn signature(config: &AgtConfig) -> gix::actor::SignatureRef<'_> {
    gix::actor::SignatureRef {
        name: BStr::new(b"agt snapshot"),
        email: BStr::new(config.agent_email.as_bytes()),
        time: gix::date::Time::now_local_or_utc(),
    }
}

fn now_ns() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn system_time_to_ns(time: Option<std::time::SystemTime>) -> Option<u128> {
    time.and_then(|time| {
        time.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_nanos())
    })
}

fn path_to_tree(path: &Path) -> String {
    let mut out = String::new();
    for (index, component) in path.components().enumerate() {
        if index > 0 {
            out.push('/');
        }
        out.push_str(&component.as_os_str().to_string_lossy());
    }
    out
}

fn normalize_rel_path(path: &Path) -> String {
    path_to_tree(path)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn is_executable(metadata: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        false
    }
}

fn metadata_change_time_ns(metadata: &Metadata) -> Option<i128> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn metadata_mode(metadata: &Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(metadata.mode())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn metadata_uid(metadata: &Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(metadata.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn metadata_gid(metadata: &Metadata) -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(metadata.gid())
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        None
    }
}

fn metadata_flags(metadata: &Metadata) -> Option<u32> {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        Some(metadata.st_flags())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = metadata;
        None
    }
}

#[cfg(target_family = "unix")]
fn get_file_id(path: impl AsRef<Path>) -> io::Result<PlatformFileId> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path.as_ref())?;
    Ok(PlatformFileId::Inode {
        device_id: metadata.dev(),
        inode_number: metadata.ino(),
    })
}

#[cfg(target_family = "windows")]
fn get_file_id(_path: impl AsRef<Path>) -> io::Result<PlatformFileId> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "file identity is not implemented on Windows yet",
    ))
}

impl RecordKind {
    fn as_u8(self) -> u8 {
        match self {
            Self::File => 1,
            Self::Executable => 2,
            Self::Symlink => 3,
        }
    }

    fn from_u8(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::File),
            2 => Ok(Self::Executable),
            3 => Ok(Self::Symlink),
            _ => bail!("Unknown record kind {value}"),
        }
    }

    fn entry_kind(self) -> EntryKind {
        match self {
            Self::File => EntryKind::Blob,
            Self::Executable => EntryKind::BlobExecutable,
            Self::Symlink => EntryKind::Link,
        }
    }
}

impl SnapshotManifest {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.extend_from_slice(MANIFEST_MAGIC);
        out.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        out.extend_from_slice(&self.created_at_ns.to_le_bytes());
        write_string(&mut out, &self.target_root)?;
        out.extend_from_slice(&(self.records.len() as u32).to_le_bytes());
        for record in &self.records {
            out.push(record.kind.as_u8());
            write_string(&mut out, &record.path)?;
            write_string(&mut out, &record.object_id)?;
            write_opt_string(&mut out, record.file_id.as_deref())?;
            write_opt_string(&mut out, record.parent_file_id.as_deref())?;
            out.extend_from_slice(&record.size.to_le_bytes());
            write_opt_u128(&mut out, record.create_ts_ns);
            write_opt_u128(&mut out, record.modified_ts_ns);
            write_opt_i128(&mut out, record.change_ts_ns);
            write_opt_u32(&mut out, record.mode);
            write_opt_u32(&mut out, record.uid);
            write_opt_u32(&mut out, record.gid);
            write_opt_u32(&mut out, record.flags);
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        // Smallest possible encoded record: kind(1) + path length prefix(4)
        // + object id length prefix(4) + file_id flag(1) + parent_file_id
        // flag(1) + size(8) + create/modified/change flags(3)
        // + mode/uid/gid/flags flags(4).
        const MIN_ENCODED_RECORD_SIZE: u64 = 26;
        let mut cursor = Cursor::new(bytes);
        let mut magic = [0_u8; 8];
        cursor.read_exact(&mut magic)?;
        if &magic != MANIFEST_MAGIC {
            bail!("Invalid snapshot manifest header");
        }

        let version = read_u32(&mut cursor)?;
        if version != MANIFEST_VERSION {
            bail!("Unsupported snapshot manifest version {version}");
        }

        let created_at_ns = read_u128(&mut cursor)?;
        let target_root = read_string(&mut cursor)?;
        let record_count = read_u32(&mut cursor)?;

        let remaining = bytes.len().saturating_sub(cursor.position() as usize);
        let max_records = remaining as u64 / MIN_ENCODED_RECORD_SIZE;
        if u64::from(record_count) > max_records {
            bail!(
                "Snapshot manifest record count {record_count} exceeds remaining input size ({remaining} bytes)"
            );
        }

        let mut records = Vec::with_capacity(record_count as usize);

        for _ in 0..record_count {
            let mut kind = [0_u8; 1];
            cursor.read_exact(&mut kind)?;
            records.push(SnapshotRecord {
                path: read_string(&mut cursor)?,
                kind: RecordKind::from_u8(kind[0])?,
                object_id: read_string(&mut cursor)?,
                file_id: read_opt_string(&mut cursor)?,
                parent_file_id: read_opt_string(&mut cursor)?,
                size: read_u64(&mut cursor)?,
                create_ts_ns: read_opt_u128(&mut cursor)?,
                modified_ts_ns: read_opt_u128(&mut cursor)?,
                change_ts_ns: read_opt_i128(&mut cursor)?,
                mode: read_opt_u32(&mut cursor)?,
                uid: read_opt_u32(&mut cursor)?,
                gid: read_opt_u32(&mut cursor)?,
                flags: read_opt_u32(&mut cursor)?,
            });
        }

        let consumed = cursor.position();
        if consumed != bytes.len() as u64 {
            bail!(
                "Snapshot manifest has {} trailing bytes",
                bytes.len() - consumed as usize
            );
        }

        Ok(Self {
            target_root,
            created_at_ns,
            records,
        })
    }
}

#[derive(Default)]
struct SnapshotDiff {
    added: Vec<String>,
    deleted: Vec<String>,
    modified: Vec<String>,
}

impl SnapshotDiff {
    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.deleted.is_empty() && self.modified.is_empty()
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) -> Result<()> {
    let len: u32 = value
        .len()
        .try_into()
        .context("String too large for snapshot manifest")?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn write_opt_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<()> {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value)
        }
        None => {
            out.push(0);
            Ok(())
        }
    }
}

fn write_opt_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
}

fn write_opt_u128(out: &mut Vec<u8>, value: Option<u128>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
}

fn write_opt_i128(out: &mut Vec<u8>, value: Option<i128>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let len = read_u32(cursor)? as usize;
    let remaining = cursor
        .get_ref()
        .len()
        .saturating_sub(cursor.position() as usize);
    if len > remaining {
        bail!("Snapshot manifest string length {len} exceeds remaining input ({remaining} bytes)");
    }
    let mut buf = vec![0_u8; len];
    cursor.read_exact(&mut buf)?;
    String::from_utf8(buf).context("Snapshot manifest contained invalid UTF-8")
}

fn read_opt_string(cursor: &mut Cursor<&[u8]>) -> Result<Option<String>> {
    let mut present = [0_u8; 1];
    cursor.read_exact(&mut present)?;
    if present[0] == 0 {
        Ok(None)
    } else {
        read_string(cursor).map(Some)
    }
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut buf = [0_u8; 4];
    cursor.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> Result<u64> {
    let mut buf = [0_u8; 8];
    cursor.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_u128(cursor: &mut Cursor<&[u8]>) -> Result<u128> {
    let mut buf = [0_u8; 16];
    cursor.read_exact(&mut buf)?;
    Ok(u128::from_le_bytes(buf))
}

fn read_opt_u32(cursor: &mut Cursor<&[u8]>) -> Result<Option<u32>> {
    let mut present = [0_u8; 1];
    cursor.read_exact(&mut present)?;
    if present[0] == 0 {
        return Ok(None);
    }
    read_u32(cursor).map(Some)
}

fn read_opt_u128(cursor: &mut Cursor<&[u8]>) -> Result<Option<u128>> {
    let mut present = [0_u8; 1];
    cursor.read_exact(&mut present)?;
    if present[0] == 0 {
        return Ok(None);
    }
    read_u128(cursor).map(Some)
}

fn read_opt_i128(cursor: &mut Cursor<&[u8]>) -> Result<Option<i128>> {
    let mut present = [0_u8; 1];
    cursor.read_exact(&mut present)?;
    if present[0] == 0 {
        return Ok(None);
    }
    let mut buf = [0_u8; 16];
    cursor.read_exact(&mut buf)?;
    Ok(Some(i128::from_le_bytes(buf)))
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn restore_would_clobber(
    repo: &gix::Repository,
    object_id: gix::ObjectId,
    kind: EntryKind,
    relative_path: &Path,
    target_root: &Path,
) -> Result<bool> {
    if kind != EntryKind::Tree {
        return Ok(path_exists(&target_root.join(relative_path)));
    }

    let mut expected_paths = HashSet::new();
    collect_tree_paths(
        repo,
        object_id,
        relative_path.to_path_buf(),
        &mut expected_paths,
    )?;
    Ok(expected_paths
        .into_iter()
        .any(|path| path_exists(&target_root.join(path))))
}

fn confirm_overwrite(path: &Path) -> Result<bool> {
    eprint!("Overwrite {}? [N/y] ", path.display());
    std::io::stderr().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y"))
}

#[cfg(test)]
mod tests {
    use super::{contained_join, RecordKind, SnapshotManifest, SnapshotRecord};
    use anyhow::Result;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    fn sample_manifest() -> SnapshotManifest {
        SnapshotManifest {
            target_root: "/tmp/example".to_string(),
            created_at_ns: 42,
            records: vec![SnapshotRecord {
                path: "file.txt".to_string(),
                kind: RecordKind::Executable,
                object_id: "abc123".to_string(),
                file_id: Some("file-id".to_string()),
                parent_file_id: Some("parent-id".to_string()),
                size: 5,
                create_ts_ns: Some(1),
                modified_ts_ns: Some(2),
                change_ts_ns: Some(3),
                mode: Some(0o100755),
                uid: Some(501),
                gid: Some(20),
                flags: Some(7),
            }],
        }
    }

    #[test]
    fn manifest_roundtrip_preserves_records() -> Result<()> {
        let manifest = sample_manifest();

        let encoded = manifest.encode()?;
        let decoded = SnapshotManifest::decode(&encoded)?;
        assert_eq!(decoded, manifest);
        Ok(())
    }

    #[test]
    fn decode_rejects_absurd_record_count() -> Result<()> {
        let manifest = sample_manifest();
        let mut encoded = manifest.encode()?;
        let record_count_offset = 8 + 4 + 16 + 4 + manifest.target_root.len();
        encoded[record_count_offset..record_count_offset + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());

        let err = SnapshotManifest::decode(&encoded).expect_err("record count must be rejected");
        assert!(err.to_string().contains("record count"), "{err}");
        Ok(())
    }

    #[test]
    fn decode_rejects_truncated_manifest() -> Result<()> {
        let encoded = sample_manifest().encode()?;
        for len in 0..encoded.len() {
            assert!(
                SnapshotManifest::decode(&encoded[..len]).is_err(),
                "truncated manifest of {len} bytes must not decode"
            );
        }
        Ok(())
    }

    #[test]
    fn decode_rejects_trailing_bytes() -> Result<()> {
        let mut encoded = sample_manifest().encode()?;
        encoded.push(0);

        let err = SnapshotManifest::decode(&encoded).expect_err("trailing bytes must be rejected");
        assert!(err.to_string().contains("trailing"), "{err}");
        Ok(())
    }

    #[test]
    fn decode_rejects_oversized_string_length() -> Result<()> {
        let mut encoded = sample_manifest().encode()?;
        // target_root string length field starts after magic(8) + version(4)
        // + created_at_ns(16).
        encoded[28..32].copy_from_slice(&u32::MAX.to_le_bytes());

        let err = SnapshotManifest::decode(&encoded).expect_err("string length must be rejected");
        assert!(err.to_string().contains("string length"), "{err}");
        Ok(())
    }

    #[test]
    fn contained_join_accepts_relative_paths() {
        let root = Path::new("/tmp/target");
        assert_eq!(
            contained_join(root, OsStr::new("a/b")).unwrap(),
            PathBuf::from("/tmp/target/a/b")
        );
        assert_eq!(
            contained_join(root, OsStr::new("file.txt")).unwrap(),
            PathBuf::from("/tmp/target/file.txt")
        );
    }

    #[test]
    fn contained_join_rejects_escape_attempts() {
        let root = Path::new("/tmp/target");
        // Interior `.` components are normalized away by `Path::components`
        // and can never escape, so they are accepted implicitly.
        for attempt in ["../x", "/abs", "a/../../b", ".", ""] {
            assert!(
                contained_join(root, OsStr::new(attempt)).is_err(),
                "{attempt} must be rejected"
            );
        }
        #[cfg(unix)]
        assert!(contained_join(root, OsStr::new("a\0b")).is_err());
    }
}
