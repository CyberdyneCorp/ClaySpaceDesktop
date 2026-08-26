//! The shapes a sculptor can place, and what each one is measured by.
//!
//! Named here rather than borrowed from the engine because the domain may not
//! depend on it — the same reason [`crate::Combine`] is named here — and the
//! adapter is where these become calls.
//!
//! The engine carries thirty primitives. Fourteen are offered, which is not a
//! judgement about the other sixteen: two of them (a plane, an infinite
//! cylinder) the engine itself calls unbounded, so they have no extent for a
//! manipulator to sit on and no influence bound for the cache to work from,
//! and the rest are either shapes a sculptor reaches for once a year or the
//! out-of-line ones — a stroke, a loft, a swept profile — that are not placed
//! objects at all. They stay reachable through the bridge for anything that
//! means one specifically.
//!
//! A shape's *parameters* are described rather than typed. A panel that has to
//! know a torus takes a major radius and then a minor one is a panel with
//! fourteen special cases in it; one that reads
//! [`Shape::parameters`] is a panel with none, and the adapter's exhaustive
//! match is what keeps the description and the engine call in step.

/// A shape that can be placed in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Shape {
    #[default]
    Box,
    Sphere,
    Cylinder,
    Cone,
    Torus,
    Capsule,
    Ellipsoid,
    Pyramid,
    /// A box with its edges rounded.
    RoundBox,
    /// A frame rather than a solid: the twelve bars of a box.
    BoxFrame,
    /// A cylinder with a rounded rim.
    RoundedCylinder,
    HexPrism,
    TriPrism,
    Octahedron,
}

/// What one of a shape's numbers means, and what it may be.
///
/// The range is not decoration: a cylinder of radius zero is not a thin
/// cylinder, it is nothing, and a panel that can reach it offers a control
/// that appears broken with nothing to say why — the same rule the combine
/// operations' distance already follows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeParameter {
    /// A stable key, for the interface to look a name up by.
    pub key: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}

impl ShapeParameter {
    const fn new(key: &'static str, default: f32, min: f32, max: f32) -> Self {
        Self {
            key,
            default,
            min,
            max,
        }
    }

    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }
}

/// A size that reads as nothing rather than as small.
const SMALLEST: f32 = 0.005;
/// Room to place something large beside the reference form without inviting a
/// shape whose influence bound covers the document.
const LARGEST: f32 = 10.0;

// Named rather than written inline, because `&[ShapeParameter::new(..)]` is a
// reference to a temporary: a const fn call is not promoted to `'static`, and
// `parameters` hands back a slice that outlives the call.
const HALF_X: ShapeParameter = ShapeParameter::new("half_x", 0.3, SMALLEST, LARGEST);
const HALF_Y: ShapeParameter = ShapeParameter::new("half_y", 0.3, SMALLEST, LARGEST);
const HALF_Z: ShapeParameter = ShapeParameter::new("half_z", 0.3, SMALLEST, LARGEST);
const RADIUS: ShapeParameter = ShapeParameter::new("radius", 0.3, SMALLEST, LARGEST);
const HALF_HEIGHT: ShapeParameter = ShapeParameter::new("half_height", 0.4, SMALLEST, LARGEST);
const HALF_DEPTH: ShapeParameter = ShapeParameter::new("half_depth", 0.3, SMALLEST, LARGEST);
const BOTTOM_RADIUS: ShapeParameter = ShapeParameter::new("bottom_radius", 0.35, SMALLEST, LARGEST);
/// A cone tapers to a point at zero, which is a cone and not a nothing, so
/// this is the one size that may reach it.
const TOP_RADIUS: ShapeParameter = ShapeParameter::new("top_radius", 0.0, 0.0, LARGEST);
const MAJOR_RADIUS: ShapeParameter = ShapeParameter::new("major_radius", 0.4, SMALLEST, LARGEST);
const MINOR_RADIUS: ShapeParameter = ShapeParameter::new("minor_radius", 0.12, SMALLEST, LARGEST);
const RADIUS_X: ShapeParameter = ShapeParameter::new("radius_x", 0.4, SMALLEST, LARGEST);
const RADIUS_Y: ShapeParameter = ShapeParameter::new("radius_y", 0.3, SMALLEST, LARGEST);
const RADIUS_Z: ShapeParameter = ShapeParameter::new("radius_z", 0.2, SMALLEST, LARGEST);
const HEIGHT: ShapeParameter = ShapeParameter::new("height", 0.6, SMALLEST, LARGEST);
const CORNER_RADIUS: ShapeParameter = ShapeParameter::new("corner_radius", 0.08, SMALLEST, LARGEST);
const THICKNESS: ShapeParameter = ShapeParameter::new("thickness", 0.04, SMALLEST, LARGEST);
const RIM_RADIUS: ShapeParameter = ShapeParameter::new("rim_radius", 0.06, SMALLEST, LARGEST);
const SIZE: ShapeParameter = ShapeParameter::new("size", 0.4, SMALLEST, LARGEST);

