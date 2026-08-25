//! Reading a reference image, and refusing what is not one.
//!
//! Every PNG here is written by the test that reads it, for the reason the
//! alpha reader's tests give: a fixture checked in beside the code drifts from
//! what the decoder is asked to handle, and the interesting cases — palette,
//! sixteen-bit, greyscale — are the ones nobody remembers to add a fixture for.

use clayspace_engine::read_reference;
use clayspace_model::{AlphaRefusal, ReferenceImage};

fn temp(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-ref-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

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

#[test]
fn a_colour_png_keeps_its_colour() {
    // A stamp is flattened to one scalar a pixel; a reference is a photograph,
    // and a photograph in grey is a different reference from the one chosen.
    let path = temp("colour.png");
    let pixels: Vec<u8> = (0..8 * 8).flat_map(|_| [200u8, 40, 10]).collect();
    write_png(
        &path,
        8,
        8,
        png::ColorType::Rgb,
        png::BitDepth::Eight,
        &pixels,
    );

    let image = read_reference(&path).expect("an RGB PNG is the ordinary case");
    assert_eq!((image.width, image.height), (8, 8));
    assert_eq!(image.pixels.len(), 8 * 8 * 4);
    assert_eq!(&image.pixels[..4], &[200, 40, 10, 255]);
    assert_eq!(
        image.name, "clayspace-ref-colour",
        "the name comes from the file's stem"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_grey_png_is_grey_and_not_red() {
    // Left in one channel, a grey photograph would be sampled as pure red.
    let path = temp("grey.png");
    write_png(
        &path,
        4,
        4,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &[90u8; 16],
    );

    let image = read_reference(&path).expect("greyscale");
    assert_eq!(&image.pixels[..4], &[90, 90, 90, 255]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_cut_out_keeps_the_alpha_it_came_with() {
    // The opacity the sculptor sets and the file's own alpha are different
    // things: a cut-out placed at half opacity should still be a cut-out.
    let path = temp("cutout.png");
    let pixels: Vec<u8> = (0..4 * 4)
        .flat_map(|i| [10u8, 20, 30, if i % 2 == 0 { 0 } else { 255 }])
        .collect();
    write_png(
        &path,
        4,
        4,
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        &pixels,
    );

    let image = read_reference(&path).expect("RGBA");
    assert_eq!(image.pixels[3], 0, "a transparent pixel came back opaque");
    assert_eq!(image.pixels[7], 255);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_palette_png_is_expanded_rather_than_read_as_indices() {
    // Screenshots and cut-outs are often palettes. An index read as a colour
    // is not a picture of anything.
    let path = temp("palette.png");
    let file = std::fs::File::create(&path).expect("create");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 4, 4);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(vec![0, 0, 0, 12, 34, 56]);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[1u8; 16]).expect("pixels");
    drop(writer);

    let image = read_reference(&path).expect("indexed");
    assert_eq!(
        &image.pixels[..4],
        &[12, 34, 56, 255],
        "the palette entry was not looked up"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_sixteen_bit_png_keeps_its_high_byte() {
    let path = temp("deep.png");
    // 0x2A80 in each channel, big-endian as PNG stores it.
    let pixels: Vec<u8> = (0..4 * 4)
        .flat_map(|_| [0x2A, 0x80, 0x2A, 0x80, 0x2A, 0x80])
        .collect();
    write_png(
        &path,
        4,
        4,
        png::ColorType::Rgb,
        png::BitDepth::Sixteen,
        &pixels,
    );

    let image = read_reference(&path).expect("sixteen bit");
    assert_eq!(&image.pixels[..4], &[0x2A, 0x2A, 0x2A, 255]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn something_that_is_not_a_png_is_refused_by_name() {
    // By name and not by handing it to a decoder: "esperava um .png" says what
    // to do about it, and "invalid chunk header" does not.
    let path = temp("photo.jpg");
    std::fs::write(&path, b"not a png").expect("write");
    assert!(matches!(
        read_reference(&path),
        Err(AlphaRefusal::NotPng { .. })
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_image_too_small_to_be_one_is_refused() {
    let path = temp("tiny.png");
    write_png(
        &path,
        1,
        1,
        png::ColorType::Grayscale,
        png::BitDepth::Eight,
        &[128],
    );
    assert!(matches!(
        read_reference(&path),
        Err(AlphaRefusal::TooSmall { .. })
    ));
    // The floor is what makes a one-pixel file a refusal rather than a
    // reference nobody can see.
    const { assert!(ReferenceImage::MIN_SIDE > 1) };
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_file_that_is_not_there_is_a_refusal_and_not_a_panic() {
    let path = temp("ausente.png");
    assert!(matches!(
        read_reference(&path),
        Err(AlphaRefusal::Unreadable(_))
    ));
}
