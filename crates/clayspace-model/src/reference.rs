//! Reference images, sat behind the sculpt.
//!
//! What an artist pins to the wall beside the monitor, put where it belongs:
//! on the plane the view preset looks down, behind the form so the silhouette
//! can be read against it. Blender calls them background images and ZBrush
//! spells the same idea as Spotlight.
//!
//! Not part of the document. A reference is what the sculptor is working
//! *from*, not what they are making — a document carrying someone else's
//! photograph is a document that cannot be shared. The settings are remembered
//! with the session instead, beside the recent files.

/// Which plane a reference sits on.
///
/// The three the view presets look down. Perspective has none: a reference is
/// a flat thing seen square on, and one hanging in a perspective view is a
/// billboard rather than a guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum RefPlane {
    /// Seen from the front, so the image faces down −Z.
    #[default]
    Front,
    /// Seen from the side, facing down −X.
    Side,
    /// Seen from above, facing down −Y.
    Top,
}

impl RefPlane {
    pub const ALL: [RefPlane; 3] = [Self::Front, Self::Side, Self::Top];

    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "Frontal",
            Self::Side => "Lateral",
            Self::Top => "Superior",
        }
    }

    /// The name this plane is written down under.
    ///
    /// Its own word rather than its position in `ALL`, so a file written by
    /// one version is still read by the next after a plane is added.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Side => "side",
            Self::Top => "top",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|plane| plane.tag() == tag)
    }

    /// The axis the plane's own normal runs along.
    pub fn axis(self) -> usize {
        match self {
            Self::Front => 2,
            Self::Side => 0,
            Self::Top => 1,
        }
    }

    /// The two axes the image lies in, across and up.
    ///
    /// Chosen so each reads the way the matching view preset shows it: a front
    /// reference is x across and y up, which is what a photograph of a face
    /// is, and a top-down one is x across and z up.
    pub fn axes(self) -> (usize, usize) {
        match self {
            Self::Front => (0, 1),
            Self::Side => (2, 1),
            Self::Top => (0, 2),
        }
    }
}

/// How one reference is placed and drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceSettings {
    pub visible: bool,
    /// 0 is invisible and 1 is opaque.
    pub opacity: f32,
    /// How tall the image stands, in document units. The width follows from
    /// the image's own proportions, so a reference is never squashed.
    pub height: f32,
    /// Where it sits in its own plane, across and up.
    pub offset: [f32; 2],
    /// How far behind the origin it sits, along the plane's normal.
    ///
    /// Behind, so the form is in front of it and the silhouette reads against
    /// it. A reference in front of the clay would be a sticker over the work.
    pub depth: f32,
}

impl Default for ReferenceSettings {
    fn default() -> Self {
        Self {
            visible: true,
            // Not opaque. A reference at full strength reads as the subject
            // and the clay in front of it reads as a smudge; half is enough to
            // trace against and little enough to see past.
            opacity: 0.5,
            height: 2.0,
            offset: [0.0; 2],
            depth: 1.5,
        }
    }
}

impl ReferenceSettings {
    /// How tall a reference can be made, in document units.
    ///
    /// A floor rather than zero: a reference scaled to nothing is gone, with
    /// no way to get it back but a number nobody can see.
    pub const HEIGHT_RANGE: std::ops::RangeInclusive<f32> = 0.05..=100.0;
    /// How far a reference can be moved or pushed back.
    pub const OFFSET_RANGE: std::ops::RangeInclusive<f32> = -100.0..=100.0;

    /// Clamped to what can be drawn and seen.
    pub fn sanitized(self) -> Self {
        Self {
            visible: self.visible,
            opacity: self.opacity.clamp(0.0, 1.0),
            height: self
                .height
                .clamp(*Self::HEIGHT_RANGE.start(), *Self::HEIGHT_RANGE.end()),
            offset: self
                .offset
                .map(|c| c.clamp(*Self::OFFSET_RANGE.start(), *Self::OFFSET_RANGE.end())),
            depth: self
                .depth
                .clamp(*Self::OFFSET_RANGE.start(), *Self::OFFSET_RANGE.end()),
        }
    }

    /// The corners of the quad, given how wide the image is against its height.
    ///
    /// Returned in world space, so the caller places nothing itself: across
    /// and up are the plane's own axes and the third is the depth.
    pub fn corners(self, plane: RefPlane, aspect: f32) -> [[f32; 3]; 4] {
        let settings = self.sanitized();
        let half_up = settings.height * 0.5;
        let half_across = half_up * aspect.max(1e-3);
        let (across, up) = plane.axes();
        let normal = plane.axis();
        // Behind the origin, and on the side the matching preset looks from.
        let depth = -settings.depth;
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(u, v)| {
            let mut at = [0.0f32; 3];
            at[across] = settings.offset[0] + u * half_across;
            at[up] = settings.offset[1] + v * half_up;
            at[normal] = depth;
            at
        })
    }
}

