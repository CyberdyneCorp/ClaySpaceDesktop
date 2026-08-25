//! Reading a reference image, and refusing what is not one.
//!
//! Every picture here is written by the test that reads it, for the reason the
//! alpha reader's tests give: a fixture checked in beside the code drifts from
//! what the decoder is asked to handle, and the interesting cases — palette,
//! sixteen-bit, greyscale, a photograph with an orientation tag — are the ones
//! nobody remembers to add a fixture for.

use clayspace_engine::read_reference;
use clayspace_model::{ReferenceImage, ReferenceRefusal};

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
fn something_that_is_not_a_picture_is_refused_by_name() {
    // By name and not by handing it to a decoder: naming the format says what
    // to do about it, and "invalid chunk header" does not.
    let path = temp("modelo.obj");
    std::fs::write(&path, b"not a picture").expect("write");
    assert!(matches!(
        read_reference(&path),
        Err(ReferenceRefusal::UnsupportedFormat { .. })
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
        Err(ReferenceRefusal::TooSmall { .. })
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
        Err(ReferenceRefusal::Unreadable(_))
    ));
}

// -- JPEG, which is what a photograph arrives as ----------------------------

/// Writes a JPEG, optionally with an EXIF block in front of the pixels.
fn write_jpeg(path: &std::path::Path, width: u16, height: u16, rgb: &[u8], exif: Option<&[u8]>) {
    let mut encoder = jpeg_encoder::Encoder::new_file(path, 100).expect("create");
    if let Some(exif) = exif {
        // APP1 is where EXIF lives, which is what a camera writes.
        encoder.add_app_segment(1, exif).expect("app segment");
    }
    encoder
        .encode(rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .expect("encode");
}

/// An EXIF block whose only entry is the orientation tag.
fn exif_orientation(tag: u16) -> Vec<u8> {
    let mut out = Vec::from(*b"Exif\0\0");
    out.extend_from_slice(b"II");
    out.extend_from_slice(&42u16.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0x0112u16.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

#[test]
fn a_jpeg_photograph_is_read() {
    // The format a reference folder is mostly full of. It was refused by name
    // before, which is the correct refusal for a format that is not read and
    // the wrong answer for the one photographs come in.
    let path = temp("foto.jpg");
    let pixels: Vec<u8> = (0..16 * 16).flat_map(|_| [180u8, 90, 40]).collect();
    write_jpeg(&path, 16, 16, &pixels, None);

    let image = read_reference(&path).expect("a JPEG is a reference");
    assert_eq!((image.width, image.height), (16, 16));
    assert_eq!(image.pixels.len(), 16 * 16 * 4);
    // JPEG is lossy, so the colour is near rather than exact — but it is that
    // colour and not another, and it is not grey.
    let first = &image.pixels[..4];
    assert!(
        (i32::from(first[0]) - 180).abs() < 12
            && (i32::from(first[1]) - 90).abs() < 12
            && (i32::from(first[2]) - 40).abs() < 12,
        "a red-brown photograph came back {first:?}"
    );
    assert_eq!(first[3], 255, "JPEG has no alpha; it should be opaque");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_other_spelling_of_the_extension_opens_too() {
    // .jpeg and .jpg are the same format, and a sculptor whose file happens to
    // carry the longer spelling should not have to rename it.
    let path = temp("retrato.jpeg");
    let pixels: Vec<u8> = (0..8 * 8).flat_map(|_| [10u8, 120, 200]).collect();
    write_jpeg(&path, 8, 8, &pixels, None);
    assert!(read_reference(&path).is_ok());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_photograph_taken_sideways_is_turned_the_right_way_up() {
    // A phone stores the sensor's own orientation and a tag saying how to turn
    // it. Ignored, the reference arrives sideways — and a sideways reference
    // is not a reference, it is a puzzle.
    let path = temp("de-lado.jpg");
    let (width, height) = (16u16, 8u16);
    let pixels: Vec<u8> = (0..u32::from(width) * u32::from(height))
        .flat_map(|_| [200u8, 200, 200])
        .collect();
    write_jpeg(&path, width, height, &pixels, Some(&exif_orientation(6)));

    let image = read_reference(&path).expect("a rotated JPEG is still a JPEG");
    assert_eq!(
        (image.width, image.height),
        (u32::from(height), u32::from(width)),
        "a quarter turn did not swap the sides"
    );
    assert_eq!(image.pixels.len(), 16 * 8 * 4);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_photograph_with_no_orientation_tag_is_left_as_it_is() {
    let path = temp("sem-tag.jpg");
    let (width, height) = (16u16, 8u16);
    let pixels: Vec<u8> = (0..u32::from(width) * u32::from(height))
        .flat_map(|_| [30u8, 30, 30])
        .collect();
    write_jpeg(&path, width, height, &pixels, None);

    let image = read_reference(&path).expect("a plain JPEG");
    assert_eq!((image.width, image.height), (16, 8));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_jpeg_that_is_not_one_is_a_refusal_and_not_a_panic() {
    // The extension opens the door; the decoder still has to agree.
    let path = temp("mentira.jpg");
    std::fs::write(&path, b"this is not a JPEG at all").expect("write");
    assert!(matches!(
        read_reference(&path),
        Err(ReferenceRefusal::Unreadable(_))
    ));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_jpeg_too_small_to_be_a_reference_is_refused_from_its_header() {
    // Refused before the decode allocates for it, which is the same order the
    // PNG side checks in.
    let path = temp("minusculo.jpg");
    write_jpeg(&path, 1, 1, &[128, 128, 128], None);
    assert!(matches!(
        read_reference(&path),
        Err(ReferenceRefusal::TooSmall { .. })
    ));
    let _ = std::fs::remove_file(&path);
}
