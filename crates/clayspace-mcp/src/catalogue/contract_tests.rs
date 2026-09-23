//! The argument contract, held over every row of the table.
//!
//! One test per clause, each driven by the table rather than by a list of
//! cases, so that a row added tomorrow is covered by the same assertions
//! without anybody remembering to add it here:
//!
//! - a key the row does not declare is refused, naming the ones it does;
//! - every argument the decoder reads is one the row declares, and every one
//!   the row declares is read by some variant of the call — an argument
//!   offered and then ignored is the same lie as one ignored unoffered;
//! - wherever the decoder reads a numeric argument, a value it cannot honour
//!   is refused: a number that is infinite as an `f32`, a fraction where a
//!   whole number belongs, and a negative one where only a count or a key
//!   makes sense.
//!
//! "Wherever the decoder reads it" is the careful part. Several actions read an
//! argument only for some of their operations — `steps` for an expanding mask
//! but not for an inverted one — so each row is exercised under every
//! combination of its choices, and the decoder's own record of what it looked
//! up decides which variants a bad value must be refused in.

use serde_json::{json, Value};

use super::actions;
use super::args::Args;
use super::table::{ActionSpec, Kind, TABLE};
use crate::session::RefusalCode;

/// A value of this kind the decoder accepts, where one can be named without
/// knowing the action.
fn acceptable(kind: Kind) -> Option<Value> {
    Some(match kind {
        Kind::Number => json!(0.5),
        Kind::Integer => json!(1),
        Kind::Boolean => json!(true),
        Kind::Text | Kind::Path => json!("x"),
        Kind::Vec2 => json!([0.0, 0.0]),
        Kind::Vec3 => json!([0.0, 0.0, 1.0]),
        Kind::IVec3 => json!([3, 3, 3]),
        Kind::Numbers => json!([0.5]),
        Kind::Indices => json!([0]),
        Kind::Choice(of) => json!(of()[0]),
        Kind::Tool => return None,
    })
}

/// The row's example with every declared argument present, under every
/// combination of its choices.
///
/// Every argument present, because the decoder stops at the first refusal: a
/// pass `move` with no `from` is refused before `to` is looked at, and a bad
/// `to` would never be seen. The tool choice is left as the example has it: it
/// narrows nothing the decoder reads, and twenty-one tools would multiply
/// every other set by twenty-one for no coverage.
fn variants(spec: &ActionSpec) -> Vec<Value> {
    let mut full: Value = serde_json::from_str(spec.example).unwrap();
    for arg in spec.arguments {
        if full.get(arg.name).is_none() {
            if let Some(value) = acceptable(arg.kind) {
                full[arg.name] = value;
            }
        }
    }
    let mut out = vec![full];
    for arg in spec.arguments {
        let Kind::Choice(of) = arg.kind else {
            continue;
        };
        out = out
            .iter()
            .flat_map(|base| {
                of().into_iter().map(move |choice| {
                    let mut next = base.clone();
                    next[arg.name] = json!(choice);
                    next
                })
            })
            .collect();
    }
    out
}

fn with(base: &Value, name: &str, value: Value) -> Value {
    let mut next = base.clone();
    next[name] = value;
    next
}

/// The values of this kind that the decoder cannot honour.
fn unacceptable(kind: Kind) -> Vec<Value> {
    // 1e39 is past `f32::MAX` (3.4e38) and so an infinity once narrowed, while
    // being an ordinary finite JSON number: the case that got through.
    match kind {
        Kind::Number => vec![json!(1e39), json!(-1e39)],
        Kind::Vec2 => vec![json!([0.0, 1e39])],
        Kind::Vec3 => vec![json!([1e39, 0.0, 0.0])],
        Kind::Numbers => vec![json!([0.5, 1e39])],
        Kind::Integer => vec![json!(-1), json!(1.5)],
        Kind::IVec3 => vec![json!([2, 2.5, 2]), json!([2, 2, 1e12])],
        Kind::Indices => vec![json!([0, -1])],
        Kind::Boolean | Kind::Text | Kind::Path | Kind::Choice(_) | Kind::Tool => Vec::new(),
    }
}

