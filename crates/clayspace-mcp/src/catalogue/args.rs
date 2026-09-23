//! Reading a tool call's arguments, and saying what was expected when they
//! cannot be read.
//!
//! Every refusal here names the argument and what it should have been. A
//! generic "invalid arguments" costs an agent a round trip to find out which
//! one, and it has no way to find out except by guessing.
//!
//! **Nothing is read that cannot be honoured.** A key the action does not
//! declare is refused rather than ignored, a negative count is refused rather
//! than cast into four billion, and a number too large for the `f32` it lands
//! in is refused rather than carried as an infinity. Each of those used to
//! answer success: `sizee` ran the command at its default, `resolution: -1`
//! wrapped to `u32::MAX` and was clamped into a plausible 512, and
//! `scale: 1e308` reached the import panel as "Scale inf". Success over a
//! document configured differently from what was asked is worse than a
//! refusal, because nothing tells the agent to look.
//!
//! What the application brings into range rather than refuses is reported
//! back instead: see [`Args::note_clamp`].

use std::cell::RefCell;

use serde_json::{json, Value};

use crate::session::{Refusal, RefusalCode};

pub type Read<T> = Result<T, Refusal>;

fn bad(message: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::BadArgument, message)
}

/// The arguments of one call.
pub struct Args<'a> {
    pub group: &'static str,
    pub action: &'a str,
    value: &'a Value,
    /// The values the decoder already knows the application will bring into
    /// range, as `{argument, asked, used}`. A `RefCell` because every reader
    /// here takes `&self`, and a decoder that had to thread `&mut` through
    /// every helper for the sake of a report is one nobody extends.
    clamps: RefCell<Vec<Value>>,
    /// Every name a decoder looked up, for the tests that hold the table's
    /// declared arguments and the decoder's reads together.
    #[cfg(test)]
    read: RefCell<Vec<String>>,
}

impl<'a> Args<'a> {
    pub fn new(group: &'static str, action: &'a str, value: &'a Value) -> Self {
        Self {
            group,
            action,
            value,
            clamps: RefCell::new(Vec::new()),
            #[cfg(test)]
            read: RefCell::new(Vec::new()),
        }
    }

    /// Every name a decoder looked up, present or not.
    #[cfg(test)]
    pub fn names_read(&self) -> Vec<String> {
        self.read.borrow().clone()
    }

    /// Refuses any key that is neither one of `accepted` nor `envelope`.
    ///
    /// `accepted` is what the action declares, and is what the refusal
    /// lists; `envelope` is what the call itself carries — the action's
    /// name, a capture — and is let through without being offered back as an
    /// argument of the action.
    ///
    /// Sorted, so that two misspellings are named in the same order every
    /// time rather than in a map's.
    pub fn accept_only(&self, accepted: &[&str], envelope: &[&str]) -> Read<()> {
        let Some(object) = self.value.as_object() else {
            return Ok(());
        };
        let mut unknown: Vec<&str> = object
            .keys()
            .map(String::as_str)
            .filter(|key| !accepted.contains(key) && !envelope.contains(key))
            .collect();
        if unknown.is_empty() {
            return Ok(());
        }
        unknown.sort_unstable();
        let takes = if accepted.is_empty() {
            "it takes no arguments".to_string()
        } else {
            format!("it takes {}", accepted.join(", "))
        };
        Err(bad(format!(
            "{}.{} has no argument {}; {takes}",
            self.group,
            self.action,
            unknown.join(", ")
        )))
    }

    /// Records that `name` was asked for as `asked` and will be applied as
    /// `used`, where the two differ.
    ///
    /// For the ranges the application enforces by clamping rather than by
    /// refusing — a slider's ends, a rebuild's finest resolution. Refusing
    /// those here would make the door stricter than the panel it drives, and
    /// a caller would learn the range by bisecting refusals; saying what was
    /// used costs one field in the answer.
    pub fn note_clamp(&self, name: &str, asked: f64, used: f64) {
        let tolerance = 1e-4 * asked.abs().max(1.0);
        if (asked - used).abs() <= tolerance {
            return;
        }
        self.clamps.borrow_mut().push(json!({
            "argument": name,
            "asked": wire_number(asked),
            "used": wire_number(used),
        }));
    }

    /// Everything [`Args::note_clamp`] recorded, in the order it was read.
    pub fn clamped(&self) -> Vec<Value> {
        self.clamps.borrow().clone()
    }

