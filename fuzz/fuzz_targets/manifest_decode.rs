#![no_main]

use libfuzzer_sys::fuzz_target;

// Guards the untrusted-input boundary of the snapshot manifest binary
// decoder: arbitrary bytes must never panic, only return Err.
fuzz_target!(|data: &[u8]| {
    let _ = agt_core::snapshot::SnapshotManifest::decode(data);
});
