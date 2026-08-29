//! Crossing between representations, and what each crossing costs.
//!
//! ClayCore carries SDF, voxel and mesh side by side and the intended workflow
//! uses more than one: block out and hard-surface on SDF, free-form sculpt on
//! voxels, refine on a mesh when the topology is one worth keeping. Crossing is
//! how a sculptor gets from one to the next.
//!
//! Every crossing is lossy, and the losses are *stated before the conversion
//! runs* rather than discovered in the result. They are computed from the
//! chosen resolution rather than written into a sentence, because a number
//! written down is wrong the first time somebody changes the default.

use crate::Representation;

/// A crossing from one representation to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Rasterize a field into cells.
    SdfToVoxel,
    /// Read occupancy back as a distance field, redistanced.
    VoxelToSdf,
    /// Triangles straight into cells, in one sampling.
    MeshToVoxel,
    /// Triangles onto a lattice as a volume item.
    MeshToSdf,
    /// Marches the field into triangles, on a layer that can be sculpted with
    /// the mesh brushes.
    SdfToMesh,
    /// The grid's exposed faces as triangles, likewise.
    VoxelToMesh,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Self::SdfToVoxel,
        Self::VoxelToSdf,
        Self::MeshToVoxel,
        Self::MeshToSdf,
        Self::SdfToMesh,
        Self::VoxelToMesh,
    ];

    pub fn from(self) -> Representation {
        match self {
            Self::SdfToVoxel | Self::SdfToMesh => Representation::Sdf,
            Self::VoxelToSdf | Self::VoxelToMesh => Representation::Voxel,
            Self::MeshToVoxel | Self::MeshToSdf => Representation::Mesh,
        }
    }

    pub fn to(self) -> Representation {
        match self {
            Self::SdfToVoxel | Self::MeshToVoxel => Representation::Voxel,
            Self::VoxelToSdf | Self::MeshToSdf => Representation::Sdf,
            Self::SdfToMesh | Self::VoxelToMesh => Representation::Mesh,
        }
    }

    /// The crossings available from a representation.
    pub fn from_representation(representation: Representation) -> Vec<Direction> {
        Self::ALL
            .into_iter()
            .filter(|direction| direction.from() == representation)
            .collect()
    }

    /// Whether the crossing needs a region to be rasterized into.
    ///
    /// A document may be unbounded, so rasterizing one needs to be told where
    /// to stop. A mesh cannot be unbounded and a grid already has bounds, so
    /// neither does.
    pub fn needs_region(self) -> bool {
        // Marching a field needs to be told where to stop for the same reason
        // rasterizing one does — the lattice is bounded even though what comes
        // off it is triangles.
        matches!(self, Self::SdfToVoxel | Self::SdfToMesh)
    }

    /// Whether the crossing is measured in cells at all.
    ///
    /// Reading a grid back does not choose a resolution — it already has one —
    /// so there is no cell size to state a cost against.
    pub fn chooses_resolution(self) -> bool {
        // Marching chooses one as surely as rasterizing does: the cell is what
        // the surface is found on, so it sets how far the surface can move and
        // what is too thin to be found at all. Reading a grid does not — the
        // grid already has a cell, and its faces are where they are.
        //
        // Mesh-to-SDF belongs here and did not. `clay_item_volume_from_mesh`
        // "samples the model onto a lattice", and the engine's own note on the
        // parameter is that leaving the cell unset "picks from the source's
        // own size" — it picks one, it does not do without one. So the
        // crossing moved the surface by half a cell and lost features thinner
        // than one all along, and this said it moved nothing and kept its
        // sharp edges. A sculptor was told a crossing was free.
        //
        // Found by placing a mesh as a boolean operand, which pays the same
        // crossing and states the same costs — and stated none.
        matches!(
            self,
            Self::SdfToVoxel | Self::MeshToVoxel | Self::SdfToMesh | Self::MeshToSdf
        )
    }

    /// Whether what comes out has a topology nothing here will change again.
    ///
    /// A mesh layer is sculpted by moving the vertices it has: no verb adds,
    /// removes or re-flows them, because that would spend the retopology an
    /// import was for. What a crossing *produces* has no retopology to spend —
    /// it is the sampling lattice's own grid, dense and uniform, with no edge
    /// loop following anything. It sculpts, and it is the input a retopology
    /// pass replaces rather than the output one produces. Worth saying before
    /// the crossing, not after.
    pub fn ends_in_fixed_topology(self) -> bool {
        self.to() == Representation::Mesh
    }
}