    fn field(&self, name: &str) -> Option<&'a Value> {
        #[cfg(test)]
        self.read.borrow_mut().push(name.to_string());
        match self.value.get(name) {
            Some(Value::Null) | None => None,
            Some(value) => Some(value),
        }
    }

    fn missing(&self, name: &str, kind: &str) -> Refusal {
        bad(format!(
            "{}.{} needs {name}, which is {kind}",
            self.group, self.action
        ))
    }

    fn wrong(&self, name: &str, kind: &str, got: &Value) -> Refusal {
        bad(format!(
            "{}.{}'s {name} is {kind}, and {got} is not",
            self.group, self.action
        ))
    }

    pub fn number(&self, name: &str) -> Read<f32> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "a number"))?;
        self.finite(name, "a number", value)
    }

    /// One JSON number as the `f32` every setting holds.
    ///
    /// JSON has no NaN or infinity to send, but it has `1e308`, and that is an
    /// infinity once it is an `f32`. Checked after the narrowing for that
    /// reason: checking the `f64` would pass exactly the value that breaks.
    fn finite(&self, name: &str, kind: &str, value: &Value) -> Read<f32> {
        let number = value
            .as_f64()
            .ok_or_else(|| self.wrong(name, kind, value))?;
        let narrowed = number as f32;
        if narrowed.is_finite() {
            Ok(narrowed)
        } else {
            Err(bad(format!(
                "{}.{}'s {name} is a finite number, and {value} is not one as a \
                 32-bit float",
                self.group, self.action
            )))
        }
    }

    pub fn optional_number(&self, name: &str) -> Read<Option<f32>> {
        match self.field(name) {
            None => Ok(None),
            Some(_) => self.number(name).map(Some),
        }
    }

    pub fn number_or(&self, name: &str, fallback: f32) -> Read<f32> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.number(name),
        }
    }

    pub fn integer(&self, name: &str) -> Read<i64> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "a whole number"))?;
        value
            .as_i64()
            .ok_or_else(|| self.wrong(name, "a whole number", value))
    }

    pub fn integer_or(&self, name: &str, fallback: i64) -> Read<i64> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.integer(name),
        }
    }

    /// A whole number that must fit `T` — a count, a size, an index.
    ///
    /// Refused rather than cast where it does not: `-1 as u32` is four
    /// billion, and whatever clamp comes after turns that into the largest
    /// value the setting allows, which is a plausible answer to a question
    /// nobody asked.
    pub fn whole<T: TryFrom<i64>>(&self, name: &str) -> Read<T> {
        let value = self.integer(name)?;
        T::try_from(value).map_err(|_| {
            let why = if value < 0 {
                "cannot be negative"
            } else {
                "is too large to hold"
            };
            bad(format!(
                "{}.{}'s {name} {why}, and {value} was given",
                self.group, self.action
            ))
        })
    }

    pub fn whole_or<T: TryFrom<i64>>(&self, name: &str, fallback: T) -> Read<T> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.whole(name),
        }
    }

    /// A whole number where absence means none — a selection cleared.
    ///
    /// Absence, and not `-1`. A negative index used to clear as well, which
    /// made every negative number mean the same thing and none of them an
    /// error.
    pub fn optional_whole<T: TryFrom<i64>>(&self, name: &str) -> Read<Option<T>> {
        match self.field(name) {
            None => Ok(None),
            Some(_) => self.whole(name).map(Some),
        }
    }

    pub fn index(&self, name: &str) -> Read<usize> {
        self.whole(name)
    }

    /// A number of passes or steps, which the model holds as an `i32` and no
    /// reading of which is negative.
    pub fn count(&self, name: &str) -> Read<i32> {
        let value: i32 = self.whole(name)?;
        if value < 0 {
            return Err(bad(format!(
                "{}.{}'s {name} cannot be negative, and {value} was given",
                self.group, self.action
            )));
        }
        Ok(value)
    }

    pub fn count_or(&self, name: &str, fallback: i32) -> Read<i32> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.count(name),
        }
    }

    pub fn boolean(&self, name: &str) -> Read<bool> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "true or false"))?;
        value
            .as_bool()
            .ok_or_else(|| self.wrong(name, "true or false", value))
    }

    pub fn boolean_or(&self, name: &str, fallback: bool) -> Read<bool> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.boolean(name),
        }
    }

    pub fn text(&self, name: &str) -> Read<String> {
        let value = self.field(name).ok_or_else(|| self.missing(name, "text"))?;
        value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| self.wrong(name, "text", value))
    }

    pub fn text_or(&self, name: &str, fallback: &str) -> Read<String> {
        match self.field(name) {
            None => Ok(fallback.to_string()),
            Some(_) => self.text(name),
        }
    }

    pub fn optional_text(&self, name: &str) -> Read<Option<String>> {
        match self.field(name) {
            None => Ok(None),
            Some(_) => self.text(name).map(Some),
        }
    }

    /// A layer key, which the wire carries as the number the engine minted.
    pub fn layer(&self, name: &str) -> Read<u64> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "a layer key"))?;
        value
            .as_u64()
            .ok_or_else(|| self.wrong(name, "a layer key, which is a whole number", value))
    }

    pub fn optional_layer(&self, name: &str) -> Read<Option<u64>> {
        match self.field(name) {
            None => Ok(None),
            Some(_) => self.layer(name).map(Some),
        }
    }

    fn numbers(&self, name: &str, wanted: usize) -> Read<Vec<f32>> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, &format!("{wanted} numbers")))?;
        let array = value
            .as_array()
            .ok_or_else(|| self.wrong(name, &format!("{wanted} numbers"), value))?;
        if array.len() != wanted {
            return Err(bad(format!(
                "{}.{}'s {name} is {wanted} numbers, and {} were given",
                self.group,
                self.action,
                array.len()
            )));
        }
        array
            .iter()
            .map(|item| self.finite(name, "a list of numbers", item))
            .collect()
    }

    pub fn vec2(&self, name: &str) -> Read<[f32; 2]> {
        let read = self.numbers(name, 2)?;
        Ok([read[0], read[1]])
    }

    pub fn vec2_or(&self, name: &str, fallback: [f32; 2]) -> Read<[f32; 2]> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.vec2(name),
        }
    }

    pub fn vec3(&self, name: &str) -> Read<[f32; 3]> {
        let read = self.numbers(name, 3)?;
        Ok([read[0], read[1], read[2]])
    }

    pub fn vec3_or(&self, name: &str, fallback: [f32; 3]) -> Read<[f32; 3]> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.vec3(name),
        }
    }

    /// Three whole numbers, each refused rather than truncated where it is a
    /// fraction or does not fit an `i32`.
    pub fn ivec3(&self, name: &str) -> Read<[i32; 3]> {
        let kind = "three whole numbers";
        let value = self.field(name).ok_or_else(|| self.missing(name, kind))?;
        let array = value
            .as_array()
            .filter(|array| array.len() == 3)
            .ok_or_else(|| self.wrong(name, kind, value))?;
        let mut read = [0; 3];
        for (slot, item) in read.iter_mut().zip(array) {
            *slot = item
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| self.wrong(name, kind, value))?;
        }
        Ok(read)
    }

    /// A list of numbers of any length — a shape's parameters, for instance.
    pub fn number_list(&self, name: &str) -> Read<Vec<f32>> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "a list of numbers"))?;
        let array = value
            .as_array()
            .ok_or_else(|| self.wrong(name, "a list of numbers", value))?;
        array
            .iter()
            .map(|item| self.finite(name, "a list of numbers", item))
            .collect()
    }

    pub fn number_list_or_empty(&self, name: &str) -> Read<Vec<f32>> {
        match self.field(name) {
            None => Ok(Vec::new()),
            Some(_) => self.number_list(name),
        }
    }

    pub fn index_list(&self, name: &str) -> Read<Vec<usize>> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "a list of whole numbers"))?;
        let array = value
            .as_array()
            .ok_or_else(|| self.wrong(name, "a list of whole numbers", value))?;
        array
            .iter()
            .map(|item| {
                item.as_u64()
                    .map(|n| n as usize)
                    .ok_or_else(|| self.wrong(name, "a list of whole numbers", item))
            })
            .collect()
    }

    pub fn text_list_or_empty(&self, name: &str) -> Read<Vec<String>> {
        let value = match self.field(name) {
            None => return Ok(Vec::new()),
            Some(value) => value,
        };
        let array = value
            .as_array()
            .ok_or_else(|| self.wrong(name, "a list of words", value))?;
        array
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| self.wrong(name, "a list of words", item))
            })
            .collect()
    }

    /// One of a named set, refused with the whole set where it is not.
    pub fn choice<T: Copy>(&self, name: &str, table: &[(&str, T)]) -> Read<T> {
        let given = self.text(name)?;
        self.choose(name, &given, table)
    }

    pub fn choice_or<T: Copy>(&self, name: &str, table: &[(&str, T)], fallback: T) -> Read<T> {
        match self.field(name) {
            None => Ok(fallback),
            Some(_) => self.choice(name, table),
        }
    }

    pub fn choose<T: Copy>(&self, name: &str, given: &str, table: &[(&str, T)]) -> Read<T> {
        table
            .iter()
            .find(|(tag, _)| *tag == given)
            .map(|(_, value)| *value)
            .ok_or_else(|| {
                let offered: Vec<&str> = table.iter().map(|(tag, _)| *tag).collect();
                bad(format!(
                    "{}.{}'s {name} is one of {}, and {given} is not",
                    self.group,
                    self.action,
                    offered.join(", ")
                ))
            })
    }

    /// A nested object, for a settings block.
    pub fn object(&self, name: &str) -> Read<Args<'_>> {
        let value = self
            .field(name)
            .ok_or_else(|| self.missing(name, "an object"))?;
        if !value.is_object() {
            return Err(self.wrong(name, "an object", value));
        }
        Ok(Args::new(self.group, self.action, value))
    }

    /// The same, or an empty one where it is absent, so that a settings block
    /// with every field defaulted need not be sent at all.
    pub fn object_or_empty(&self, name: &str) -> Read<Args<'_>> {
        const EMPTY: &Value = &Value::Null;
        match self.field(name) {
            None => Ok(Args::new(self.group, self.action, EMPTY)),
            Some(_) => self.object(name),
        }
    }
}