#[test]
fn unknown_keys_are_rejected() {
    for spec in TABLE {
        let example: Value = serde_json::from_str(spec.example).unwrap();
        let misspelt = with(&example, "zz_not_an_argument", json!(1));
        let args = Args::new(spec.group, spec.name, &misspelt);
        let refusal = actions::build(spec.group, spec.name, &args).expect_err(&format!(
            "{}.{} accepted a key it does not declare",
            spec.group, spec.name
        ));
        assert_eq!(refusal.code, RefusalCode::BadArgument);
        assert!(
            refusal.message.contains("zz_not_an_argument"),
            "{}.{}: {}",
            spec.group,
            spec.name,
            refusal.message
        );
        for arg in spec.arguments {
            assert!(
                refusal.message.contains(arg.name),
                "{}.{}'s refusal does not list {}: {}",
                spec.group,
                spec.name,
                arg.name,
                refusal.message
            );
        }
    }
}

/// The misspelling the issue was filed with, and the envelope a group call
/// carries, which is not an argument of the action and is not refused as one.
#[test]
fn a_misspelt_argument_is_refused_and_the_envelope_is_not() {
    let call = json!({ "action": "set_size", "sizee": 0.5, "size": 0.1 });
    let args = Args::new("brush", "set_size", &call);
    let refusal = actions::build("brush", "set_size", &args).unwrap_err();
    assert!(refusal.message.contains("sizee"), "{}", refusal.message);
    assert!(
        refusal.message.contains("takes size"),
        "{}",
        refusal.message
    );

    let call = json!({ "action": "set_size", "size": 0.1, "capture": "viewport", "width": 64 });
    let args = Args::new("brush", "set_size", &call);
    assert!(actions::build("brush", "set_size", &args).is_ok());
}

#[test]
fn every_argument_the_builder_reads_is_declared() {
    for spec in TABLE {
        let mut read_somewhere: Vec<String> = Vec::new();
        for variant in variants(spec) {
            let args = Args::new(spec.group, spec.name, &variant);
            let _ = actions::build(spec.group, spec.name, &args);
            for name in args.names_read() {
                assert!(
                    spec.arguments.iter().any(|arg| arg.name == name),
                    "{}.{} reads {name}, which its row does not declare, so a caller \
                     sending it would be refused",
                    spec.group,
                    spec.name
                );
                read_somewhere.push(name);
            }
        }
        for arg in spec.arguments {
            assert!(
                read_somewhere.iter().any(|name| name == arg.name),
                "{}.{} declares {} and never reads it",
                spec.group,
                spec.name,
                arg.name
            );
        }
    }
}

#[test]
fn a_value_that_cannot_be_honoured_is_refused_wherever_it_is_read() {
    for spec in TABLE {
        for arg in spec.arguments {
            for bad in unacceptable(arg.kind) {
                let mut read = false;
                for variant in variants(spec) {
                    let call = with(&variant, arg.name, bad.clone());
                    let args = Args::new(spec.group, spec.name, &call);
                    let built = actions::build(spec.group, spec.name, &args);
                    if !args.names_read().iter().any(|name| name == arg.name) {
                        continue;
                    }
                    read = true;
                    assert!(
                        built.is_err(),
                        "{}.{} accepted {} = {bad} in {call}",
                        spec.group,
                        spec.name,
                        arg.name
                    );
                }
                assert!(
                    read,
                    "{}.{}'s {} was never read, so {bad} was never tested",
                    spec.group, spec.name, arg.name
                );
            }
        }
    }
}

/// `-1 as u32` is how `resolution: -1` became 512: wrapped to four billion,
/// then clamped to the finest the rebuild allows.
#[test]
fn a_negative_resolution_is_refused_rather_than_wrapped() {
    let call = json!({ "resolution": -1 });
    let args = Args::new("layer", "set_remesh", &call);
    let refusal = actions::build("layer", "set_remesh", &args).unwrap_err();
    assert!(
        refusal.message.contains("cannot be negative"),
        "{}",
        refusal.message
    );
}

#[test]
fn an_import_ceiling_and_scale_are_refused_where_they_cannot_be_held() {
    for call in [
        json!({ "max_vertices": -1 }),
        json!({ "max_triangles": -1 }),
        json!({ "scale": 1e308 }),
    ] {
        let args = Args::new("exchange", "set_import", &call);
        assert!(
            actions::build("exchange", "set_import", &args).is_err(),
            "{call} was accepted"
        );
    }
}

