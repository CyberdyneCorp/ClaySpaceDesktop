//! The PNG half of the reference reader.

use std::path::Path;

use clayspace_model::ReferenceRefusal;

use super::{check_filled, check_size, Decoded};

pub(super) fn read(path: &Path) -> Result<Decoded, ReferenceRefusal> {
    let file =
        std::fs::File::open(path).map_err(|e| ReferenceRefusal::Unreadable(e.to_string()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    // Palette entries expanded to colour, and a transparency chunk to an alpha
    // channel. The alpha reader can afford to read an index as a grey level
    // because a stamp is one scalar either way; a reference is a picture, and
    // a palette index read as a colour is not a picture of anything.
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| ReferenceRefusal::Unreadable(e.to_string()))?;

    let info = reader.info();
    let (width, height) = (info.width, info.height);
    check_size(width, height)?;

    let size = reader
        .output_buffer_size()
        .ok_or_else(|| ReferenceRefusal::Unreadable("dimensões impossíveis".into()))?;
    let mut buffer = vec![0u8; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|e| ReferenceRefusal::Unreadable(e.to_string()))?;

    let pixels = to_rgba(
        &buffer[..frame.buffer_size()],
        frame.color_type,
        frame.bit_depth,
    );
    check_filled(&pixels, width, height)?;
    Ok(Decoded {
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
