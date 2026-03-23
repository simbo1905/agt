#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildChannel {
    Local,
    Nightly,
}

pub fn parse_build_channel(value: Option<&str>) -> Result<BuildChannel, String> {
    match value {
        Some("nightly") => Ok(BuildChannel::Nightly),
        Some("") | None => Ok(BuildChannel::Local),
        Some(other) => Err(format!("unsupported AGT_BUILD_CHANNEL: {other}")),
    }
}

pub fn parse_release_ref(ref_name: &str) -> Option<&str> {
    let version = ref_name.strip_prefix("release/")?;
    if is_valid_release_version(version) {
        Some(version)
    } else {
        None
    }
}

pub fn is_valid_release_version(version: &str) -> bool {
    let (base, suffix) = match version.split_once('-') {
        Some((base, suffix)) => (base, Some(suffix)),
        None => (version, None),
    };

    if !is_valid_semver_triplet(base) {
        return false;
    }

    suffix.is_none_or(is_valid_suffix)
}

pub fn format_local_version(base: &str, sha: &str, dirty: bool) -> String {
    let mut version = format!("{base}-{sha}");
    if dirty {
        version.push_str("+dirty");
    }
    version
}

pub fn format_nightly_version(build_date: &str, sha: &str) -> String {
    format!("{build_date}-{sha}")
}

fn is_valid_semver_triplet(value: &str) -> bool {
    let mut parts = value.split('.');
    let a = parts.next();
    let b = parts.next();
    let c = parts.next();

    parts.next().is_none()
        && a.is_some_and(is_numeric)
        && b.is_some_and(is_numeric)
        && c.is_some_and(is_numeric)
}

fn is_numeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_valid_suffix(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::{
        format_local_version, format_nightly_version, is_valid_release_version,
        parse_build_channel, parse_release_ref, BuildChannel,
    };

    #[test]
    fn parses_build_channels() {
        assert_eq!(parse_build_channel(None), Ok(BuildChannel::Local));
        assert_eq!(parse_build_channel(Some("")), Ok(BuildChannel::Local));
        assert_eq!(
            parse_build_channel(Some("nightly")),
            Ok(BuildChannel::Nightly)
        );
        assert_eq!(
            parse_build_channel(Some("bogus")),
            Err(String::from("unsupported AGT_BUILD_CHANNEL: bogus"))
        );
    }

    #[test]
    fn parses_release_refs() {
        assert_eq!(parse_release_ref("release/0.2.0"), Some("0.2.0"));
        assert_eq!(parse_release_ref("release/0.2.0-rc1"), Some("0.2.0-rc1"));
        assert_eq!(parse_release_ref("0.2.0"), None);
        assert_eq!(parse_release_ref("release/v0.2.0"), None);
    }

    #[test]
    fn validates_release_versions() {
        assert!(is_valid_release_version("0.2.0"));
        assert!(is_valid_release_version("12.34.56-rc1"));
        assert!(is_valid_release_version("12.34.56-rc.1"));
        assert!(!is_valid_release_version("v0.2.0"));
        assert!(!is_valid_release_version("0.1"));
        assert!(!is_valid_release_version("0.2.0-"));
        assert!(!is_valid_release_version("0.2.0+build"));
    }

    #[test]
    fn formats_versions() {
        assert_eq!(
            format_nightly_version("2026.03.22", "abcdef123456"),
            "2026.03.22-abcdef123456"
        );
        assert_eq!(
            format_local_version("0.2.0", "abcdef123456", false),
            "0.2.0-abcdef123456"
        );
        assert_eq!(
            format_local_version("0.2.0", "abcdef123456", true),
            "0.2.0-abcdef123456+dirty"
        );
    }
}
