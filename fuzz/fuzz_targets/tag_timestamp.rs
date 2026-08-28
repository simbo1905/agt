#![no_main]

use libfuzzer_sys::fuzz_target;

// Guards the snapshot tag-name -> timestamp parser: arbitrary tag strings
// must never panic, and a successful parse must be self-consistent
// (re-parsing the canonical decimal form either rejects it or yields the
// same timestamp).
fuzz_target!(|data: &[u8]| {
    let tag = String::from_utf8_lossy(data);
    if let Some(ts) = agt_core::snapshot::is_timestamp_tag(&tag) {
        let digits: String = tag.chars().filter(|c| c.is_ascii_digit()).collect();
        assert!(
            digits.chars().count() >= 17,
            "tag {tag:?} parsed to {ts} with fewer than 17 digits"
        );
        let canonical = ts.to_string();
        let reparsed = agt_core::snapshot::is_timestamp_tag(&canonical);
        assert!(
            reparsed.is_none_or(|value| value == ts),
            "canonical timestamp {canonical} re-parsed to {reparsed:?}, expected {ts}"
        );
    }
});