/// What the conversion panel is set to.
///
/// The direction and the resolution are the whole of the decision, and the
/// costs follow from them — so this is what a command carries and the costs are
/// recomputed rather than sent alongside, which would let the two disagree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConversionSettings {
    pub direction: Direction,
    /// The cell size a crossing into voxels would use, in document units.
    pub cell_size: f32,
    /// How much the lattice is filtered on the way out of a grid.
    ///
    /// 0 keeps the terracing and loses nothing; 1 is what an organic sculpt
    /// wants. Meaningless in the directions that do not read a grid.
    pub blur: i32,
    /// Whether the crossing replaces the layer it read.
    ///
    /// Off by default, which is the crossing that cannot lose work: the
    /// source stays and a sculptor who dislikes the result removes the layer
    /// it made. On, the source leaves as the result arrives and the result
    /// stands where it stood — which is what a sculptor means by converting
    /// *this* layer, and what avoids a stack of supplanted originals nobody
    /// meant to keep.
    pub in_place: bool,
}

impl Default for ConversionSettings {
    fn default() -> Self {
        Self {
            direction: Direction::SdfToVoxel,
            // The brick cache's own cell, so a first crossing lands at the
            // resolution the rest of the application already works at.
            cell_size: 0.02,
            blur: 1,
            in_place: false,
        }
    }
}

impl ConversionSettings {
    /// The bounds a cell size is clamped to.
    ///
    /// Not a matter of taste: below the floor a crossing of any real extent
    /// exceeds the memory budget, and above the ceiling the result is coarser
    /// than the form it came from.
    pub const CELL_RANGE: std::ops::RangeInclusive<f32> = 0.002..=0.2;

    pub fn sanitized(mut self) -> Self {
        self.cell_size = self
            .cell_size
            .clamp(*Self::CELL_RANGE.start(), *Self::CELL_RANGE.end());
        self.blur = self.blur.clamp(0, 2);
        self
    }
}

/// What is wrong with a voxel grid, before it is baked.
///
/// Reported before anything is repaired, always: a repair changes the sculpt,
/// and a sculptor who cannot see what it would change is being asked to
/// consent to something unstated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RepairReport {
    /// Empty regions the outside cannot reach.
    pub enclosed_voids: usize,
    /// Their total size, in cells.
    pub void_cells: usize,
    pub largest_void: usize,
    /// Set when there are no enclosed voids at all.
    pub airtight: bool,
}

/// Why a conversion was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The source has no bounds and no region was given, so there is nothing
    /// to say where the rasterization stops.
    UnboundedRegion,
    /// The grid the chosen resolution would build does not fit the budget.
    OverBudget { cells: u64, budget_bytes: u64 },
    /// The source carries nothing to convert.
    SourceEmpty,
    /// This crossing starts from a different representation.
    WrongSource {
        needs: Representation,
        active: Representation,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnboundedRegion => f.write_str(
                "this layer has no bounds, so a region is needed to say where \
                 the rasterization stops",
            ),
            Self::OverBudget {
                cells,
                budget_bytes,
            } => write!(
                f,
                "that resolution needs {cells} cells, past the {} MB budget",
                budget_bytes / (1024 * 1024)
            ),
            Self::SourceEmpty => f.write_str("this layer carries nothing to convert"),
            Self::WrongSource { needs, active } => write!(
                f,
                "that crossing starts from a {} layer; this one is {}",
                needs.label(),
                active.label()
            ),
        }
    }
}