/// A number as the answer carries it. An `f32` widened to `f64` prints as
/// `0.10000000149011612`, which is exact and useless to anyone reading it.
fn wire_number(value: f64) -> Value {
    (value as f32)
        .to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(Value::Null, Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args<'a>(value: &'a Value) -> Args<'a> {
        Args::new("sculpt", "stroke", value)
    }

    #[test]
    fn a_missing_argument_names_itself_and_its_kind() {
        let value = json!({});
        let refusal = args(&value).number("radius").unwrap_err();
        assert_eq!(refusal.code, RefusalCode::BadArgument);
        assert!(
            refusal.message.contains("sculpt.stroke"),
            "{}",
            refusal.message
        );
        assert!(refusal.message.contains("radius"), "{}", refusal.message);
        assert!(refusal.message.contains("a number"), "{}", refusal.message);
    }

    #[test]
    fn a_wrongly_typed_argument_says_what_it_got() {
        let value = json!({ "radius": "large" });
        let refusal = args(&value).number("radius").unwrap_err();
        assert!(refusal.message.contains("large"), "{}", refusal.message);
    }

    #[test]
    fn a_null_is_an_absence_rather_than_a_value() {
        let value = json!({ "radius": null });
        assert_eq!(args(&value).number_or("radius", 0.5).unwrap(), 0.5);
    }

    #[test]
    fn a_vector_of_the_wrong_length_says_both_lengths() {
        let value = json!({ "at": [1.0, 2.0] });
        let refusal = args(&value).vec3("at").unwrap_err();
        assert!(refusal.message.contains("3 numbers"), "{}", refusal.message);
        assert!(
            refusal.message.contains("2 were given"),
            "{}",
            refusal.message
        );
    }

    #[test]
    fn a_choice_is_refused_with_the_whole_set() {
        let value = json!({ "falloff": "sharp" });
        let refusal = args(&value)
            .choice("falloff", &[("smooth", 1), ("linear", 2)])
            .unwrap_err();
        assert!(
            refusal.message.contains("smooth, linear"),
            "{}",
            refusal.message
        );
        assert!(refusal.message.contains("sharp"), "{}", refusal.message);
    }

    #[test]
    fn a_negative_index_is_refused_rather_than_wrapped() {
        let value = json!({ "index": -1 });
        assert!(args(&value).index("index").is_err());
    }

    #[test]
    fn an_absent_settings_block_reads_as_all_defaults() {
        let value = json!({});
        let outer = args(&value);
        let nested = outer.object_or_empty("settings").unwrap();
        assert_eq!(nested.number_or("scale", 1.0).unwrap(), 1.0);
    }
}