impl Shape {
    /// Every shape, in the order the picker presents them: the four a sculptor
    /// reaches for first, then the rest.
    pub const ALL: [Shape; 14] = [
        Self::Box,
        Self::Sphere,
        Self::Cylinder,
        Self::Cone,
        Self::Torus,
        Self::Capsule,
        Self::Ellipsoid,
        Self::Pyramid,
        Self::RoundBox,
        Self::BoxFrame,
        Self::RoundedCylinder,
        Self::HexPrism,
        Self::TriPrism,
        Self::Octahedron,
    ];

    /// The fallback name, in the interface's own language.
    ///
    /// The localised one comes from the view's table, indexed by position in
    /// [`Shape::ALL`], as a tool's does.
    pub fn label(self) -> &'static str {
        match self {
            Self::Box => "Caixa",
            Self::Sphere => "Esfera",
            Self::Cylinder => "Cilindro",
            Self::Cone => "Cone",
            Self::Torus => "Toro",
            Self::Capsule => "Cápsula",
            Self::Ellipsoid => "Elipsoide",
            Self::Pyramid => "Pirâmide",
            Self::RoundBox => "Caixa arredondada",
            Self::BoxFrame => "Moldura",
            Self::RoundedCylinder => "Cilindro arredondado",
            Self::HexPrism => "Prisma hexagonal",
            Self::TriPrism => "Prisma triangular",
            Self::Octahedron => "Octaedro",
        }
    }

    /// A stable name, for anything that has to write a shape down.
    ///
    /// Not [`Shape::label`], which is interface text: it is Portuguese, it is
    /// translated, and a saved file that used it would read differently in a
    /// different language. Not the position in [`Shape::ALL`] either — that is
    /// presentation order, and reordering the picker would silently reinterpret
    /// every document already written.
    pub fn key(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Sphere => "sphere",
            Self::Cylinder => "cylinder",
            Self::Cone => "cone",
            Self::Torus => "torus",
            Self::Capsule => "capsule",
            Self::Ellipsoid => "ellipsoid",
            Self::Pyramid => "pyramid",
            Self::RoundBox => "round-box",
            Self::BoxFrame => "box-frame",
            Self::RoundedCylinder => "rounded-cylinder",
            Self::HexPrism => "hex-prism",
            Self::TriPrism => "tri-prism",
            Self::Octahedron => "octahedron",
        }
    }

    /// The shape a key names, if it names one.
    ///
    /// `None` for a key this build does not know, which is what a document
    /// written by a later version looks like. The caller drops the object
    /// rather than guessing at it.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|shape| shape.key() == key)
    }

    /// What this shape is measured by, in the order the adapter reads them.
    pub fn parameters(self) -> &'static [ShapeParameter] {
        match self {
            Self::Box => &[HALF_X, HALF_Y, HALF_Z],
            Self::Sphere => &[RADIUS],
            Self::Cylinder => &[RADIUS, HALF_HEIGHT],
            Self::Cone => &[HALF_HEIGHT, BOTTOM_RADIUS, TOP_RADIUS],
            Self::Torus => &[MAJOR_RADIUS, MINOR_RADIUS],
            Self::Capsule => &[RADIUS, HALF_HEIGHT],
            Self::Ellipsoid => &[RADIUS_X, RADIUS_Y, RADIUS_Z],
            Self::Pyramid => &[HEIGHT],
            Self::RoundBox => &[HALF_X, HALF_Y, HALF_Z, CORNER_RADIUS],
            Self::BoxFrame => &[HALF_X, HALF_Y, HALF_Z, THICKNESS],
            Self::RoundedCylinder => &[RADIUS, RIM_RADIUS, HALF_HEIGHT],
            Self::HexPrism | Self::TriPrism => &[RADIUS, HALF_DEPTH],
            Self::Octahedron => &[SIZE],
        }
    }

    /// The numbers a freshly placed one carries.
    pub fn defaults(self) -> Vec<f32> {
        self.parameters().iter().map(|p| p.default).collect()
    }

    /// The given numbers, brought inside what each parameter allows and made
    /// the right length.
    ///
    /// A short list is filled from the defaults and a long one is cut, because
    /// a document written by another version of this application is a thing
    /// that happens and losing the object over it would be worse than losing
    /// one of its numbers.
    pub fn sanitised(self, values: &[f32]) -> Vec<f32> {
        self.parameters()
            .iter()
            .enumerate()
            .map(|(at, parameter)| parameter.clamp(*values.get(at).unwrap_or(&parameter.default)))
            .collect()
    }
}