/// What a crossing will cost, at the resolution it is being asked for.
///
/// Computed rather than written down. A sculptor changing the cell size should
/// watch these move, because that is the whole of the decision they are making.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    /// How far the surface can move, in document units. Half a cell: a cell is
    /// either in or out, so the surface lands on the nearer cell boundary.
    pub surface_movement: f32,
    /// A feature thinner than this can vanish, because no cell centre falls
    /// inside it. Nothing downstream can invent what was never stored.
    pub vanishing_feature: f32,
    /// How many cells the region holds at this resolution.
    pub cells: u64,
    /// Whether the procedural edit list survives the crossing.
    ///
    /// It does not, in any direction that ends in cells or vertices: once
    /// rasterized, the parametric items behind the sculpt are not reachable
    /// from the other side. That is why a crossing produces a new layer rather
    /// than replacing one.
    pub keeps_history: bool,
    /// Whether a sharp edge survives, or becomes a staircase at the cell size.
    pub keeps_sharp_edges: bool,
    /// Whether surface colour survives the crossing.
    pub keeps_colour: bool,
    /// Whether what comes out has a topology nothing here will change again.
    pub fixed_topology: bool,
}

impl Cost {
    /// What the crossing costs at `cell_size`, over a region of `extent`.
    ///
    /// `extent` is the region's size in document units along each axis.
    pub fn of(direction: Direction, cell_size: f32, extent: [f32; 3]) -> Self {
        let cell_size = cell_size.max(f32::EPSILON);
        let cells = if direction.chooses_resolution() {
            extent
                .iter()
                .map(|span| ((span / cell_size).ceil().max(1.0)) as u64)
                .product()
        } else {
            0
        };
        Self {
            surface_movement: if direction.chooses_resolution() {
                cell_size * 0.5
            } else {
                0.0
            },
            vanishing_feature: if direction.chooses_resolution() {
                cell_size
            } else {
                0.0
            },
            cells,
            // Nothing carries the edit list across. The SDF side *is* its
            // history, and neither cells nor vertices hold one.
            keeps_history: false,
            // Only the directions that do not quantise onto a lattice.
            keeps_sharp_edges: !direction.chooses_resolution(),
            // Every direction here carries colour: the tape's colour field
            // reaches the palette, the palette reaches one volume item per
            // entry, and a mesh's vertex colours reach the palette directly.
            // The one that would not is a mesh through a *field*, which is why
            // mesh-to-voxel is direct rather than a detour.
            keeps_colour: true,
            fixed_topology: direction.ends_in_fixed_topology(),
        }
    }

