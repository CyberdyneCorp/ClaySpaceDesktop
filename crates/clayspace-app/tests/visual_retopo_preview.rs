//! A finished retopology, seen while it waits to be accepted or discarded.
//!
//! The job's result is held rather than placed, so the viewport has to draw a
//! mesh the document does not hold yet. These drive a real retopology to that
//! point and hold what the preview promises on screen: it is drawn in place of
//! its source, its layout can be looked at before it is kept, discarding it
//! puts the source back pixel for pixel, and accepting it draws what the
//! preview drew.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_retopo_preview
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument, EngineRetopologiser};
use clayspace_model::{
    Direction, RetopoModel, RetopoResult, RetopoSettings, Retopologiser, SceneModel, UvDisplay,
    UvSettings,
};
use clayspace_view::{GpuMesh, Image, MeshSpan};
use support::{differing_pixels, framed, Harness, RENDER_NOISE};

fn with_uvs() -> RetopoSettings {
    RetopoSettings {
        target_quads: 600,
        uv: Some(UvSettings {
            max_chart_distortion: 0.0,
            ..UvSettings::default()
        }),
        ..RetopoSettings::default()
    }
}

/// A document whose active layer has been read and retopologised, and the
/// result the job would hand back — held, not placed.
fn held(document: &mut ClayDocument, settings: RetopoSettings) -> RetopoResult {
    let source = document.retopo_source().expect("the source reads");
    EngineRetopologiser
        .run(&source, settings, &|_, _| {}, &|| false)
        .expect("the retopology runs")
}

/// The carried layers as the application uploads them: with each layer's
/// authored edges, so a placed retopology's polyframe draws its quads.
fn upload_layers(harness: &mut Harness, document: &mut ClayDocument) {
    let (positions, normals, colors, indices, spans) = document.visible_mesh_geometry();
    let vertices: Vec<clayspace_view::Vertex> = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .map(|((position, normal), color)| clayspace_view::Vertex {
            position,
            normal,
            color,
            mask: 0.0,
        })
        .collect();
    let spans: Vec<MeshSpan> = spans
        .into_iter()
        .map(|span| MeshSpan::with_edges(span.layer, span.indices, span.edges))
        .collect();
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
}

/// Pixels a seam could have drawn: red well above both other channels.
fn seam_red(image: &Image) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| p[0] as i32 > p[1] as i32 + 80 && p[0] as i32 > p[2] as i32 + 80)
        .count()
}

/// Pixels `b` draws darker than `a` in every channel.
fn darkened(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(pa, pb)| (0..3).all(|c| pa[c] > pb[c].saturating_add(RENDER_NOISE)))
        .count()
}

