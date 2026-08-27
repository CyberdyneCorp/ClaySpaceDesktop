//! The bake-and-replace tools, and the resolution they bake at.
//!
//! Suavizar, Relaxar, Planar and Polir do not stamp. They sample a region of
//! the document into a volume, modify that volume, and add it back over the
//! top with `CLAY_OP_REPLACE`. Two things about that are easy to get wrong and
//! both show as a crumbling, blocky patch rather than a smoothed one:
//!
//!   * the volume is sampled coarser than the surface being displayed, so the
//!     replacement is chunkier than what it replaced;
//!   * a live stroke applies the tool per segment, so a gesture stacks one
//!     baked volume per segment, each replacing an overlapping region.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_bake_tools --release -- --nocapture
//! open target/visual
//! ```

mod support;

use clayspace_app::SharedDocument;
use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use clayspace_vm::{Command, SculptViewModel};

/// What this fixture sculpts with, and what the roughening below matches.
///
/// Off, and switched off on the ViewModel too. This test is about *crumbling*
/// — whether a region verb applied once per segment shreds the surface — and
/// the roughness ceiling below was measured on an unmirrored subject. Leaving
/// the ViewModel's default X on while roughening with it off would be a
/// document whose mirror changes mid-session, which no sculptor's does: the
/// bake would turn the layer mirror on under itself and add a reflected copy
/// of the roughening on top of the original. `sdf_brushes.rs` is where
/// symmetry is measured.
const SYMMETRY: [bool; 3] = [false; 3];
use support::Harness;

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

/// Roughness: how much neighbouring pixels disagree across the subject.
///
/// A smoothed patch should be *less* rough than what it replaced. Crumbling
/// reads as a large jump between adjacent pixels, which a mean difference
/// against a reference would not separate from a legitimate change of shape.
fn roughness(image: &Image, background: [u8; 4]) -> f64 {
    let mut total = 0u64;
    let mut counted = 0u64;
    for y in 1..image.height {
        for x in 1..image.width {
            let here = image.pixel(x, y);
            if (0..3).all(|c| here[c].abs_diff(background[c]) <= 12) {
                continue;
            }
            let left = image.pixel(x - 1, y);
            let up = image.pixel(x, y - 1);
            let step = (0..3)
                .map(|c| here[c].abs_diff(left[c]) as u64 + here[c].abs_diff(up[c]) as u64)
                .max()
                .unwrap_or(0);
            total += step;
            counted += 1;
        }
    }
    if counted == 0 {
        return 0.0;
    }
    total as f64 / counted as f64
}

