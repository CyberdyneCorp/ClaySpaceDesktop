use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, LayerOperation, MaskModel, SculptModel, ToolKind,
};

fn mesh() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backend");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("starting form");
    document
        .convert_layer(Direction::SdfToMesh, 0.06, 0)
        .expect("convert to mesh");
    document
}

fn taper(document: &mut ClayDocument) {
    document
        .apply_operation(LayerOperation::Taper {
            axis: [0.0, 1.0, 0.0],
            span: 2.0,
            scale_start: 1.0,
            scale_end: 0.5,
        })
        .expect("taper");
}

#[test]
fn a_painted_mask_protects_mesh_vertices_from_a_deformer() {
    let mut free = mesh();
    let before = free.visible_mesh_geometry().0;
    taper(&mut free);
    let after_free = free.visible_mesh_geometry().0;
    assert_ne!(before, after_free, "taper moved no vertices without a mask");

    let mut frozen = mesh();
    frozen
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 10.0,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("paint mask");
    assert!(frozen.mask_state().painted_cells > 0);
    let before_frozen = frozen.visible_mesh_geometry().0;
    let weights = frozen.mask_at(&before_frozen).expect("mask weights");
    taper(&mut frozen);
    let after_frozen = frozen.visible_mesh_geometry().0;
    let mut protected = 0;
    let mut frozen_motion = 0.0;
    let mut free_motion = 0.0;
    for (index, ((before, after), weight)) in before_frozen
        .iter()
        .zip(&after_frozen)
        .zip(weights)
        .enumerate()
    {
        if weight > 0.99 {
            protected += 1;
            frozen_motion += before
                .iter()
                .zip(after)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>();
            free_motion += before
                .iter()
                .zip(&after_free[index])
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>();
        }
    }
    assert!(protected > 0, "no fully masked vertices were sampled");
    assert!(free_motion > 0.0);
    assert!(
        frozen_motion < free_motion * 0.1,
        "mask failed to protect vertices: {frozen_motion} motion against {free_motion} unmasked"
    );
}
