//! A retopology's UV layout, seen on the layer it was laid out on.
//!
//! The chain the retopology workspace promises ends at "quality / preview":
//! the quads are laid out, the layer carries the layout, and the sculptor can
//! look at it on the form — a checker that stretches where the layout does,
//! islands told apart, seams drawn where the charts were cut. These drive a
//! real retopology through the renderer's own mesh-layer path.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_uv_preview
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Direction, LayerKey, RetopoModel, RetopoSettings, RetopoUv, SceneModel, UvDisplay, UvSettings,
};
use clayspace_view::Image;
use support::{differing_pixels, framed, Harness, RENDER_NOISE};

/// The starting form, meshed, retopologised with UVs, and the sculpt hidden
/// so the result is the only thing drawn. Returns the result's key.
fn laid_out() -> Option<(ClayDocument, LayerKey)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.05, 0).ok()?;
    let source = document.scene().active_layer()?.key;
    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            uv: Some(UvSettings {
                max_chart_distortion: 0.0,
                ..UvSettings::default()
            }),
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");
    assert!(
        matches!(outcome.uv, RetopoUv::Laid(_)),
        "UVs were asked for and the outcome is {:?}",
        outcome.uv
    );
    let placed = document.scene().active_layer()?.key;
    document
        .set_layer_visible(source, false)
        .expect("the sculpt hides");
    Some((document, placed))
}

/// Pixels a seam could have drawn: red well above both other channels.
fn seam_red(image: &Image) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| p[0] as i32 > p[1] as i32 + 80 && p[0] as i32 > p[2] as i32 + 80)
        .count()
}

/// Pixels `b` draws darker than `a` by more than the render noise, in every
/// channel — a dark checker square where the material was.
fn darkened(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(pa, pb)| (0..3).all(|c| pa[c] > pb[c].saturating_add(RENDER_NOISE)))
        .count()
}

#[test]
fn the_uv_checker_islands_and_seams_draw_on_the_result() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, placed)) = laid_out() else {
        return;
    };
    let camera = framed(&document);
    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
    let surface = clayspace_view::GpuMesh::new(&gpu);
    let preview = document
        .uv_preview(placed)
        .expect("readable")
        .expect("the result carries a layout");

    let plain = harness.capture(&surface, &camera, false, "74-uv-off");

    harness
        .renderer
        .set_uv_preview(&gpu, Some((&preview, UvDisplay::Checker)));
    assert_eq!(harness.renderer.uv_preview_layer(), Some(placed));
    let checker = harness.capture(&surface, &camera, false, "74-uv-checker");

    harness
        .renderer
        .set_uv_preview(&gpu, Some((&preview, UvDisplay::Islands)));
    let islands = harness.capture(&surface, &camera, false, "74-uv-islands");

    harness.renderer.set_uv_preview(&gpu, None);
    assert_eq!(harness.renderer.uv_preview_layer(), None);
    let again = harness.capture(&surface, &camera, false, "74-uv-off-again");

    let dark = darkened(&plain, &checker);
    let (red_before, red_checker, red_islands) =
        (seam_red(&plain), seam_red(&checker), seam_red(&islands));
    let tinted = differing_pixels(&checker, &islands);
    let lingering = differing_pixels(&plain, &again);
    println!(
        "checker darkened {dark} pixels; seam-red pixels {red_before} off, \
         {red_checker} checker, {red_islands} islands; islands differ from \
         the checker at {tinted}; {lingering} linger after switching off"
    );

    assert!(
        dark > 3000,
        "the checker darkened {dark} pixels, so its dark squares did not draw \
         — see target/visual/74-uv-checker.png"
    );
    assert!(
        red_checker > red_before + 60 && red_islands > red_before + 60,
        "the seams drew {red_checker} (checker) and {red_islands} (islands) \
         red pixels against {red_before} without them — see \
         target/visual/74-uv-checker.png"
    );
    assert!(
        tinted > 3000,
        "tinting the islands changed {tinted} pixels of the checker — see \
         target/visual/74-uv-islands.png"
    );
    assert_eq!(
        lingering, 0,
        "the layer did not go back to its own material when the preview was \
         switched off"
    );
}

/// A preview of a layer the viewport is not drawing draws nothing: it stands
/// in for a span of the carried buffer, and a hidden layer has none.
#[test]
fn a_hidden_layer_shows_no_uv_preview() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, placed)) = laid_out() else {
        return;
    };
    let preview = document
        .uv_preview(placed)
        .expect("readable")
        .expect("the result carries a layout");
    document.set_layer_visible(placed, false).expect("hides");
    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
    harness
        .renderer
        .set_uv_preview(&gpu, Some((&preview, UvDisplay::Checker)));
    assert_eq!(harness.renderer.uv_preview_layer(), None);
}
