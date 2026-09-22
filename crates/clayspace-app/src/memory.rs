//! One memory figure, assembled in one place, and the check that it is right.
//!
//! Three figures used to describe the same thing and none of them matched the
//! process. The status area showed the brick cache's payload (0.00 GB), the
//! agent's `state.memory` showed the engine's ledger (13 MB, later 225–359 MB),
//! and the process footprint was 26 GB in one audited session and 0.9–3.84 GB
//! in another. The engine's ledger was not wrong, it was partial: it walks the
//! document and the surfaces handed to it, and everything this application
//! holds to *draw* the document — the geometry the viewport keeps per key, the
//! buffers it uploads into, the staging a write takes, the targets a frame is
//! rendered into — was in none of the three.
//!
//! [`ledger`] is now the only place the figure is put together, and the
//! status area and the agent both read it through the same meter. [`FootprintWatch`]
//! compares it with what the operating system charges the process, because a
//! ledger that only counts what it knows about cannot notice what it does not:
//! the 26 GB session would have been flagged within seconds of passing 1 GB.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clayspace_engine::ClayDocument;
use clayspace_model::{DrawingMemory, MemoryDiagnostics};
use clayspace_view::Gpu;

use crate::SurfaceGeometry;

/// The document's memory with everything held beside it: the engine's report
/// with the surfaces folded in, the brick cache, and the drawing.
///
/// `None` where the engine refused the question. Not free — the cache is
/// walked and each surface asked — so it is read on the meter's clock.
pub fn ledger(document: &ClayDocument, drawing: DrawingMemory) -> Option<MemoryDiagnostics> {
    let mut memory = document.memory_diagnostics()?;
    memory.drawing = drawing;
    Some(memory)
}

/// What the viewport holds to draw the document.
///
/// The geometry from the store that keeps it, and the rest from the device's
/// own gauge — every renderer's buffers, the window's framebuffer and any
/// capture target alive at the moment, not only the surface's.
pub fn drawing(geometry: &SurfaceGeometry, gpu: &Gpu) -> DrawingMemory {
    let device = gpu.memory();
    DrawingMemory {
        geometry: geometry.resident_bytes(),
        buffers: device.buffers,
        staging: device.staging,
        targets: device.targets,
    }
}

// -- the check ---------------------------------------------------------------

/// How many times the ledger the footprint may reach before it is reported.
///
/// Two, as the audit that found the gap suggested. The ledger is a floor —
/// allocator rounding, driver bookkeeping and fragmentation are invisible
/// from inside — so the footprint is always somewhat above it; twice is past
/// anything those explain.
pub const FOOTPRINT_MULTIPLE: u64 = 2;

/// What the process costs with no document in it: the code, the graphics
/// driver, the interface's fonts and textures.
///
/// Added to the allowance rather than folded into the multiple, because it is
/// a constant and a multiple of a small document would be smaller than it —
/// an empty document would be reported on every reading. Half a gigabyte is
/// generous for this application at rest and far below either audited gap.
pub const FOOTPRINT_ALLOWANCE: u64 = 512 * 1024 * 1024;

/// Whether a footprint is more than the ledger and the process at rest can
/// explain.
pub fn footprint_is_unexplained(footprint: u64, in_use: u64) -> bool {
    footprint
        > in_use
            .saturating_mul(FOOTPRINT_MULTIPLE)
            .saturating_add(FOOTPRINT_ALLOWANCE)
}

/// Reports a footprint the ledger does not explain, once per doubling.
///
/// Once when it is first seen, and again whenever the footprint has doubled
/// since the last report: a leak is a figure that keeps growing, and a line a
/// second saying the same thing is noise a person learns to skip. A footprint
/// that falls back within bounds re-arms it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FootprintWatch {
    /// The footprint last reported, while it is still unexplained.
    reported: Option<u64>,
}

impl FootprintWatch {
    /// The line to log for this reading, if one is due.
    pub fn observe(&mut self, footprint: u64, ledger: &MemoryDiagnostics) -> Option<String> {
        if !footprint_is_unexplained(footprint, ledger.in_use()) {
            self.reported = None;
            return None;
        }
        if self
            .reported
            .is_some_and(|reported| footprint < reported.saturating_mul(2))
        {
            return None;
        }
        self.reported = Some(footprint);
        Some(unexplained_line(footprint, ledger))
    }
}

/// The report, with the ledger's breakdown so the reader can see which part
/// did not grow with the footprint.
fn unexplained_line(footprint: u64, ledger: &MemoryDiagnostics) -> String {
    let drawing = &ledger.drawing;
    format!(
        "memory: the process holds {} but the ledger accounts for {} \
         (engine {}, cache {}, geometry {}, buffers {}, staging {}, targets {}); \
         {} is unaccounted for",
        megabytes(footprint),
        megabytes(ledger.in_use()),
        megabytes(ledger.total),
        megabytes(ledger.cache_bytes),
        megabytes(drawing.geometry),
        megabytes(drawing.buffers),
        megabytes(drawing.staging),
        megabytes(drawing.targets),
        megabytes(footprint.saturating_sub(ledger.in_use())),
    )
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

// -- reading the footprint ---------------------------------------------------

/// What the operating system charges this process, in bytes, or `None` where
/// it cannot be read here.
///
/// On Linux, the resident set from `/proc`, which is a file read. On macOS,
/// the *footprint* — the figure Activity Monitor shows, which counts graphics
/// memory the resident set does not — and since reading it without `unsafe`
/// means asking `top`, it takes most of a second: never call this on the
/// interface thread. [`FootprintProbe`] is how the application reads it.
pub fn footprint() -> Option<u64> {
    platform_footprint()
}

#[cfg(target_os = "linux")]
fn platform_footprint() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kilobytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kilobytes * 1024)
}

