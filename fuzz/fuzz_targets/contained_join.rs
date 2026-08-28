#![no_main]

use libfuzzer_sys::fuzz_target;
use std::ffi::OsStr;
use std::path::Path;

// Guards the restore-time path containment invariant: for any relative path
// piece, `contained_join` either rejects it or produces a path that stays
// lexically inside the target root. A violation is a path-escape bug.
fuzz_target!(|data: &[u8]| {
    #[cfg(unix)]
    let relative: &OsStr = {
        use std::os::unix::ffi::OsStrExt;
        OsStr::from_bytes(data)
    };
    #[cfg(not(unix))]
    let relative: &OsStr = OsStr::new(&String::from_utf8_lossy(data));

    let root = Path::new("/tmp/agt-fuzz-target-root");
    if let Ok(joined) = agt_core::snapshot::contained_join(root, relative) {
        assert!(
            joined.starts_with(root),
            "contained_join escaped the target root: {relative:?} -> {joined:?}"
        );
        for component in joined.strip_prefix(root).expect("prefix checked").components() {
            assert!(
                matches!(component, std::path::Component::Normal(_)),
                "contained_join produced non-Normal component {component:?} for {relative:?}"
            );
        }
    }
});