/// What kind of item a layer holds, from the point of view of a sculptor
/// looking for something to grab.
///
/// Derived from the primitive rather than recorded beside it. The engine
/// already writes the primitive into the document and reads it back, so an
/// object survives a save and a reopen with nothing extra written; a side-car
/// list of "which nodes are mine" would be a second source of truth that any
/// document touched elsewhere immediately falsifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// A shape a sculptor placed, and can select, move and re-combine.
    Object,
    /// A sculpting stroke. Over when the pointer came up — see the manipulator
    /// requirement for why it is not a target.
    Stroke,
    /// The tube a curve was applied as.
    Curve,
    /// A rig's skin.
    Armature,
}

impl ItemKind {
    /// Whether the manipulator and the object list address this.
    pub fn is_object(self) -> bool {
        self == Self::Object
    }
}

/// Where an object is, as the document addresses it.
///
/// The node id is the engine's, carried opaquely: the domain may not depend on
/// the engine, and it does not need to — it never does anything with this but
/// hand it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId {
    pub layer: crate::LayerKey,
    /// The engine's node id for the item.
    pub node: u32,
}

/// A placed object, as the interface needs to present one.
///
/// Read from the document rather than cached: the engine holds the truth about
/// where an object is and how it combines, and a copy of it here is the shape
/// that produces two answers to "where is the cylinder" the first time an undo
/// runs. What is held is what a list row needs to draw itself.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneObject {
    pub id: ObjectId,
    pub shape: Shape,
    pub parameters: Vec<f32>,
    /// How it meets what is under it.
    pub combine: crate::CombineSettings,
    pub position: [f32; 3],
    /// Axis and angle, as every rotation in the engine's interface is given.
    pub rotation_axis: [f32; 3],
    pub rotation_angle: f32,
    /// Uniform. The engine's transforms take one factor and not three; see
    /// [`crate::GizmoMode::Scale`] and what the manipulator offers for one.
    pub scale: f32,
}

impl SceneObject {
    /// What the interface calls it: the shape's name, which is all a placed
    /// primitive has until somebody gives it another.
    pub fn label(&self) -> &'static str {
        self.shape.label()
    }
}

/// Where an object can live.
///
/// One place rather than a sentence repeated at each refusal: an object is an
/// item in an SDF layer's ordered list, and a grid and a mesh have no such
/// list to put one in. A voxel layer could take a rasterized copy — the engine
/// has the call — but a rasterized copy is not live, and everything an object
/// is for here depends on its being live.
pub const OBJECT_VERBS: crate::Verbs = crate::Verbs {
    sdf: Some("clay_layer_add_item"),
    voxel: None,
    mesh: None,
};

/// What a manipulator is acting on.
///
/// The four things that carry a transform, as against the cage's control
/// points, which the manipulator moves one at a time and the application owns
/// outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoTarget {
    /// A placed shape, addressed by its node.
    Object(ObjectId),
    /// A whole layer, everything it holds moving together.
    Layer(crate::LayerKey),
    /// A curve's selected control points, moved as a group.
    ///
    /// The one target that is not an engine transform: a curve's points belong
    /// to the application while it is being authored, so this resolves through
    /// the point-set path the cage already uses.
    Curve,
}

