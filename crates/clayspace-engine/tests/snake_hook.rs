//! Snake Hook (Puxar) on a field: the mask, the root, the tip and the seam.
//!
//! Four defects in one path, each held here against the fixture sphere:
//!
//! - **The mask was sampled at the path's samples only.** A tendril is a tube
//!   around its path, so the curve joining the samples either side of a
//!   masked band ran straight through it and the band was pulled like open
//!   surface. The tendril is gated by the mask now, over everything it
//!   reaches.
//! - **The root swelled as the pull extended**, because the chain's links were
//!   smooth-unioned into one another and every span the spline was
//!   tessellated into swelled the ones beside it.
//! - **The tip overshot the end of the path** by a whole tip radius and more:
//!   a swept sphere caps its curve with a half ball, and the same link blend
//!   inflated the tube further.
//! - **A hard crease where the tendril met the surface**, which the grid
//!   samples into a sawtooth along the seam.
//!
//! Measured through `pick`, which reads the brick cache the viewport draws.
//! The document's own raycast stops a little short of the surface once a gate
//! lowers the field's step scale — about 0.009 everywhere, pulled or not —
//! which would read as material the mask let through.
//!
//! ```sh
//! cargo test -p clayspace-engine --test snake_hook
//! ```

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, MaskModel, SculptModel, ToolKind};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.18,
        intensity: 0.9,
        ..BrushSettings::default()
    }
}

