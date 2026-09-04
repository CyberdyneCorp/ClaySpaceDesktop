//! Automasking: the gates a brush applies to itself, and the two that do not
//! cross the ABI.
//!
//! The engine's own note calls these "the gates the brush applies to ITSELF —
//! do not cross onto a face pointing the other way, do not drag the mesh's
//! open border, stay in the polygroup this stroke started in, protect the
//! crevices", composed into the per-vertex weight by multiplication and
//! applied last, so a stamp asking for none of them is bit-identical to one
//! from before automasking existed.
//!
//! Five factors are declared and **three of them cross**. Cavity needs a field
//! to measure cavity from and surface-group needs the document's group
//! lattice, and both are callbacks on the C++ side that a flat descriptor
//! cannot carry — so setting their bits from C is inert rather than an error.
//! ClayCore v0.78.0 names the pair among its known limits and says
//! "unchanged from v0.73.0", and
//! `two_of_the_five_automask_factors_are_declared_and_inert` is that sentence
//! measured: it fails the day the descriptor carrying their inputs lands, and
//! that is the day this application can offer a cavity gate.
//!
//! The three that do cross are held here too, and not only as a control.
//! v0.78.0 is the release in which `clay_dynamic_sculptor_stamp` stopped
//! dropping the automask it was handed — the fixed path this application uses
//! has always honoured it, and the release notes ask a host that sets factors
//! to check the result is what it wanted rather than assuming a regression.
//! **This application sets none**: `stroke_mesh` sends `Automask::default()`
//! by name, so there is nothing in the shipped behaviour for that fix to have
//! changed. These are what make the offer measurable when a brush setting is
//! drawn for it.

use claycore::{Automask, Mesh, MeshBrush, MeshSculptor, MeshStamp};

// -- fixtures ---------------------------------------------------------------

/// A closed unit sphere, dense enough that a gate has something to exclude.
fn sphere() -> Mesh {
    let mut document = claycore::Document::new().expect("document");
    let layer = document.add_sdf_layer("corpo").expect("layer");
    document
        .add_item(layer, &claycore::Item::sphere(1.0).expect("sphere"))
        .expect("add");
    document
        .mesh(claycore::MeshParams {
            voxel_size: Some(0.03),
            ..Default::default()
        })
        .expect("mesh it")
}

/// A flat sheet with an open border on all four sides, which is what a
/// boundary gate is about.
fn sheet(divisions: usize) -> Mesh {
    let mut text = String::new();
    let step = 2.0 / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -1.0 + step * x as f32,
                -1.0 + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            let a = z * stride + x + 1;
            text.push_str(&format!(
                "f {} {} {} {}\n",
                a,
                a + stride,
                a + stride + 1,
                a + 1
            ));
        }
    }
    let path = std::env::temp_dir().join(format!(
        "claycore-automask-sheet-{}.obj",
        std::process::id()
    ));
    std::fs::write(&path, text).expect("write the sheet");
    let mesh = Mesh::load(&path).expect("load the sheet");
    let _ = std::fs::remove_file(&path);
    mesh
}

/// One Draw dab through a fresh sculptor, and what came of it.
///
/// Returns the vertices the engine says it moved and the positions afterwards,
/// so a gate can be read as a count *and* as a shape — a factor that changed
/// the count without changing where anything went would be a coincidence, and
/// one that changed neither did nothing.
fn dab(mesh: Mesh, center: [f32; 3], radius: f32, automask: Automask) -> (usize, Vec<[f32; 3]>) {
    // Taken by value and rebuilt per call by the caller: `clay_mesh` has no
    // copy across the C ABI, and a sculptor writes through the mesh it was
    // built on, so two dabs cannot share one.
    let mut worked = mesh;
    let mut sculptor = MeshSculptor::new(&mut worked, 1e-5).expect("a sculptor");
    let moved = sculptor
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center,
                radius,
                strength: 0.6,
                // A straight-line footprint: a surface walk is a second gate
                // on where the dab reaches, and the question here is what the
                // automask does.
                geodesic: false,
                automask,
                ..MeshStamp::default()
            },
            None,
            None,
        )
        .expect("stamp");
    (moved, worked.positions().to_vec())
}

// -- the three that cross ---------------------------------------------------

