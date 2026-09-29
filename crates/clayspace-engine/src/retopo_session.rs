//! Authored retopology input kept beside the sculpt document.
//!
//! A guide or density dab is not geometry, and ClayCore's file has no slot for
//! it. The companion file is versioned and written after the sculpt succeeds.

use std::path::{Path, PathBuf};

use clayspace_model::{DensityDab, FlowGuide, FlowGuideMode, RetopoGuidance};

const HEADER: &str = "clayspace-retopo 1";

pub fn sidecar_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".retopo");
    path.with_file_name(name)
}

pub fn write_guidance(path: &Path, guidance: &RetopoGuidance) -> std::io::Result<()> {
    if guidance.guides.is_empty() && guidance.density.is_empty() {
        return match std::fs::remove_file(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
    }
    if !guidance.valid() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "retopology guidance contains invalid values",
        ));
    }
    let mut text = String::from(HEADER);
    text.push('\n');
    for guide in &guidance.guides {
        let mode = match guide.mode {
            FlowGuideMode::Orientation => "orientation",
            FlowGuideMode::Topology => "topology",
        };
        text.push_str(&format!(
            "guide {} {} {mode} {} {}\n",
            guide.strength,
            guide.radius,
            u8::from(guide.closed),
            guide.points.len()
        ));
        for point in &guide.points {
            text.push_str(&format!("point {} {} {}\n", point[0], point[1], point[2]));
        }
    }
    for dab in &guidance.density {
        text.push_str(&format!(
            "density {} {} {} {} {}\n",
            dab.position[0], dab.position[1], dab.position[2], dab.radius, dab.multiplier
        ));
    }
    let temporary = path.with_extension("retopo.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)
}

pub fn read_guidance(path: &Path) -> RetopoGuidance {
    let Ok(text) = std::fs::read_to_string(path) else {
        return RetopoGuidance::default();
    };
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return RetopoGuidance::default();
    }
    let mut guidance = RetopoGuidance::default();
    while let Some(line) = lines.next() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("guide") => {
                if let Some(guide) = read_guide(&mut fields, &mut lines) {
                    guidance.guides.push(guide);
                }
            }
            Some("density") => {
                if let Some(dab) = read_density(&mut fields) {
                    guidance.density.push(dab);
                }
            }
            _ => {}
        }
    }
    guidance
}

fn read_guide<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
    lines: &mut impl Iterator<Item = &'a str>,
) -> Option<FlowGuide> {
    let strength = fields.next()?.parse().ok()?;
    let radius = fields.next()?.parse().ok()?;
    let mode = match fields.next()? {
        "orientation" => FlowGuideMode::Orientation,
        "topology" => FlowGuideMode::Topology,
        _ => return None,
    };
    let closed = match fields.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let count: usize = fields.next()?.parse().ok()?;
    if !(2..=100_000).contains(&count) {
        return None;
    }
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        let mut point = lines.next()?.split_whitespace();
        if point.next()? != "point" {
            return None;
        }
        points.push([
            point.next()?.parse().ok()?,
            point.next()?.parse().ok()?,
            point.next()?.parse().ok()?,
        ]);
    }
    let guide = FlowGuide {
        points,
        strength,
        radius,
        mode,
        closed,
    };
    guide.valid().then_some(guide)
}

fn read_density<'a>(fields: &mut impl Iterator<Item = &'a str>) -> Option<DensityDab> {
    let dab = DensityDab {
        position: [
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
            fields.next()?.parse().ok()?,
        ],
        radius: fields.next()?.parse().ok()?,
        multiplier: fields.next()?.parse().ok()?,
    };
    dab.valid().then_some(dab)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_and_density_survive_save_and_reopen() {
        let path = std::env::temp_dir().join(format!(
            "clayspace-retopo-session-{}.clayspace.retopo",
            std::process::id()
        ));
        let guidance = RetopoGuidance {
            guides: vec![FlowGuide {
                points: vec![[0.0, 1.0, 0.0], [0.5, 1.0, 0.0]],
                strength: 0.8,
                radius: 0.25,
                mode: FlowGuideMode::Topology,
                closed: false,
            }],
            density: vec![DensityDab {
                position: [0.1, 1.0, 0.0],
                radius: 0.2,
                multiplier: 2.0,
            }],
        };
        write_guidance(&path, &guidance).expect("save guidance");
        assert_eq!(read_guidance(&path), guidance);
        write_guidance(&path, &RetopoGuidance::default()).expect("clear guidance");
        assert!(!path.exists());
    }
}
