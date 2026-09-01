//! What the incremental re-mesh actually draws.
//!
//! While a stroke is in progress the viewport does not re-mesh the model. It
//! meshes the dirty bricks and splices them into the geometry it already has.
//! That path had tests for its *cost* — keys touched, milliseconds — and for
//! its counts, and none at all for its picture. It shipped speckling the
//! surface with dark slivers along every stroke, which no count or timing
//! would ever have named.
//!
//! So the reference here is the same document meshed from scratch. The
//! incremental result must look like it. Both are written to `target/visual/`
//! along with the difference, so a failure can be looked at.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_incremental
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_view::Image;
use support::{framed, Harness};

/// A dab is local, so a whole-image mean would drown it. This is the share of
/// pixels allowed to differ past the noise floor.
///
/// Zero, and it stays zero — what changed is the floor. `compare` counted a
/// pixel as differing at eight levels out of 255, which a tile-based GPU
/// crosses on the silhouette of an unchanged frame: measured on a macOS
/// runner, four pixels of a settled surface and one after four strokes, in a
/// speckled ring around the subject's edge and nothing else. Missing geometry
/// does not look like that — it is a solid patch, hundreds of pixels at
/// dozens of levels — so the threshold moved to `RENDER_NOISE` and the share
/// allowed past it is still none at all.
const TOLERATED: f64 = 0.0;

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A stroke dragged across the front of the model, in world space.
///
/// Deliberately oblique to the brick grid: a stroke along an axis crosses
/// bricks face to face, which is the easy case. Sculptors do not draw along
/// axes.
fn drag(document: &mut ClayDocument) {
    for step in 0..10 {
        let t = step as f32 / 9.0;
        let x = -0.45 + t * 0.9;
        let y = -0.28 + t * 0.52;
        let z = (1.0 - x * x - y * y).max(0.05).sqrt();
        let samples = [GestureSample {
            position: [x, y, z],
            pressure: 1.0,
            time: t,
        }];
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &samples,
                [false; 3],
            )
            .expect("the stroke was refused");
    }
}

/// How many pixels differ between two frames, and how far the worst one is.
fn compare(a: &Image, b: &Image) -> (f64, u8) {
    let mut differing = 0usize;
    let mut worst = 0u8;
    for y in 0..a.height.min(b.height) {
        for x in 0..a.width.min(b.width) {
            let (pa, pb) = (a.pixel(x, y), b.pixel(x, y));
            let gap = (0..3).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
            if gap > support::RENDER_NOISE {
                differing += 1;
                worst = worst.max(gap);
            }
        }
    }
    let total = (a.width * a.height) as f64;
    (differing as f64 / total, worst)
}

/// Writes the difference so a failure is something to look at, not a number.
fn save_difference(a: &Image, b: &Image, name: &str) {
    let mut pixels = vec![0u8; a.pixels.len()];
    for i in (0..a.pixels.len()).step_by(4) {
        let gap = (0..3)
            .map(|c| a.pixels[i + c].abs_diff(b.pixels[i + c]))
            .max()
            .unwrap_or(0);
        // Amplified: a two-level difference is invisible against black and is
        // exactly the kind that turns out to matter.
        let lit = gap.saturating_mul(8);
        pixels[i] = lit;
        pixels[i + 1] = lit;
        pixels[i + 2] = lit;
        pixels[i + 3] = 255;
    }
    support::save(
        &Image {
            width: a.width,
            height: a.height,
            pixels,
        },
        name,
    );
}