    /// Whether this many cells fits a byte budget, at `bytes_per_cell`.
    pub fn within(&self, budget_bytes: u64, bytes_per_cell: u64) -> Result<(), Refusal> {
        if self.cells.saturating_mul(bytes_per_cell) > budget_bytes {
            return Err(Refusal::OverBudget {
                cells: self.cells,
                budget_bytes,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_direction_goes_somewhere_else() {
        for direction in Direction::ALL {
            assert_ne!(
                direction.from(),
                direction.to(),
                "{direction:?} does not cross anything"
            );
        }
    }

    #[test]
    fn a_mesh_can_reach_both_of_the_others() {
        let from_mesh = Direction::from_representation(Representation::Mesh);
        assert_eq!(from_mesh.len(), 2, "a mesh converts two ways");
    }

    /// The reason the numbers are computed: they have to follow the slider.
    #[test]
    fn a_finer_cell_moves_the_surface_less_and_costs_more_cells() {
        let extent = [1.0, 1.0, 1.0];
        let coarse = Cost::of(Direction::SdfToVoxel, 0.1, extent);
        let fine = Cost::of(Direction::SdfToVoxel, 0.01, extent);

        assert!(
            fine.surface_movement < coarse.surface_movement,
            "a finer cell has to place the surface more precisely"
        );
        assert!(
            fine.vanishing_feature < coarse.vanishing_feature,
            "a finer cell has to keep smaller features"
        );
        assert!(
            fine.cells > coarse.cells,
            "a finer cell has to cost more storage"
        );
    }

    #[test]
    fn the_surface_moves_by_half_a_cell() {
        let cost = Cost::of(Direction::SdfToVoxel, 0.02, [1.0; 3]);
        assert_eq!(cost.surface_movement, 0.01);
        assert_eq!(cost.vanishing_feature, 0.02);
    }

    /// Reading a grid back chooses no resolution, so it has no cell-sized
    /// losses to state — and stating one anyway would be an invention.
    #[test]
    fn a_direction_that_chooses_no_resolution_states_no_cell_cost() {
        let cost = Cost::of(Direction::VoxelToSdf, 0.02, [1.0; 3]);
        assert_eq!(cost.surface_movement, 0.0);
        assert_eq!(cost.vanishing_feature, 0.0);
        assert!(cost.keeps_sharp_edges);
    }

    #[test]
    fn no_direction_carries_the_edit_list() {
        for direction in Direction::ALL {
            assert!(
                !Cost::of(direction, 0.02, [1.0; 3]).keeps_history,
                "{direction:?} claims to keep the procedural history"
            );
        }
    }

    #[test]
    fn a_resolution_past_the_budget_is_refused_with_the_budget() {
        let cost = Cost::of(Direction::SdfToVoxel, 0.001, [1.0; 3]);
        let error = cost
            .within(512 * 1024 * 1024, 4)
            .expect_err("a billion cells does not fit 512 MB");
        assert!(
            error.to_string().contains("512 MB"),
            "the refusal must name the budget it would exceed: {error}"
        );
    }

    #[test]
    fn a_resolution_inside_the_budget_is_allowed() {
        let cost = Cost::of(Direction::SdfToVoxel, 0.02, [1.0; 3]);
        assert!(cost.within(512 * 1024 * 1024, 4).is_ok());
    }
}

/// What the deform panel is set to.
///
/// The two whole-form deformers a panel can express. A lattice is the third and
/// is not here: it is dragged by its control points, so what it needs is a cage
/// in the viewport rather than four numbers, and a panel offering it would be
/// offering a control that cannot say what it does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeformSettings {
    pub verb: DeformVerb,
    /// Which axis the effect ramps along. Normalised before use.
    pub axis: [f32; 3],
    /// How far along that axis the ramp reaches, in document units.
    pub span: f32,
    /// Taper's cross-section scale at each end of the span.
    pub scale_start: f32,
    pub scale_end: f32,
    /// Twist's rotation across the span, in degrees.
    ///
    /// Degrees here and radians at the engine: a panel that asks for radians
    /// asks a sculptor to do arithmetic to make a quarter turn.
    pub degrees: f32,
}

/// Which whole-form deformer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeformVerb {
    /// The cross-section scale ramps along an axis.
    #[default]
    Taper,
    /// Rotation about an axis ramps along it.
    Twist,
}

impl DeformVerb {
    pub const ALL: [DeformVerb; 2] = [Self::Taper, Self::Twist];

    pub fn label(self) -> &'static str {
        match self {
            Self::Taper => "Afunilar",
            Self::Twist => "Torcer",
        }
    }

    /// Whether the scale controls do anything for this verb.
    pub fn takes_a_scale(self) -> bool {
        self == Self::Taper
    }

    /// Whether the angle control does.
    pub fn takes_an_angle(self) -> bool {
        self == Self::Twist
    }
}

impl Default for DeformSettings {
    fn default() -> Self {
        Self {
            verb: DeformVerb::Taper,
            // Up: the axis a form is most often tapered or twisted along.
            axis: [0.0, 1.0, 0.0],
            span: 2.0,
            scale_start: 1.0,
            scale_end: 0.5,
            degrees: 45.0,
        }
    }
}

impl DeformSettings {
    /// What a span may be, in document units.
    pub const SPAN_RANGE: std::ops::RangeInclusive<f32> = 0.1..=10.0;
    /// What a cross-section scale may be.
    ///
    /// Zero would collapse the section to a point, which is a degenerate
    /// surface rather than a taper; the floor keeps it a form.
    pub const SCALE_RANGE: std::ops::RangeInclusive<f32> = 0.05..=4.0;
    /// A full turn each way, and no more: past it the twist wraps and the
    /// number stops describing what is seen.
    pub const DEGREES_RANGE: std::ops::RangeInclusive<f32> = -360.0..=360.0;

