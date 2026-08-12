//! Rigging with ZSpheres, through the whole application path.
//!
//! The engine tests say the skin appears; the ViewModel tests say the gestures
//! mean the right thing. Neither says whether a rig is *readable* — whether
//! the spheres, the links and the skin can be told apart on screen, which is
//! the whole point of drawing scaffolding over a model.
//!
//! Frames land in `target/visual/armature-*`.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_armature -- --nocapture
//! open target/visual
//! ```

mod support;

use std::time::Instant;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::SkinSettings;
use clayspace_view::{ArmatureView, Camera, Image};
use clayspace_vm::{ArmatureViewModel, Grab};
use support::Harness;

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

/// Grows a rig the way a person would: one drag per limb, out of the sphere
/// before it.
///
/// Deliberately the gesture path rather than `add_zsphere` directly — a rig
/// authored by calling the model would not prove that press and drag produce
/// the tree the sculptor meant.
fn author(vm: &mut ArmatureViewModel) {
    // A torso up the middle, from the hips.
    vm.begin([0.0, -0.5, 0.0]);
    for chest in [[0.0f32, -0.1, 0.0], [0.0, 0.3, 0.0]] {
        let from = vm.selected().get().expect("a selection");
        vm.press(Grab::Grow(from), [0.0, 0.0, 0.0]);
        vm.drag(chest);
        vm.release();
    }

    let shoulders = vm.selected().get().expect("the top of the chest");

    // A head, on the mirror plane: symmetric authoring must not double it.
    vm.press(Grab::Grow(shoulders), [0.0, 0.3, 0.0]);
    vm.drag([0.0, 0.7, 0.0]);
    vm.release();

    // Arms, mirrored: one drag makes two.
    vm.press(Grab::Grow(shoulders), [0.0, 0.3, 0.0]);
    vm.drag([0.45, 0.3, 0.0]);
    vm.release();
    let upper = vm.selected().get().expect("an upper arm");
    vm.press(Grab::Grow(upper), [0.45, 0.3, 0.0]);
    vm.drag([0.8, 0.05, 0.0]);
    vm.release();

    // Legs, also mirrored, off the hips.
    vm.press(Grab::Grow(0), [0.0, -0.5, 0.0]);
    vm.drag([0.22, -0.9, 0.0]);
    vm.release();
    let thigh = vm.selected().get().expect("a thigh");
    vm.press(Grab::Grow(thigh), [0.22, -0.9, 0.0]);
    vm.drag([0.25, -1.35, 0.0]);
    vm.release();
}

/// Renders the document's surface with the rig drawn over it.
fn capture(
    harness: &mut Harness,
    document: &SharedDocument,
    geometry: &mut SurfaceGeometry,
    vm: &ArmatureViewModel,
    camera: &Camera,
    name: &str,
) -> Image {
    let gpu = harness.gpu.clone();
    document
        .with(|document| geometry.rebuild(&gpu, document))
        .expect("mesh the rig");

    let tree = vm.tree().get().clone().expect("a tree");
    let thickness = vm.skin().get().thickness;
    let spheres: Vec<([f32; 3], f32)> = tree
        .nodes
        .iter()
        .map(|node| (node.position, node.radius * thickness))
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
    harness.capture(geometry.mesh(), camera, false, name)
}

fn setup() -> Option<(Harness, SharedDocument, SurfaceGeometry, ArmatureViewModel)> {
    let harness = Harness::new()?;
    let policy = BackendPolicy::discover(None).ok()?;
    let document = SharedDocument::new(ClayDocument::new(policy).ok()?);
    let geometry = SurfaceGeometry::new(&harness.gpu);
    let vm = ArmatureViewModel::new(Box::new(document.clone()));
    Some((harness, document, geometry, vm))
}