/// A backface gate holds the far side of a sphere down while the near side
/// rises.
///
/// The arrangement is the one the gate exists for: a radius wide enough to
/// swallow the sphere, so an ungated stamp drags the surface it is pointing
/// away from.
///
/// **Read as displacement and not as a count, and the reason is worth having
/// written down.** The gate is a factor in the *weight* rather than a test for
/// membership, so `stamp`'s count follows it only where the weight reaches
/// exactly zero. At a quarter turn — full strength to 90 degrees, zero at 180
/// — nothing on a sphere is exactly zero and the count is identical gated and
/// ungated, 62,576 either way, while the surface plainly moves differently.
/// At a sixth of a turn the count does fall, to 15,657. A test written on the
/// count alone would therefore pass or fail on the angle it happened to pick,
/// which is how this one was written first.
///
/// Measured on v0.78.0 at a sixth of a turn: the antipode goes to −0.846
/// ungated, dragged 0.154 toward the brush on the far side of the sphere, and
/// stays at −1.000 gated.
#[test]
fn a_backface_gate_holds_the_far_side_of_a_sphere_down() {
    let reference = sphere();
    let antipode = reference
        .positions()
        .iter()
        .enumerate()
        .min_by(|a, b| a.1[2].total_cmp(&b.1[2]))
        .map(|(index, _)| index)
        .expect("the fixture meshed something");
    assert!(
        reference.positions()[antipode][2] < -0.99,
        "the fixture's furthest vertex is not on the far pole, so it is not \
         the vertex this gate is about"
    );

    let (open, open_positions) = dab(sphere(), [0.0, 0.0, 1.0], 2.5, Automask::default());
    assert_eq!(
        open_positions.len(),
        reference.positions().len(),
        "the mesher is not deterministic across rebuilds, so an index taken \
         from one build does not name the same vertex in another"
    );
    let (gated, gated_positions) = dab(
        sphere(),
        [0.0, 0.0, 1.0],
        2.5,
        Automask {
            // Full strength up to this angle and zero at twice it, so a sixth
            // of a turn closes the gate on anything more than a third of a
            // turn from the brush's own facing.
            normal_angle: Some(std::f32::consts::FRAC_PI_6),
            ..Automask::default()
        },
    );
    println!(
        "  normal-angle automask: {open} reached ungated and {gated} gated; \
         the antipode went to {:.3} ungated and {:.3} gated",
        open_positions[antipode][2], gated_positions[antipode][2]
    );
    assert!(open > 0, "the fixture's dab reached nothing");
    assert!(
        open_positions[antipode][2] > -0.9,
        "the ungated dab did not drag the far side ({:.3}), so the comparison \
         says nothing about the gate",
        open_positions[antipode][2]
    );
    assert!(
        gated_positions[antipode][2] < -0.99,
        "the normal-angle gate let the dab reach the far side: the antipode \
         went to {:.3} against {:.3} ungated",
        gated_positions[antipode][2],
        open_positions[antipode][2]
    );
    assert_ne!(
        gated_positions, open_positions,
        "the normal-angle gate changed nothing at all, which is what the two \
         inert factors do"
    );
}

/// A boundary gate fades the open border a dab is dragged across.
///
/// Read as a *shape* rather than as a count, because the border vertices are
/// reached either way — what the gate changes is how far they go. The corner
/// of the sheet is the vertex furthest into the border's rings.
#[test]
fn a_boundary_gate_holds_the_open_border_a_dab_is_dragged_across() {
    let mesh = sheet(12);
    let corner = mesh
        .positions()
        .iter()
        .position(|p| p[0] < -0.99 && p[2] < -0.99)
        .expect("the sheet has a corner");

    let (_, open) = dab(sheet(12), [-1.0, 0.0, -1.0], 1.0, Automask::default());
    let (_, held) = dab(
        sheet(12),
        [-1.0, 0.0, -1.0],
        1.0,
        Automask {
            boundary_rings: std::num::NonZeroU32::new(4),
            ..Automask::default()
        },
    );
    println!(
        "  boundary automask: the corner rose {:.4} ungated and {:.4} with \
         four rings of fade",
        open[corner][1], held[corner][1]
    );
    assert!(
        open[corner][1] > 1e-3,
        "the ungated dab did not lift the corner, so the comparison says nothing"
    );
    assert!(
        held[corner][1] < open[corner][1] * 0.9,
        "four rings of boundary fade left the corner where an ungated dab put \
         it: {:.4} against {:.4}",
        held[corner][1],
        open[corner][1]
    );
}

