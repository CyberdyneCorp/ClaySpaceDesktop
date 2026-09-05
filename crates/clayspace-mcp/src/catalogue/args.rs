//! Reading a tool call's arguments, and saying what was expected when they
//! cannot be read.
//!
//! Every refusal here names the argument and what it should have been. A
//! generic "invalid arguments" costs an agent a round trip to find out which
//! one, and it has no way to find out except by guessing.

use serde_json::Value;

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
}

impl<'a> Args<'a> {
    pub fn new(group: &'static str, action: &'a str, value: &'a Value) -> Self {
        Self {
            group,
            action,
            value,
        }
    }

    fn field(&self, name: &str) -> Option<&'a Value> {
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
        value
            .as_f64()
            .map(|n| n as f32)
            .ok_or_else(|| self.wrong(name, "a number", value))
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

    pub fn index(&self, name: &str) -> Read<usize> {
        let value = self.integer(name)?;
        usize::try_from(value)
            .map_err(|_| bad(format!("{name} cannot be negative, and {value} is")))
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
            .map(|item| {
                item.as_f64()
                    .map(|n| n as f32)
                    .ok_or_else(|| self.wrong(name, "a list of numbers", item))
            })
            .collect()
    }

    pub fn vec2(&self, name: &str) -> Read<[f32; 2]> {
        let read = self.numbers(name, 2)?;
        Ok([read[0], read[1]])
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

    pub fn ivec3(&self, name: &str) -> Read<[i32; 3]> {
        let read = self.numbers(name, 3)?;
        Ok([read[0] as i32, read[1] as i32, read[2] as i32])
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
            .map(|item| {
                item.as_f64()
                    .map(|n| n as f32)
                    .ok_or_else(|| self.wrong(name, "a list of numbers", item))
            })
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
        Ok(Args {
            group: self.group,
            action: self.action,
            value,
        })
    }

    /// The same, or an empty one where it is absent, so that a settings block
    /// with every field defaulted need not be sent at all.
    pub fn object_or_empty(&self, name: &str) -> Read<Args<'_>> {
        const EMPTY: &Value = &Value::Null;
        match self.field(name) {
            None => Ok(Args {
                group: self.group,
                action: self.action,
                value: EMPTY,
            }),
            Some(_) => self.object(name),
        }
    }
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
