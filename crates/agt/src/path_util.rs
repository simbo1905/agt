use anyhow::{Context, Result};
#[cfg(any(windows, test))]
use std::ffi::OsStr;
#[cfg(windows)]
use std::path::{Component, Prefix};
use std::path::{Path, PathBuf};

// This path simplification logic is adapted from the `dunce` crate:
// <https://crates.io/crates/dunce>
// Repository: <https://gitlab.com/kornelski/dunce>
// Original license options: CC0-1.0 OR MIT-0 OR Apache-2.0
// Incorporated here under the MIT-0 option.
// Modified from original to expose only the path helpers AGT uses.
// See the root LICENSE file for the full third-party notice.

pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("Failed to canonicalize {}", path.display()))?;
    Ok(simplify(&normalize_windows_path_for_git(&canonical)))
}

pub fn canonicalize_or_original(path: &Path) -> PathBuf {
    canonicalize(path).unwrap_or_else(|_| simplify(&normalize_windows_path_for_git(path)))
}

#[cfg(windows)]
fn normalize_windows_path_for_git(path: &Path) -> PathBuf {
    let Some(path_string) = path.to_str() else {
        return path.to_path_buf();
    };

    if let Some(without_verbatim) = path_string.strip_prefix("\\\\?\\") {
        let normalized = without_verbatim
            .strip_prefix("UNC\\")
            .map(|unc_path| format!("\\\\{unc_path}"))
            .unwrap_or_else(|| without_verbatim.to_string());
        PathBuf::from(normalized)
    } else {
        path.to_path_buf()
    }
}

#[cfg(not(windows))]
fn normalize_windows_path_for_git(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub fn simplify(path: &Path) -> PathBuf {
    try_simplified(path)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(any(windows, test))]
fn windows_char_len(s: &OsStr) -> usize {
    #[cfg(not(windows))]
    let len = s
        .to_string_lossy()
        .chars()
        .map(|c| if c as u32 <= 0xFFFF { 1 } else { 2 })
        .sum();
    #[cfg(windows)]
    let len = std::os::windows::ffi::OsStrExt::encode_wide(s).count();
    len
}

#[cfg(any(windows, test))]
fn is_valid_filename(file_name: &OsStr) -> bool {
    if file_name.len() > 255 && windows_char_len(file_name) > 255 {
        return false;
    }

    let byte_str = if let Some(s) = file_name.to_str() {
        s.as_bytes()
    } else {
        return false;
    };
    if byte_str.is_empty() {
        return false;
    }
    if byte_str.iter().any(|&c| {
        matches!(
            c,
            0..=31 | b'<' | b'>' | b':' | b'"' | b'/' | b'\\' | b'|' | b'?' | b'*'
        )
    }) {
        return false;
    }
    if matches!(byte_str.last(), Some(b' ' | b'.')) {
        return false;
    }
    true
}

#[cfg(any(windows, test))]
const RESERVED_NAMES: [&str; 22] = [
    "AUX", "NUL", "PRN", "CON", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[cfg(any(windows, test))]
fn is_reserved<P: AsRef<OsStr>>(file_name: P) -> bool {
    if let Some(name) = Path::new(&file_name)
        .file_stem()
        .and_then(|s| s.to_str()?.split('.').next())
    {
        let trimmed = name.trim_end_matches(' ');
        return trimmed.len() <= 4
            && RESERVED_NAMES
                .iter()
                .any(|reserved| trimmed.eq_ignore_ascii_case(reserved));
    }
    false
}

#[cfg(not(windows))]
const fn try_simplified(_path: &Path) -> Option<&Path> {
    None
}

#[cfg(windows)]
fn try_simplified(path: &Path) -> Option<&Path> {
    let mut components = path.components();
    match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::VerbatimDisk(..) => {}
            _ => return None,
        },
        _ => return None,
    }
    let stripped_path = components.as_path();

    for component in components {
        match component {
            Component::RootDir => {}
            Component::Normal(file_name) => {
                if !is_valid_filename(file_name) || is_reserved(file_name) {
                    return None;
                }
            }
            _ => return None,
        }
    }

    let path_os_str = stripped_path.as_os_str();
    if path_os_str.len() > 260 && windows_char_len(path_os_str) > 260 {
        return None;
    }
    Some(stripped_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved() {
        assert!(is_reserved("CON"));
        assert!(is_reserved("con"));
        assert!(is_reserved("con.con"));
        assert!(is_reserved("COM4"));
        assert!(is_reserved("COM4.txt"));
        assert!(is_reserved("COM4 .txt"));
        assert!(is_reserved("con."));
        assert!(is_reserved("con ."));
        assert!(is_reserved("con  "));
        assert!(is_reserved("con . "));
        assert!(is_reserved("con . .txt"));
        assert!(is_reserved("con.....txt"));
        assert!(is_reserved("PrN....."));
        assert!(is_reserved("nul.tar.gz"));

        assert!(!is_reserved(" PrN....."));
        assert!(!is_reserved(" CON"));
        assert!(!is_reserved("COM0"));
        assert!(!is_reserved("COM77"));
        assert!(!is_reserved(" CON "));
        assert!(!is_reserved(".CON"));
        assert!(!is_reserved("@CON"));
        assert!(!is_reserved("not.CON"));
        assert!(!is_reserved("CON。"));
    }

    #[test]
    fn len() {
        assert_eq!(1, windows_char_len(OsStr::new("a")));
        assert_eq!(1, windows_char_len(OsStr::new("€")));
        assert_eq!(1, windows_char_len(OsStr::new("本")));
        assert_eq!(2, windows_char_len(OsStr::new("🧐")));
        assert_eq!(2, windows_char_len(OsStr::new("®®")));
    }

    #[test]
    fn valid() {
        assert!(!is_valid_filename("..".as_ref()));
        assert!(!is_valid_filename(".".as_ref()));
        assert!(!is_valid_filename("aaaaaaaaaa:".as_ref()));
        assert!(!is_valid_filename("ą:ą".as_ref()));
        assert!(!is_valid_filename("".as_ref()));
        assert!(!is_valid_filename("a ".as_ref()));
        assert!(!is_valid_filename(" a. ".as_ref()));
        assert!(!is_valid_filename("a/".as_ref()));
        assert!(!is_valid_filename("/a".as_ref()));
        assert!(!is_valid_filename("/".as_ref()));
        assert!(!is_valid_filename("\\".as_ref()));
        assert!(!is_valid_filename("\\a".as_ref()));
        assert!(!is_valid_filename("<x>".as_ref()));
        assert!(!is_valid_filename("a*".as_ref()));
        assert!(!is_valid_filename("?x".as_ref()));
        assert!(!is_valid_filename("a\0a".as_ref()));
        assert!(!is_valid_filename("\x1f".as_ref()));
        assert!(!is_valid_filename("a".repeat(257).as_ref()));

        assert!(is_valid_filename("®".repeat(254).as_ref()));
        assert!(is_valid_filename("ファイル".as_ref()));
        assert!(is_valid_filename("a".as_ref()));
        assert!(is_valid_filename("a.aaaaaaaa".as_ref()));
        assert!(is_valid_filename("a........a".as_ref()));
        assert!(is_valid_filename("       b".as_ref()));
    }
}
