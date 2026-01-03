use anyhow::Result;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn scan_modified_files(root: &Path, since_timestamp: i64) -> Result<Vec<PathBuf>> {
    let threshold = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(since_timestamp as u64);
    let mut files = Vec::new();

    for entry in jwalk::WalkDir::new(root)
        .skip_hidden(false)
        .process_read_dir(|_depth, _path, _state, children| {
            children.retain(|entry| {
                entry.as_ref().map_or(true, |dir_entry| dir_entry.file_name != OsStr::new(".git"))
            });
        })
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_file() {
            let metadata = entry.metadata()?;
            let mtime = metadata.modified()?;
            if mtime >= threshold {
                files.push(entry.path().strip_prefix(root)?.to_path_buf());
            }
        }
    }

    Ok(files)
}
