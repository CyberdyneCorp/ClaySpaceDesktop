//! Placed objects: the shapes a sculptor puts in a layer and can come back to.
//!
//! Two halves. The first is a mapping — the domain's [`Shape`] and its
//! described parameters onto the engine's typed [`claycore::Primitive`] — and
//! it is exhaustive on purpose, for the reason `engine_op` is: an unlisted arm
//! falling through to something plausible is how a planing tool came to
//! deposit spheres with nothing saying so.
//!
//! The second is a record the engine will not keep. The ABI sets a node's
//! transform, its parameters and its operation and reads none of them back;
//! `clay_layer_node_prim` answers which primitive a node is and that is all.
//! So an object's state is held here and the engine is written from it — the
//! same arrangement the armature already has, and for the same reason: "the
//! engine's parent array has no getter — positions and radii read back, the
//! topology does not."
//!
//! What that costs is a second source of truth, and it is bounded as tightly
//! as it can be: the table holds only what cannot be read back, and it follows
//! the engine's history by depth rather than drifting away from it.
//!
//! The engine's influence bound was tried as a hedge — draw the manipulator on
//! the box the engine reports, so it is right even when the table is not — and
//! it does not work here. The application mirrors its layers in X by default,
//! so a node's bound covers the object *and its reflection* and centres on the
//! mirror plane: an object placed at 0.9 reported its position as the origin.
//! The bound answers what to dirty. The table answers where things are.

use claycore::{NodeId, Primitive};
use clayspace_model::{CombineSettings, ItemKind, ObjectId, SceneObject, Shape};

/// A placed object's state, as only the application knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    pub layer: clayspace_model::LayerKey,
    pub node: NodeId,
    pub shape: Shape,
    pub parameters: Vec<f32>,
    pub combine: CombineSettings,
    pub position: [f32; 3],
    pub rotation_axis: [f32; 3],
    pub rotation_angle: f32,
    /// Uniform: the engine's transforms take one factor, not three.
    pub scale: f32,
}

impl PlacedObject {
    /// What a freshly placed object carries.
    pub fn new(
        layer: clayspace_model::LayerKey,
        node: NodeId,
        shape: Shape,
        parameters: Vec<f32>,
        combine: CombineSettings,
        position: [f32; 3],
    ) -> Self {
        Self {
            layer,
            node,
            shape,
            parameters,
            combine,
            position,
            // The engine requires a non-zero axis even for no rotation: "a
            // second convention for 'no rotation' would be one more thing to
            // get wrong."
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.0,
            scale: 1.0,
        }
    }

    pub fn id(&self) -> ObjectId {
        ObjectId {
            layer: self.layer,
            node: self.node.get(),
        }
    }

    /// What the interface presents.
    ///
    /// The position comes from the table and not from the node's influence
    /// bound, which was tried as a hedge against a stale table and is wrong
    /// under the application's own defaults: the layer is mirrored in X, so a
    /// node's bound covers the object *and its reflection* and centres on the
    /// mirror plane. An object placed at 0.9 reported its position as the
    /// origin.
    ///
    /// So the bound answers what to dirty, and the table answers where things
    /// are. Which leaves the table's staleness to be handled where it is
    /// actually created — by history — rather than papered over here.
    pub fn presented(&self) -> SceneObject {
        SceneObject {
            id: self.id(),
            shape: self.shape,
            parameters: self.parameters.clone(),
            combine: self.combine,
            position: self.position,
            rotation_axis: self.rotation_axis,
            rotation_angle: self.rotation_angle,
            scale: self.scale,
        }
    }
}

/// What kind of item a primitive makes.
///
/// **Not the objecthood rule**, though it was designed as one. The intent was
/// that an item is an object unless its primitive is one of the three the
/// application makes for other reasons, which would have let an object survive
/// a reopen with nothing extra written to the document.
///
/// It does not hold: an SDF stamping stroke places `Item::sphere` per stamp,
/// so a worked layer is full of sphere items that nobody placed and the rule
/// would list a row for each. The spec asks for the opposite in as many words
/// — "the two objects are listed and reachable, and the strokes do not each
/// take a row".
///
/// So objecthood is *recorded* instead, in the table the readback gap already
/// forces the application to keep and save. This survives as the answer to a
/// narrower question: what an item is when the table has no opinion, which is
/// what a scene panel needs to tell a rig from a curve.
pub fn kind_of(prim: i32) -> ItemKind {
    match prim {
        p if p == claycore::prim::STROKE => ItemKind::Stroke,
        p if p == claycore::prim::SWEPT => ItemKind::Curve,
        p if p == claycore::prim::ARMATURE => ItemKind::Armature,
        _ => ItemKind::Object,
    }
}