#[test]
fn an_incremental_stroke_draws_what_a_full_remesh_would() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    let camera = framed(&document);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the first mesh");
    harness.capture(geometry.mesh(), &camera, false, "17-incremental-before");

    // The path the application runs while the pointer is down.
    drag(&mut document);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("the incremental re-mesh");
    // What the sculptor sees *while dragging*, before anything settles. This
    // is the claim the per-vertex ownership fix makes: a partial re-mesh keeps
    // the triangles it could not regenerate instead of clearing them, so the
    // mid-drag picture is already right rather than right-once-you-let-go.
    let dragging = harness.capture(geometry.mesh(), &camera, false, "17-incremental-dragging");
    // And what they are left with when the pointer comes up.
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("settle");
    let incremental = harness.capture(geometry.mesh(), &camera, false, "17-incremental-after");

    // The same document, meshed from nothing.
    let mut reference_geometry = SurfaceGeometry::new(&harness.gpu);
    reference_geometry
        .settle(&harness.gpu, &mut document)
        .expect("the clean reference mesh");
    let reference = harness.capture(
        reference_geometry.mesh(),
        &camera,
        false,
        "17-incremental-reference",
    );

    save_difference(&incremental, &reference, "17-incremental-difference");
    save_difference(&dragging, &reference, "17-dragging-difference");

    // Mid-drag the surface *is* missing triangles, and cannot not be: a subset
    // mesh omits the ones straddling its boundary (ClayCore #66). What is
    // owed is that it stays a seam — a thin trace along the edit — rather than
    // spreading, and that settling closes it completely, which the assertion
    // below checks.
    let (drag_share, drag_worst) = compare(&dragging, &reference);
    println!("DRAGGING share={drag_share:.6} worst={drag_worst}");
    assert!(
        drag_share < 0.005,
        "mid-drag {:.3}% of the frame differs from a full re-mesh (worst \
         {drag_worst} levels). That is past a seam — see \
         target/visual/17-dragging-difference.png",
        drag_share * 100.0
    );

    let (share, worst) = compare(&incremental, &reference);
    println!("SINGLE share={share:.6} worst={worst}");
    assert!(
        share <= TOLERATED,
        "{:.3}% of pixels differ from a full re-mesh, the worst by {worst} \\
         levels. The incremental path is drawing something the model does not \\
         contain — see target/visual/17-incremental-difference.png",
        share * 100.0
    );
}

#[test]
fn many_strokes_do_not_accumulate_damage() {
    // One dab can hide a seam defect by luck of where it lands. A session's
    // worth of strokes across the surface cannot.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    let camera = framed(&document);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("first mesh");

    for pass in 0..4 {
        let offset = pass as f32 * 0.11 - 0.16;
        for step in 0..8 {
            let t = step as f32 / 7.0;
            let x = -0.4 + t * 0.8;
            let y = offset;
            let z = (1.0 - x * x - y * y).max(0.05).sqrt();
            document
                .apply_stroke(
                    ToolKind::Padrao,
                    BrushSettings::default(),
                    &[GestureSample {
                        position: [x, y, z],
                        pressure: 1.0,
                        time: t,
                    }],
                    [false; 3],
                )
                .expect("stroke");
        }
        geometry
            .sync(&harness.gpu, &mut document)
            .expect("incremental re-mesh");
        geometry
            .settle(&harness.gpu, &mut document)
            .expect("settle");
    }

    let incremental = harness.capture(geometry.mesh(), &camera, false, "18-many-incremental");

    let mut reference_geometry = SurfaceGeometry::new(&harness.gpu);
    reference_geometry
        .settle(&harness.gpu, &mut document)
        .expect("clean reference mesh");
    let reference = harness.capture(
        reference_geometry.mesh(),
        &camera,
        false,
        "18-many-reference",
    );
    save_difference(&incremental, &reference, "18-many-difference");

    let (share, worst) = compare(&incremental, &reference);
    println!("MANY share={share:.6} worst={worst}");
    assert!(
        share <= TOLERATED,
        "after four strokes {:.3}% of pixels differ from a full re-mesh, the \\
         worst by {worst} levels — see target/visual/18-many-difference.png",
        share * 100.0
    );
}

#[test]
fn the_per_key_split_draws_what_the_engine_meshed() {
    // Upstream of the incremental question entirely: `SurfaceGeometry` takes
    // the engine's mesh apart into per-key pieces so a dab can replace one of
    // them. If that split loses or duplicates a triangle, both the
    // incremental path and the full rebuild are wrong together — and a test
    // that compares them to each other sees nothing.
    //
    // So this compares against the engine's own mesh, uploaded whole.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    let camera = framed(&document);
    drag(&mut document);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");
    let split = harness.capture(geometry.mesh(), &camera, false, "19-split");

    // The same bricks, meshed by the engine and handed straight to the GPU.
    let whole = {
        let (mesh, _) = document
            .cache()
            .mesh(
                Some(document.document()),
                clayspace_engine::claycore::BrickMeshParams {
                    gradient_normals: true,
                    colors: false,
                    gradient_eps: None,
                },
                &[],
            )
            .expect("mesh the whole cache");
        let gpu_mesh = support::upload_engine_mesh(&harness.gpu, &mesh);
        harness.capture(&gpu_mesh, &camera, false, "19-whole")
    };
    save_difference(&split, &whole, "19-split-difference");

    let (share, worst) = compare(&split, &whole);
    println!("SPLIT share={share:.6} worst={worst}");
    assert!(
        worst < 60,
        "the per-key split differs from the engine's own mesh by up to {worst} \
         levels over {:.3}% of the frame — see target/visual/19-split-difference.png",
        share * 100.0
    );
}
