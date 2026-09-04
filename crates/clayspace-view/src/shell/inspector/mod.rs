//! What can be controlled about the representation being sculpted.
//!
//! One section, in a fixed slot in the right region, whose *contents* change
//! with the active layer and whose position does not. The panel around it
//! stays where it is: a sculptor moving between an SDF layer and a grid should
//! find the material where they left it, not a rearranged inspector.
//!
//! # Only what the domain has
//!
//! Each of these shows the controls and facts that actually exist for its
//! representation, and nothing else. The concept art this was drawn from lists
//! a field quality, an evaluation resolution, a surface offset, a field
//! smoothness, a voxel size, a grid bounds, a filtering mode, a normals
//! control and a subdivision level. Not one of them is a thing this
//! application's domain or the pinned engine can express per layer, and
//! drawing a control for a value nothing reads is worse than leaving the space
//! empty — it is an interface that lies about what the program does.
//!
//! One item on that list has stopped being unexpressible, and now has its
//! control. ClayCore 0.78.0's hierarchy has a subdivision level per surface —
//! two of them, since where the brush writes and what is drawn are independent
//! — and both are per-layer state a hierarchy row carries, so both are drawn.
//! It is the only entry on that list this rule ever excluded for want of an
//! engine rather than for want of something to say.
//!
//! What is genuinely here is small, and three of the four representations carry
//! most of their story elsewhere on purpose:
//!
//! - a **field**'s combine vocabulary is in the options bar, because it
//!   belongs to the stroke rather than to the layer;
//! - a **grid**'s recorded passes are nested under the layer they were
//!   recorded on, in the left stack, because a pass has no meaning apart from
//!   that grid;
//! - and a **hierarchy**'s passes are the same arrangement for the same reason;
//! - and the advisory to collapse a costly field stays under the layer list,
//!   where it appears only when the engine is actually advising it.
//!
//! Duplicating any of those here would give a sculptor two places to look and
//! two places to keep in agreement.

use super::*;

mod mesh;
pub mod multires;
mod sdf;
mod voxel;

/// The contextual section for whatever is being sculpted.
pub fn representation_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    match state.representation {
        Representation::Sdf => sdf::show(ui, state),
        Representation::Voxel => voxel::show(ui, state, queue),
        Representation::Mesh => mesh::show(ui, state),
        Representation::Multires => multires::show(ui, state, queue),
    }
}