/// The engine's typed shape for one the interface offers.
///
/// The domain describes a shape's parameters rather than typing them, so that
/// a panel has no special case per shape; this is where the description
/// becomes the engine's own ordering, once, with the count guaranteed by
/// `Shape::sanitised`.
pub fn primitive_of(shape: Shape, parameters: &[f32]) -> Primitive {
    let p = shape.sanitised(parameters);
    match shape {
        Shape::Box => Primitive::Box {
            half: [p[0], p[1], p[2]],
        },
        Shape::Sphere => Primitive::Sphere { radius: p[0] },
        Shape::Cylinder => Primitive::Cylinder {
            radius: p[0],
            half_height: p[1],
        },
        Shape::Cone => Primitive::Cone {
            half_height: p[0],
            bottom: p[1],
            top: p[2],
        },
        Shape::Torus => Primitive::Torus {
            major: p[0],
            minor: p[1],
        },
        // The interface offers a radius and a half-height, because a capsule
        // standing up is what a sculptor wants nine times in ten; the engine
        // takes the two points that radius is swept between.
        Shape::Capsule => Primitive::Capsule {
            from: [0.0, -p[1], 0.0],
            to: [0.0, p[1], 0.0],
            radius: p[0],
        },
        Shape::Ellipsoid => Primitive::Ellipsoid {
            radii: [p[0], p[1], p[2]],
        },
        Shape::Pyramid => Primitive::Pyramid { height: p[0] },
        Shape::RoundBox => Primitive::RoundBox {
            half: [p[0], p[1], p[2]],
            radius: p[3],
        },
        Shape::BoxFrame => Primitive::BoxFrame {
            half: [p[0], p[1], p[2]],
            thickness: p[3],
        },
        Shape::RoundedCylinder => Primitive::RoundedCylinder {
            radius: p[0],
            rim: p[1],
            half_height: p[2],
        },
        Shape::HexPrism => Primitive::HexPrism {
            radius: p[0],
            half_depth: p[1],
        },
        Shape::TriPrism => Primitive::TriPrism {
            radius: p[0],
            half_depth: p[1],
        },
        Shape::Octahedron => Primitive::Octahedron { size: p[0] },
    }
}

/// The box two states of one node between them reach.
///
/// A move has to dirty where the object went *and* where it came from:
/// refilling only the destination leaves the surface the object used to cut
/// still cut. `None` from either side means no finite box exists and the whole
/// layer is the honest answer — which an ordinary shape placed with
/// `Intersect` reaches, since a non-local op anywhere in the subtree removes
/// the bound.
pub fn union(
    before: Option<([f32; 3], [f32; 3])>,
    after: Option<([f32; 3], [f32; 3])>,
) -> Option<([f32; 3], [f32; 3])> {
    match (before, after) {
        (Some((amin, amax)), Some((bmin, bmax))) => Some((
            std::array::from_fn(|i| amin[i].min(bmin[i])),
            std::array::from_fn(|i| amax[i].max(bmax[i])),
        )),
        _ => None,
    }
}

// -- the side-car -----------------------------------------------------------

/// Where an object table lives for a document at `path`.
///
/// Beside it rather than inside it: the `.clay` format is the engine's and
/// this is the application's own bookkeeping, so writing into the engine's
/// file would mean owning a format we do not own. A document opened without
/// its side-car still opens and still sculpts; its placed shapes are simply
/// not offered as objects.
pub fn sidecar_for(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".objects");
    path.with_file_name(name)
}

/// The first line of the file, so a later format can be told from this one.
const HEADER: &str = "clayspace-objects 1";

/// Writes the table, one object per line.
///
/// Line-based and by hand rather than a serialiser, for the reason the
/// benchmark's baseline is: one shape, one writer, and a dependency in the
/// graph is a thing the licence audit carries forever. Values are separated by
/// spaces and nothing here can contain one — a shape key, an operation key and
/// numbers.
pub fn write_table(path: &std::path::Path, objects: &[PlacedObject]) -> std::io::Result<()> {
    let mut out = String::from(HEADER);
    out.push('\n');
    for object in objects {
        out.push_str(&format!(
            "{} {} {} {} {} {}",
            object.layer.0,
            object.node.get(),
            object.shape.key(),
            object.combine.op.key(),
            object.combine.blend.key(),
            object.combine.radius,
        ));
        for value in [
            object.position[0],
            object.position[1],
            object.position[2],
            object.rotation_axis[0],
            object.rotation_axis[1],
            object.rotation_axis[2],
            object.rotation_angle,
            object.scale,
        ] {
            out.push_str(&format!(" {value}"));
        }
        out.push_str(&format!(" {}", object.parameters.len()));
        for value in &object.parameters {
            out.push_str(&format!(" {value}"));
        }
        out.push('\n');
    }
    std::fs::write(path, out)
}

