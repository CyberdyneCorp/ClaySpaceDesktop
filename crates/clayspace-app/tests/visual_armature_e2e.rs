//! Rigging a character the way the application actually starts.
//!
//! `visual_armature` builds a rig too, but on an *empty* document — and the
//! application never has one. It opens on a starting form, so the first
//! ZSphere used to union into a sphere that was already there: rigging looked
//! and behaved like ordinary sculpting with a lump in the middle, and no test
//! could see it because no test began where the application begins.
//!
//! So this one starts where it starts, goes through the composition root's own
//! types, and looks at the result.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_armature_e2e -- --nocapture
//! open target/visual
//! ```

mod support;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Representation, SceneModel, SculptModel};
use clayspace_view::{ArmatureView, Camera, Image};
use clayspace_vm::{ArmatureViewModel, Grab, SceneViewModel};
use support::Harness;

/// The document the application opens with: a starting form on one layer.
fn as_the_app_starts() -> Option<(Harness, SharedDocument, SurfaceGeometry)> {
    let harness = Harness::new()?;
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let document = SharedDocument::new(document);
    let geometry = SurfaceGeometry::new(&harness.gpu);
    Some((harness, document, geometry))
}

/// How much of the frame is not background.
fn covered(image: &Image, background: [u8; 4]) -> f64 {
    let mut lit = 0usize;
    for y in 0..image.height {
        for x in 0..image.width {
            let pixel = image.pixel(x, y);
            if (0..3).any(|c| pixel[c].abs_diff(background[c]) > 12) {
                lit += 1;
            }
        }
    }
    lit as f64 / (image.width * image.height) as f64
}

/// Is there surface at this point?
fn solid_at(document: &SharedDocument, at: [f32; 3]) -> bool {
    let origin = [at[0], at[1], at[2] + 4.0];
    document
        .pick(origin, [0.0, 0.0, -1.0])
        .map(|hit| hit[2] > at[2] - 0.5)
        .unwrap_or(false)
}

/// A biped, grown one drag at a time out of the sphere before it — which is
/// the gesture, and therefore the thing worth testing.
fn grow_a_character(vm: &mut ArmatureViewModel) {
    vm.begin([0.0, -0.5, 0.0]);

    // Spine, up the middle. On the mirror plane, so each is added once.
    for up in [[0.0f32, -0.1, 0.0], [0.0, 0.3, 0.0]] {
        let from = vm.selected().get().expect("a selection");
        vm.press(Grab::Grow(from), [0.0, 0.0, 0.0]);
        vm.drag(up);
        vm.release();
    }
    let shoulders = vm.selected().get().expect("the top of the spine");

    // Head.
    vm.press(Grab::Grow(shoulders), [0.0, 0.3, 0.0]);
    vm.drag([0.0, 0.72, 0.0]);
    vm.release();

    // Arms, mirrored: one drag makes two.
    vm.press(Grab::Grow(shoulders), [0.0, 0.3, 0.0]);
    vm.drag([0.45, 0.3, 0.0]);
    vm.release();
    let upper = vm.selected().get().expect("an upper arm");
    vm.press(Grab::Grow(upper), [0.45, 0.3, 0.0]);
    vm.drag([0.82, 0.06, 0.0]);
    vm.release();

    // Legs, also mirrored.
    vm.press(Grab::Grow(0), [0.0, -0.5, 0.0]);
    vm.drag([0.22, -0.92, 0.0]);
    vm.release();
    let thigh = vm.selected().get().expect("a thigh");
    vm.press(Grab::Grow(thigh), [0.22, -0.92, 0.0]);
    vm.drag([0.25, -1.4, 0.0]);
    vm.release();
}

