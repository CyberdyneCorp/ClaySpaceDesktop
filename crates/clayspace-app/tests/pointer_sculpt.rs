//! From a pointer position to a changed surface.
//!
//! The chain the application actually runs — viewport rectangle, ray, pick,
//! stroke, re-mesh — with nothing stubbed but the window. Every sculpting test
//! before this one started at the engine or the ViewModel and so could not
//! have caught a brush that never receives a click, which is what shipped.

use clayspace_app::{ray_at, SharedDocument};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::SculptModel;
use clayspace_view::Camera;
use clayspace_vm::{Axis, Command, SculptViewModel};

/// The viewport the shell leaves in a 1280×800 window, near enough.
fn viewport() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(278.0, 92.0), egui::pos2(1032.0, 688.0))
}

struct Fixture {
    document: SharedDocument,
    sculpt: SculptViewModel,
    camera: Camera,
}

impl Fixture {
    fn new() -> Option<Self> {
        let policy = BackendPolicy::discover(None).ok()?;
        let document = ClayDocument::new(policy)
            .and_then(ClayDocument::with_starting_form)
            .ok()?;
        let document = SharedDocument::new(document);
        let sculpt = SculptViewModel::new(Box::new(document.clone()));

        let mut camera = Camera::default();
        match SculptModel::bounds(&document) {
            Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
            None => camera.frame_default(),
        }

        Some(Self {
            document,
            sculpt,
            camera,
        })
    }

    /// Where the surface is under a viewport position.
    fn pick(&self, point: egui::Pos2) -> Option<[f32; 3]> {
        let (origin, direction) = ray_at(&self.camera, viewport(), point)?;
        self.sculpt.pick(origin, direction)
    }

    /// One complete stroke, as dragging across the model would produce it.
    fn stroke(&mut self, points: &[egui::Pos2]) -> usize {
        let mut landed = 0;
        for (i, point) in points.iter().enumerate() {
            let Some(position) = self.pick(*point) else {
                continue;
            };
            let command = if landed == 0 {
                Command::BeginStroke {
                    position,
                    pressure: 1.0,
                }
            } else {
                Command::ContinueStroke {
                    position,
                    pressure: 1.0,
                }
            };
            self.sculpt
                .dispatch(command)
                .unwrap_or_else(|e| panic!("sample {i} was refused: {e}"));
            landed += 1;
        }
        if landed > 0 {
            self.sculpt.dispatch(Command::EndStroke).expect("end");
        }
        landed
    }

    /// The mesh the viewport would draw, from the cache the viewport meshes.
    ///
    /// This is the only measure that answers the user's question. Raycasting
    /// the document does not: measured across dabs at different places it
    /// reports the same displacement wherever the stroke lands, so it says
    /// "something happened" without saying what or where.
    fn meshed(&self) -> (usize, u64) {
        self.document.with(|document| {
            let (mesh, _) = document
                .cache()
                .mesh(Some(document.document()), Default::default(), &[])
                .expect("mesh the cache");
            // Hashed positions, not a vertex count and a bounding box. Those
            // were tried and neither moves: marching the same bricks yields
            // the same topology, and a bulge on a sphere's flank does not
            // widen its axis-aligned bounds. A test can be as blind as the
            // code it is checking.
            let mut hash = 1469598103934665603u64;
            for position in mesh.positions() {
                for channel in position {
                    for byte in ((channel * 4096.0).round() as i32).to_le_bytes() {
                        hash ^= byte as u64;
                        hash = hash.wrapping_mul(1099511628211);
                    }
                }
            }
            (mesh.vertex_count(), hash)
        })
    }

    /// How far the surface is from the origin along `direction`.
    ///
    /// Radial rather than axis-aligned: the starting form is a sphere, so a
    /// ray straight in along its normal measures displacement directly and
    /// measures it the same way wherever on the model it is taken. Probing
    /// down -Z instead reads the flank of a bulge rather than its peak once
    /// the surface tilts, which made an earlier version of this test report
    /// "nothing happened" for strokes that plainly had.
    fn radius_along(&self, direction: [f32; 3]) -> Option<f32> {
        let length = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt();
        if length < 1e-6 {
            return None;
        }
        let unit = direction.map(|c| c / length);
        let origin = unit.map(|c| c * 4.0);
        let inward = unit.map(|c| -c);
        self.sculpt
            .pick(origin, inward)
            .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
    }
}

#[test]
fn the_centre_of_the_viewport_is_on_the_model() {
    // The framing puts the subject in the middle of the viewport. If a ray
    // through the middle misses, the ray and the framing disagree and every
    // click will miss too.
    let Some(fixture) = Fixture::new() else {
        return;
    };
    assert!(
        fixture.pick(viewport().center()).is_some(),
        "a ray through the centre of the viewport did not meet the model"
    );
}