/// `n + 1` samples along `at(t)` for `t` in `[0, 1]`.
fn along(n: usize, at: impl Fn(f32) -> [f32; 3]) -> Vec<GestureSample> {
    (0..=n)
        .map(|step| {
            let t = step as f32 / n as f32;
            GestureSample {
                position: at(t),
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// Delivers a pull the way the interface does: one gesture, and segments each
/// carrying the whole path so far, ending at each of `ends`.
fn pull(document: &mut ClayDocument, path: &[GestureSample], ends: &[usize]) {
    document.begin_gesture();
    for &end in ends {
        segment(document, &path[..=end]);
    }
    document.end_gesture();
}

fn segment(document: &mut ClayDocument, so_far: &[GestureSample]) {
    document
        .apply_stroke(ToolKind::Puxar, brush(), so_far, [false; 3])
        .expect("the pull was refused");
}

/// Where the surface stands, looking straight down at `(x, 0)`.
fn top_at(document: &ClayDocument, x: f32) -> f32 {
    document
        .pick([x, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .unwrap_or(f32::NAN)
}

/// A band of mask across the top of the sphere, along y at x = 0.
fn mask_a_band(document: &mut ClayDocument) {
    let samples = along(8, |t| {
        let y = (t - 0.5) * 1.2;
        [0.0, y, (1.0 - y * y).sqrt()]
    });
    let painted = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    document
        .apply_stroke(ToolKind::Mascara, painted, &samples, [false; 3])
        .expect("the mask stroke was refused");
    assert!(document.mask_state().present, "the mask painted nothing");
}

/// A pull lying along the top of the sphere, across x = 0.
fn across_the_top() -> Vec<GestureSample> {
    along(20, |t| {
        let a = (t - 0.5) * 1.4;
        [a.sin(), 0.0, a.cos()]
    })
}

/// How far a pull across the top raised the surface at each of `xs`.
fn rise_across_the_top(masked: bool, xs: &[f32]) -> Vec<f32> {
    let mut document = document();
    if masked {
        mask_a_band(&mut document);
    }
    let before: Vec<f32> = xs.iter().map(|&x| top_at(&document, x)).collect();
    pull(&mut document, &across_the_top(), &[5, 10, 15, 20]);
    xs.iter()
        .zip(before)
        .map(|(&x, was)| top_at(&document, x) - was)
        .collect()
}

#[test]
fn snake_hook_respects_the_mask() {
    // Measured before the fix, at the band's centre: 0.270 masked against
    // 0.269 open — the mask made no difference at all. After it, 0.000
    // against 0.136, and the open surface either side rises the same whether
    // or not the band is there.
    const CENTRE: usize = 1;
    let xs = [-0.5, 0.0, 0.5];
    let open = rise_across_the_top(false, &xs);
    let masked = rise_across_the_top(true, &xs);
    println!("open {open:?}\nmasked {masked:?}");

    assert!(
        open[CENTRE] > 0.05,
        "the open pull raised the centre by only {:.4}, so the fixture proves \
         nothing about the mask",
        open[CENTRE]
    );
    assert!(
        masked[CENTRE].abs() < 0.005,
        "the pull raised the masked band by {:.4} (open: {:.4}). The tendril \
         has to be gated by the mask over everything it reaches, not only at \
         its samples",
        masked[CENTRE],
        open[CENTRE]
    );
    for side in [0, 2] {
        assert!(
            (masked[side] - open[side]).abs() < 0.005,
            "outside the band the masked pull rose {:.4} against {:.4} open, \
             so the gate reached past what was painted",
            masked[side],
            open[side]
        );
    }
}

#[test]
fn a_pull_does_not_move_its_root() {
    // Measured before the fix, from a first segment of six samples: the root
    // thickened by 0.046 as the pull went on, and by 0.25 from a first segment
    // of two. The chain's links were smooth-unioned into one another, so
    // every span the curve was tessellated into swelled the ones beside it,
    // and a root swelled until enough of the pull lay past it.
    let path = along(16, |t| {
        let a = t * 1.6;
        [0.95 + a.sin() * 0.6, 0.0, 0.35 - (1.0 - a.cos()) * 0.3]
    });
    let probes = [0.97f32, 1.0, 1.03, 1.06, 1.09];
    let mut document = document();
    document.begin_gesture();
    segment(&mut document, &path[..=5]);
    let first: Vec<f32> = probes.iter().map(|&x| top_at(&document, x)).collect();
    for end in [9, 12, 16] {
        segment(&mut document, &path[..=end]);
    }
    document.end_gesture();
    let last: Vec<f32> = probes.iter().map(|&x| top_at(&document, x)).collect();
    println!("first {first:?}\nlast  {last:?}");

    for ((x, was), is) in probes.iter().zip(&first).zip(&last) {
        assert!(was.is_finite() && is.is_finite(), "no surface at x = {x}");
        assert!(
            (is - was).abs() < 2e-3,
            "the root moved from {was:.4} to {is:.4} at x = {x} as the pull \
             went on"
        );
    }
}

#[test]
fn a_pull_stops_at_the_path_end() {
    // Straight out of the side of the sphere. Measured before the fix: a pull
    // ending at 1.4 stood out to 1.701, and one ending at 1.8 to 2.047.
    let path = along(16, |t| [1.0 + t * 0.8, 0.0, 0.0]);
    for (ends, end) in [(&[4usize, 8][..], 1.4f32), (&[4, 8, 12, 16][..], 1.8)] {
        let mut document = document();
        pull(&mut document, &path, ends);
        let tip = document
            .pick([4.0, 0.0, 0.0], [-1.0, 0.0, 0.0])
            .map(|hit| hit[0])
            .unwrap_or(f32::NAN);
        assert!(
            tip <= end + ClayDocument::VOXEL_SIZE,
            "a pull ending at {end} stood out to {tip:.4}"
        );
        assert!(
            tip >= end - ClayDocument::VOXEL_SIZE,
            "a pull ending at {end} stopped short, at {tip:.4}"
        );
    }
}

#[test]
fn the_tendril_meets_the_surface_without_a_crease() {
    // Looking down across the join of a tendril pulled straight out of the
    // side, where the sphere's steep flank meets the tendril's top. A hard
    // union turns the slope there within one step — a crease the grid samples
    // into a sawtooth — and a fillet turns it across the fillet's width.
    //
    // Read off the field rather than the cache: the crease is a property of
    // the field, and the cache's triangles turn at every cell whatever the
    // field does. Without a mask the raycast carries no step-scale offset.
    const STEP: f32 = 0.01;
    let path = along(16, |t| [1.0 + t * 0.8, 0.0, 0.0]);
    let mut document = document();
    pull(&mut document, &path, &[4, 8, 12, 16]);
    let heights: Vec<f32> = (0..=30)
        .map(|i| {
            let x = 0.85 + i as f32 * STEP;
            document
                .document()
                .raycast([x, 0.0, 4.0], [0.0, 0.0, -1.0])
                .ok()
                .flatten()
                .map(|hit| hit.position[2])
                .unwrap_or(f32::NAN)
        })
        .collect();
    let sharpest = heights
        .windows(3)
        .map(|w| {
            let before = (w[1] - w[0]).atan2(STEP);
            let after = (w[2] - w[1]).atan2(STEP);
            (after - before).abs()
        })
        .fold(0.0f32, f32::max);
    println!("sharpest turn {sharpest:.3} rad, heights {heights:?}");
    assert!(
        sharpest.is_finite() && sharpest < 0.2,
        "the surface turns by {sharpest:.3} rad in one {STEP} step across the \
         join, which is a crease rather than a fillet"
    );
}

#[test]
fn a_pull_shorter_than_its_tip_still_pulls() {
    // Trimming the tip back by its own radius leaves no curve until the pull
    // is about as long as the root is wide, and a short tug still has to raise
    // the surface. It stands a little past the pointer: the whole of a short
    // pull lies inside the fillet that joins it to the form. And it grows into
    // the trimmed curve without a jump as the pull outgrows its root.
    const SIZE: f32 = 0.18;
    let tip_of = |length: f32| {
        let path = along(6, |t| [1.0 + t * length, 0.0, 0.0]);
        let mut document = document();
        pull(&mut document, &path, &[3, 6]);
        document
            .pick([4.0, 0.0, 0.0], [-1.0, 0.0, 0.0])
            .map(|hit| hit[0])
            .unwrap_or(f32::NAN)
    };
    let short = tip_of(0.1);
    assert!(
        short > 1.05,
        "a 0.1 pull left the surface at {short:.4}, as if it were not there"
    );
    assert!(
        short <= 1.1 + SIZE * 0.5,
        "a 0.1 pull stood out to {short:.4}, further past the pointer than \
         the fillet reaches"
    );
    let (below, above) = (tip_of(0.9 * SIZE), tip_of(1.1 * SIZE));
    assert!(
        above > below && above - below < 0.05,
        "the tip jumped from {below:.4} to {above:.4} as the pull outgrew its \
         root"
    );
}