/// Asking for connectivity on a mesh that is one connected piece changes
/// nothing, and that is the assertion.
///
/// The factor crosses — this is not the inert pair — and a sphere is the case
/// where it has nothing to exclude, so it is where "crosses and is correct"
/// and "crosses and is ignored" look alike and only the third factor can tell
/// them apart. Kept because it is the cheapest guard against a gate that
/// closes on everything the moment its bit is set.
#[test]
fn a_connectivity_gate_on_one_connected_piece_takes_nothing_away() {
    let (open, open_positions) = dab(sphere(), [0.0, 0.0, 1.0], 0.6, Automask::default());
    let (connected, connected_positions) = dab(
        sphere(),
        [0.0, 0.0, 1.0],
        0.6,
        Automask {
            topology_connected: true,
            ..Automask::default()
        },
    );
    assert_eq!(
        connected, open,
        "a connectivity gate on a single connected sphere moved a different \
         number of vertices"
    );
    assert_eq!(
        connected_positions, open_positions,
        "a connectivity gate on a single connected sphere moved them somewhere \
         else"
    );
}

// -- the two that do not ----------------------------------------------------

/// The tripwire. Cavity and surface-group are accepted and do nothing.
///
/// Held against the same dab with no automask at all, position for position,
/// so it cannot pass by the gate merely being weak: the header's claim is that
/// the bit is *inert*, which is an equality and not an inequality. Both are
/// asked for at once and then each on its own, because a pair tested only
/// together would go on passing if one of them started working.
///
/// Measured on v0.78.0: 62,576 vertices reached with no automask, 62,576 with
/// cavity at full strength, 62,576 with surface-group, 62,576 with both — and
/// every position identical to the last bit in all four.
///
/// When this fails, the descriptor that carries their inputs has landed. That
/// is good news twice over: `clayspace_model::Shaping` can offer a cavity gate
/// — the setting a sculptor reaches for to build up a crease without filling
/// it — and `Automask`'s doc comment about two inert fields comes out with it.
#[test]
fn two_of_the_five_automask_factors_are_declared_and_inert() {
    let (open, open_positions) = dab(sphere(), [0.0, 0.0, 1.0], 2.5, Automask::default());
    assert!(open > 0, "the fixture's dab reached nothing");

    for (what, automask) in [
        (
            "cavity",
            Automask {
                cavity_strength: Some(1.0),
                ..Automask::default()
            },
        ),
        (
            "surface group",
            Automask {
                surface_group: true,
                ..Automask::default()
            },
        ),
        (
            "both",
            Automask {
                cavity_strength: Some(1.0),
                surface_group: true,
                ..Automask::default()
            },
        ),
    ] {
        let (moved, positions) = dab(sphere(), [0.0, 0.0, 1.0], 2.5, automask);
        println!("  {what}: {moved} moved against {open} with no automask");
        assert_eq!(
            moved, open,
            "the {what} automask changed how many vertices a dab moved \
             ({moved} against {open}). It is documented as inert from C — \
             ClayCore v0.78.0 lists it under known limits, unchanged from \
             v0.73.0 — so this is the descriptor carrying its inputs having \
             landed, and clayspace_model::Shaping can offer the gate"
        );
        assert_eq!(
            positions, open_positions,
            "the {what} automask moved the same vertices somewhere else, so it \
             is no longer inert"
        );
    }
}

/// And a factor that is inert is still not an error.
///
/// The other half of the header's sentence, and the half a host depends on: a
/// descriptor carrying a bit the ABI cannot honour is accepted rather than
/// refused, so a future setting can be written into `Shaping` and sent without
/// a version check.
#[test]
fn an_inert_factor_is_accepted_rather_than_refused() {
    let mesh = sphere();
    let mut worked = mesh;
    let mut sculptor = MeshSculptor::new(&mut worked, 1e-5).expect("a sculptor");
    sculptor
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center: [0.0, 0.0, 1.0],
                radius: 0.5,
                strength: 0.4,
                automask: Automask {
                    cavity_strength: Some(1.0),
                    surface_group: true,
                    ..Automask::default()
                },
                ..MeshStamp::default()
            },
            None,
            None,
        )
        .expect("an inert automask factor is not a refusal");
}
