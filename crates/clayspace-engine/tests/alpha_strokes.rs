//! An alpha stamp reaching each of the three representations.
//!
//! Three separate routes into the engine, which is why there are three tests
//! rather than one. A field gets a deformer appended to the stamp item, a grid
//! gets a carve modulated per cell, a mesh gets the block in the brush
//! descriptor. Nothing above this layer knows that, and nothing above it
//! should — but a route that is wired and does nothing looks exactly like one
//! that works, so each is measured against the surface it moved.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Alpha, BrushSettings, CombineSettings, Direction, ExchangeModel, ExportSettings, GestureSample,
    ImportSettings, Representation, SceneModel, SculptModel, ToolKind,
};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A stamp with structure in it — concentric rings, so a surface it modulates
/// is not merely offset but textured.
fn rings(side: u32) -> Alpha {
    let samples = (0..side)
        .flat_map(|y| {
            (0..side).map(move |x| {
                let (dx, dy) = (
                    x as f32 / (side - 1) as f32 - 0.5,
                    y as f32 / (side - 1) as f32 - 0.5,
                );
                let r = (dx * dx + dy * dy).sqrt() * 12.0;
                (r.sin() * 0.5 + 0.5).clamp(0.0, 1.0)
            })
        })
        .collect();
    Alpha {
        name: "anéis".into(),
        width: side,
        height: side,
        samples,
    }
}

fn brush(alpha: bool) -> BrushSettings {
    BrushSettings {
        size: 0.35,
        intensity: 1.0,
        alpha,
        ..BrushSettings::default()
    }
}

