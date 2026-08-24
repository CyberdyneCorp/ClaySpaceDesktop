//! Reading a reference image off disk.
//!
//! The sibling of the alpha reader beside it, and for the same reason: ClayCore
//! decodes no images and says so in as many words, so decoding lives in the
//! adapter. PNG only, and refused by name rather than handed to a decoder that
//! will fail with a message about chunk headers.
//!
//! Where a stamp is flattened to one scalar per pixel, a reference keeps its
//! colour: it is a photograph, and a photograph in grey is a different
//! reference from the one the sculptor chose.

use std::path::Path;

use clayspace_model::{AlphaRefusal, ReferenceImage};

/// Reads a PNG into a reference image.
///
/// Any bit depth and colour type the decoder handles; the result is RGBA, eight
/// bits a channel, which is what a texture takes. A file with no alpha of its
/// own comes back opaque — the *opacity* a sculptor sets is a property of the
/// reference and not of the file, so a photograph and a cut-out are placed the
/// same way.
///
/// Shares [`AlphaRefusal`] rather than inventing a parallel set: the reasons a
/// PNG cannot be used are the same reasons whichever thing it was going to be,
/// and one of them already reads well in three languages.
pub fn read_reference(path: &Path) -> Result<ReferenceImage, AlphaRefusal> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "png" {
        return Err(AlphaRefusal::NotPng { extension });
    }

    let file = std::fs::File::open(path).map_err(|e| AlphaRefusal::Unreadable(e.to_string()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    // Palette entries expanded to colour, and a transparency chunk to an alpha
    // channel. The alpha reader can afford to read an index as a grey level
    // because a stamp is one scalar either way; a reference is a picture, and
    // a palette index read as a colour is not a picture of anything.
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| AlphaRefusal::Unreadable(e.to_string()))?;

    // Checked before the buffer is allocated, as the alpha reader does: a
    // malformed header can claim a gigapixel image and `output_buffer_size`
    // would be asked for it.
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    if width < ReferenceImage::MIN_SIDE || height < ReferenceImage::MIN_SIDE {
        return Err(AlphaRefusal::TooSmall { width, height });
    }
    if width > ReferenceImage::MAX_SIDE || height > ReferenceImage::MAX_SIDE {
        return Err(AlphaRefusal::TooLarge { width, height });
    }

    let size = reader
        .output_buffer_size()
        .ok_or_else(|| AlphaRefusal::Unreadable("dimensões impossíveis".into()))?;
    let mut buffer = vec![0u8; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|e| AlphaRefusal::Unreadable(e.to_string()))?;

    let pixels = to_rgba(
        &buffer[..frame.buffer_size()],
        frame.color_type,
        frame.bit_depth,
    );
    let wanted = width as usize * height as usize * 4;
    if pixels.len() != wanted {
        return Err(AlphaRefusal::Truncated {
            expected: wanted,
            found: pixels.len(),
        });
    }

    Ok(ReferenceImage {
        name: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("referência")
            .to_string(),
        width,
        height,
        pixels,
    })
}

/// Four bytes a pixel, whatever the file stored.
fn to_rgba(bytes: &[u8], color: png::ColorType, depth: png::BitDepth) -> Vec<u8> {
    let channels = color.samples();
    // Sixteen-bit files arrive big-endian, two bytes a sample; the high byte
    // is the one a texture keeps. The same trade the alpha reader makes, and
    // here it is not even a trade: the target format is eight bits a channel.
    let stride = if depth == png::BitDepth::Sixteen {
        2
    } else {
        1
    };
    let step = channels * stride;
    if step == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(bytes.len() / step * 4);
    for pixel in bytes.chunks_exact(step) {
        let at = |channel: usize| pixel[channel * stride];
        let (r, g, b, a) = match color {
            // Grey, and grey with alpha. Spread across the three channels
            // rather than left in one: a texture sampled for colour would
            // otherwise read a grey photograph as red.
            png::ColorType::Grayscale => (at(0), at(0), at(0), 255),
            png::ColorType::GrayscaleAlpha => (at(0), at(0), at(0), at(1)),
            png::ColorType::Rgb => (at(0), at(1), at(2), 255),
            png::ColorType::Rgba => (at(0), at(1), at(2), at(3)),
            // Indexed is expanded to RGB before it reaches here. Anything
            // else is read as far as it goes and made opaque, which is a
            // reference that looks wrong rather than one that refuses.
            _ => (at(0), at(channels.saturating_sub(1)), at(0), 255),
        };
        out.extend_from_slice(&[r, g, b, a]);
    }
    out
}
