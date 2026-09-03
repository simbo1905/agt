//! TTY-gated progress rendering for long-running snapshot operations.
//!
//! Contract (issue #28): progress is a human affordance only. It renders on
//! stderr exclusively when stderr is an interactive terminal, quiet flags
//! always suppress it, and `AGT_NO_PORCELINE` (spelling per the original
//! spec) forces it off for scripting. When suppressed there is zero cost: no
//! pre-walk, no per-file bookkeeping, and stdout stays byte-identical.

use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

/// Environment variable that forces progress off even on a TTY. Any non-empty
/// value counts as set.
pub const NO_PORCELINE_ENV: &str = "AGT_NO_PORCELINE";

/// How often the progress line may be redrawn while a walk is in flight.
const RENDER_INTERVAL: Duration = Duration::from_millis(100);

/// Decided gating for progress rendering, computed once up front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressGating {
    /// Progress line renders on stderr at all (TTY && !quiet && !no-porcelain).
    pub enabled: bool,
    /// An accurate percentage is shown because a pre-walk computed totals
    /// (`-v` on save). Never true unless `enabled` is.
    pub accurate: bool,
}

/// Pure decision function so the gating truth table is exhaustively testable.
pub fn gating(is_tty: bool, quiet: bool, no_porceline: bool, verbose: bool) -> ProgressGating {
    let enabled = is_tty && !quiet && !no_porceline;
    ProgressGating {
        enabled,
        accurate: enabled && verbose,
    }
}

impl ProgressGating {
    /// Decides gating from the real environment: stderr TTY state, the
    /// `AGT_NO_PORCELINE` variable, and the caller's quiet/verbose flags.
    pub fn from_env(quiet: bool, verbose: bool) -> Self {
        let is_tty = io::stderr().is_terminal();
        let no_porceline =
            std::env::var_os(NO_PORCELINE_ENV).is_some_and(|value| !value.is_empty());
        gating(is_tty, quiet, no_porceline, verbose)
    }
}

/// Carriage-return-updated single-line progress renderer on stderr.
pub struct Progress {
    gating: ProgressGating,
    total_files: Option<u64>,
    total_bytes: Option<u64>,
    files: u64,
    bytes: u64,
    rendered: bool,
    line_width: usize,
    next_render: Instant,
}

impl Progress {
    /// A progress handle that never renders and costs (almost) nothing.
    pub fn disabled() -> Self {
        Self {
            gating: ProgressGating {
                enabled: false,
                accurate: false,
            },
            total_files: None,
            total_bytes: None,
            files: 0,
            bytes: 0,
            rendered: false,
            line_width: 0,
            next_render: Instant::now(),
        }
    }

    pub fn new(gating: ProgressGating) -> Self {
        if !gating.enabled {
            return Self::disabled();
        }
        Self {
            gating,
            total_files: None,
            total_bytes: None,
            files: 0,
            bytes: 0,
            rendered: false,
            line_width: 0,
            next_render: Instant::now(),
        }
    }

    /// Installs pre-walked totals so an accurate percentage can be shown.
    pub fn set_totals(&mut self, files: u64, bytes: u64) {
        if !self.gating.enabled {
            return;
        }
        self.total_files = Some(files);
        self.total_bytes = Some(bytes);
    }

    /// Records one captured file of the given size and redraws if due.
    pub fn tick(&mut self, file_bytes: u64) {
        if !self.gating.enabled {
            return;
        }
        self.files += 1;
        self.bytes += file_bytes;
        let now = Instant::now();
        if !self.rendered || now >= self.next_render {
            self.rendered = true;
            self.next_render = now + RENDER_INTERVAL;
            self.draw();
        }
    }

    /// Ends the progress line so subsequent stderr output starts on a fresh
    /// line. No-op when nothing was ever rendered.
    pub fn finish(&mut self) {
        if self.rendered {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr);
            let _ = stderr.flush();
        }
    }

    fn draw(&mut self) {
        let line = self.render_line();
        let mut stderr = io::stderr().lock();
        let padding = self.line_width.saturating_sub(line.chars().count());
        let ok =
            write!(stderr, "\r{line}{}", " ".repeat(padding)).is_ok() && stderr.flush().is_ok();
        if ok {
            self.line_width = line.chars().count();
        }
    }

    /// The progress line itself; kept pure for unit testing.
    fn render_line(&self) -> String {
        let bytes = format_bytes(self.bytes);
        match self.total_files {
            Some(total) if total > 0 => {
                let percent = self.files.saturating_mul(100) / total;
                let total_bytes = match self.total_bytes {
                    Some(total_bytes) => format!("/{}", format_bytes(total_bytes)),
                    None => String::new(),
                };
                format!(
                    "snapshot: {percent}% ({}/{total} files, {bytes}{total_bytes})",
                    self.files
                )
            }
            _ => format!("snapshot: {} files, {bytes}", self.files),
        }
    }
}

