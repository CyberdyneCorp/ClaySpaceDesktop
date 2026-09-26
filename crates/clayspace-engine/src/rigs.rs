//! Each rig's skin thickness, kept beside the document.
//!
//! The `.clayspace` holds a rig's radii with its thickness already applied and
//! nothing that says what the thickness was, so without this a reopened rig
//! comes back at the default with the multiplier baked into what it reports as
//! authored — the surface is the same and the slider is not. The thickness is
//! the rig's own (one per subtool that holds a rig), so it is keyed the way the
//! hierarchies are: by stack position, which is what a reopened document mints
//! its rows from. See [`crate::multires::Saved`] for why a position and not a
//! key or a name.
//!
//! Bookkeeping rather than work: a missing or malformed file reopens every rig
//! at the default thickness over the same surface, so every failure is a
//! dropped row and never a refused file.

/// Where the rig thicknesses live for a document at `path`.
pub fn sidecar_for(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".rigs");
    path.with_file_name(name)
}

/// The first line of the file, so a later format can be told from this one.
const HEADER: &str = "clayspace-rigs 1";

/// One rig's thickness, and the row it belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SavedSkin {
    /// Counted from the bottom of the stack.
    pub position: usize,
    pub thickness: f32,
}

/// Writes one line per rig, or removes the file when there is none to write.
///
/// Removed rather than left standing, for the reason a hierarchy side-car is:
/// a stale file beside a document whose rigs are all at the default would hand
/// a thickness to whatever rig later stands at that position.
pub fn write_skins(path: &std::path::Path, skins: &[SavedSkin]) -> std::io::Result<()> {
    if skins.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
    }
    let mut out = String::from(HEADER);
    out.push('\n');
    for skin in skins {
        out.push_str(&format!("{} {}\n", skin.position, skin.thickness));
    }
    std::fs::write(path, out)
}

/// Reads the thicknesses back, dropping any row this build cannot read.
pub fn read_skins(path: &std::path::Path) -> Vec<SavedSkin> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Vec::new();
    }
    lines.filter_map(read_row).collect()
}

fn read_row(line: &str) -> Option<SavedSkin> {
    let mut fields = line.split_whitespace();
    let position = fields.next()?.parse().ok()?;
    let thickness: f32 = fields.next()?.parse().ok()?;
    // A thickness that is not a positive number describes no skin; the rig
    // reopens at the default rather than at something the slider cannot show.
    (thickness.is_finite() && thickness > 0.0).then_some(SavedSkin {
        position,
        thickness,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "clayspace-rigs-{name}-{}.clayspace",
            std::process::id()
        ))
    }

    #[test]
    fn thicknesses_round_trip_by_position() {
        let sidecar = sidecar_for(&scratch("round-trip"));
        assert!(sidecar.to_string_lossy().ends_with(".clayspace.rigs"));
        let skins = [
            SavedSkin {
                position: 1,
                thickness: 2.0,
            },
            SavedSkin {
                position: 3,
                thickness: 0.35,
            },
        ];
        write_skins(&sidecar, &skins).expect("write");
        assert_eq!(read_skins(&sidecar), skins);

        write_skins(&sidecar, &[]).expect("remove");
        assert!(!sidecar.exists(), "nothing to keep leaves no file behind");
        assert!(read_skins(&sidecar).is_empty());
    }

    #[test]
    fn a_malformed_row_is_dropped_and_the_rest_kept() {
        let sidecar = sidecar_for(&scratch("malformed"));
        std::fs::write(&sidecar, format!("{HEADER}\n0 1.5\nx 2\n2 -1\n3\n4 0.5\n")).expect("write");
        let read = read_skins(&sidecar);
        let _ = std::fs::remove_file(&sidecar);
        assert_eq!(
            read,
            [
                SavedSkin {
                    position: 0,
                    thickness: 1.5
                },
                SavedSkin {
                    position: 4,
                    thickness: 0.5
                },
            ]
        );
    }

    #[test]
    fn an_unknown_header_reads_as_nothing() {
        let sidecar = sidecar_for(&scratch("header"));
        std::fs::write(&sidecar, "clayspace-rigs 9\n0 2\n").expect("write");
        let read = read_skins(&sidecar);
        let _ = std::fs::remove_file(&sidecar);
        assert!(read.is_empty());
    }
}
