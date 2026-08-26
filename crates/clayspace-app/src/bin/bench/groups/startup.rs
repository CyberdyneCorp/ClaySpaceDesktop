//! Everything between launching and being able to show a window.

use std::time::Instant;

use clayspace_engine::{BackendPolicy, ClayDocument};

use crate::figures::{ms, Figure};
use crate::run::Run;
use crate::skip::Skip;

/// Not a window appearing — that needs a display, and the budget this stands
/// against is 2 seconds for exactly this work plus the presentation.
pub fn measure(run: &mut Run) {
    let started = Instant::now();
    let Ok(policy) = BackendPolicy::discover(None) else {
        return run.skip("startup", Skip::NoBackends);
    };
    let discovery = started.elapsed();

    let document = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form);
    let ready = started.elapsed();
    drop(document);

    run.insert("startup.backend_discovery", Figure::ms(ms(discovery), None));
    // The window has to be up within 2 s including discovery; this is the
    // engine-side share of that.
    run.insert(
        "startup.to_first_document",
        Figure::ms(ms(ready), Some(2000.0)),
    );
}