/// Which picture formats a reference can be loaded from.
///
/// PNG for drawings and cut-outs, JPEG for photographs — between them that is
/// what a sculptor's reference folder actually holds. Named here rather than
/// in the reader so the file dialog, the refusal message and the decoder
/// cannot come to three different answers about what is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefFormat {
    Png,
    Jpeg,
}

impl RefFormat {
    /// Every extension that opens, lower case and without the dot.
    pub const EXTENSIONS: [&'static str; 3] = ["png", "jpg", "jpeg"];

    /// What a file's extension says it is, or `None` for anything else.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }
}

/// Why a file could not become a reference.
///
/// Its own type rather than the alpha reader's, which it borrowed at first:
/// the two now accept different formats, and a refusal that tells a sculptor
/// loading a photograph that "alphas are read only in PNG" is answering a
/// question nobody asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceRefusal {
    /// Not a format that opens. Stated by name rather than left to a decoder
    /// error naming a library the sculptor has never heard of.
    UnsupportedFormat {
        extension: String,
    },
    /// The file could not be read or decoded.
    Unreadable(String),
    TooSmall {
        width: u32,
        height: u32,
    },
    TooLarge {
        width: u32,
        height: u32,
    },
    /// The pixels do not fill the dimensions the header claims.
    Truncated {
        expected: usize,
        found: usize,
    },
}

impl std::fmt::Display for ReferenceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat { extension } if extension.is_empty() => {
                f.write_str("as referências são lidas em PNG e JPEG; este arquivo não tem extensão")
            }
            Self::UnsupportedFormat { extension } => write!(
                f,
                "as referências são lidas em PNG e JPEG; este é um {}",
                extension.to_uppercase()
            ),
            Self::Unreadable(why) => write!(f, "a imagem não pôde ser lida: {why}"),
            Self::TooSmall { width, height } => write!(
                f,
                "uma referência de {width}×{height} não dá o que ver; \
                 o mínimo é {min}×{min}",
                min = ReferenceImage::MIN_SIDE
            ),
            Self::TooLarge { width, height } => write!(
                f,
                "uma referência de {width}×{height} passa do limite de {max}×{max}",
                max = ReferenceImage::MAX_SIDE
            ),
            Self::Truncated { expected, found } => write!(
                f,
                "a imagem terminou antes do fim: {found} de {expected} bytes"
            ),
        }
    }
}

impl std::error::Error for ReferenceRefusal {}

/// A loaded reference image.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceImage {
    /// What the interface calls it. The file's stem.
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// `width * height` pixels, row-major, RGBA.
    pub pixels: Vec<u8>,
}

impl ReferenceImage {
    /// The smallest image that is an image.
    pub const MIN_SIDE: u32 = 2;
    /// The largest one that is worth putting on a texture.
    ///
    /// The same ceiling the alpha loader takes, and for the same reason: past
    /// it the refusal is kinder than the allocation.
    pub const MAX_SIDE: u32 = 8192;

    /// How wide the image is against its height.
    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            return 1.0;
        }
        self.width as f32 / self.height as f32
    }
}

/// One plane's reference, as a session remembers it.
///
/// The path and not the pixels. A session that cached the images would be a
/// second copy of someone else's photograph, kept without being asked; a path
/// that no longer resolves is simply dropped on the way in.
#[derive(Debug, Clone, PartialEq)]
pub struct RememberedReference {
    pub plane: RefPlane,
    pub path: std::path::PathBuf,
    pub settings: ReferenceSettings,
}

/// Writes the remembered references as one line each.
///
/// Tab-separated with the path last, so a path containing a space — or a
/// comma, or an equals sign — comes back the way it went in.
pub fn write_references(entries: &[RememberedReference]) -> String {
    let mut text = String::new();
    for entry in entries {
        let s = entry.settings.sanitized();
        let visible = if s.visible { 1 } else { 0 };
        text.push_str(&format!(
            "{}\t{visible}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            entry.plane.tag(),
            s.opacity,
            s.height,
            s.offset[0],
            s.offset[1],
            s.depth,
            entry.path.to_string_lossy(),
        ));
    }
    text
}

/// Reads back what `write_references` wrote, skipping anything it cannot.
///
/// A line that does not parse is dropped rather than defaulted: a reference
/// placed somewhere the sculptor did not put it is worse than one that is
/// simply gone, because the second is obvious and the first is not.
pub fn read_references(text: &str) -> Vec<RememberedReference> {
    text.lines().filter_map(read_reference_line).collect()
}

