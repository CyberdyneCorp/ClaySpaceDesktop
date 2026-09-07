//! Can a sculptor reach the cut tool through the interface?
//!
//! The engine tests prove a drawn shape removes the right half. This proves
//! the press, the drag and the release reach it — the wire that was missing
//! when the tool was on the shelf and did nothing.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{OutlineFrame, ToolKind};
use clayspace_vm::{Command, CutViewModel, SculptViewModel};

fn document() -> Option<SharedDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    Some(SharedDocument::new(document))
}

/// The frame the viewport builds looking along -Z.
fn frame() -> OutlineFrame {
    OutlineFrame {
        origin: [0.0; 3],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [2.0, 2.0],
    }
}

fn inside(document: &SharedDocument, at: [f32; 3]) -> bool {
    document.with(|d| {
        d.document()
            .eval_points(None, &[at])
            .ok()
            .and_then(|v| v.first().copied())
            .is_some_and(|value| value < 0.0)
    })
}

/// Press, drag, release — the three the viewport sends — and the form loses
/// the half the direction named.
#[test]
fn a_drawn_line_reaches_the_document_through_the_view_model() {
    let Some(document) = document() else {
        return;
    };
    let mut cut = CutViewModel::new(Box::new(document.clone()));

    assert!(inside(&document, [0.0, -0.55, 0.0]), "fixture");

    cut.dispatch(&Command::BeginCut([-0.9, 0.0]));
    for at in [[-0.4, 0.0], [0.2, 0.0], [0.9, 0.0]] {
        cut.dispatch(&Command::ExtendCut(at));
    }
    assert!(
        cut.draft().get().is_some(),
        "the draft is what the overlay draws while the pointer is down"
    );
    cut.dispatch(&Command::EndCut(frame()));

    assert!(
        cut.draft().get().is_none(),
        "the draft outlived its gesture"
    );
    assert!(cut.notice().get().is_none(), "the cut was refused");
    assert!(
        !inside(&document, [0.0, -0.55, 0.0]),
        "a line drawn rightwards should have taken the half below it"
    );
    assert!(
        inside(&document, [0.0, 0.55, 0.0]),
        "and should have left the half above"
    );
}

/// The tool is what decides a press draws a cut, so the shelf entry is the
/// switch and nothing else has to know.
#[test]
fn the_cut_is_the_tool_the_shelf_selects() {
    let Some(document) = document() else {
        return;
    };
    let mut sculpt = SculptViewModel::new(Box::new(document.clone()));
    sculpt.dispatch(Command::SelectTool(ToolKind::Trim)).ok();
    assert_eq!(*sculpt.tool().get(), ToolKind::Trim);
}
