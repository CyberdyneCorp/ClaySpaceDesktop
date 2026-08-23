//! Reading an alpha stamp, and refusing what is not one.
//!
//! Every PNG here is written by the test that reads it. A fixture checked in
//! beside the code drifts from what the decoder is asked to handle, and the
//! interesting cases — sixteen-bit, palette, greyscale-with-alpha — are exactly
//! the ones nobody remembers to add a fixture for.

use clayspace_engine::read_alpha;
use clayspace_model::{Alpha, AlphaRefusal};

fn temp(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-alpha-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Writes a PNG with the given colour type and depth, and a gradient in it.
fn write_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &[u8],
) {
    let file = std::fs::File::create(path).expect("create");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(color);
    encoder.set_depth(depth);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(data).expect("pixels");
}

/// A grey ramp, dark on the left and light on the right.
fn ramp(width: u32, height: u32) -> Vec<u8> {
    (0..height)
        .flat_map(|_| (0..width).map(move |x| (x * 255 / (width - 1)) as u8))
        .collect()
}

#[test]
fn a_greyscale_png_becomes_a_stamp() {
    let path = temp("grey.png");
    write_png(
        &path,
        16,
        8,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &ramp(16, 8),
    );

    let alpha = read_alpha(&path).expect("a greyscale PNG is the ordinary case");
    assert_eq!((alpha.width, alpha.height), (16, 8));
    assert_eq!(alpha.samples.len(), 16 * 8);
    assert_eq!(
        alpha.name, "clayspace-alpha-grey",
        "the name comes from the file's stem"
    );
    // The ramp survives: dark on the left, light on the right, in 0..=1.
    assert!(alpha.samples[0] < 0.02, "the left edge should be near zero");
    assert!(
        alpha.samples[15] > 0.98,
        "the right edge should be near one"
    );
    assert!(
        alpha.samples.iter().all(|s| (0.0..=1.0).contains(s)),
        "a sample escaped 0..=1"
    );
    let _ = std::fs::remove_file(&path);
}

/// An alpha library's stamps are greyscale in intent and often RGB in storage.
/// Refusing those would refuse most of what a sculptor already owns.
#[test]
fn a_colour_png_is_flattened_rather_than_refused() {
    let path = temp("rgb.png");
    // Pure red, whose Rec. 601 luma is 0.299 — a value no channel holds on its
    // own, so a decoder that just took the first channel would give 1.0.
    let pixels: Vec<u8> = (0..8 * 8).flat_map(|_| [255u8, 0, 0]).collect();
    write_png(
        &path,
        8,
        8,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &pixels,
    );

    let alpha = read_alpha(&path).expect("an RGB PNG");
    assert!(
        (alpha.samples[0] - 0.299).abs() < 0.01,
        "pure red flattened to {}, not its luma",
        alpha.samples[0]
    );
    let _ = std::fs::remove_file(&path);
}

/// Sixteen-bit stamps are common in alpha libraries and arrive big-endian at
/// two bytes a sample. Reading them as eight-bit would halve the width and
/// give a stamp the wrong shape rather than a wrong value, which is worse.
#[test]
fn a_sixteen_bit_png_keeps_its_dimensions() {
    let path = temp("deep.png");
    // A ramp in the high bytes; the low bytes are below what the falloff
    // resolves and are dropped.
    let pixels: Vec<u8> = (0..8u32)
        .flat_map(|_| (0..8u32).flat_map(|x| [(x * 255 / 7) as u8, 0]))
        .collect();
    write_png(
        &path,
        8,
        8,
        png::ColorType::Grayscale,
        png::BitDepth::Sixteen,
        &pixels,
    );

    let alpha = read_alpha(&path).expect("a sixteen-bit PNG");
    assert_eq!(
        alpha.samples.len(),
        64,
        "a sixteen-bit file read as eight-bit gives twice the samples"
    );
    assert!(alpha.samples[0] < 0.02 && alpha.samples[7] > 0.98);
    let _ = std::fs::remove_file(&path);
}

/// PNG only, and said by name — not handed to a decoder that fails with a
/// message about chunk headers.
#[test]
fn a_file_that_is_not_a_png_is_refused_by_name() {
    let path = temp("stamp.jpg");
    std::fs::write(&path, b"not really a jpeg either").expect("write");
    let error = read_alpha(&path).expect_err("a JPEG is not read");
    assert!(
        matches!(error, AlphaRefusal::NotPng { .. }),
        "refused for the wrong reason: {error}"
    );
    assert!(error.to_string().contains("PNG"), "{error}");
    // And it is refused for its extension rather than its contents, so a file
    // that is not even a JPEG still gets the same sentence.
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_png_that_is_not_an_image_is_refused_readably() {
    let path = temp("broken.png");
    std::fs::write(&path, b"PNG\r\n but not really").expect("write");
    let error = read_alpha(&path).expect_err("not a decodable PNG");
    assert!(
        matches!(error, AlphaRefusal::Unreadable(_)),
        "refused for the wrong reason: {error}"
    );
    let _ = std::fs::remove_file(&path);
}

/// One pixel across has nothing to interpolate between. The engine refuses it
/// too, and this is where the reason can be a sentence.
#[test]
fn a_stamp_too_small_to_interpolate_is_refused_before_the_engine_sees_it() {
    let path = temp("sliver.png");
    write_png(
        &path,
        1,
        8,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &[128u8; 8],
    );
    let error = read_alpha(&path).expect_err("one pixel across");
    assert!(
        matches!(error, AlphaRefusal::TooSmall { width: 1, .. }),
        "refused for the wrong reason: {error}"
    );
    let _ = std::fs::remove_file(&path);
}

/// A file with no extension at all still gets a sentence rather than an empty
/// one with a stray "this is a ." in it.
#[test]
fn a_file_with_no_extension_is_refused_readably() {
    let path = temp("bare");
    std::fs::write(&path, b"whatever").expect("write");
    let error = read_alpha(&path).expect_err("no extension");
    let message = error.to_string();
    assert!(message.contains("PNG"), "{message}");
    assert!(
        !message.contains(" ."),
        "a stray empty extension: {message}"
    );
    let _ = std::fs::remove_file(&path);
}

/// What the decoder produces has to satisfy the domain's own check, or the two
/// disagree about what a valid stamp is.
#[test]
fn what_the_decoder_produces_passes_the_domain_check() {
    let path = temp("valid.png");
    write_png(
        &path,
        32,
        16,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &ramp(32, 16),
    );
    let alpha = read_alpha(&path).expect("a stamp");
    assert!(
        alpha.clone().validated().is_ok(),
        "the decoder produced something the domain refuses"
    );
    assert!(alpha.width >= Alpha::MIN_SIDE && alpha.width <= Alpha::MAX_SIDE);
    let _ = std::fs::remove_file(&path);
}
