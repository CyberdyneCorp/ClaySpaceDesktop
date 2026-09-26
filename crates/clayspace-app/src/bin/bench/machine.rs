//! Which machine the figures were taken on.
//!
//! `Conditions` says which platform, architecture and backend, which is enough
//! to refuse a comparison between unlike runs and not enough to explain one
//! between like runs. Two aarch64 Macs on Metal differ by a factor of several:
//! a hosted CI runner is a three-core virtual machine and a workstation is not.
//! So the recorded file names the processor, the core count, the memory, the
//! operating system and — where the run is on a CI runner — the runner image,
//! and a comparison against a different machine says so above its table.
//!
//! Read with ordinary safe code, as `load.rs` does: `sysctl` on macOS,
//! `/proc` and `/etc/os-release` on Linux. Every field is optional because a
//! machine that will not say is still a machine the benchmark can run on.

use std::process::Command;

/// What the recording machine was.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Machine {
    /// The processor, as the operating system names it.
    pub cpu: Option<String>,
    /// Logical cores available to this process.
    pub cores: Option<usize>,
    /// Physical memory, whole gibibytes.
    pub memory_gib: Option<u64>,
    /// Operating system and version.
    pub os: Option<String>,
    /// The CI runner image and its version, when the run is on one.
    pub runner: Option<String>,
}

impl Machine {
    /// Reads this machine.
    pub fn sample() -> Self {
        Self {
            cpu: cpu(),
            cores: std::thread::available_parallelism().ok().map(|n| n.get()),
            memory_gib: memory_bytes().map(|bytes| bytes / (1 << 30)),
            os: os(),
            runner: runner(),
        }
    }

    /// The fields that are known, as `key: value` pairs in a fixed order.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        [
            ("cpu", self.cpu.clone()),
            ("cores", self.cores.map(|n| n.to_string())),
            ("memory_gib", self.memory_gib.map(|n| n.to_string())),
            ("os", self.os.clone()),
            ("runner", self.runner.clone()),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect()
    }

    /// One line for the report header.
    pub fn describe(&self) -> String {
        let fields = self.fields();
        if fields.is_empty() {
            return "machine not identified".into();
        }
        fields
            .iter()
            .map(|(key, value)| format!("{key} {value}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn sysctl(name: &str) -> Option<String> {
    let out = Command::new("sysctl").args(["-n", name]).output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn cpu() -> Option<String> {
    if cfg!(target_os = "macos") {
        return sysctl("machdep.cpu.brand_string");
    }
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    text.lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, name)| name.trim().to_string())
}

fn memory_bytes() -> Option<u64> {
    if cfg!(target_os = "macos") {
        return sysctl("hw.memsize")?.parse().ok();
    }
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kib: u64 = text
        .lines()
        .find(|line| line.starts_with("MemTotal:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some(kib * 1024)
}

fn os() -> Option<String> {
    if cfg!(target_os = "macos") {
        let out = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        let version = String::from_utf8(out.stdout).ok()?;
        return Some(format!("macOS {}", version.trim()));
    }
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"').to_string())
}

/// GitHub's hosted runners export the image and its version; anywhere else
/// this is absent, which is the honest answer.
fn runner() -> Option<String> {
    let image = std::env::var("ImageOS").ok()?;
    Some(match std::env::var("ImageVersion") {
        Ok(version) => format!("{image} {version}"),
        Err(_) => image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unidentified_machine_says_so() {
        assert_eq!(Machine::default().describe(), "machine not identified");
    }

    #[test]
    fn only_the_known_fields_are_listed() {
        let machine = Machine {
            cpu: Some("Apple M1 (Virtual)".into()),
            cores: Some(3),
            ..Machine::default()
        };
        assert_eq!(
            machine.fields(),
            vec![("cpu", "Apple M1 (Virtual)".into()), ("cores", "3".into())]
        );
        assert_eq!(machine.describe(), "cpu Apple M1 (Virtual), cores 3");
    }

    #[test]
    fn this_machine_can_at_least_count_its_cores() {
        assert!(Machine::sample().cores.is_some_and(|n| n >= 1));
    }
}
