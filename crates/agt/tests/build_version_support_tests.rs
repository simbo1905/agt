#[path = "../../../build/build_version_support.rs"]
mod build_version_support;

#[test]
fn release_refs_require_release_prefix_and_numeric_semver() {
    assert_eq!(
        build_version_support::parse_release_ref("release/0.1.0"),
        Some("0.1.0")
    );
    assert_eq!(
        build_version_support::parse_release_ref("release/0.1.0-rc1"),
        Some("0.1.0-rc1")
    );
    assert_eq!(
        build_version_support::parse_release_ref("release/v0.1.0"),
        None
    );
    assert_eq!(
        build_version_support::parse_release_ref("release/test-001"),
        None
    );
}

#[test]
fn nightly_versions_use_date_then_sha() {
    assert_eq!(
        build_version_support::format_nightly_version("2026.03.22", "abcdef123456"),
        "2026.03.22-abcdef123456"
    );
}

#[test]
fn local_versions_use_cargo_base_sha_and_optional_dirty_suffix() {
    assert_eq!(
        build_version_support::format_local_version("0.1.0", "abcdef123456", false),
        "0.1.0-abcdef123456"
    );
    assert_eq!(
        build_version_support::format_local_version("0.1.0", "abcdef123456", true),
        "0.1.0-abcdef123456+dirty"
    );
}
