# agt-core fuzzing

Fuzz targets for the snapshot subsystem's untrusted-input boundary (crates/agt-core).

Run a target (nightly required; fuzzing is NOT part of CI or `cargo test`):

    cargo +nightly fuzz run <target> -- -max_total_time=60 -max_len=65536

Targets:

- `manifest_decode` — arbitrary bytes into `SnapshotManifest::decode`; must never panic.
- `contained_join` — arbitrary path pieces through the restore-time containment
  helper; any result outside the target root is a crash.
- `tag_timestamp` — arbitrary tag names through `is_timestamp_tag`; no panic,
  and successful parses must be self-consistent.

Crashes/artifacts land in `fuzz/artifacts/`. Reproduce with
`cargo +nightly fuzz run <target> fuzz/artifacts/<file>`.