fn read_reference_line(line: &str) -> Option<RememberedReference> {
    let mut fields = line.splitn(8, '\t');
    let plane = RefPlane::from_tag(fields.next()?.trim())?;
    let visible = fields.next()?.trim() != "0";
    let mut number = || fields.next()?.trim().parse::<f32>().ok();
    let opacity = number()?;
    let height = number()?;
    let across = number()?;
    let up = number()?;
    let depth = number()?;
    // The path last and untrimmed: leading whitespace in a file name is legal
    // and trimming it would look up a file that does not exist.
    let path = fields.next()?;
    if path.is_empty() {
        return None;
    }
    Some(RememberedReference {
        plane,
        path: std::path::PathBuf::from(path),
        settings: ReferenceSettings {
            visible,
            opacity,
            height,
            offset: [across, up],
            depth,
        }
        .sanitized(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_sits_behind_the_origin() {
        // In front of it, a reference is a sticker over the work rather than
        // something to read the silhouette against.
        for plane in RefPlane::ALL {
            let corners = ReferenceSettings::default().corners(plane, 1.0);
            let axis = plane.axis();
            for corner in corners {
                assert!(
                    corner[axis] < 0.0,
                    "a {} reference sits at {} on its own axis",
                    plane.label(),
                    corner[axis]
                );
            }
        }
    }

    #[test]
    fn the_image_keeps_its_proportions() {
        // A reference squashed to a square is a reference that lies about what
        // it is a picture of.
        let settings = ReferenceSettings {
            height: 2.0,
            ..Default::default()
        };
        let wide = settings.corners(RefPlane::Front, 2.0);
        let (across, up) = RefPlane::Front.axes();
        let width = wide[1][across] - wide[0][across];
        let tall = wide[2][up] - wide[1][up];
        assert!(
            (width - tall * 2.0).abs() < 1e-4,
            "a 2:1 image came out {width} by {tall}"
        );
    }

    #[test]
    fn each_plane_lies_in_its_own_two_axes() {
        // And no plane repeats an axis, which would give it no thickness to
        // face along.
        for plane in RefPlane::ALL {
            let (across, up) = plane.axes();
            let normal = plane.axis();
            assert_ne!(across, up);
            assert_ne!(across, normal);
            assert_ne!(up, normal);
        }
    }

    #[test]
    fn the_settings_are_clamped_to_what_can_be_seen() {
        let mad = ReferenceSettings {
            visible: true,
            opacity: 4.0,
            height: 0.0,
            offset: [1e9, -1e9],
            depth: 1e9,
        }
        .sanitized();
        assert_eq!(mad.opacity, 1.0);
        assert!(
            mad.height > 0.0,
            "a reference scaled to nothing is gone with no way back"
        );
        assert!(mad.offset[0].is_finite() && mad.depth.is_finite());
    }

    #[test]
    fn it_starts_visible_and_half_way_through() {
        // Opaque, a reference reads as the subject and the clay in front of it
        // reads as a smudge.
        let settings = ReferenceSettings::default();
        assert!(settings.visible);
        assert!(settings.opacity > 0.0 && settings.opacity < 1.0);
    }

    #[test]
    fn a_placement_comes_back_the_way_it_went_in() {
        let entries = vec![
            RememberedReference {
                plane: RefPlane::Front,
                path: "/casa/ana/desenhos/rosto de frente.png".into(),
                settings: ReferenceSettings {
                    visible: false,
                    opacity: 0.25,
                    height: 3.5,
                    offset: [0.5, -0.75],
                    depth: 2.0,
                },
            },
            RememberedReference {
                plane: RefPlane::Top,
                path: "/casa/ana/desenhos/de cima.png".into(),
                settings: ReferenceSettings::default(),
            },
        ];
        assert_eq!(read_references(&write_references(&entries)), entries);
    }

    #[test]
    fn a_line_that_does_not_parse_is_dropped_rather_than_defaulted() {
        // A reference placed somewhere the sculptor did not put it is worse
        // than one that is gone: the second is obvious and the first is not.
        let good = write_references(&[RememberedReference {
            plane: RefPlane::Side,
            path: "/casa/ana/lado.png".into(),
            settings: ReferenceSettings::default(),
        }]);
        let text = format!("lixo\nfront\t1\tnão é um número\n\n{good}");
        let read = read_references(&text);
        assert_eq!(read.len(), 1, "a corrupted line was believed: {read:?}");
        assert_eq!(read[0].plane, RefPlane::Side);
    }

    #[test]
    fn a_plane_is_written_down_by_name() {
        // Its own word rather than its position, so a file written by one
        // version still reads after a plane is added to the list.
        for plane in RefPlane::ALL {
            assert_eq!(RefPlane::from_tag(plane.tag()), Some(plane));
        }
        assert_eq!(RefPlane::from_tag("oblíquo"), None);
    }
}