#[test]
fn a_click_on_the_model_changes_what_the_viewport_would_draw() {
    let Some(mut fixture) = Fixture::new() else {
        return;
    };
    let before = fixture.meshed();

    let centre = viewport().center();
    let landed = fixture.stroke(&[
        centre,
        centre + egui::vec2(6.0, 0.0),
        centre + egui::vec2(12.0, 3.0),
    ]);
    assert_eq!(landed, 3, "some samples missed a model that fills the view");

    assert_ne!(
        fixture.meshed(),
        before,
        "the click reached the engine and the mesh the viewport draws came \
         back identical, which is a brush that appears to do nothing"
    );
}

#[test]
fn a_click_off_the_model_changes_nothing() {
    // The corner of the viewport is background. A stroke there must not
    // deposit anything — and must not be mistaken for a stroke that failed.
    let Some(mut fixture) = Fixture::new() else {
        return;
    };
    let before = fixture.meshed();

    let corner = viewport().min + egui::vec2(6.0, 6.0);
    assert!(
        fixture.pick(corner).is_none(),
        "the corner of the viewport was taken to be on the model"
    );
    assert_eq!(fixture.stroke(&[corner]), 0);
    assert_eq!(
        fixture.meshed(),
        before,
        "a stroke on empty space changed the model"
    );
}

#[test]
fn a_position_outside_the_viewport_makes_no_ray() {
    let Some(fixture) = Fixture::new() else {
        return;
    };
    for outside in [
        egui::pos2(100.0, 400.0),                 // left panel
        egui::pos2(1200.0, 400.0),                // right panel
        egui::pos2(viewport().center().x, 40.0),  // menu bar
        egui::pos2(viewport().center().x, 760.0), // shelf
    ] {
        assert!(
            ray_at(&fixture.camera, viewport(), outside).is_none(),
            "a ray was built for {outside:?}, which is over a panel"
        );
    }
}

#[test]
fn symmetry_reaches_the_viewport() {
    // It did not, for the whole of 0.26 and 0.27: `clay_set_layer_mirror`
    // stored the plane but per-item participation defaulted to excluded, so
    // the mirror was dead and a stroke moved only the side it was drawn on.
    // ClayCore 0.28.0 makes participation default to mirrored (#60), and this
    // test — which used to assert the far side stayed *still* — now asserts
    // that both sides move.
    let Some(mut fixture) = Fixture::new() else {
        return;
    };
    // X is on from the start, as the design asks.
    assert_eq!(*fixture.sculpt.symmetry().get(), [true, false, false]);

    let at = viewport().center() + egui::vec2(70.0, 0.0);
    let target = fixture.pick(at).expect("that point is on the model");
    assert!(
        target[0].abs() > 0.2,
        "the sample landed at x = {}, too near the mirror to tell the two apart",
        target[0]
    );
    let mirrored = [-target[0], target[1], target[2]];

    let near_before = fixture.radius_along(target).expect("the near side");
    let far_before = fixture.radius_along(mirrored).expect("the far side");

    let samples: Vec<egui::Pos2> = (0..4)
        .map(|step| at + egui::vec2(step as f32 * 3.0, 0.0))
        .collect();
    assert!(fixture.stroke(&samples) > 0, "the samples all missed");

    assert!(
        (fixture.radius_along(target).expect("near") - near_before).abs() > 0.002,
        "the stroke did not move the side it was drawn on"
    );

    // The whole point: the far side moves too, and it moves through the brick
    // cache the viewport meshes from rather than only in the document field.
    let far_moved = (fixture.radius_along(mirrored).expect("far") - far_before).abs();
    assert!(
        far_moved > 0.002,
        "the mirrored half did not reach the viewport; it moved by {far_moved}"
    );
}

#[test]
fn without_symmetry_only_the_drawn_side_moves() {
    // The other half of the pair: turn the mirror off and a stroke on one side
    // leaves the other alone. Worth keeping now that the mirror works, because
    // "always mirrors" and "never mirrors" fail this suite identically
    // otherwise.
    let Some(mut fixture) = Fixture::new() else {
        return;
    };
    assert_eq!(
        *fixture.sculpt.symmetry().get(),
        [true, false, false],
        "the interface and the document must start out agreeing"
    );
    fixture
        .sculpt
        .dispatch(Command::ToggleSymmetry(Axis::X))
        .expect("symmetry off");
    assert_eq!(*fixture.sculpt.symmetry().get(), [false, false, false]);

    let at = viewport().center() + egui::vec2(70.0, 0.0);
    let target = fixture.pick(at).expect("that point is on the model");
    let mirrored = [-target[0], target[1], target[2]];
    let near_before = fixture.radius_along(target).expect("the near side");
    let far_before = fixture.radius_along(mirrored).expect("the far side");

    let samples: Vec<egui::Pos2> = (0..4)
        .map(|step| at + egui::vec2(step as f32 * 3.0, 0.0))
        .collect();
    assert!(fixture.stroke(&samples) > 0);

    // The near side must move, or this passes for the wrong reason.
    assert!(
        (fixture.radius_along(target).expect("near") - near_before).abs() > 0.002,
        "the stroke did nothing at all, so it proves nothing about the mirror"
    );
    let far_moved = (fixture.radius_along(mirrored).expect("far") - far_before).abs();
    assert!(
        far_moved < 0.002,
        "with no mirror set the far side still moved by {far_moved}"
    );
}