#[test]
fn a_held_retopology_draws_in_place_of_its_source_and_accepting_draws_it_again() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let policy = BackendPolicy::discover(None).expect("a backend");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("the starting form");
    document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("meshed");
    let source = document.scene().active_layer().expect("active").key;
    let camera = framed(&document);
    let surface = GpuMesh::new(&harness.gpu);
    let gpu = harness.gpu.clone();

    upload_layers(&mut harness, &mut document);
    let sculpt = harness.capture(&surface, &camera, false, "75-retopo-source");
    harness.renderer.set_polyframe(&gpu, true);
    let sculpt_wire = harness.capture(&surface, &camera, false, "75-retopo-source-polyframe");
    harness.renderer.set_polyframe(&gpu, false);

    let result = held(&mut document, with_uvs());
    let preview = document.retopo_preview(&result).expect("the held preview");
    let show = |harness: &mut Harness, display| {
        harness
            .renderer
            .set_retopo_preview(&gpu, Some((&preview, result.edges.as_slice(), display)));
    };

    show(&mut harness, UvDisplay::Off);
    assert_eq!(harness.renderer.retopo_preview_source(), Some(source));
    assert!(!harness.renderer.retopo_preview_shows_layout());
    let material = harness.capture(&surface, &camera, false, "75-retopo-held");
    harness.renderer.set_polyframe(&gpu, true);
    let wire = harness.capture(&surface, &camera, false, "75-retopo-held-polyframe");
    harness.renderer.set_polyframe(&gpu, false);

    show(&mut harness, UvDisplay::Checker);
    assert!(harness.renderer.retopo_preview_shows_layout());
    let checker = harness.capture(&surface, &camera, false, "75-retopo-held-checker");
    show(&mut harness, UvDisplay::Islands);
    let islands = harness.capture(&surface, &camera, false, "75-retopo-held-islands");

    // Discarded: the source comes back exactly as it was drawn.
    harness.renderer.set_retopo_preview(&gpu, None);
    assert_eq!(harness.renderer.retopo_preview_source(), None);
    let discarded = harness.capture(&surface, &camera, false, "75-retopo-discarded");
    harness.renderer.set_polyframe(&gpu, true);
    let discarded_wire = harness.capture(&surface, &camera, false, "75-retopo-discarded-polyframe");
    harness.renderer.set_polyframe(&gpu, false);

    // Accepted: placed as the job used to place it, the source hidden so the
    // layer is drawn alone — which is what the preview showed.
    document
        .place_retopology(&result, with_uvs())
        .expect("the preview is accepted");
    document.set_layer_visible(source, false).expect("hides");
    upload_layers(&mut harness, &mut document);
    let accepted = harness.capture(&surface, &camera, false, "75-retopo-accepted");
    harness.renderer.set_polyframe(&gpu, true);
    let accepted_wire = harness.capture(&surface, &camera, false, "75-retopo-accepted-polyframe");

    let replaced = differing_pixels(&sculpt, &material);
    let wire_replaced = differing_pixels(&sculpt_wire, &wire);
    let dark = darkened(&material, &checker);
    let (red_material, red_checker, red_islands) =
        (seam_red(&material), seam_red(&checker), seam_red(&islands));
    let tinted = differing_pixels(&checker, &islands);
    let lingering = differing_pixels(&sculpt, &discarded);
    let lingering_wire = differing_pixels(&sculpt_wire, &discarded_wire);
    let as_held = differing_pixels(&material, &accepted);
    let as_held_wire = differing_pixels(&wire, &accepted_wire);
    println!(
        "held preview differs from the source at {replaced} px ({wire_replaced} with the \
         polyframe); checker darkened {dark} px; seam-red {red_material} material, \
         {red_checker} checker, {red_islands} islands; islands differ from the checker \
         at {tinted} px; after discard {lingering} px linger ({lingering_wire} with the \
         polyframe); accepted differs from the held preview at {as_held} px \
         ({as_held_wire} with the polyframe)"
    );

    assert!(
        wire_replaced > 3000,
        "the preview's quads did not replace the sculpt's triangles under the \
         polyframe ({wire_replaced} px) — see target/visual/75-retopo-held-polyframe.png"
    );
    assert!(
        dark > 3000,
        "the held preview's checker darkened {dark} px — see \
         target/visual/75-retopo-held-checker.png"
    );
    assert!(
        red_checker > red_material + 60 && red_islands > red_material + 60,
        "the held preview's seams drew {red_checker}/{red_islands} red px against \
         {red_material} — see target/visual/75-retopo-held-checker.png"
    );
    assert!(
        tinted > 3000,
        "islands changed only {tinted} px of the checker"
    );
    assert_eq!(lingering, 0, "discarding left the preview on screen");
    assert_eq!(lingering_wire, 0, "discarding left the preview's edges");
    assert_eq!(
        as_held, 0,
        "the accepted layer is drawn differently from the preview that was accepted"
    );
    assert_eq!(
        as_held_wire, 0,
        "the accepted layer's polyframe differs from the preview's"
    );
}

/// A field source is not a carried span and cannot be cut out of the field's
/// one surface, so its preview is drawn over it — outside the carried layers'
/// pass, which a scene with no mesh layer never enters.
#[test]
fn a_preview_of_a_field_source_draws_in_a_scene_with_no_mesh_layer() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let policy = BackendPolicy::discover(None).expect("a backend");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("the starting form");
    let camera = framed(&document);
    let surface = GpuMesh::new(&harness.gpu);
    let gpu = harness.gpu.clone();
    let empty = harness.capture(&surface, &camera, false, "75-retopo-field-empty");

    let result = held(
        &mut document,
        RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        },
    );
    let preview = document.retopo_preview(&result).expect("the held preview");
    harness.renderer.set_retopo_preview(
        &gpu,
        Some((&preview, result.edges.as_slice(), UvDisplay::Checker)),
    );
    assert!(
        !harness.renderer.retopo_preview_shows_layout(),
        "a result with no layout was drawn as one"
    );
    let drawn = harness.capture(&surface, &camera, false, "75-retopo-field-held");
    let covered = differing_pixels(&empty, &drawn);
    println!("the held preview of a field source covers {covered} px");
    assert!(
        covered > 10_000,
        "the preview of a field source drew {covered} px — see \
         target/visual/75-retopo-field-held.png"
    );
}