    pub fn sanitized(mut self) -> Self {
        self.span = self
            .span
            .clamp(*Self::SPAN_RANGE.start(), *Self::SPAN_RANGE.end());
        self.scale_start = self
            .scale_start
            .clamp(*Self::SCALE_RANGE.start(), *Self::SCALE_RANGE.end());
        self.scale_end = self
            .scale_end
            .clamp(*Self::SCALE_RANGE.start(), *Self::SCALE_RANGE.end());
        self.degrees = self
            .degrees
            .clamp(*Self::DEGREES_RANGE.start(), *Self::DEGREES_RANGE.end());
        // A zero axis names no direction, and the engine would be asked to
        // ramp along nothing. Up is the fallback rather than an error: the
        // control that produces this is three sliders, and one of them
        // reaching zero mid-drag is not a mistake worth a dialog.
        let length = (self.axis[0].powi(2) + self.axis[1].powi(2) + self.axis[2].powi(2)).sqrt();
        self.axis = if length < 1e-6 {
            [0.0, 1.0, 0.0]
        } else {
            [
                self.axis[0] / length,
                self.axis[1] / length,
                self.axis[2] / length,
            ]
        };
        self
    }

    /// The operation these settings describe.
    ///
    /// Built here rather than in the interface so the degrees-to-radians
    /// conversion and the normalisation happen once, where they can be tested.
    pub fn operation(self) -> crate::LayerOperation {
        let settings = self.sanitized();
        match settings.verb {
            DeformVerb::Taper => crate::LayerOperation::Taper {
                axis: settings.axis,
                span: settings.span,
                scale_start: settings.scale_start,
                scale_end: settings.scale_end,
            },
            DeformVerb::Twist => crate::LayerOperation::Twist {
                axis: settings.axis,
                span: settings.span,
                angle: settings.degrees.to_radians(),
            },
        }
    }
}

#[cfg(test)]
mod deform_tests {
    use super::*;

    #[test]
    fn a_zero_axis_becomes_a_direction_rather_than_an_error() {
        let settings = DeformSettings {
            axis: [0.0; 3],
            ..Default::default()
        }
        .sanitized();
        let length =
            (settings.axis[0].powi(2) + settings.axis[1].powi(2) + settings.axis[2].powi(2)).sqrt();
        assert!((length - 1.0).abs() < 1e-5, "the axis is not a direction");
    }

    #[test]
    fn an_axis_is_normalised() {
        let settings = DeformSettings {
            axis: [0.0, 5.0, 0.0],
            ..Default::default()
        }
        .sanitized();
        assert_eq!(settings.axis, [0.0, 1.0, 0.0]);
    }

    /// The panel asks for degrees and the engine takes radians. Doing the
    /// conversion here is what keeps it out of three call sites.
    #[test]
    fn a_twist_reaches_the_engine_in_radians() {
        let settings = DeformSettings {
            verb: DeformVerb::Twist,
            degrees: 180.0,
            ..Default::default()
        };
        match settings.operation() {
            crate::LayerOperation::Twist { angle, .. } => {
                assert!(
                    (angle - std::f32::consts::PI).abs() < 1e-5,
                    "half a turn reached the engine as {angle} radians"
                );
            }
            other => panic!("a twist became {other:?}"),
        }
    }

    #[test]
    fn a_taper_carries_both_of_its_scales() {
        let settings = DeformSettings {
            verb: DeformVerb::Taper,
            scale_start: 1.5,
            scale_end: 0.25,
            ..Default::default()
        };
        match settings.operation() {
            crate::LayerOperation::Taper {
                scale_start,
                scale_end,
                ..
            } => {
                assert_eq!((scale_start, scale_end), (1.5, 0.25));
            }
            other => panic!("a taper became {other:?}"),
        }
    }

    /// A section scaled to nothing is a degenerate surface rather than a
    /// taper, so the control cannot reach it.
    #[test]
    fn a_scale_cannot_collapse_the_section() {
        let settings = DeformSettings {
            scale_end: 0.0,
            ..Default::default()
        }
        .sanitized();
        assert!(settings.scale_end >= *DeformSettings::SCALE_RANGE.start());
    }

    #[test]
    fn only_the_verb_that_uses_a_control_offers_it() {
        assert!(DeformVerb::Taper.takes_a_scale());
        assert!(!DeformVerb::Taper.takes_an_angle());
        assert!(DeformVerb::Twist.takes_an_angle());
        assert!(!DeformVerb::Twist.takes_a_scale());
    }
}
