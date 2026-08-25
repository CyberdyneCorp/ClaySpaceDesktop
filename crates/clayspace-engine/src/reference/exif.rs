//! The one EXIF tag a reference cares about: which way up it is.
//!
//! A photograph off a phone is almost always stored in the sensor's own
//! orientation with a tag saying how to turn it. A viewer that ignores the tag
//! shows a sideways picture — and a sideways reference is not a reference, it
//! is a puzzle. Decoders generally leave this to the caller, so this is the
//! caller doing it.
//!
//! Only the orientation tag is read. Everything else in an EXIF block is
//! someone's camera model, their lens, and often where they were standing,
//! none of which this application has any business holding.

/// How a stored image must be turned to be seen the right way up.
///
/// The eight values EXIF defines, named for what they do rather than for their
/// numbers — `Orientation::Rotate90` is what tag value 6 means, and nobody
/// remembers that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Upright,
    FlipHorizontal,
    Rotate180,
    FlipVertical,
    /// Mirrored, then turned a quarter clockwise.
    TransposeCw,
    Rotate90,
    /// Mirrored, then turned a quarter anticlockwise.
    TransposeCcw,
    Rotate270,
}

impl Orientation {
    /// What EXIF's tag 0x0112 means, or upright for anything unrecognised.
    ///
    /// Unrecognised rather than refused: a corrupt orientation is a reason to
    /// show the picture as stored, not a reason to refuse the picture.
    pub fn from_tag(value: u16) -> Self {
        match value {
            2 => Self::FlipHorizontal,
            3 => Self::Rotate180,
            4 => Self::FlipVertical,
            5 => Self::TransposeCw,
            6 => Self::Rotate90,
            7 => Self::TransposeCcw,
            8 => Self::Rotate270,
            _ => Self::Upright,
        }
    }

    /// Whether applying this swaps width and height.
    pub fn turns_sideways(self) -> bool {
        matches!(
            self,
            Self::TransposeCw | Self::Rotate90 | Self::TransposeCcw | Self::Rotate270
        )
    }

    /// Turns the pixels, returning them with the new dimensions.
    ///
    /// Done to the pixels once here rather than to the quad's corners at draw
    /// time: the placement is the sculptor's, and a rotation folded into it
    /// would make "flip this reference" and "the camera was held sideways"
    /// the same control.
    pub fn apply(self, pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
        if self == Self::Upright {
            return (pixels.to_vec(), width, height);
        }
        let (w, h) = (width as usize, height as usize);
        let (out_w, out_h) = if self.turns_sideways() {
            (h, w)
        } else {
            (w, h)
        };
        let mut out = vec![0u8; out_w * out_h * 4];
        for y in 0..out_h {
            for x in 0..out_w {
                let (sx, sy) = self.source_of(x, y, out_w, out_h);
                let from = (sy * w + sx) * 4;
                let to = (y * out_w + x) * 4;
                out[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
            }
        }
        (out, out_w as u32, out_h as u32)
    }

    /// Where an output pixel reads from in the stored image.
    ///
    /// Written as a pull rather than a push so every output pixel is written
    /// exactly once — a push leaves holes the moment the arithmetic is off by
    /// one, and holes in a photograph are hard to see and easy to ship.
    fn source_of(self, x: usize, y: usize, out_w: usize, out_h: usize) -> (usize, usize) {
        let (last_x, last_y) = (out_w - 1, out_h - 1);
        match self {
            Self::Upright => (x, y),
            Self::FlipHorizontal => (last_x - x, y),
            Self::Rotate180 => (last_x - x, last_y - y),
            Self::FlipVertical => (x, last_y - y),
            Self::TransposeCw => (y, x),
            Self::Rotate90 => (y, last_x - x),
            Self::TransposeCcw => (last_y - y, last_x - x),
            Self::Rotate270 => (last_y - y, x),
        }
    }
}

/// Reads the orientation out of a JPEG's EXIF block.
///
/// EXIF is a TIFF header in a JPEG's clothing: a byte-order mark, a magic 42,
/// an offset to the first directory, and then a count of twelve-byte entries.
/// Only the first directory is walked — the orientation lives there, and the
/// thumbnail's own copy in the second is about the thumbnail.
pub fn orientation_of(exif: &[u8]) -> Orientation {
    // Some producers keep the "Exif\0\0" preamble on the block; others hand
    // over the TIFF header directly.
    let tiff = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
    let Some(order) = ByteOrder::read(tiff) else {
        return Orientation::Upright;
    };

    let first = order.u32(tiff, 4) as usize;
    if first < 8 || first + 2 > tiff.len() {
        return Orientation::Upright;
    }
    let count = order.u16(tiff, first) as usize;
    for entry in 0..count {
        let at = first + 2 + entry * 12;
        if at + 12 > tiff.len() {
            break;
        }
        if order.u16(tiff, at) != 0x0112 {
            continue;
        }
        // A SHORT, so the value sits in the first two bytes of the four-byte
        // value field — at the front whichever way the file is ordered,
        // because the field itself is read in that order.
        return Orientation::from_tag(order.u16(tiff, at + 8));
    }
    Orientation::Upright
}

/// Which way round the numbers in this file are written.
#[derive(Clone, Copy)]
enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    fn read(tiff: &[u8]) -> Option<Self> {
        if tiff.len() < 8 {
            return None;
        }
        let order = match &tiff[..2] {
            b"II" => Self::Little,
            b"MM" => Self::Big,
            _ => return None,
        };
        // The magic 42, which is what says the byte order was read correctly
        // rather than guessed from two plausible letters.
        (order.u16(tiff, 2) == 42).then_some(order)
    }