/// Placing, selecting and re-placing the objects a layer holds.
///
/// A second trait beside [`crate::SculptModel`] rather than more methods on
/// it, for the reason the model already separates `apply_stroke` from
/// `apply_operation`: these are not edits a gesture expresses, and a double
/// that models no objects should be able to say so by not implementing this
/// rather than by spelling out ten refusals.
///
/// Every method is provided, and every provided one refuses. So a partial
/// implementation is possible and an accidental one is not: a type that
/// forgets `place_object` refuses to place rather than silently doing nothing.
pub trait ObjectModel {
    /// The placed objects in the active layer, in the order it holds them.
    ///
    /// Read from the document each time. `&mut self` because the engine's own
    /// readback may compile the document, which is not a `&self` operation
    /// however much it reads like one.
    fn objects(&mut self) -> Vec<SceneObject> {
        Vec::new()
    }

    /// Which object the manipulator and the options bar are addressing.
    fn selected_object(&self) -> Option<ObjectId> {
        None
    }

    /// Selects one, or clears the selection.
    fn select_object(&mut self, id: Option<ObjectId>) {
        let _ = id;
    }

    /// Places a shape in the active layer and selects it.
    fn place_object(
        &mut self,
        shape: Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: crate::CombineSettings,
    ) -> Result<ObjectId, crate::ModelError> {
        let _ = (shape, parameters, at, combine);
        Err(self.no_objects_here())
    }

    /// Re-places one. `scale` is uniform: the engine's transforms take one
    /// factor, not three.
    fn set_object_transform(
        &mut self,
        id: ObjectId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: f32,
    ) -> Result<(), crate::ModelError> {
        let _ = (id, position, rotation_axis, rotation_angle, scale);
        Err(self.no_objects_here())
    }

    /// Exchanges what shape it is, keeping everything that belongs to the node
    /// rather than to the primitive — its transform, its operation, its place
    /// in the order.
    fn set_object_shape(
        &mut self,
        id: ObjectId,
        shape: Shape,
        parameters: &[f32],
    ) -> Result<(), crate::ModelError> {
        let _ = (id, shape, parameters);
        Err(self.no_objects_here())
    }

    /// Changes how it meets what is under it.
    fn set_object_combine(
        &mut self,
        id: ObjectId,
        combine: crate::CombineSettings,
    ) -> Result<(), crate::ModelError> {
        let _ = (id, combine);
        Err(self.no_objects_here())
    }

    fn remove_object(&mut self, id: ObjectId) -> Result<(), crate::ModelError> {
        let _ = id;
        Err(self.no_objects_here())
    }

    /// Where the thing the manipulator is on currently stands.
    ///
    /// `None` when there is nothing to transform, which is also the answer for
    /// a target that has gone — "a selection outlives the nodes in it".
    fn target_transform(&mut self, target: GizmoTarget) -> Option<crate::Transform> {
        let _ = target;
        None
    }

    /// Opens a gesture on a target, so everything until it closes is one
    /// undo step.
    ///
    /// A drag sets a transform every frame, and thirty frames of dragging a
    /// cylinder across a form is one thing a sculptor did. Without this it is
    /// thirty entries in the history, and taking it back means thirty undos.
    ///
    /// Idempotent and forgiving: a gesture that is never closed is closed by
    /// the next one that opens, because a group left open swallows every edit
    /// after it.
    fn begin_target_drag(&mut self, target: GizmoTarget) {
        let _ = target;
    }

    /// Closes it.
    fn end_target_drag(&mut self) {}

    /// Puts it there.
    ///
    /// One call for all of them, because a drag means the same thing whatever
    /// it is on: this is where a position, an axis-angle and a scale become
    /// the engine call that applies to this kind of target.
    fn set_target_transform(
        &mut self,
        target: GizmoTarget,
        transform: crate::Transform,
    ) -> Result<(), crate::ModelError> {
        let _ = (target, transform);
        Err(self.no_objects_here())
    }

