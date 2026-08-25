//! Reading a reference image off disk.
//!
//! The sibling of the alpha reader beside it, and for the same reason: ClayCore
//! decodes no images and says so in as many words, so decoding lives in the
//! adapter. PNG and JPEG, refused by name rather than handed to a decoder that
//! will fail with a message about chunk headers.
//!
//! Where a stamp is flattened to one scalar per pixel, a reference keeps its
//! colour: it is a photograph, and a photograph in grey is a different
//! reference from the one the sculptor chose.

use std::path::Path;

use clayspace_model::{RefFormat, ReferenceImage, ReferenceRefusal};

mod exif;
mod jpeg;
mod png_reader;

/// Reads a picture into a reference image.
///
/// The result is RGBA, eight bits a channel, which is what a texture takes. A
/// file with no alpha of its own comes back opaque — the *opacity* a sculptor
/// sets is a property of the reference and not of the file, so a photograph
/// and a cut-out are placed the same way.
pub fn read_reference(path: &Path) -> Result<ReferenceImage, ReferenceRefusal> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let Some(format) = RefFormat::from_extension(&extension) else {
        return Err(ReferenceRefusal::UnsupportedFormat { extension });
    };

    let decoded = match format {
        RefFormat::Png => png_reader::read(path)?,
        RefFormat::Jpeg => jpeg::read(path)?,
    };

    Ok(ReferenceImage {
        name: path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("referência")
            .to_string(),
        width: decoded.width,
        height: decoded.height,
        pixels: decoded.pixels,
    })
}

/// A decoded picture, before it is given a name.
pub(crate) struct Decoded {
    pub width: u32,
    pub height: u32,
    /// `width * height` pixels, row-major, RGBA.
    pub pixels: Vec<u8>,
}

/// Refuses a size that is not worth putting on a texture.
///
/// Called with the *header's* dimensions, before any buffer is allocated: a
/// malformed header can claim a gigapixel image and the allocation would be
/// made before anyone asked whether the image was usable.
pub(crate) fn check_size(width: u32, height: u32) -> Result<(), ReferenceRefusal> {
    if width < ReferenceImage::MIN_SIDE || height < ReferenceImage::MIN_SIDE {
        return Err(ReferenceRefusal::TooSmall { width, height });
    }
    if width > ReferenceImage::MAX_SIDE || height > ReferenceImage::MAX_SIDE {
        return Err(ReferenceRefusal::TooLarge { width, height });
    }
    Ok(())
}

/// Refuses a buffer that does not fill the dimensions it claims.
pub(crate) fn check_filled(pixels: &[u8], width: u32, height: u32) -> Result<(), ReferenceRefusal> {
    let expected = width as usize * height as usize * 4;
    if pixels.len() != expected {
        return Err(ReferenceRefusal::Truncated {
            expected,
            found: pixels.len(),
        });
    }
    Ok(())
}