#[cfg(target_os = "macos")]
fn platform_footprint() -> Option<u64> {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("/usr/bin/top")
        .args(["-l", "1", "-pid", &pid, "-stats", "mem"])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    parse_top_memory(text.lines().last()?)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn platform_footprint() -> Option<u64> {
    None
}

/// `top`'s memory column: a number with a unit, and sometimes a trailing `+`
/// or `-` saying which way it moved since the last sample.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_top_memory(field: &str) -> Option<u64> {
    let field = field.trim().trim_end_matches(['+', '-']);
    let split = field.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let (number, unit) = field.split_at(split);
    let number: f64 = number.parse().ok()?;
    let scale: u64 = match unit {
        "B" => 1,
        "K" => 1 << 10,
        "M" => 1 << 20,
        "G" => 1 << 30,
        "T" => 1 << 40,
        _ => return None,
    };
    Some((number * scale as f64) as u64)
}

/// The footprint, read off the interface thread on a clock of its own.
///
/// A thread that reads [`footprint`] every [`FootprintProbe::INTERVAL`] and
/// leaves the answer where the meter can take it for the cost of a load. The
/// meter reads the ledger once a second; the footprint changes no faster than
/// a document does, and on macOS one reading costs most of a second of a
/// subprocess, so it keeps its own, slower clock.
#[derive(Debug, Clone)]
pub struct FootprintProbe {
    /// The last reading, zero before the first or where none can be taken.
    latest: Arc<AtomicU64>,
}

impl FootprintProbe {
    pub const INTERVAL: Duration = Duration::from_secs(10);

    /// Starts the reading thread. It runs for the life of the process, which
    /// is the life of the application this is built for.
    pub fn spawn() -> Self {
        let latest = Arc::new(AtomicU64::new(0));
        let writer = Arc::clone(&latest);
        let started = std::thread::Builder::new()
            .name("footprint".into())
            .spawn(move || loop {
                writer.store(footprint().unwrap_or(0), Ordering::Relaxed);
                std::thread::sleep(Self::INTERVAL);
            });
        if let Err(error) = started {
            eprintln!("memory: the footprint will not be checked: {error}");
        }
        Self { latest }
    }

    /// The last footprint read, or `None` before the first or where the
    /// platform offers none.
    pub fn latest(&self) -> Option<u64> {
        Some(self.latest.load(Ordering::Relaxed)).filter(|bytes| *bytes > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn ledger_of(in_use: u64) -> MemoryDiagnostics {
        MemoryDiagnostics {
            total: in_use,
            ..Default::default()
        }
    }

    /// The first audited session: 26 GB against 13 MB. The second: 3.84 GB
    /// against 359 MB. Both are past what the ledger and a process at rest
    /// explain; a small document in a process at rest is not.
    #[test]
    fn the_audited_gaps_are_unexplained_and_a_process_at_rest_is_not() {
        assert!(footprint_is_unexplained(26 * 1024 * MB, 13 * MB));
        assert!(footprint_is_unexplained(3840 * MB, 359 * MB));
        assert!(!footprint_is_unexplained(300 * MB, 13 * MB));
        assert!(!footprint_is_unexplained(900 * MB, 359 * MB));
    }

    #[test]
    fn an_unexplained_footprint_is_reported_with_the_breakdown() {
        let mut watch = FootprintWatch::default();
        let mut ledger = ledger_of(13 * MB);
        ledger.drawing.buffers = 2 * MB;
        let line = watch
            .observe(26 * 1024 * MB, &ledger)
            .expect("a 26 GB footprint against 15 MB must be reported");
        assert!(line.contains("26624.0 MB"), "{line}");
        assert!(line.contains("buffers 2.0 MB"), "{line}");
        assert!(line.contains("unaccounted"), "{line}");
    }

    /// Once per doubling, so a leak reads as a figure that keeps growing and
    /// not as the same line once a second.
    #[test]
    fn a_growing_gap_is_reported_again_only_when_it_doubles() {
        let mut watch = FootprintWatch::default();
        let ledger = ledger_of(10 * MB);
        assert!(watch.observe(1024 * MB, &ledger).is_some());
        assert!(watch.observe(1500 * MB, &ledger).is_none());
        assert!(watch.observe(2048 * MB, &ledger).is_some());
        // Back within bounds re-arms it.
        assert!(watch.observe(100 * MB, &ledger).is_none());
        assert!(watch.observe(1024 * MB, &ledger).is_some());
    }

    #[test]
    fn tops_memory_column_is_read_with_its_unit() {
        assert_eq!(parse_top_memory("33M"), Some(33 * MB));
        assert_eq!(parse_top_memory("1234K+"), Some(1234 * 1024));
        assert_eq!(parse_top_memory("2G-"), Some(2048 * MB));
        assert_eq!(parse_top_memory(" 512B "), Some(512));
        assert_eq!(parse_top_memory("MEM"), None);
    }

    /// Where the platform can be read at all, it reads as something a process
    /// could be: more than nothing, less than a terabyte.
    #[test]
    fn the_footprint_can_be_read_where_the_platform_offers_it() {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            let bytes = footprint().expect("this platform offers a footprint");
            assert!(bytes > MB && bytes < 1 << 40, "{bytes}");
        }
    }
}