    /// Which object a ray meets, where it meets one.
    ///
    /// The engine attributes a hit to "the item whose field is closest at the
    /// hit point, so a subtract item is attributed the surface it carved" —
    /// clicking the wall of a hole selects the shape that cut it, which is
    /// what a sculptor means by clicking there.
    fn pick_object(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<ObjectId> {
        let _ = (origin, direction);
        None
    }

    /// What a ray meets, whether or not it can be transformed.
    ///
    /// The difference from [`ObjectModel::pick_object`] is the whole of task
    /// 4.7: a click on a sculpting stroke has to *say* that a stroke cannot be
    /// transformed, and a `None` from `pick_object` cannot tell that apart
    /// from a click on nothing at all.
    fn pick_item(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<ItemKind> {
        let _ = (origin, direction);
        None
    }

    /// The refusal every provided method gives, in one place.
    fn no_objects_here(&self) -> crate::ModelError {
        crate::ModelError::Unavailable(crate::Unavailable::NoVerbHere {
            active: crate::Representation::Sdf,
            verbs: OBJECT_VERBS,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A type that models no objects refuses every one of them rather than
    /// silently doing nothing, which is what makes the provided bodies safe to
    /// have at all.
    #[test]
    fn a_model_with_no_objects_refuses_them() {
        struct Nothing;
        impl ObjectModel for Nothing {}

        let mut nothing = Nothing;
        assert!(nothing
            .place_object(
                Shape::Box,
                &Shape::Box.defaults(),
                [0.0; 3],
                crate::CombineSettings::default()
            )
            .is_err());
        assert!(nothing.objects().is_empty());
        assert!(nothing.selected_object().is_none());
        assert!(nothing.pick_object([0.0; 3], [0.0, 0.0, 1.0]).is_none());
    }

    #[test]
    fn an_object_needs_a_field_to_live_in() {
        assert!(OBJECT_VERBS.on(crate::Representation::Sdf).is_some());
        assert!(OBJECT_VERBS.on(crate::Representation::Voxel).is_none());
        assert!(OBJECT_VERBS.on(crate::Representation::Mesh).is_none());
    }

    #[test]
    fn every_key_is_distinct_and_round_trips() {
        let mut seen = std::collections::BTreeSet::new();
        for shape in Shape::ALL {
            assert!(seen.insert(shape.key()), "{shape:?} shares a key");
            assert_eq!(Shape::from_key(shape.key()), Some(shape));
        }
    }

    #[test]
    fn an_unknown_key_is_none_rather_than_a_guess() {
        assert_eq!(Shape::from_key("dodecahedron"), None);
        assert_eq!(Shape::from_key(""), None);
    }

    /// A key is written into saved documents, so changing one silently
    /// reinterprets every file already on disk. This is the list, and moving a
    /// name here means a migration.
    #[test]
    fn the_keys_are_what_they_were() {
        assert_eq!(Shape::Box.key(), "box");
        assert_eq!(Shape::RoundedCylinder.key(), "rounded-cylinder");
        assert_eq!(Shape::Octahedron.key(), "octahedron");
    }

    #[test]
    fn every_shape_is_offered_once() {
        let mut seen = std::collections::BTreeSet::new();
        for shape in Shape::ALL {
            assert!(seen.insert(shape.label()), "{shape:?} shares a label");
        }
        assert_eq!(seen.len(), Shape::ALL.len());
    }

    /// The two the engine names unbounded are not here, and neither is
    /// anything else without a size.
    #[test]
    fn every_offered_shape_has_something_to_measure_it_by() {
        for shape in Shape::ALL {
            assert!(
                !shape.parameters().is_empty(),
                "{shape:?} has no parameters, so it has no extent either"
            );
        }
    }

    #[test]
    fn defaults_are_inside_their_own_ranges() {
        for shape in Shape::ALL {
            for parameter in shape.parameters() {
                assert!(
                    parameter.default >= parameter.min && parameter.default <= parameter.max,
                    "{shape:?}'s {} defaults outside its range",
                    parameter.key
                );
            }
        }
    }

    /// A size of zero is not a small shape, it is no shape, and only a cone's
    /// tip is allowed to reach it.
    #[test]
    fn a_size_cannot_be_dragged_to_nothing() {
        for shape in Shape::ALL {
            for parameter in shape.parameters() {
                if shape == Shape::Cone && parameter.key == "top_radius" {
                    continue;
                }
                assert!(
                    parameter.min > 0.0,
                    "{shape:?}'s {} can be taken to zero",
                    parameter.key
                );
            }
        }
    }

    #[test]
    fn a_short_parameter_list_is_filled_rather_than_refused() {
        let filled = Shape::Box.sanitised(&[1.0]);
        assert_eq!(filled.len(), 3);
        assert_eq!(filled[0], 1.0);
        assert_eq!(filled[1], HALF_Y.default);
    }

    #[test]
    fn a_size_out_of_range_is_brought_back_in() {
        let clamped = Shape::Sphere.sanitised(&[-4.0]);
        assert_eq!(clamped, vec![RADIUS.min]);
    }
}
