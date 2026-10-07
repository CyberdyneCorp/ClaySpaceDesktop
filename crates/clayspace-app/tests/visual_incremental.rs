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

/// How far an undone frame may stray from the frame before the edit.
struct Allowance {
    /// Pixels that may differ at all.
    pixels: usize,
    /// The widest gap any of them may have.
    levels: u8,
}

/// An undo that put the surface back exactly.
///
/// The same mesh drawn twice is the same frame to the last level, so a surface
/// the undo really restored draws nothing different at all. The allowance is
/// the silhouette speckle a tile-based GPU can leave on an unchanged surface, a
/// handful of pixels and none of them past `RENDER_NOISE`. The audit's figure
/// (issue #196, I16) was 1,996 pixels at up to 19 levels, which this fails on
/// the count alone.
///
/// A debug build is allowed more pixels, not more levels. On the `macos-14`
/// runners this failed at 108 pixels and 3 levels — 107 pixels by one level
/// and one by three, scattered over the form — in the CPU-only debug job on
/// three runs (29 and 30 September, 7 October 2026) and in the Metal debug
/// job on one. The captures of the 7 October run say which frame is off: the
/// debug job's *first* frame of the starting form differs from the release
/// job's by those same 108 pixels in four unrelated tests (the cage at rest,
/// the mask's absence, this test's frame before the edit), while its undone
/// frame matches the release job's frames exactly. The undo is exact; it is
/// the debug build's first picture that differs. Release stays at the
/// handful. A stale brick is a patch dozens of levels deep, and this still
/// fails on it.
const EXACT: Allowance = Allowance {
    pixels: if cfg!(debug_assertions) { 128 } else { 16 },
    levels: support::RENDER_NOISE,
};

/// An undo across a smooth seam, which is not yet exact on every machine.
///
/// Measured on the `macos-14` runners, in the Metal and the CPU-only jobs
/// alike: 334 pixels at up to 16 levels, a thin ring on the sibling's
/// silhouette, the shape of the audit's figure. On a workstation the whole
/// case differs by 2 pixels. It is the frame *before* the edit that is off:
/// the undone frame matches the workstation's, and so did the first frame of
/// a second test building the same scene in the same run. So the brick build
/// that followed the placement disagrees, inside the blend band, with a later
/// refill of the same field — the class ClayCore #649 leaves open at v0.120.1,
/// and not something this side pads its bounds for. Bounded rather than
/// excused: a stale brick from the edit is a patch dozens of levels deep, and
/// this still fails on it.
const SMOOTH_SEAM: Allowance = Allowance {
    pixels: 1_000,
    levels: support::RENDER_NOISE,
};

/// Pixels that differ at all, and the widest gap among them.
fn exact_difference(a: &Image, b: &Image) -> (usize, u8) {
    let mut differing = 0usize;
    let mut worst = 0u8;
    for y in 0..a.height.min(b.height) {
        for x in 0..a.width.min(b.width) {
            let (pa, pb) = (a.pixel(x, y), b.pixel(x, y));
            let gap = (0..3).map(|c| pa[c].abs_diff(pb[c])).max().unwrap_or(0);
            if gap > 0 {
                differing += 1;
                worst = worst.max(gap);
            }
        }
    }
    (differing, worst)
}

/// Strokes, undoes every stroke, and holds the frame to the one before them.
///
/// The picture goes through the path the application runs: an incremental
/// sync while the pointer is down, a settle when it comes up, and the same
/// again after the undo. A stale brick anywhere between the edit's bound and
/// the undo's shows up as pixels rather than as a count.
fn assert_undo_restores_the_frame(
    harness: &Harness,
    mut document: ClayDocument,
    name: &str,
    allowed: Allowance,
) {
    use clayspace_model::SculptModel;
    let camera = framed(&document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("the first mesh");
    let before = harness.capture(geometry.mesh(), &camera, false, &format!("{name}-before"));
    let depth = document.history().depth;

    drag(&mut document);
    geometry.sync(&harness.gpu, &mut document).expect("sync");
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("settle");
    // Back to the depth before the drag, however many entries it banked.
    let banked = document.history().depth - depth;
    assert!(banked > 0, "the drag banked nothing to undo");
    for _ in 0..banked {
        assert!(document.undo().expect("undo"), "nothing was left to undo");
    }
    geometry.sync(&harness.gpu, &mut document).expect("sync");
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("settle");
    let undone = harness.capture(geometry.mesh(), &camera, false, &format!("{name}-undone"));
    save_difference(&before, &undone, &format!("{name}-difference"));

    let (differing, worst) = exact_difference(&before, &undone);
    println!("{name}: {differing} px differ after the undo, worst {worst}");
    assert!(
        differing <= allowed.pixels && worst <= allowed.levels,
        "the undone frame differs from the one before the edit in {differing} \
         pixels, the worst by {worst} levels — see target/visual/{name}-difference.png"
    );
}

/// Issue #196 (I16): an undo draws the frame it took back.
#[test]
fn an_undo_draws_the_frame_it_took_back() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(document) = document() else {
        return;
    };
    assert_undo_restores_the_frame(&harness, document, "20-undo", EXACT);
}

/// The same across a smooth seam with a sibling placed after the form.
///
/// The case ClayCore v0.120.1 fixed (ClayCore #650): an edit's bound left out
/// a later smooth sibling's blend support, so the bricks along the seam kept
/// what the edit had drawn there after the edit was undone. What is left is
/// [`SMOOTH_SEAM`].
#[test]
fn an_undo_across_a_smooth_seam_draws_the_frame_it_took_back() {
    use clayspace_model::{Combine, CombineSettings, ObjectModel, Shape};
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    let smooth = CombineSettings {
        op: Combine::Add,
        radius: 0.25,
        ..CombineSettings::default()
    };
    document
        .place_object(Shape::Sphere, &[0.45], [0.2, 0.15, 0.85], smooth)
        .expect("the sibling");
    assert_undo_restores_the_frame(&harness, document, "21-undo-seam", SMOOTH_SEAM);
}
