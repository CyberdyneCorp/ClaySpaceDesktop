//! Stable words for the domain's enumerations, on the wire.
//!
//! Not `label()`, which is interface text: it is Portuguese, it is translated,
//! and an agent that learned "Arredondado" would be driving a different
//! application in Spanish. Not the position in an `ALL` array either — that is
//! presentation order, and reordering a shelf would silently reinterpret every
//! call an agent had memorised.
//!
//! The domain already draws this distinction for the things it stores —
//! `ToolKind::key`, `Shape::key`, `Combine::key`, `RefPlane::tag`,
//! `Locale::tag` — and those are used here rather than shadowed. The tables
//! below are for the enumerations that had no reason to have one until there
//! was a wire.

use clayspace_model::{
    BlendProfile, BooleanOp, Combine, CurveJoin, CurveProfile, DeformVerb, Direction, ExportMesher,
    ExtrudeSide, Falloff, GizmoMode, ImportAs, InsertAs, Locale, MaskGesture, RefPlane,
    Representation, Shape, ToolKind, ViewPresetKind, VoxelDisplay,
};
use clayspace_vm::Axis;

/// The tag of a value, found by walking the table it is in.
pub fn tag_of<T: PartialEq + Copy>(table: &[(&'static str, T)], value: T) -> &'static str {
    table
        .iter()
        .find(|(_, candidate)| *candidate == value)
        .map(|(tag, _)| *tag)
        .unwrap_or("?")
}

/// Every tag in a table, for a refusal that names the whole set and for the
/// schema an agent reads.
pub fn tags_of<T>(table: &[(&'static str, T)]) -> Vec<&'static str> {
    table.iter().map(|(tag, _)| *tag).collect()
}

pub fn tools() -> Vec<(&'static str, ToolKind)> {
    ToolKind::ALL
        .iter()
        .map(|kind| (kind.key(), *kind))
        .collect()
}

pub fn shapes() -> Vec<(&'static str, Shape)> {
    Shape::ALL
        .iter()
        .map(|shape| (shape.key(), *shape))
        .collect()
}

pub fn combines() -> Vec<(&'static str, Combine)> {
    Combine::ALL.iter().map(|op| (op.key(), *op)).collect()
}

pub fn blends() -> Vec<(&'static str, BlendProfile)> {
    BlendProfile::ALL
        .iter()
        .map(|blend| (blend.key(), *blend))
        .collect()
}

pub fn booleans() -> Vec<(&'static str, BooleanOp)> {
    BooleanOp::ALL.iter().map(|op| (op.key(), *op)).collect()
}

pub fn planes() -> Vec<(&'static str, RefPlane)> {
    RefPlane::ALL
        .iter()
        .map(|plane| (plane.tag(), *plane))
        .collect()
}

pub fn locales() -> Vec<(&'static str, Locale)> {
    Locale::ALL
        .iter()
        .map(|locale| (locale.tag(), *locale))
        .collect()
}

pub const REPRESENTATIONS: &[(&str, Representation)] = &[
    ("field", Representation::Sdf),
    ("grid", Representation::Voxel),
    ("mesh", Representation::Mesh),
    ("hierarchy", Representation::Multires),
];

pub const FALLOFFS: &[(&str, Falloff)] = &[
    ("constant", Falloff::Constant),
    ("linear", Falloff::Linear),
    ("smooth", Falloff::Smooth),
    ("gaussian", Falloff::Gaussian),
];

pub const GIZMO_MODES: &[(&str, GizmoMode)] = &[
    ("move", GizmoMode::Move),
    ("rotate", GizmoMode::Rotate),
    ("scale", GizmoMode::Scale),
];

pub const AXES: &[(&str, Axis)] = &[("x", Axis::X), ("y", Axis::Y), ("z", Axis::Z)];

pub const GESTURES: &[(&str, MaskGesture)] = &[
    ("brush", MaskGesture::Brush),
    ("lasso", MaskGesture::Lasso),
    ("rectangle", MaskGesture::Rectangle),
];

pub const JOINS: &[(&str, CurveJoin)] = &[
    ("corners", CurveJoin::Corners),
    ("through", CurveJoin::Through),
    ("rounded", CurveJoin::Rounded),
];

pub const PROFILES: &[(&str, CurveProfile)] = &[
    ("circle", CurveProfile::Circle),
    ("square", CurveProfile::Square),
    ("hexagon", CurveProfile::Hexagon),
    ("triangle", CurveProfile::Triangle),
];

pub const INSERT_AS: &[(&str, InsertAs)] =
    &[("subtool", InsertAs::Subtool), ("object", InsertAs::Object)];

pub const VIEW_PRESETS: &[(&str, ViewPresetKind)] = &[
    ("perspective", ViewPresetKind::Perspective),
    ("front", ViewPresetKind::Front),
    ("side", ViewPresetKind::Side),
    ("top", ViewPresetKind::Top),
];

pub const VOXEL_DISPLAYS: &[(&str, VoxelDisplay)] = &[
    ("boxes", VoxelDisplay::Boxes),
    ("smooth", VoxelDisplay::Smooth),
];

pub const DIRECTIONS: &[(&str, Direction)] = &[
    ("field-to-grid", Direction::SdfToVoxel),
    ("grid-to-field", Direction::VoxelToSdf),
    ("mesh-to-grid", Direction::MeshToVoxel),
    ("mesh-to-field", Direction::MeshToSdf),
    ("field-to-mesh", Direction::SdfToMesh),
    ("grid-to-mesh", Direction::VoxelToMesh),
    ("mesh-to-hierarchy", Direction::MeshToMultires),
    ("hierarchy-to-mesh", Direction::MultiresToMesh),
];

pub const IMPORT_AS: &[(&str, ImportAs)] =
    &[("reference", ImportAs::Reference), ("clay", ImportAs::Clay)];

pub const MESHERS: &[(&str, ExportMesher)] = &[
    ("watertight", ExportMesher::Watertight),
    ("fast", ExportMesher::Fast),
    ("sharp", ExportMesher::Sharp),
];

pub const EXTRUDE_SIDES: &[(&str, ExtrudeSide)] = &[
    ("outward", ExtrudeSide::Outward),
    ("inward", ExtrudeSide::Inward),
    ("centred", ExtrudeSide::Centred),
];

pub const DEFORM_VERBS: &[(&str, DeformVerb)] =
    &[("taper", DeformVerb::Taper), ("twist", DeformVerb::Twist)];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A table with a repeated tag would make one of its values unreachable
    /// from the wire, silently.
    fn distinct<T>(table: &[(&'static str, T)]) -> bool {
        let tags: HashSet<&str> = table.iter().map(|(tag, _)| *tag).collect();
        tags.len() == table.len()
    }

    #[test]
    fn every_table_has_distinct_tags() {
        assert!(distinct(REPRESENTATIONS));
        assert!(distinct(FALLOFFS));
        assert!(distinct(GIZMO_MODES));
        assert!(distinct(AXES));
        assert!(distinct(GESTURES));
        assert!(distinct(JOINS));
        assert!(distinct(PROFILES));
        assert!(distinct(INSERT_AS));
        assert!(distinct(VIEW_PRESETS));
        assert!(distinct(VOXEL_DISPLAYS));
        assert!(distinct(DIRECTIONS));
        assert!(distinct(IMPORT_AS));
        assert!(distinct(MESHERS));
        assert!(distinct(EXTRUDE_SIDES));
        assert!(distinct(DEFORM_VERBS));
        assert!(distinct(&tools()));
        assert!(distinct(&shapes()));
        assert!(distinct(&combines()));
        assert!(distinct(&blends()));
        assert!(distinct(&booleans()));
        assert!(distinct(&planes()));
        assert!(distinct(&locales()));
    }

    /// The table is the whole enumeration, or something the interface offers
    /// is unreachable to an agent.
    #[test]
    fn every_table_covers_its_enumeration() {
        assert_eq!(REPRESENTATIONS.len(), Representation::ALL.len());
        assert_eq!(FALLOFFS.len(), Falloff::ALL.len());
        assert_eq!(GIZMO_MODES.len(), GizmoMode::ALL.len());
        assert_eq!(AXES.len(), Axis::ALL.len());
        assert_eq!(GESTURES.len(), MaskGesture::ALL.len());
        assert_eq!(JOINS.len(), CurveJoin::ALL.len());
        assert_eq!(PROFILES.len(), CurveProfile::ALL.len());
        assert_eq!(INSERT_AS.len(), InsertAs::ALL.len());
        assert_eq!(VIEW_PRESETS.len(), ViewPresetKind::ALL.len());
        assert_eq!(VOXEL_DISPLAYS.len(), VoxelDisplay::ALL.len());
        assert_eq!(DIRECTIONS.len(), Direction::ALL.len());
        assert_eq!(IMPORT_AS.len(), ImportAs::ALL.len());
        assert_eq!(MESHERS.len(), ExportMesher::ALL.len());
        assert_eq!(EXTRUDE_SIDES.len(), ExtrudeSide::ALL.len());
        assert_eq!(DEFORM_VERBS.len(), DeformVerb::ALL.len());
        assert_eq!(tools().len(), ToolKind::ALL.len());
        assert_eq!(shapes().len(), Shape::ALL.len());
        assert_eq!(combines().len(), Combine::ALL.len());
    }

    #[test]
    fn a_tag_finds_its_value_and_back() {
        assert_eq!(
            tag_of(REPRESENTATIONS, Representation::Multires),
            "hierarchy"
        );
        assert_eq!(tag_of(FALLOFFS, Falloff::Gaussian), "gaussian");
        assert_eq!(tags_of(AXES), vec!["x", "y", "z"]);
    }

    /// The interface's own words are Portuguese and are not what an agent
    /// learns. This is the test that stops someone reaching for `label()`.
    #[test]
    fn no_tag_is_interface_text() {
        for (tag, _) in REPRESENTATIONS
            .iter()
            .chain(FALLOFFS.iter().map(|_| &REPRESENTATIONS[0]))
        {
            assert!(tag.is_ascii(), "{tag}");
        }
        for (tag, kind) in tools() {
            assert!(tag.is_ascii(), "{tag}");
            assert_ne!(tag, kind.label());
        }
    }
}
