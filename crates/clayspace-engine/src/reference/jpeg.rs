//! The JPEG half of the reference reader.
//!
//! What a photograph arrives as. Decoded to RGB and made opaque: JPEG has no
//! alpha channel, and the opacity a sculptor sets is a property of the
//! reference rather than of the file.

use std::path::Path;

use clayspace_model::ReferenceRefusal;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

use super::exif::{orientation_of, Orientation};
use super::{check_filled, check_size, Decoded};

pub(super) fn read(path: &Path) -> Result<Decoded, ReferenceRefusal> {
    let bytes = std::fs::read(path).map_err(|e| ReferenceRefusal::Unreadable(e.to_string()))?;

    // RGB out whatever came in, so a greyscale photograph is grey rather than
    // read as one channel and sampled as red.
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&bytes), options);
    decoder
        .decode_headers()
        .map_err(|e| ReferenceRefusal::Unreadable(e.to_string()))?;
    let info = decoder
        .info()
        .ok_or_else(|| ReferenceRefusal::Unreadable("sem cabeçalho".into()))?;

    // Checked from the header, before the decode allocates for it.
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    check_size(width, height)?;

    // Read before decoding, because decoding consumes the borrow and this is
    // the only tag worth keeping.
    let orientation = info
        .exif_data
        .as_deref()
        .map(orientation_of)
        .unwrap_or(Orientation::Upright);

    let rgb = decoder
        .decode()
        .map_err(|e| ReferenceRefusal::Unreadable(e.to_string()))?;
    let pixels = opaque_rgba(&rgb);
    check_filled(&pixels, width, height)?;

    // A phone stores the sensor's own orientation and a tag saying how to turn
    // it. Turned here, once, so the placement the sculptor sets is about the
    // reference and not about how the camera was held.
    let (pixels, width, height) = orientation.apply(&pixels, width, height);
    Ok(Decoded {
        width,
        height,
        pixels,
    })
}

/// Three channels in, four out, opaque.
fn opaque_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
    for pixel in rgb.chunks_exact(3) {
        out.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    out
}