    fn u16(self, bytes: &[u8], at: usize) -> u16 {
        let Ok(pair) = <[u8; 2]>::try_from(&bytes[at..at + 2]) else {
            return 0;
        };
        match self {
            Self::Little => u16::from_le_bytes(pair),
            Self::Big => u16::from_be_bytes(pair),
        }
    }

    fn u32(self, bytes: &[u8], at: usize) -> u32 {
        let Ok(quad) = <[u8; 4]>::try_from(&bytes[at..at + 4]) else {
            return 0;
        };
        match self {
            Self::Little => u32::from_le_bytes(quad),
            Self::Big => u32::from_be_bytes(quad),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An EXIF block carrying one orientation entry.
    fn exif_with(tag: u16, big_endian: bool) -> Vec<u8> {
        let mut out = Vec::from(*b"Exif\0\0");
        let u16b = |v: u16| -> [u8; 2] {
            if big_endian {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let u32b = |v: u32| -> [u8; 4] {
            if big_endian {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        out.extend_from_slice(if big_endian { b"MM" } else { b"II" });
        out.extend_from_slice(&u16b(42));
        out.extend_from_slice(&u32b(8)); // the first directory follows the header
        out.extend_from_slice(&u16b(1)); // one entry
        out.extend_from_slice(&u16b(0x0112)); // orientation
        out.extend_from_slice(&u16b(3)); // SHORT
        out.extend_from_slice(&u32b(1)); // one of them
        out.extend_from_slice(&u16b(tag));
        out.extend_from_slice(&u16b(0)); // the rest of the value field
        out.extend_from_slice(&u32b(0)); // no next directory
        out
    }

    #[test]
    fn the_tag_is_read_whichever_way_the_file_is_ordered() {
        // Canon writes big-endian and most phones write little; a reader that
        // handles one shows half the world's photographs sideways.
        for big_endian in [false, true] {
            assert_eq!(
                orientation_of(&exif_with(6, big_endian)),
                Orientation::Rotate90,
                "big_endian={big_endian}"
            );
        }
    }

    #[test]
    fn a_block_that_is_not_exif_reads_as_upright() {
        // Upright and not a refusal: a corrupt orientation is a reason to show
        // the picture as stored, not a reason to refuse the picture.
        assert_eq!(orientation_of(b""), Orientation::Upright);
        assert_eq!(orientation_of(b"not exif at all"), Orientation::Upright);
        // Right marker, wrong magic.
        assert_eq!(
            orientation_of(b"Exif\0\0II\x07\x00\x08\x00\x00\x00"),
            Orientation::Upright
        );
        // A tag value nobody defines.
        assert_eq!(orientation_of(&exif_with(99, false)), Orientation::Upright);
    }

    /// A 2×3 image whose pixels are numbered, so a turn can be read off.
    fn numbered() -> (Vec<u8>, u32, u32) {
        let mut pixels = Vec::new();
        for i in 0..6u8 {
            pixels.extend_from_slice(&[i, i, i, 255]);
        }
        (pixels, 2, 3)
    }

    fn firsts(pixels: &[u8]) -> Vec<u8> {
        pixels.chunks_exact(4).map(|p| p[0]).collect()
    }

    #[test]
    fn a_quarter_turn_swaps_the_sides() {
        let (pixels, w, h) = numbered();
        let (turned, tw, th) = Orientation::Rotate90.apply(&pixels, w, h);
        assert_eq!((tw, th), (h, w), "a quarter turn did not swap the sides");
        assert_eq!(turned.len(), pixels.len(), "pixels were lost in the turn");
    }

    #[test]
    fn every_turn_keeps_every_pixel() {
        // A push-based rotation leaves holes the moment the arithmetic is off
        // by one, and holes in a photograph are hard to see and easy to ship.
        let (pixels, w, h) = numbered();
        let original: Vec<u8> = {
            let mut v = firsts(&pixels);
            v.sort_unstable();
            v
        };
        for orientation in [
            Orientation::FlipHorizontal,
            Orientation::Rotate180,
            Orientation::FlipVertical,
            Orientation::TransposeCw,
            Orientation::Rotate90,
            Orientation::TransposeCcw,
            Orientation::Rotate270,
        ] {
            let (turned, tw, th) = orientation.apply(&pixels, w, h);
            assert_eq!(
                (tw as usize * th as usize * 4),
                turned.len(),
                "{orientation:?} produced the wrong number of pixels"
            );
            let mut seen = firsts(&turned);
            seen.sort_unstable();
            assert_eq!(seen, original, "{orientation:?} lost or repeated a pixel");
        }
    }

    #[test]
    fn turning_it_all_the_way_round_gets_back_where_it_started() {
        let (pixels, w, h) = numbered();
        let (once, w1, h1) = Orientation::Rotate90.apply(&pixels, w, h);
        let (twice, w2, h2) = Orientation::Rotate90.apply(&once, w1, h1);
        let (thrice, w3, h3) = Orientation::Rotate90.apply(&twice, w2, h2);
        let (back, w4, h4) = Orientation::Rotate90.apply(&thrice, w3, h3);
        assert_eq!((w4, h4), (w, h));
        assert_eq!(back, pixels, "four quarter turns did not come back round");
    }

    #[test]
    fn an_upright_picture_is_left_alone() {
        let (pixels, w, h) = numbered();
        let (same, sw, sh) = Orientation::Upright.apply(&pixels, w, h);
        assert_eq!((same, sw, sh), (pixels, w, h));
    }
}