#[test]
fn a_rig_started_on_the_starting_form_gets_a_layer_of_its_own() {
    // The bug this file exists for. The application opens on a form; starting
    // a rig must not union the first ZSphere into it.
    let Some((harness, document, mut geometry)) = as_the_app_starts() else {
        return;
    };
    let mut scene = SceneViewModel::new(Box::new(document.clone()));
    let before: Vec<String> = document
        .scene()
        .layers
        .iter()
        .map(|l| l.name.clone())
        .collect();
    assert_eq!(before.len(), 1, "the application starts with one layer");

    let mut vm = ArmatureViewModel::new(Box::new(document.clone()));
    // Deliberately off to one side of the starting form, so a union would be
    // unmistakable rather than hidden inside it.
    vm.begin([0.0, 0.0, 0.0]);
    scene.refresh();

    let after = document.scene();
    assert_eq!(
        after.layers.len(),
        2,
        "the rig did not get a layer of its own: {:?}",
        after
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect::<Vec<_>>()
    );
    let rig = after
        .layers
        .iter()
        .find(|l| !before.contains(&l.name))
        .expect("a new layer");
    assert_eq!(rig.representation, Representation::Sdf);

    // The sculpt steps out of the way rather than being deleted: still in the
    // stack, still in the document, one click from coming back.
    let form = after
        .layers
        .iter()
        .find(|l| before.contains(&l.name))
        .expect("the starting form is still a layer");
    assert!(
        !form.visible,
        "the starting form is still shown, so the rig is being built inside it"
    );
    assert!(rig.visible, "the rig itself is hidden");

    let gpu = harness.gpu.clone();
    document
        .with(|d| geometry.rebuild(&gpu, d))
        .expect("mesh the visible layers");
}

#[test]
fn a_character_grows_out_of_the_starting_document() {
    let Some((mut harness, document, mut geometry)) = as_the_app_starts() else {
        return;
    };
    let background = harness.background();
    let camera = Camera::default();

    let gpu = harness.gpu.clone();
    document
        .with(|d| geometry.rebuild(&gpu, d))
        .expect("the starting form");
    let opening = harness.capture(geometry.mesh(), &camera, false, "e2e-1-as-it-opens");

    let mut vm = ArmatureViewModel::new(Box::new(document.clone()));
    grow_a_character(&mut vm);

    let tree = vm.tree().get().clone().expect("a tree");
    println!(
        "character: {} spheres, {} links",
        tree.nodes.len(),
        tree.links().len()
    );
    // Nine drags: four up the middle, added once each, and four that mirror
    // into pairs.
    assert_eq!(
        tree.nodes.len(),
        12,
        "the rig is not the shape it was drawn"
    );

    let gpu = harness.gpu.clone();
    document
        .with(|d| geometry.rebuild(&gpu, d))
        .expect("mesh the character");

    let thickness = vm.skin().get().thickness;
    let spheres: Vec<([f32; 3], f32)> = tree
        .nodes
        .iter()
        .map(|n| (n.position, n.radius * thickness))
        .collect();
    let links = tree.links();
    harness.renderer.set_armature(
        &gpu,
        ArmatureView {
            spheres: &spheres,
            links: &links,
            selected: *vm.selected().get(),
            root: Some(0),
        },
    );
    // Framed on what is now there, as `frame_all` does in the application —
    // the character is taller than the ball it replaced and would otherwise
    // run off the top and bottom of the capture.
    let mut framed = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => framed.frame_bounds(min.into(), max.into()),
        None => framed.frame_default(),
    }
    let rigged = harness.capture(geometry.mesh(), &framed, false, "e2e-2-character");

    let (was, now) = (covered(&opening, background), covered(&rigged, background));
    println!("coverage: opening {was:.3}, with the character {now:.3}");

    // Not "more than before": the sphere is hidden now, and a character is
    // thinner than a radius-1 ball, so coverage legitimately *drops*. What
    // matters is that something substantial is drawn and that it is not the
    // thing that was there before.
    assert!(
        now > 0.04,
        "almost nothing is drawn where the character should be: {now:.3}"
    );
    let changed = (0..opening.height)
        .flat_map(|y| (0..opening.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (opening.pixel(*x, *y), rigged.pixel(*x, *y));
            (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0) > 8
        })
        .count();
    println!("{changed} pixels differ from the opening frame");
    assert!(
        changed > 20_000,
        "the frame barely changed: the rig may be hidden inside the form"
    );

    // The limbs are where they were drawn, both sides of the mirror.
    for (name, at) in [
        ("the head", [0.0f32, 0.72, 0.0]),
        ("the right hand", [0.82, 0.06, 0.0]),
        ("the left hand", [-0.82, 0.06, 0.0]),
        ("the right foot", [0.25, -1.4, 0.0]),
        ("the left foot", [-0.25, -1.4, 0.0]),
    ] {
        assert!(solid_at(&document, at), "{name} is not there");
    }

    // And what is drawn is the rig, not the rig plus a sphere: the starting
    // form is a radius-1 ball at the origin, so if it were still visible the
    // whole character would be inside it.
    assert!(
        !solid_at(&document, [0.0, 0.0, 0.95]),
        "the starting form is still being drawn around the character"
    );
}

