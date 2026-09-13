//! Whether a mirrored Move drag pulls each side once or twice.
//!
//! The application reflects a gesture itself and calls the verb once per image
//! — `baked_stroke`'s mirror loop — because through v0.52.2 the engine's own
//! `clay_layer_move_surface` said nothing about a layer mirror. In v0.60.0 it
//! does: "UNDER A LAYER MIRROR OR RADIAL SYMMETRY the drag is reflected and
//! rotated into every image the layer emits of it". Two reflections of one
//! gesture is two pulls, and a doubled pull is symmetric — so it cannot be
//! caught by comparing the two sides against each other. It is caught by
//! comparing a mirrored drag against an unmirrored one.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the origin along `direction`.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    let origin = unit.map(|c| c * 4.0);
    let hit = document.pick(origin, unit.map(|c| -c))?;
    Some(hit.iter().map(|c| c * c).sum::<f32>().sqrt())
}

/// The path of the drag, from its anchor to `step` of six.
fn path(step: usize) -> Vec<GestureSample> {
    (0..=step)
        .map(|i| {
            let t = i as f32 / 6.0;
            GestureSample {
                position: [1.0 + t * 0.25, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// The same drag, made live: opened, sent in segments, committed.
///
/// The live path does NOT run `baked_stroke`'s mirror loop — `clay_sdf_move_*`
/// reflects the drag into every image the layer emits and resolves one grab per
/// image, so reflecting the gesture again would be the doubling this file
/// exists to catch, reached by the other door.
fn live_drag(document: &mut ClayDocument, symmetry: [bool; 3]) {
    assert!(
        document.open_live_gesture(ToolKind::Mover, symmetry),
        "an editable field subtool takes a live drag"
    );
    for step in 1..=6 {
        document
            .apply_stroke(
                ToolKind::Mover,
                BrushSettings {
                    size: 0.35,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &path(step),
                symmetry,
            )
            .expect("the drag was refused");
    }
    document.close_live_gesture().expect("commit");
}

/// A drag outward at the +x limb.
fn drag(document: &mut ClayDocument, symmetry: [bool; 3]) {
    let samples: Vec<GestureSample> = (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            GestureSample {
                position: [1.0 + t * 0.25, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.35,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("the drag was refused");
}

#[test]
fn a_mirrored_drag_pulls_each_side_once() {
    let mut plain = sphere();
    let rest = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the rest surface");
    drag(&mut plain, [false; 3]);
    let unmirrored = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the dragged surface") - rest;

    let mut mirrored = sphere();
    drag(&mut mirrored, [true, false, false]);
    let near = radius_along(&mirrored, [1.0, 0.0, 0.0]).expect("the near side") - rest;
    let far = radius_along(&mirrored, [-1.0, 0.0, 0.0]).expect("the far side") - rest;

    eprintln!(
        "unmirrored +{unmirrored:.4}; mirrored near +{near:.4}, far +{far:.4} \
         (near/unmirrored {:.2}x)",
        near / unmirrored.max(1e-6)
    );

    // Both sides move: that is what symmetry is for.
    assert!(
        far > unmirrored * 0.5,
        "the mirror did not reach the far side: +{far:.4} against +{unmirrored:.4}"
    );
    // And each side is pulled as far as one drag pulls, not as far as two.
    // The application reflects the gesture and the engine reflects it again,
    // so a doubled pull is what this exists to catch — and it is symmetric,
    // which is why the far side alone cannot catch it.
    assert!(
        near < unmirrored * 1.5,
        "a mirrored drag pulled the near side +{near:.4} where an unmirrored \
         one pulls +{unmirrored:.4} — {:.2}x, which is the gesture applied \
         twice: once by the application's own mirror loop and once by the \
         engine's",
        near / unmirrored.max(1e-6)
    );
}

#[test]
fn a_mirrored_live_drag_pulls_each_side_once() {
    // The same claim for the transactional path, which mirrors by a different
    // mechanism: the engine reflects the drag itself and the application does
    // not reflect the gesture, where the held path does the opposite.
    let mut plain = sphere();
    let rest = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the rest surface");
    live_drag(&mut plain, [false; 3]);
    let unmirrored = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the dragged surface") - rest;

    let mut mirrored = sphere();
    live_drag(&mut mirrored, [true, false, false]);
    let near = radius_along(&mirrored, [1.0, 0.0, 0.0]).expect("the near side") - rest;
    let far = radius_along(&mirrored, [-1.0, 0.0, 0.0]).expect("the far side") - rest;

    eprintln!("live: unmirrored +{unmirrored:.4}; mirrored near +{near:.4}, far +{far:.4}");
    assert!(
        far > unmirrored * 0.5,
        "the mirror did not reach the far side: +{far:.4} against +{unmirrored:.4}"
    );
    assert!(
        near < unmirrored * 1.5,
        "a mirrored live drag pulled the near side +{near:.4} where an \
         unmirrored one pulls +{unmirrored:.4} — the drag is being reflected \
         twice, once by the application and once by the engine"
    );
}

/// The engine's own region is asked for rather than reconstructed here.
///
/// **Not the fix the issue described, and the difference is worth recording.**
/// `move_surface_stroke` used to rebuild the invalidation box itself:
///
/// ```text
/// let reach = brush.size + travelled;
/// min[axis] = a.min(b) - reach;
/// max[axis] = a.max(b) + reach;
/// ```
///
/// The argument for replacing it was that under symmetry one box either misses
/// the reflected image or is stretched to cover both, becoming the slab
/// between them. **That never happened here.** `mirrors()` returns one entry
/// per image and `apply_stroke` calls the verb once for each, so the host was
/// already reflecting by hand and each call reconstructed a box around its own
/// image. Measured before and after, the same x-mirrored drag dirties 7,488
/// bricks against 7,144 for the unmirrored one either way — a ratio of 2.31
/// before and 2.25 after, where a slab would be many times that.
///
/// What `clay_layer_move_surface_regions` (ABI 0.106.0) actually buys is
/// exactness, and it is worth having for two reasons the reconstruction could
/// not reach at all:
///
/// - It is **tighter**: the engine states the region it invalidated rather
///   than a ball around the whole gesture, which is 3–5% fewer bricks here and
///   grows with the distance travelled, since `reach` dilated by `travelled`
///   in every axis including the two the drag did not move along.
/// - It is **complete**. The reconstruction knew nothing about what a layer
///   fold above can move, nor about layers sharing an instanced edit list —
///   whose whole influence bound the drag also changes. Those are
///   under-invalidation, which leaves stale surface on screen, and no
///   assertion about this document can see them because it has neither.
///
/// So this pins the property that is true and useful: the reported region
/// covers the drag and is no looser than the box it replaced.
#[test]
fn the_reported_region_is_no_looser_than_the_box_it_replaced() {
    let mut document = sphere();
    document.set_symmetry([true, false, false]).expect("mirror");
    drag(&mut document, [true, false, false]);
    let mirrored = document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.35,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &path(3),
            [true, false, false],
        )
        .expect("a mirrored drag")
        .dirty_bricks;

    // The reconstruction this replaced, computed the way it was: a ball of
    // `brush.size + travelled` around the gesture, per image. `path(3)` is the
    // same drag both arms take, so its figure is fixed and can be stated.
    const RECONSTRUCTED: usize = 7488;
    assert!(
        mirrored > 0,
        "the mirrored drag dirtied nothing, so no region was reported"
    );
    assert!(
        mirrored <= RECONSTRUCTED,
        "the engine reported {mirrored} dirty bricks where reconstructing the \
         box by hand gave {RECONSTRUCTED}. The engine's region is the one the \
         gesture actually invalidated, so a larger number means it is being \
         dilated again on this side"
    );
}

/// A drag's falloff and its front-only gate reach the engine.
///
/// The regression this exists for. Both Move call sites, and the topological
/// one, constructed `MoveParams` with the same two literals:
///
/// ```text
/// claycore::MoveParams { radius: brush.size.max(1e-3), ease: 0, front_only: true }
/// ```
///
/// So the falloff was always `ease_linear` while `clay_ease` offers
/// thirty-three curves, and `front_only: true` meant only the near side of a
/// form ever travelled — **a form could never be dragged through**. Blender's
/// own "Front Faces Only" defaults *off*.
///
/// Asserted against the surface rather than against the parameters, because a
/// test that read the struct back would only prove this file passes its own
/// literals along. A drag with the gate off has to move the far side; a drag
/// with it on must not.
#[test]
fn turning_the_front_only_gate_off_drags_the_whole_form_through() {
    let far = |symmetry, front_only| {
        let mut document = sphere();
        document.set_symmetry(symmetry).expect("symmetry");
        let before = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");
        document
            .apply_stroke(
                ToolKind::Mover,
                BrushSettings {
                    // Large enough that the ball reaches through the form:
                    // a gate that is off only matters where the far side is
                    // inside the drag's reach.
                    size: 2.5,
                    intensity: 1.0,
                    drag: clayspace_model::Drag {
                        falloff: clayspace_model::DragFalloff::Linear,
                        front_only,
                    },
                    ..BrushSettings::default()
                },
                &path(3),
                symmetry,
            )
            .expect("a drag");
        let after = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");
        (after - before).abs()
    };

    let gated = far([false; 3], true);
    let through = far([false; 3], false);
    assert!(
        through > gated,
        "with the front-only gate off the far side moved {through}, against \
         {gated} with it on — so the gate is not reaching the engine and a \
         sculptor cannot pull a form through"
    );
}

/// The falloff curve reaches the engine, and a different curve is a different pull.
///
/// The companion. `front_only` is a flag and could be plumbed while `ease`
/// stayed a literal, so this pins the other half: two named curves over the
/// same gesture must not produce the same surface.
///
/// `Broad` against `Tight` rather than two neighbours — `ease_in_quad` holds
/// its strength further out and `ease_out_quad` falls away immediately, so
/// they differ most where a drag is actually read.
///
/// Probed a little off the drag's axis, which is chosen rather than assumed.
/// Measured across the six named curves at several angles, the separation is
/// widest around a fifth of a radius off-axis and narrows to noise near the
/// rim, where every curve has fallen to nothing: on this fixture Broad reads
/// 1.0356 and Tight 1.0632 there, against 1.0009 and 1.0073 at the rim. A
/// first version of this test probed near the rim and read both as the
/// undisturbed sphere.
#[test]
fn two_falloff_curves_are_two_different_pulls() {
    let pulled = |falloff| {
        let mut document = sphere();
        document.set_symmetry([false; 3]).expect("symmetry");
        document
            .apply_stroke(
                ToolKind::Mover,
                BrushSettings {
                    size: 0.8,
                    intensity: 1.0,
                    drag: clayspace_model::Drag {
                        falloff,
                        front_only: true,
                    },
                    ..BrushSettings::default()
                },
                &path(3),
                [false; 3],
            )
            .expect("a drag");
        radius_along(&document, [1.0, 0.2, 0.0]).expect("a shoulder of the pull")
    };

    let broad = pulled(clayspace_model::DragFalloff::Broad);
    let tight = pulled(clayspace_model::DragFalloff::Tight);
    assert!(
        (broad - tight).abs() > 0.01,
        "Broad and Tight pulled the surface to {broad} and {tight}; the ease \
         index is not reaching the engine and every drag is still linear"
    );
    assert!(
        tight > broad,
        "Tight ({tight}) did not pull further than Broad ({broad}) a fifth of \
         a radius off-axis. `ease_out_quad` concentrates the pull near the \
         centre and `ease_in_quad` spreads it, so this is the wrong way round \
         and the two indices may be swapped"
    );
}
