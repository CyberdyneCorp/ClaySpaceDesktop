//! Re-snapping a retopologised mesh onto a sculpt that has moved.
//!
//! **The case this exists for**: retopology has started, the sculptor goes back
//! to the field and changes the form, and the low-poly no longer follows it.
//! Redoing the retopology would throw away the topology; this keeps it exactly
//! and moves the vertices.
//!
//! It **completes and flags** rather than refusing or silently stretching, and
//! that is the whole of its contract: it reports the maximum and RMS deviation
//! and names the vertices that moved further than a caller-set threshold. A
//! host that drops those figures turns "completed with a warning" back into a
//! silent stretch.

use cyberremesh_sys as sys;

use crate::error::{check, Result};
use crate::mesh::Mesh;

/// What a conform came to.
#[derive(Debug, Clone, PartialEq)]
pub struct Conformed {
    pub moved_vertices: usize,
    /// The furthest any vertex had to travel.
    pub max_deviation: f32,
    pub rms_deviation: f32,
    /// Vertices that moved further than the threshold.
    ///
    /// The count is the engine's own and may exceed what `flagged` holds: the
    /// buffer is bounded, and a conform that flagged ten thousand vertices
    /// should say so rather than hand back the first hundred as though that
    /// were all of them.
    pub flagged_count: usize,
    /// The flagged vertex indices actually returned, up to the cap asked for.
    pub flagged: Vec<u32>,
}

impl Conformed {
    /// Whether the report is complete or truncated.
    pub fn all_flagged_returned(&self) -> bool {
        self.flagged.len() == self.flagged_count
    }
}

/// Re-snaps `edit` onto `target`, preserving its topology exactly.
///
/// `max_flagged` bounds how many flagged indices come back; the count always
/// reports the true total.
pub fn conform(
    edit: &mut Mesh,
    target: &Mesh,
    threshold: f32,
    max_flagged: usize,
) -> Result<Conformed> {
    let mut report = sys::CyberConformReport::default();
    let mut flagged = vec![0u32; max_flagged];
    check(
        // SAFETY: a valid mesh the engine moves vertices in, a valid target it
        // only reads, a descriptor written on success, and a destination
        // holding exactly the `max_flagged` indices its capacity is declared
        // as. A zero capacity is passed with a null-safe empty vector, which
        // the engine reads as "report the count and return none".
        unsafe {
            sys::cyber_conform(
                edit.as_ptr(),
                target.as_ptr(),
                threshold,
                &mut report,
                flagged.as_mut_ptr(),
                max_flagged,
            )
        },
        "cyber_conform",
    )?;
    flagged.truncate(report.flaggedCount.min(max_flagged));
    Ok(Conformed {
        moved_vertices: report.movedVertices,
        max_deviation: report.maxDeviation,
        rms_deviation: report.rmsDeviation,
        flagged_count: report.flaggedCount,
        flagged,
    })
}