/// Draws a stroke with one tool and reports the picture and the node count.
fn stroke_with(harness: &mut Harness, tool: ToolKind, name: &str) -> Option<Applied> {
    let mut document = document()?;
    let camera = framed(&document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).ok()?;

    // A bumpy surface first, so smoothing has something to smooth. A tool that
    // is meant to reduce roughness cannot be judged on a sphere.
    for i in 0..14 {
        let t = i as f32 / 13.0;
        let angle = (t - 0.5) * 1.1;
        let (s, c) = angle.sin_cos();
        let wobble = if i % 2 == 0 { 0.04 } else { -0.02 };
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.12,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [s * (1.0 + wobble), 0.1 + wobble, c * (1.0 + wobble)],
                    pressure: 1.0,
                    time: t,
                }],
                // The symmetry the ViewModel below will ask for. Roughening
                // with it off and then sculpting with it on is a document
                // whose mirror changes mid-session, which no sculptor's does —
                // and it turned the layer mirror on under the bake, adding a
                // reflected copy of this roughening on top of itself.
                SYMMETRY,
            )
            .ok()?;
    }
    geometry.rebuild(&harness.gpu, &mut document).ok()?;
    let before = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        &format!("bake-{name}-before"),
    );
    let nodes_before = SculptModel::stats(&document).objects;

    // Then the tool under test, driven through the ViewModel exactly as a drag
    // delivers it — which is what decides whether it is applied once or once
    // per segment.
    let shared = SharedDocument::new(document);
    let mut vm = SculptViewModel::new(Box::new(shared.clone()));
    vm.dispatch(Command::SelectTool(tool)).ok()?;
    // The ViewModel starts with X on; this fixture is unmirrored throughout.
    for (index, axis) in [
        clayspace_vm::Axis::X,
        clayspace_vm::Axis::Y,
        clayspace_vm::Axis::Z,
    ]
    .into_iter()
    .enumerate()
    {
        if vm.symmetry().get()[index] != SYMMETRY[index] {
            vm.dispatch(Command::ToggleSymmetry(axis)).ok()?;
        }
    }

    // The stroke itself, which this had stopped delivering.
    //
    // #13 meant to add the symmetry loop above and replaced this with it, so
    // from then on the function selected a tool, set the mirror and captured
    // the frame it started from: all eight captures were one identical image
    // and every figure below compared a surface to itself. Restored in the
    // order the deletion implies — symmetry first, because the ViewModel reads
    // it when the stroke begins.
    for i in 0..8 {
        let t = i as f32 / 7.0;
        let angle = (t - 0.5) * 1.1;
        let (s, c) = angle.sin_cos();
        let position = [s * 1.01, 0.1, c * 1.01];
        let command = if i == 0 {
            Command::BeginStroke {
                position,
                pressure: 1.0,
                modifiers: Default::default(),
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        vm.dispatch(command).ok()?;
    }
    vm.dispatch(Command::EndStroke).ok()?;

    shared
        .with(|document| geometry.rebuild(&harness.gpu, document))
        .ok()?;
    let after = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        &format!("bake-{name}-after"),
    );

    let background = harness.background();
    let _ = nodes_before;
    Some(Applied {
        moved: support::differing_pixels(&before, &after),
        rough_before: roughness(&before, background),
        rough_after: roughness(&after, background),
    })
}

/// What one tool did to the fixture.
///
/// `moved` is here because nothing used to notice when the answer was
/// "nothing at all": with the stroke missing, `rough_before` and `rough_after`
/// were read off the same image and every assertion below was satisfied by a
/// tool that had never run. A roughness that did not change is not evidence
/// the tool is gentle, and this is what tells the two apart.
struct Applied {
    /// Pixels the stroke moved past the render noise. The caller reads this
    /// before it reads any roughness — see the assertion it feeds.
    moved: usize,
    rough_before: f64,
    rough_after: f64,
}