#[test]
fn a_rig_is_readable_over_the_skin_it_produced() {
    let Some((mut harness, document, mut geometry, mut vm)) = setup() else {
        return;
    };
    let background = harness.background();
    let camera = Camera::default();

    let started = Instant::now();
    author(&mut vm);
    let authored = started.elapsed();

    let tree = vm.tree().get().clone().expect("a tree");
    println!(
        "rig: {} spheres, {} links, authored in {:?}",
        tree.nodes.len(),
        tree.links().len(),
        authored
    );

    // Nine drags. Four of them run up the middle and are added once — hips,
    // two chest, head — and four grow limbs that mirror into pairs.
    assert_eq!(
        tree.nodes.len(),
        12,
        "the rig is not the shape it was drawn"
    );

    let with_rig = capture(
        &mut harness,
        &document,
        &mut geometry,
        &vm,
        &camera,
        "armature-rig",
    );

    // The skin alone, for comparison: the same document with nothing drawn
    // over it.
    let gpu = harness.gpu.clone();
    harness.renderer.set_armature(
        &gpu,
        ArmatureView {
            spheres: &[],
            links: &[],
            selected: None,
            root: None,
        },
    );
    let skin_only = harness.capture(geometry.mesh(), &camera, false, "armature-skin");

    let (rigged, bare) = (
        covered(&with_rig, background),
        covered(&skin_only, background),
    );
    println!("coverage: skin {bare:.3}, skin and rig {rigged:.3}");
    assert!(bare > 0.05, "the rig produced almost no surface: {bare:.3}");

    // Counted rather than compared by coverage. The first version of the
    // scaffolding drew its hoops flush with the skin, where at a joint the
    // skin *is* the sphere: coverage was 0.097 either way and a rig was
    // invisible over the model it had just produced. Comparing the two frames
    // pixel by pixel is what catches that.
    let annotated = (0..with_rig.height)
        .flat_map(|y| (0..with_rig.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (skin_only.pixel(*x, *y), with_rig.pixel(*x, *y));
            (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0) > 8
        })
        .count();
    println!("{annotated} pixels carry the scaffolding");
    assert!(
        annotated > 500,
        "the rig is not visible over its own skin: {annotated} pixels, \
         coverage {rigged:.3} against {bare:.3}"
    );
}

#[test]
fn the_skin_slider_thickens_the_whole_rig() {
    let Some((mut harness, document, mut geometry, mut vm)) = setup() else {
        return;
    };
    let background = harness.background();
    let camera = Camera::default();
    author(&mut vm);

    vm.set_skin(SkinSettings { thickness: 0.8 });
    let thin = capture(
        &mut harness,
        &document,
        &mut geometry,
        &vm,
        &camera,
        "armature-skin-thin",
    );
    vm.set_skin(SkinSettings { thickness: 1.8 });
    let thick = capture(
        &mut harness,
        &document,
        &mut geometry,
        &vm,
        &camera,
        "armature-skin-thick",
    );

    let (a, b) = (covered(&thin, background), covered(&thick, background));
    println!("coverage: thin {a:.3}, thick {b:.3}");
    assert!(b > a, "raising the thickness did not fill out the rig");

    // And the authored radii are untouched, so the slider is reversible.
    let tree = vm.tree().get().clone().expect("a tree");
    assert!((tree.nodes[0].radius - 0.3).abs() < 1e-5);
}

#[test]
fn moving_a_shoulder_carries_the_arm_on_screen() {
    // The puppet rule, seen rather than asserted about: the frames before and
    // after must differ, and they must differ where the arm went.
    let Some((mut harness, document, mut geometry, mut vm)) = setup() else {
        return;
    };
    let camera = Camera::default();
    author(&mut vm);
    let before = capture(
        &mut harness,
        &document,
        &mut geometry,
        &vm,
        &camera,
        "armature-before-move",
    );

    // The right upper arm, found by where it sits rather than by index.
    let tree = vm.tree().get().clone().expect("a tree");
    let shoulder = tree
        .nodes
        .iter()
        .position(|n| n.position[0] > 0.4 && n.position[1] > 0.2)
        .expect("an upper arm") as u32;

    vm.press(Grab::Move(shoulder), tree.nodes[shoulder as usize].position);
    vm.drag([0.6, 0.9, 0.0]);
    vm.release();

    let after = capture(
        &mut harness,
        &document,
        &mut geometry,
        &vm,
        &camera,
        "armature-after-move",
    );

    let moved = (0..before.height)
        .flat_map(|y| (0..before.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (before.pixel(*x, *y), after.pixel(*x, *y));
            (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0) > 8
        })
        .count();
    println!("{moved} pixels changed when the shoulder was lifted");
    assert!(moved > 200, "lifting the shoulder changed almost nothing");

    // The forearm hangs off it and must have come along.
    let after_tree = vm.tree().get().clone().expect("a tree");
    let forearm = after_tree
        .nodes
        .iter()
        .filter(|n| n.parent == shoulder)
        .count();
    assert!(forearm > 0, "the shoulder lost its forearm");
    for node in after_tree.nodes.iter().filter(|n| n.parent == shoulder) {
        assert!(
            node.position[1] > 0.4,
            "the forearm stayed behind at {:?}",
            node.position
        );
    }
}