fn arc() -> Vec<GestureSample> {
    (0..6)
        .map(|i| {
            let t = i as f32 / 5.0;
            GestureSample {
                position: [(t - 0.5) * 0.4, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// The surface's height at a grid of points across the stroke.
///
/// One ray down the middle is not enough to see a stamp: a relief displacement
/// saturates at roughly the brush radius, so the point directly under the
/// stroke reads the same with a stamp and without while everything around it
/// differs. A stamp is a *texture*, and a texture is what varies from place to
/// place — so the comparison samples places.
fn profile(doc: &ClayDocument) -> Vec<Option<f32>> {
    let mut heights = Vec::new();
    for i in 0..7 {
        for j in 0..7 {
            let x = (i as f32 / 6.0 - 0.5) * 0.5;
            let y = (j as f32 / 6.0 - 0.5) * 0.5;
            heights.push(doc.pick([x, y, 4.0], [0.0, 0.0, -1.0]).map(|hit| hit[2]));
        }
    }
    heights
}

/// The field: a stroke takes no stamp, and this is where that is held.
///
/// The one route of the three that does not work, and the engine's limit
/// rather than a wiring gap. `clay_layer_apply_stroke` uses its item as a
/// template scaled per stamp and does not carry the deformer chain hung off
/// it — measured at the engine boundary in `claycore/tests/alpha_deformer.rs`,
/// where a placed item with the same alpha *does* change the surface and does
/// grade with the amplitude.
///
/// So the application refuses it by name in `AlphaSupport` rather than passing
/// an alpha that would be silently discarded, and this holds that the refusal
/// is honoured: with a stamp loaded and the brush set to use it, the field is
/// left exactly as it would have been.
#[test]
fn a_field_stroke_is_unchanged_by_a_loaded_stamp() {
    let (Some(mut plain), Some(mut stamped)) = (document(), document()) else {
        return;
    };
    for doc in [&mut plain, &mut stamped] {
        doc.set_combine(CombineSettings::for_strokes());
    }
    stamped.set_alpha(Some(rings(64)));

    let _ = plain.apply_stroke(ToolKind::Padrao, brush(false), &arc(), [false; 3]);
    let _ = stamped.apply_stroke(ToolKind::Padrao, brush(true), &arc(), [false; 3]);

    assert_eq!(
        profile(&plain),
        profile(&stamped),
        "a field stroke now differs with a stamp loaded. If the engine has \
         started carrying the template's deformer chain, that is good news — \
         AlphaSupport::of and claycore's alpha_deformer test should change \
         with it. If not, an alpha is reaching the field by some other route \
         and nothing says what it does."
    );
}

/// And a brush that is not set to use one is unaffected by a loaded stamp —
/// otherwise the flag is decorative and every brush is stamped.
#[test]
fn a_brush_not_set_to_use_a_stamp_ignores_the_loaded_one() {
    let (Some(mut without), Some(mut with_loaded)) = (document(), document()) else {
        return;
    };
    with_loaded.set_alpha(Some(rings(64)));

    let _ = without.apply_stroke(ToolKind::Padrao, brush(false), &arc(), [false; 3]);
    let _ = with_loaded.apply_stroke(ToolKind::Padrao, brush(false), &arc(), [false; 3]);

    assert_eq!(
        profile(&without),
        profile(&with_loaded),
        "a stamp was applied to a brush that is not set to use one"
    );
}

/// The grid: the stamp routes to the alpha carve, which is its own entry point
/// because the ordinary voxel verbs carry no alpha.
///
/// Removing the source layer is what makes this measurable, and it is the
/// whole trick. A voxel layer *is* evaluated into the document's field and
/// `pick` finds it — but the grid is quantised inward from the field it was
/// rasterized from, so with the source still present every ray meets the
/// original sphere and both grids answer identically no matter what the stroke
/// did. `stats` is no help either: it reports what the viewport last meshed,
/// and nothing has meshed here. Crossing back into a field afterwards gives a
/// surface that is the grid's alone.
#[test]
fn a_stamp_changes_what_a_grid_stroke_leaves() {
    let mut surfaces = Vec::new();
    for stamped in [false, true] {
        let Some(mut doc) = document() else {
            return;
        };
        let source = doc.scene().active.expect("the starting form");
        doc.convert_layer(Direction::SdfToVoxel, 0.04, 1)
            .expect("rasterize the starting form");
        doc.remove_layer(source)
            .expect("drop the field it came from");
        if stamped {
            doc.set_alpha(Some(rings(64)));
        }

        let outcome = doc
            .apply_stroke(ToolKind::Padrao, brush(stamped), &arc(), [false; 3])
            .expect("a voxel stroke");
        assert!(
            outcome.changed,
            "the stroke reached no cell at all, so the comparison says nothing"
        );

        doc.convert_layer(Direction::VoxelToSdf, 0.04, 1)
            .expect("read the grid back as a field");
        surfaces.push(profile(&doc));
    }

    assert_ne!(
        surfaces[0], surfaces[1],
        "the same stroke with and without a stamp left the same grid, so the \
         alpha carve is not being reached"
    );
}

/// The mesh: the stamp travels in the brush descriptor's own block.
#[test]
fn a_stamp_changes_what_a_mesh_stroke_leaves() {
    let path = std::env::temp_dir().join("clayspace-alpha-mesh.obj");
    let mut docs = Vec::new();
    for stamped in [false, true] {
        let Some(mut doc) = document() else {
            return;
        };
        if !stamped {
            let _ = std::fs::remove_file(&path);
            doc.export_mesh(&path, ExportSettings::default())
                .expect("export the starting form");
        }
        doc.import_mesh(&path, ImportSettings::default())
            .expect("import it back as a mesh layer");
        let key = doc
            .scene()
            .layers
            .iter()
            .find(|layer| layer.representation == Representation::Mesh)
            .map(|layer| layer.key)
            .expect("a mesh layer");
        doc.set_active_layer(key).expect("activate");
        if stamped {
            doc.set_alpha(Some(rings(64)));
        }
        doc.apply_stroke(ToolKind::Padrao, brush(stamped), &arc(), [false; 3])
            .expect("a mesh stroke");
        docs.push(doc);
    }
    let _ = std::fs::remove_file(&path);

    let heights: Vec<Vec<Option<f32>>> = docs.iter().map(profile).collect();
    assert_ne!(
        heights[0], heights[1],
        "the same mesh stroke with and without a stamp moved the vertices the \
         same way, so the descriptor's alpha block is not being filled"
    );
}