/// Human-plausible binary units (KiB/MiB/GiB), compact and stable in width.
fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * KIB;
    const GIB: f64 = 1024.0 * MIB;
    // Progress sizes are nowhere near f64's 2^53 exact-integer limit.
    #[allow(clippy::cast_precision_loss)]
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        if value / MIB >= 100.0 {
            format!("{:.0} MiB", value / MIB)
        } else {
            format!("{:.1} MiB", value / MIB)
        }
    } else if value >= KIB {
        if value / KIB >= 100.0 {
            format!("{:.0} KiB", value / KIB)
        } else {
            format!("{:.1} KiB", value / KIB)
        }
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{format_bytes, gating, Progress, ProgressGating};

    #[test]
    fn gating_truth_table() {
        // (is_tty, quiet, no_porceline, verbose) -> (enabled, accurate)
        let cases = [
            ((false, false, false, false), (false, false)),
            ((false, false, false, true), (false, false)),
            ((false, false, true, false), (false, false)),
            ((false, false, true, true), (false, false)),
            ((false, true, false, false), (false, false)),
            ((false, true, false, true), (false, false)),
            ((false, true, true, false), (false, false)),
            ((false, true, true, true), (false, false)),
            ((true, false, false, false), (true, false)),
            ((true, false, false, true), (true, true)),
            ((true, false, true, false), (false, false)),
            ((true, false, true, true), (false, false)),
            ((true, true, false, false), (false, false)),
            ((true, true, false, true), (false, false)),
            ((true, true, true, false), (false, false)),
            ((true, true, true, true), (false, false)),
        ];
        for (input, expected) in cases {
            let gate = gating(input.0, input.1, input.2, input.3);
            assert_eq!(
                gate,
                ProgressGating {
                    enabled: expected.0,
                    accurate: expected.1,
                },
                "gating({tty}, {quiet}, {no_porceline}, {verbose})",
                tty = input.0,
                quiet = input.1,
                no_porceline = input.2,
                verbose = input.3,
            );
        }
    }

    #[test]
    fn disabled_progress_never_renders() {
        let mut progress = Progress::disabled();
        progress.set_totals(10, 100);
        progress.tick(50);
        progress.tick(50);
        progress.finish();
        assert!(!progress.rendered);
        assert_eq!(progress.files, 0);
    }

    #[test]
    fn render_line_without_totals_is_indeterminate() {
        let mut progress = Progress::new(gating(true, false, false, false));
        progress.tick(512);
        progress.tick(1024);
        assert_eq!(progress.render_line(), "snapshot: 2 files, 1.5 KiB");
    }

    #[test]
    fn render_line_with_totals_shows_percent() {
        let mut progress = Progress::new(gating(true, false, false, true));
        progress.set_totals(293, 456 * 1024 * 1024);
        for _ in 0..123 {
            progress.tick(0);
        }
        assert_eq!(
            progress.render_line(),
            "snapshot: 41% (123/293 files, 0 B/456 MiB)"
        );
    }

    #[test]
    fn render_line_with_zero_total_falls_back() {
        let mut progress = Progress::new(gating(true, false, false, true));
        progress.set_totals(0, 0);
        progress.tick(0);
        assert_eq!(progress.render_line(), "snapshot: 1 files, 0 B");
    }

    #[test]
    fn byte_formatting_uses_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KiB");
        assert_eq!(format_bytes(1536), "1.5 KiB");
        assert_eq!(format_bytes(512 * 1024), "512 KiB");
        assert_eq!(format_bytes(456 * 1024 * 1024), "456 MiB");
        assert_eq!(format_bytes(1024 * 1024 + 512 * 1024), "1.5 MiB");
        assert_eq!(format_bytes(1_100 * 1024 * 1024), "1.1 GiB");
    }
}