/// A reference offset that is not two numbers used to become the default
/// offset, because its refusal was thrown away.
#[test]
fn a_malformed_reference_offset_is_refused() {
    for offset in [json!("bad"), json!([1])] {
        let call = json!({ "plane": "front", "offset": offset });
        let args = Args::new("reference", "set", &call);
        assert!(
            actions::build("reference", "set", &args).is_err(),
            "{call} was accepted"
        );
    }
}

/// Absence clears a selection; a negative index is an error rather than a
/// second way of spelling it.
#[test]
fn a_selection_is_cleared_by_absence_and_not_by_a_negative_index() {
    for (group, action, name) in [
        ("curve", "select_point", "index"),
        ("lattice", "select_point", "index"),
        ("armature", "select", "sphere"),
    ] {
        let absent = json!({});
        let args = Args::new(group, action, &absent);
        assert!(actions::build(group, action, &args).is_ok());

        let negative = json!({ name: -1 });
        let args = Args::new(group, action, &negative);
        assert!(
            actions::build(group, action, &args).is_err(),
            "{group}.{action} took -1 as a clear"
        );
    }
}

#[test]
fn mask_apply_carries_its_steps() {
    for (op, expected) in [
        ("expand", clayspace_model::MaskOp::Expand(5)),
        ("contract", clayspace_model::MaskOp::Contract(5)),
        ("smooth", clayspace_model::MaskOp::Smooth(5)),
    ] {
        let call = json!({ "op": op, "steps": 5 });
        let args = Args::new("mask", "apply", &call);
        assert_eq!(
            actions::build("mask", "apply", &args).unwrap(),
            clayspace_vm::Command::ApplyMaskOp(expected)
        );
    }
}

/// Degrees on the wire, as `describe` says; radians in the command, as the
/// panel sends it.
#[test]
fn an_azimuth_in_degrees_reaches_the_brush_in_radians() {
    let call = json!({ "azimuth": 90.0 });
    let args = Args::new("brush", "set_azimuth", &call);
    match actions::build("brush", "set_azimuth", &args).unwrap() {
        clayspace_vm::Command::SetBrushAzimuth(radians) => {
            assert!(
                (radians - std::f32::consts::FRAC_PI_2).abs() < 1e-6,
                "{radians}"
            )
        }
        other => panic!("built {other:?}"),
    }
    assert!(args.clamped().is_empty(), "{:?}", args.clamped());
}

#[test]
fn clamped_values_are_reported() {
    for (group, action, call, argument, used) in [
        (
            "layer",
            "set_remesh",
            json!({ "resolution": 4096 }),
            "resolution",
            json!(512.0),
        ),
        (
            "view",
            "set_surface_opacity",
            json!({ "opacity": 5.0 }),
            "opacity",
            json!(1.0),
        ),
        (
            "view",
            "set_grid_display",
            json!({ "display": "smooth", "blur_passes": 7 }),
            "blur_passes",
            json!(3.0),
        ),
        (
            "brush",
            "set_intensity",
            json!({ "intensity": 3.0 }),
            "intensity",
            json!(1.0),
        ),
        (
            "reference",
            "set",
            json!({ "plane": "front", "opacity": -3.0 }),
            "opacity",
            json!(0.0),
        ),
        ("convert", "set", json!({ "blur": 9 }), "blur", json!(2.0)),
    ] {
        let args = Args::new(group, action, &call);
        actions::build(group, action, &args)
            .unwrap_or_else(|e| panic!("{group}.{action} refused {call}: {e}"));
        let clamped = args.clamped();
        assert_eq!(
            clamped.len(),
            1,
            "{group}.{action} {call} reported {clamped:?}"
        );
        assert_eq!(clamped[0]["argument"], argument);
        assert_eq!(clamped[0]["used"], used, "{group}.{action}: {clamped:?}");
    }
}

#[test]
fn a_value_already_in_range_reports_no_clamp() {
    for spec in TABLE {
        let example: Value = serde_json::from_str(spec.example).unwrap();
        let args = Args::new(spec.group, spec.name, &example);
        actions::build(spec.group, spec.name, &args).unwrap();
        assert!(
            args.clamped().is_empty(),
            "{}.{}'s own example is reported clamped: {:?}",
            spec.group,
            spec.name,
            args.clamped()
        );
    }
}