#[test]
fn posing_the_character_carries_the_limb() {
    // The puppet rule, on a character rather than a stick: lift a shoulder and
    // the whole arm goes, through the application's own path.
    let Some((harness, document, mut geometry)) = as_the_app_starts() else {
        return;
    };
    let mut vm = ArmatureViewModel::new(Box::new(document.clone()));
    grow_a_character(&mut vm);

    let tree = vm.tree().get().clone().expect("a tree");
    let shoulder = tree
        .nodes
        .iter()
        .position(|n| n.position[0] > 0.4 && n.position[1] > 0.2)
        .expect("a right upper arm") as u32;

    assert!(
        solid_at(&document, [0.82, 0.06, 0.0]),
        "the hand starts here"
    );

    vm.press(Grab::Move(shoulder), tree.nodes[shoulder as usize].position);
    vm.drag([0.45, 0.85, 0.0]);
    vm.release();

    let gpu = harness.gpu.clone();
    document
        .with(|d| geometry.rebuild(&gpu, d))
        .expect("re-mesh the pose");

    assert!(
        solid_at(&document, [0.82, 0.61, 0.0]),
        "the hand did not follow the shoulder up"
    );
    assert!(
        !solid_at(&document, [0.82, 0.06, 0.0]),
        "the arm left surface behind where it used to be"
    );
    // The other arm stayed put: moving a subtree must not move its mirror.
    assert!(
        solid_at(&document, [-0.82, 0.06, 0.0]),
        "the left arm moved with the right one"
    );
}

#[test]
fn the_skin_preview_off_shows_the_zspheres_and_the_membrane() {
    // ZBrush's `A`: while building a rig you look at the spheres, and every so
    // often you look at what they make. Captured both ways, because "the
    // scaffolding stands alone" is a claim about a picture.
    let Some((mut harness, document, mut geometry)) = as_the_app_starts() else {
        return;
    };
    let background = harness.background();
    let mut vm = ArmatureViewModel::new(Box::new(document.clone()));
    grow_a_character(&mut vm);

    let gpu = harness.gpu.clone();
    document
        .with(|d| geometry.rebuild(&gpu, d))
        .expect("mesh the character");
    let mut camera = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    let tree = vm.tree().get().clone().expect("a tree");
    let thickness = vm.skin().get().thickness;
    let spheres: Vec<([f32; 3], f32)> = tree
        .nodes
        .iter()
        .map(|n| (n.position, n.radius * thickness))
        .collect();
    let links = tree.links();
    harness.renderer.set_armature(
        &gpu,
        ArmatureView {
            spheres: &spheres,
            links: &links,
            selected: None,
            root: Some(0),
        },
    );

    let with_skin = harness.capture(geometry.mesh(), &camera, false, "e2e-3-skin-preview-on");

    // Preview off: the surface is simply not drawn, which is what the
    // composition root does with an empty mesh.
    let nothing = clayspace_view::GpuMesh::new(&gpu);
    let spheres_only = harness.capture(&nothing, &camera, false, "e2e-4-skin-preview-off");

    let (skinned, bare) = (
        covered(&with_skin, background),
        covered(&spheres_only, background),
    );
    println!("coverage: skin {skinned:.3}, spheres only {bare:.3}");

    // The scaffolding stands alone rather than vanishing with the surface —
    // that is the whole point of the mode.
    assert!(
        bare > 0.01,
        "with the skin off there is almost nothing left to see: {bare:.3}"
    );
    // And it is meaningfully less than the skinned view, or the toggle did
    // nothing.
    assert!(
        bare < skinned,
        "turning the skin off did not change what is drawn"
    );
}
