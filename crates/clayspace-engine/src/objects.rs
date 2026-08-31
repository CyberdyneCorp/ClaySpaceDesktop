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
use clayspace_model::{CombineSettings, ItemKind, ObjectId, ObjectSource, SceneObject, Shape};

/// A placed object's state, as only the application knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    pub layer: clayspace_model::LayerKey,
    pub node: NodeId,
    pub source: ObjectSource,
    pub parameters: Vec<f32>,
    pub combine: CombineSettings,
    pub position: [f32; 3],
    pub rotation_axis: [f32; 3],
    pub rotation_angle: f32,
    /// One factor per axis, applied in the object's own local frame.
    ///
    /// Uniform once, on the belief that "the engine's transforms take one
    /// factor, not three". A node's has taken three since ABI 0.54.0.
    pub scale: [f32; 3],
}

impl PlacedObject {
    /// What a freshly placed object carries.
    pub fn new(
        layer: clayspace_model::LayerKey,
        node: NodeId,
        source: ObjectSource,
        parameters: Vec<f32>,
        combine: CombineSettings,
        position: [f32; 3],
    ) -> Self {
        Self {
            layer,
            node,
            source,
            parameters,
            combine,
            position,
            // The engine requires a non-zero axis even for no rotation: "a
            // second convention for 'no rotation' would be one more thing to
            // get wrong."
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.0,
            scale: [1.0; 3],
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
            source: self.source.clone(),
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
        // The source, as one field: a shape's key, or `mesh` and the layer it
        // came from. A mesh operand is a *copy* — the item in this layer is
        // the sampled volume, and the source layer is only recorded so a row
        // can say what it was made from.
        let source = match &object.source {
            ObjectSource::Shape(shape) => shape.key().to_string(),
            ObjectSource::Mesh { from, .. } => format!("mesh:{}", from.0),
        };
        out.push_str(&format!(
            "{} {} {} {} {} {}",
            object.layer.0,
            object.node.get(),
            source,
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
            // The first component stands where the single uniform scale
            // stood, so a build that predates the per-axis one reads a
            // squashed object as evenly scaled rather than failing to read it.
            object.scale[0],
        ] {
            out.push_str(&format!(" {value}"));
        }
        out.push_str(&format!(" {}", object.parameters.len()));
        for value in &object.parameters {
            out.push_str(&format!(" {value}"));
        }
        // The other two components, appended after the counted run rather
        // than beside the first. A positional format cannot grow in the
        // middle: a reader that predates them takes the fields it knows in
        // order, stops at the end of the parameters, and never looks at these
        // — so an older build opens a document written by this one and reads
        // every object as uniformly scaled, which is a degradation and not a
        // corruption. Growing in the middle would have shifted the parameter
        // count and made the row unreadable.
        out.push_str(&format!(" {} {}", object.scale[1], object.scale[2]));
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

/// A source field, as `write_table` wrote it.
///
/// A mesh operand's *name* is not written: it is the source layer's, and a
/// layer that has been renamed since should read as it is called now rather
/// than as it was called then. A layer that has been removed leaves the name
/// empty, which a row shows as the crossing it was.
fn read_source(field: &str) -> Option<ObjectSource> {
    if let Some(key) = field.strip_prefix("mesh:") {
        return Some(ObjectSource::Mesh {
            from: clayspace_model::LayerKey(key.parse().ok()?),
            name: String::new(),
        });
    }
    clayspace_model::Shape::from_key(field).map(ObjectSource::Shape)
}

fn read_row(line: &str) -> Option<PlacedObject> {
    let mut fields = line.split_whitespace();
    let layer = clayspace_model::LayerKey(fields.next()?.parse().ok()?);
    let node = NodeId::restored(fields.next()?.parse().ok()?);
    let source = read_source(fields.next()?)?;
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
    // Where the uniform scale stood, and now the first of three.
    let scale_x = number(&mut fields)?;

    let count: usize = fields.next()?.parse().ok()?;
    let mut parameters = Vec::with_capacity(count.min(16));
    for _ in 0..count {
        parameters.push(number(&mut fields)?);
    }

    // Brought into range on the way in: a file written by another version may
    // carry a size this one does not allow, and clamping it is better than
    // dropping the object over it. A mesh is measured by itself and carries
    // none.
    let parameters = match source.shape() {
        Some(shape) => shape.sanitised(&parameters),
        None => Vec::new(),
    };
    // Absent in anything written before the per-axis scale, which is what
    // makes those documents still open: no pair, so the one factor is all
    // three. A half-written pair falls back the same way rather than taking
    // one component and inventing the other.
    let scale = match (number(&mut fields), number(&mut fields)) {
        (Some(y), Some(z)) => [scale_x, y, z],
        _ => [scale_x; 3],
    };

    Some(PlacedObject {
        layer,
        node,
        source,
        parameters,
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

    /// One object, with a scale to round-trip.
    fn placed(scale: [f32; 3]) -> PlacedObject {
        PlacedObject {
            layer: clayspace_model::LayerKey(3),
            node: NodeId::restored(9),
            source: ObjectSource::Shape(Shape::Capsule),
            parameters: Shape::Capsule.defaults(),
            combine: CombineSettings::default(),
            position: [1.5, -2.0, 0.25],
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.5,
            scale,
        }
    }

    /// A squashed object survives being written and read back.
    #[test]
    fn a_per_axis_scale_round_trips_through_the_side_car() {
        let directory = std::env::temp_dir().join("clayspace-objects-per-axis");
        std::fs::create_dir_all(&directory).expect("a place to write");
        let path = directory.join("squashed.clay.objects");
        let object = placed([2.0, 1.0, 0.5]);
        write_table(&path, std::slice::from_ref(&object)).expect("written");

        let read = read_table(&path);
        assert_eq!(read.len(), 1, "the row did not come back");
        assert_eq!(
            read[0].scale, object.scale,
            "the stretch was lost between writing and reading"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A document written before the per-axis scale still opens.
    ///
    /// The reason the two extra components are appended *after* the counted
    /// run of parameters rather than beside the first: a positional format
    /// cannot grow in the middle. This is a row exactly as the previous
    /// version wrote one — a single scale, then the parameter count and its
    /// values, and nothing after — and it has to read as evenly scaled rather
    /// than being dropped.
    #[test]
    fn a_row_written_before_the_per_axis_scale_reads_as_uniform() {
        let defaults = Shape::Capsule.defaults();
        let mut row = format!(
            "3 9 {} {} {} {} 1.5 -2 0.25 0 1 0 0.5 1.75 {}",
            Shape::Capsule.key(),
            clayspace_model::Combine::default().key(),
            clayspace_model::BlendProfile::default().key(),
            clayspace_model::CombineSettings::default().radius,
            defaults.len(),
        );
        for value in &defaults {
            row.push_str(&format!(" {value}"));
        }

        let object = read_row(&row).expect("a row from the previous format did not read at all");
        assert_eq!(
            object.scale, [1.75; 3],
            "the one scale a previous version wrote should be all three"
        );
        assert_eq!(object.position, [1.5, -2.0, 0.25]);
    }

    /// And a row from this version is readable by the previous one's rules:
    /// everything it knows about is in the same place, in the same order.
    #[test]
    fn the_fields_a_previous_version_reads_have_not_moved() {
        let directory = std::env::temp_dir().join("clayspace-objects-order");
        std::fs::create_dir_all(&directory).expect("a place to write");
        let path = directory.join("order.clay.objects");
        write_table(&path, &[placed([2.0, 1.0, 0.5])]).expect("written");
        let text = std::fs::read_to_string(&path).expect("read back");
        let row = text.lines().nth(1).expect("a row");
        let fields: Vec<&str> = row.split_whitespace().collect();

        // Six named fields (layer, node, source, op, blend, radius), then
        // eight numbers (a position, a rotation axis, an angle, and the first
        // scale), then the parameter count and its run — the layout that was
        // there before, with the extra two after everything it describes.
        const NAMED: usize = 6;
        const NUMBERS: usize = 8;
        let count: usize = fields[NAMED + NUMBERS]
            .parse()
            .unwrap_or_else(|_| panic!("no parameter count at {}: {fields:?}", NAMED + NUMBERS));
        assert_eq!(
            fields.len(),
            NAMED + NUMBERS + 1 + count + 2,
            "a field moved: a reader that predates the per-axis scale walks \
             this row by position and would take the wrong one — {fields:?}"
        );
        assert_eq!(
            fields[NAMED + NUMBERS - 1],
            "2",
            "the first scale is not where it was: {fields:?}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// And why the table has to record objecthood rather than derive it: a
    /// stamping stroke deposits spheres, and a placed sphere is the same
    /// primitive. Nothing in the document tells them apart.
    #[test]
    fn a_source_round_trips_through_the_side_car() {
        for source in [
            ObjectSource::Shape(Shape::Cylinder),
            ObjectSource::Mesh {
                from: clayspace_model::LayerKey(7),
                name: "Parafuso".into(),
            },
        ] {
            let written = match &source {
                ObjectSource::Shape(shape) => shape.key().to_string(),
                ObjectSource::Mesh { from, .. } => format!("mesh:{}", from.0),
            };
            let read = read_source(&written).expect("a source");
            // The name is not written — a renamed layer should read as it is
            // called now — so the comparison is on what is.
            match (&source, &read) {
                (ObjectSource::Shape(a), ObjectSource::Shape(b)) => assert_eq!(a, b),
                (ObjectSource::Mesh { from: a, .. }, ObjectSource::Mesh { from: b, .. }) => {
                    assert_eq!(a, b)
                }
                _ => panic!("{source:?} read back as {read:?}"),
            }
        }
    }

    #[test]
    fn an_unknown_source_is_dropped_rather_than_guessed_at() {
        assert!(read_source("dodecahedron").is_none());
        assert!(read_source("mesh:not-a-number").is_none());
    }

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
    ///
    /// Both halves, and the second is the one that matters: not panicking is
    /// cheap, and a NaN that sails through the clamp and out to the engine
    /// panics nothing here and ruins the layer there. `nan` parses as a
    /// perfectly good f32, so a hand-edited `.objects` row is enough.
    #[test]
    fn a_wrong_parameter_list_is_brought_into_range_rather_than_panicking() {
        let hostile: [&[f32]; 5] = [
            &[],
            &[f32::NAN; 8],
            &[-100.0; 8],
            &[f32::INFINITY; 8],
            &[f32::NEG_INFINITY; 8],
        ];
        for shape in Shape::ALL {
            for given in hostile {
                for (value, parameter) in shape.sanitised(given).iter().zip(shape.parameters()) {
                    assert!(
                        (parameter.min..=parameter.max).contains(value),
                        "{shape:?}'s {} came out of {given:?} as {value}",
                        parameter.key
                    );
                }
                // And whatever the mapping derives from them — a capsule's two
                // end points, a cone's resolved sine and cosine — is finite
                // too, since that is the block the engine actually reads.
                for value in primitive_of(shape, given).params() {
                    assert!(
                        value.is_finite(),
                        "{shape:?} handed the engine {value} from {given:?}"
                    );
                }
            }
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
