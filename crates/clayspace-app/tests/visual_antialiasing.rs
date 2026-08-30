//! Silhouettes, on a device that will not multisample.
//!
//! Multisampling is the answer wherever it is available, and where it is the
//! post-process pass does not run at all: four samples and a blur over the top
//! is paying twice to lose detail once, and what a blur loses on a sculpt is
//! fine crease mistaken for stair-step.
//!
//! Where it is *not* available the alternative is not four samples, it is a
//! stair-stepped silhouette against a flat ground — which is the most visible
//! thing that can be wrong with a frame, and was what such a device got.
//!
//! Testing it needs a device that refuses to multisample, and every device this
//! runs on will. So the sample count is chosen rather than discovered:
//! `MsaaQuality::Off` is the same code path a refusing device takes.

mod support;

use clayspace_view::{Camera, GpuMesh, Image, MsaaQuality, OffscreenTarget, Renderer, Vertex};

/// A device drawing with one sample per pixel, with a renderer on it.
fn single_sampled() -> Option<(clayspace_view::Gpu, Renderer, OffscreenTarget)> {
    let mut gpu = pollster::block_on(clayspace_view::Gpu::headless()).ok()?;
    gpu.set_msaa(MsaaQuality::Off);
    let renderer = Renderer::new(&gpu, OffscreenTarget::FORMAT);
    let target = OffscreenTarget::new(&gpu, 320, 240);
    assert_eq!(target.framebuffer().samples(), 1);
    Some((gpu, renderer, target))
}

/// A triangle with one deliberately shallow edge, which is where stair-steps
/// are longest and most visible.
fn wedge(gpu: &clayspace_view::Gpu) -> GpuMesh {
    let vertex = |position: [f32; 3]| Vertex {
        position,
        normal: [0.0, 0.0, 1.0],
        color: [1.0, 1.0, 1.0],
        mask: 0.0,
    };
    let mut mesh = GpuMesh::new(gpu);
    mesh.upload(
        gpu,
        &[
            vertex([-0.95, -0.35, 0.0]),
            vertex([0.95, -0.30, 0.0]),
            vertex([0.0, 0.85, 0.0]),
        ],
        &[0, 1, 2],
    );
    mesh
}

/// How many distinct values the frame holds along its edges.
///
/// A hard edge is two values with nothing between them; an anti-aliased one is
/// a run of intermediate values along it. Counting the intermediates is the
/// most direct measure of whether anything smoothed the edge, and it does not
/// depend on where the edge happens to fall.
fn intermediate_pixels(image: &Image, ground: [u8; 4], lit: u8) -> usize {
    let mut count = 0;
    for y in 0..image.height {
        for x in 0..image.width {
            let value = image.pixel(x, y)[0];
            let is_ground = (0..3).all(|c| image.pixel(x, y)[c].abs_diff(ground[c]) < 6);
            if !is_ground && value.abs_diff(lit) > 8 {
                count += 1;
            }
        }
    }
    count
}

/// The brightest value in the frame, which is the flat interior of the form.
fn brightest(image: &Image) -> u8 {
    image
        .pixels
        .chunks_exact(4)
        .map(|p| p[0])
        .max()
        .unwrap_or(255)
}

/// A single-sampled device gets its silhouette smoothed, and the pass is what
/// does it.
///
/// Against the same frame with the pass switched off, because that is the only
/// thing there is to compare against: the filter reads the frame's own colour,
/// so no second render exists that should look like it.
#[test]
fn a_device_that_will_not_multisample_still_gets_a_smooth_silhouette() {
    let Some((gpu, mut renderer, target)) = single_sampled() else {
        return;
    };
    let mesh = wedge(&gpu);
    let mut camera = Camera::default();
    camera.frame_bounds([-0.95, -0.35, 0.0].into(), [0.95, 0.85, 0.0].into());

    let empty = GpuMesh::new(&gpu);
    let ground = target
        .capture(&gpu, &renderer, &camera, &empty, false)
        .pixel(0, 0);

    renderer.set_antialias(false);
    let hard = target.capture(&gpu, &renderer, &camera, &mesh, false);
    support::save(&hard, "9b-fxaa-off");
    renderer.set_antialias(true);
    let smooth = target.capture(&gpu, &renderer, &camera, &mesh, false);
    support::save(&smooth, "9b-fxaa-on");

    let lit = brightest(&hard);
    let before = intermediate_pixels(&hard, ground, lit);
    let after = intermediate_pixels(&smooth, ground, lit);
    println!("single-sampled silhouette: {before} intermediate pixels -> {after}");

    assert!(
        after > before * 2,
        "the silhouette held {before} pixels between the form's value and the \
         ground's without the pass and {after} with it, which is not an edge \
         being resolved — see target/visual/9b-fxaa-on.png"
    );
    // And it is an edge being resolved rather than the frame being blurred:
    // the form's interior is flat, and a filter that softened it would put
    // intermediate values across the whole of it.
    let covered = smooth.pixels_differing_from(ground, 6);
    assert!(
        after * 2 < covered,
        "{after} of {covered} covered pixels hold an intermediate value, which \
         is the form being blurred rather than its outline being resolved"
    );
}

/// And a multisampled one does not run it.
///
/// The property that keeps the two from being paid for together. It is checked
/// through the framebuffer rather than through a timing: a device that
/// multisamples has no target for the pass to read, which is what makes
/// running it unrepresentable rather than merely unwise.
#[test]
fn a_multisampled_device_has_nothing_to_post_process() {
    let Some(gpu) = pollster::block_on(clayspace_view::Gpu::headless()).ok() else {
        return;
    };
    let target = OffscreenTarget::new(&gpu, 64, 64);
    let framebuffer = target.framebuffer();
    if framebuffer.samples() > 1 {
        assert!(
            framebuffer.antialias_view().is_none(),
            "a multisampled framebuffer allocated a post-process target it \
             must never use"
        );
    }

    let mut off = pollster::block_on(clayspace_view::Gpu::headless()).expect("a second device");
    off.set_msaa(MsaaQuality::Off);
    let single = OffscreenTarget::new(&off, 64, 64);
    assert!(
        single.framebuffer().antialias_view().is_some(),
        "a single-sampled framebuffer has nowhere to draw the scene before \
         anti-aliasing it"
    );
}

/// The scene still reaches the caller's target either way.
///
/// The post-process pass changes *where the scene is drawn*, which is the kind
/// of change that produces a blank window rather than a wrong colour.
#[test]
fn the_frame_arrives_whichever_way_it_was_drawn() {
    for quality in [MsaaQuality::Off, MsaaQuality::X4] {
        let Ok(mut gpu) = pollster::block_on(clayspace_view::Gpu::headless()) else {
            return;
        };
        gpu.set_msaa(quality);
        let renderer = Renderer::new(&gpu, OffscreenTarget::FORMAT);
        let target = OffscreenTarget::new(&gpu, 160, 120);
        let mesh = wedge(&gpu);
        let mut camera = Camera::default();
        camera.frame_bounds([-0.95, -0.35, 0.0].into(), [0.95, 0.85, 0.0].into());

        let empty = GpuMesh::new(&gpu);
        let ground = target
            .capture(&gpu, &renderer, &camera, &empty, false)
            .pixel(0, 0);
        let image = target.capture(&gpu, &renderer, &camera, &mesh, false);
        let covered = image.pixels_differing_from(ground, 6);
        println!(
            "{quality:?} -> {} samples: {covered} pixels drawn",
            target.framebuffer().samples()
        );
        assert!(
            covered > 1_000,
            "{quality:?} drew {covered} pixels, so the frame did not reach the \
             caller's target"
        );
    }
}