/// Reads a table back, dropping anything this build cannot make sense of.
///
/// Every failure is a dropped row rather than a refused file. A side-car is
/// bookkeeping beside the document, and a document that opens without its
/// objects is far better than one that will not open because a line is
/// malformed — the sculpture is in the `.clay`.
pub fn read_table(path: &std::path::Path) -> Vec<PlacedObject> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Vec::new();
    }
    lines.filter_map(read_row).collect()
}

fn read_row(line: &str) -> Option<PlacedObject> {
    let mut fields = line.split_whitespace();
    let layer = clayspace_model::LayerKey(fields.next()?.parse().ok()?);
    let node = NodeId::restored(fields.next()?.parse().ok()?);
    let shape = clayspace_model::Shape::from_key(fields.next()?)?;
    let op = clayspace_model::Combine::from_key(fields.next()?)?;
    let blend = clayspace_model::BlendProfile::from_key(fields.next()?)?;
    let radius = fields.next()?.parse().ok()?;

    // The rest are numbers, in a fixed order and then a counted run. Taken
    // from one iterator rather than through a closure over it, which borrows
    // `fields` for longer than the count below can wait for.
    let number =
        |fields: &mut std::str::SplitWhitespace| -> Option<f32> { fields.next()?.parse().ok() };
    let position = [
        number(&mut fields)?,
        number(&mut fields)?,
        number(&mut fields)?,
    ];
    let rotation_axis = [
        number(&mut fields)?,
        number(&mut fields)?,
        number(&mut fields)?,
    ];
    let rotation_angle = number(&mut fields)?;
    let scale = number(&mut fields)?;

    let count: usize = fields.next()?.parse().ok()?;
    let mut parameters = Vec::with_capacity(count.min(16));
    for _ in 0..count {
        parameters.push(number(&mut fields)?);
    }

    Some(PlacedObject {
        layer,
        node,
        shape,
        // Brought into range on the way in: a file written by another version
        // may carry a size this one does not allow, and clamping it is better
        // than dropping the object over it.
        parameters: shape.sanitised(&parameters),
        combine: CombineSettings { op, blend, radius },
        position,
        rotation_axis,
        rotation_angle,
        scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_the_application_makes_are_not_objects() {
        assert_eq!(kind_of(claycore::prim::STROKE), ItemKind::Stroke);
        assert_eq!(kind_of(claycore::prim::SWEPT), ItemKind::Curve);
        assert_eq!(kind_of(claycore::prim::ARMATURE), ItemKind::Armature);
    }

    /// And why the table has to record objecthood rather than derive it: a
    /// stamping stroke deposits spheres, and a placed sphere is the same
    /// primitive. Nothing in the document tells them apart.
    #[test]
    fn a_stroke_stamp_is_indistinguishable_from_a_placed_sphere() {
        let placed = primitive_of(Shape::Sphere, &[0.3]);
        let stamp = claycore::Primitive::Sphere { radius: 0.3 };
        assert_eq!(placed.prim(), stamp.prim());
        assert_eq!(kind_of(placed.prim()), kind_of(stamp.prim()));
    }

    /// The parameter description and the engine's own ordering have to agree,
    /// and this is the only place they can disagree.
    #[test]
    fn every_shape_maps_to_a_primitive_the_engine_accepts() {
        for shape in Shape::ALL {
            let primitive = primitive_of(shape, &shape.defaults());
            claycore::Item::of(primitive)
                .unwrap_or_else(|e| panic!("{shape:?} maps to something the engine refused: {e}"));
        }
    }

    /// A short or wild list is brought into range rather than panicking on an
    /// index, because a document written by another version of this
    /// application is a thing that happens.
    #[test]
    fn a_wrong_parameter_list_does_not_panic() {
        for shape in Shape::ALL {
            let _ = primitive_of(shape, &[]);
            let _ = primitive_of(shape, &[f32::NAN; 8]);
            let _ = primitive_of(shape, &[-100.0; 8]);
        }
    }

    #[test]
    fn a_move_dirties_where_it_came_from_as_well() {
        let before = Some(([0.0; 3], [1.0; 3]));
        let after = Some(([2.0; 3], [3.0; 3]));
        assert_eq!(union(before, after), Some(([0.0; 3], [3.0; 3])));
    }

    #[test]
    fn a_move_with_no_finite_bound_on_either_side_is_the_whole_layer() {
        assert_eq!(union(None, Some(([0.0; 3], [1.0; 3]))), None);
        assert_eq!(union(Some(([0.0; 3], [1.0; 3])), None), None);
    }
}
