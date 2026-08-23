//! Reading an alpha stamp off disk.
//!
//! ClayCore decodes no images and says so in as many words: "THE ENGINE DECODES
//! NO IMAGES. A host with an alpha has already loaded a PNG; it hands over the
//! samples. That rule is what keeps an image decoder out of a library that
//! compiles to five backends." So the decoding is here, in the adapter, beside
//! the mesh import that is the other place a file becomes something the domain
//! can hold.
//!
//! PNG only. It is lossless, which matters for a stamp — a JPEG's ringing
//! around an edge becomes a ridge in the surface — and it is what every alpha
//! library ships. A file that is not one is refused by name rather than handed
//! to a decoder that will fail with a message about chunk headers.

use std::path::Path;

use clayspace_model::{Alpha, AlphaRefusal};

/// Reads a PNG into a stamp.
///
/// Any bit depth and colour type the decoder handles; the result is one scalar
/// per pixel in 0..=1. Colour is flattened to luminance rather than refused: an
/// alpha library's stamps are greyscale in intent and often RGB in storage, and
/// refusing those would refuse most of what a sculptor already owns.
pub fn read_alpha(path: &Path) -> Result<Alpha, AlphaRefusal> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "png" {
        return Err(AlphaRefusal::NotPng { extension });
    }

    let file = std::fs::File::open(path).map_err(|e| AlphaRefusal::Unreadable(e.to_string()))?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .map_err(|e| AlphaRefusal::Unreadable(e.to_string()))?;

    // The dimensions are checked before the buffer is allocated. A malformed
    // header can claim a gigapixel image, and `output_buffer_size` would be
    // asked for it.
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    if width < Alpha::MIN_SIDE || height < Alpha::MIN_SIDE {
        return Err(AlphaRefusal::TooSmall { width, height });
    }
    if width > Alpha::MAX_SIDE || height > Alpha::MAX_SIDE {
        return Err(AlphaRefusal::TooLarge { width, height });
    }

    let size = reader
        .output_buffer_size()
        .ok_or_else(|| AlphaRefusal::Unreadable("dimensões impossíveis".into()))?;
    let mut buffer = vec![0u8; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|e| AlphaRefusal::Unreadable(e.to_string()))?;

    let samples = to_scalars(
        &buffer[..frame.buffer_size()],
        frame.color_type,
        frame.bit_depth,
    );
    Alpha {
        name: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("alfa")
            .to_string(),
        width,
        height,
        samples,
    }
    .validated()
}

/// One scalar per pixel, whatever the file stored.
fn to_scalars(bytes: &[u8], color: png::ColorType, depth: png::BitDepth) -> Vec<f32> {
    let channels = color.samples();
    // Sixteen-bit files are delivered big-endian, two bytes a sample. Only the
    // high byte is read: a stamp is a height field and the low byte is below
    // what the falloff resolves, so keeping it would cost twice the memory for
    // nothing visible.
    let stride = if depth == png::BitDepth::Sixteen {
        2
    } else {
        1
    };
    let step = channels * stride;
    if step == 0 {
        return Vec::new();
    }

    bytes
        .chunks_exact(step)
        .map(|pixel| luminance(pixel, color, stride))
        .collect()
}

/// The stamp's value at one pixel, in 0..=1.
fn luminance(pixel: &[u8], color: png::ColorType, stride: usize) -> f32 {
    let at = |channel: usize| pixel[channel * stride] as f32 / 255.0;
    match color {
        // Already one channel, with or without an alpha channel beside it.
        png::ColorType::Grayscale | png::ColorType::GrayscaleAlpha => at(0),
        // Rec. 601 luma, which is what a greyscale conversion means to
        // everybody who made the stamp.
        png::ColorType::Rgb | png::ColorType::Rgba => 0.299 * at(0) + 0.587 * at(1) + 0.114 * at(2),
        // Palettes are expanded by the decoder before this sees them unless
        // the caller asked otherwise, which it does not; treat an index as a
        // grey so a stamp is still produced rather than a panic.
        png::ColorType::Indexed => at(0),
    }
}