#[test]
fn the_smoothing_tools_smooth_rather_than_crumble() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    println!(
        "\n{:<10} {:>12} {:>12} {:>10}",
        "tool", "rough before", "rough after", "px moved"
    );
    let mut measured = Vec::new();
    for tool in [
        ToolKind::Suavizar,
        ToolKind::Relaxar,
        ToolKind::Planar,
        ToolKind::Polir,
    ] {
        let name = format!("{tool:?}").to_lowercase();
        let Some(applied) = stroke_with(&mut harness, tool, &name) else {
            return;
        };
        println!(
            "{:<10} {:>12.2} {:>12.2} {:>10}",
            format!("{tool:?}"),
            applied.rough_before,
            applied.rough_after,
            applied.moved
        );
        measured.push((tool, applied));
    }
    println!();

    // Before anything is concluded from the roughness: the tool has to have
    // done something. This is the assertion whose absence let #13 delete the
    // stroke and leave the suite green for four releases — every figure below
    // was read off two copies of one frame, and every one of them agreed.
    let untouched: Vec<String> = measured
        .iter()
        .filter(|(_, applied)| applied.moved < 50)
        .map(|(tool, applied)| format!("{tool:?} moved {} pixels", applied.moved))
        .collect();
    assert!(
        untouched.is_empty(),
        "a smoothing tool left the picture where it found it: {untouched:?}. \
         Either the stroke is not reaching the document or the tool is doing \
         nothing — see target/visual/bake-*.png"
    );

    // These tools do not yet leave the surface smoother than they found it,
    // and this pins how far off that is rather than pretending otherwise.
    //
    // The figure below is 5.83, not the 5.00 this comment used to carry. Two
    // faults were stacking to produce that number, and they cancelled into a
    // reading that looked like success. #13 removed the stroke, so `before`
    // and `after` were roughness read off *the same image* — and the helper
    // returned the second one `as usize`, truncating 5.83 to 5. A surface
    // compared with itself therefore reported smoothing by 0.40, on all four
    // tools, for four releases.
    //
    // What is fixed: applying them per segment of a live stroke stacked one
    // baked volume per segment and the surface came back crumbling, at 13 and
    // 9 against a baseline of 4.9. They are region operations now, applied
    // once per gesture.
    //
    // What was left, and is now fixed: the region was replaced with a hard
    // CLAY_OP_REPLACE, which held both fields live at the boundary. The baked
    // volume tied with the field beneath it at every sample plane, and the
    // branch switching between two fields that touch rippled the *normals* at
    // the cell wavelength — the zero set was exact and the shading was not,
    // which is why this measures a rendered image rather than probing the
    // surface. That note guessed "a blended replace rather than a hard one",
    // which is exactly what ClayCore 0.28.0 delivered: `clay_volume_params`
    // gained a feather, and these four tools now bake with one.
    //
    // Measured on this machine, same stroke, only the feather changing:
    //
    //   hard replace (0.27 and before)   7.00
    //   feathered (0.28)                 5.83   <- this, measured whole
    //   untouched baseline               4.88
    //
    // The middle row read 5.00 while the truncation was in place. It is 5.83
    // against a 5.40 surface, so the honest statement is that the feather
    // stopped the crumbling — 13 and 9 before it — and did not reach
    // smoothing. The ceiling catches a regression towards the hard replace
    // with room to spare rather than pinning 5.83 exactly.
    let ceiling = 6.0;
    // Over every tool, not only the ones that came out rougher than they went
    // in. The ceiling used to be applied to a list already filtered by
    // `after > before`, so a tool that smoothed a little and was still far
    // past the ceiling passed — and once all four smoothed, the list was empty
    // and the bound stopped being checked at all.
    let over: Vec<String> = measured
        .iter()
        .filter(|(_, applied)| applied.rough_after > ceiling)
        .map(|(tool, a)| format!("{tool:?} {:.1}->{:.1}", a.rough_before, a.rough_after))
        .collect();
    assert!(
        over.is_empty(),
        "these tools are rougher than the {ceiling} this path is known to \
         leave: {over:?}. Past that is the crumbling that came from applying \
         a region operation per stroke segment — see target/visual/bake-*.png"
    );

    // No assertion that these smooth, because measured whole they do not. The
    // name is "smooth rather than crumble" and the ceiling above is the
    // crumbling half; the smoothing half is what the roughness table is for,
    // and it is printed so that a change either way is visible in the log.
}

#[test]
fn the_region_tools_are_applied_once_for_the_whole_gesture() {
    // The rule that fixed the crumbling, stated where it can be checked
    // without a GPU. A region operation does not decompose: applied per
    // segment it stacks a replacement per segment over overlapping ground.
    for tool in [
        ToolKind::Suavizar,
        ToolKind::Relaxar,
        ToolKind::Planar,
        ToolKind::Polir,
    ] {
        assert!(
            tool.is_region_based(),
            "{tool:?} bakes and replaces a region, so it must not be applied \
             per segment of a live stroke"
        );
    }
    for tool in [ToolKind::Padrao, ToolKind::Inflar, ToolKind::Camada] {
        assert!(
            !tool.is_region_based(),
            "{tool:?} stamps, and must stay live under the pointer"
        );
    }
}
