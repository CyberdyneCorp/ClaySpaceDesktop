//! The baseline file: written and read by hand rather than by a dependency.
//!
//! The shape is small and stable, and a serialiser in the dependency graph is
//! a thing the audit has to consider forever for one file.
//!
//! The reader used to be `str::find` over the raw text, which was defensible
//! while the file was one flat map of numbers. It is not defensible with a
//! nested `scenes` map and a `skipped` section beside the figures: positional
//! string searching is how a subtly wrong comparison gets shipped, and a wrong
//! comparison is worse than none. So this parses.

use std::collections::BTreeMap;
use std::io::{Error, ErrorKind, Result};

use clayspace_app::Conditions;

use crate::load::Load;
use crate::run::Run;

/// A recorded run, as read back from a baseline file.
#[derive(Debug, Clone, PartialEq)]
pub struct Baseline {
    pub scenes: BTreeMap<String, String>,
    pub platform: String,
    pub architecture: String,
    pub backend: String,
    pub engine: String,
    pub figures: BTreeMap<String, f64>,
    /// What that run did not measure, and why.
    pub skipped: BTreeMap<String, String>,
    /// The one-minute load per core the recording machine was under, where the
    /// run reported one. Absent from baselines recorded before this was kept,
    /// which is why it is an `Option` rather than a defaulted number: "quiet"
    /// and "did not say" are different claims.
    pub load_per_core: Option<f64>,
}

pub fn write(path: &str, where_: &Conditions, load: Option<&Load>, run: &Run) -> Result<()> {
    std::fs::write(path, render(where_, load, run))
}

fn render(where_: &Conditions, load: Option<&Load>, run: &Run) -> String {
    let mut out = String::from("{\n  \"conditions\": {\n    \"scenes\": {\n");
    let members: Vec<String> = where_
        .scenes
        .iter()
        .map(|(member, revision)| format!("      \"{member}\": \"{revision}\""))
        .collect();
    out.push_str(&members.join(",\n"));
    out.push_str("\n    },\n");
    out.push_str(&format!("    \"platform\": \"{}\",\n", where_.platform));
    out.push_str(&format!(
        "    \"architecture\": \"{}\",\n",
        where_.architecture
    ));
    out.push_str(&format!("    \"backend\": \"{}\",\n", where_.backend));
    out.push_str(&format!("    \"engine\": \"{}\",\n", where_.engine));
    out.push_str(&format!(
        "    \"viewport\": [{}, {}]",
        where_.viewport.0, where_.viewport.1
    ));
    match load {
        Some(load) => out.push_str(&format!(
            ",\n    \"load_per_core\": {:.4}\n  }},\n",
            load.per_core()
        )),
        None => out.push_str("\n  },\n"),
    }

    let figures: Vec<String> = run
        .figures()
        .iter()
        .map(|(name, figure)| format!("    \"{name}\": {:.4}", figure.value))
        .collect();
    out.push_str("  \"figures\": {\n");
    out.push_str(&figures.join(",\n"));
    out.push_str("\n  },\n");

    let skipped: Vec<String> = run
        .skips()
        .iter()
        .map(|(prefix, why)| format!("    \"{prefix}\": \"{}\"", why.reason()))
        .collect();
    out.push_str("  \"skipped\": {\n");
    out.push_str(&skipped.join(",\n"));
    if !skipped.is_empty() {
        out.push('\n');
    }
    out.push_str("  }\n}\n");
    out
}

pub fn read(path: &str) -> Result<Baseline> {
    parse(&std::fs::read_to_string(path)?)
}

fn parse(text: &str) -> Result<Baseline> {
    let root = Parser::new(text).document()?.into_object()?;
    let conditions = field(&root, "conditions")?.clone().into_object()?;
    let strings = |value: Value| -> Result<BTreeMap<String, String>> {
        value
            .into_object()?
            .into_iter()
            .map(|(key, value)| Ok((key, value.into_string()?)))
            .collect()
    };
    Ok(Baseline {
        // Absent in a baseline recorded before the suite had more than one
        // member. Left empty rather than refused here, so that the refusal
        // comes from the comparison with the rest of the mismatches and reads
        // as one.
        scenes: match conditions.get("scenes") {
            Some(value) => strings(value.clone())?,
            None => BTreeMap::new(),
        },
        platform: field(&conditions, "platform")?.clone().into_string()?,
        architecture: field(&conditions, "architecture")?.clone().into_string()?,
        backend: field(&conditions, "backend")?.clone().into_string()?,
        engine: field(&conditions, "engine")?.clone().into_string()?,
        load_per_core: conditions
            .get("load_per_core")
            .and_then(|v| v.clone().into_number().ok()),
        figures: field(&root, "figures")?
            .clone()
            .into_object()?
            .into_iter()
            .map(|(key, value)| Ok((key, value.into_number()?)))
            .collect::<Result<_>>()?,
        // Absent in a baseline recorded before skips were reported, which is
        // not an error: it means that run skipped nothing it told us about.
        skipped: match root.get("skipped") {
            Some(value) => strings(value.clone())?,
            None => BTreeMap::new(),
        },
    })
}

fn field<'a>(object: &'a BTreeMap<String, Value>, key: &str) -> Result<&'a Value> {
    object.get(key).ok_or_else(|| bad(format!("no {key}")))
}

fn bad(what: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidData, what.into())
}

/// Just enough JSON for this one file.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Str(String),
    Num(f64),
    Arr(Vec<Value>),
    Obj(BTreeMap<String, Value>),
}

impl Value {
    fn into_object(self) -> Result<BTreeMap<String, Value>> {
        match self {
            Self::Obj(map) => Ok(map),
            other => Err(bad(format!("expected an object, found {other:?}"))),
        }
    }

    fn into_string(self) -> Result<String> {
        match self {
            Self::Str(text) => Ok(text),
            other => Err(bad(format!("expected a string, found {other:?}"))),
        }
    }

    fn into_number(self) -> Result<f64> {
        match self {
            Self::Num(value) => Ok(value),
            other => Err(bad(format!("expected a number, found {other:?}"))),
        }
    }
}

struct Parser<'a> {
    rest: &'a str,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self { rest: text }
    }

    /// The whole file: one value, and nothing after it but space.
    fn document(mut self) -> Result<Value> {
        let value = self.value()?;
        self.space();
        if !self.rest.is_empty() {
            return Err(bad(format!("trailing text: {}", self.rest.trim())));
        }
        Ok(value)
    }

    fn space(&mut self) {
        self.rest = self.rest.trim_start();
    }

    fn peek(&mut self) -> Option<char> {
        self.space();
        self.rest.chars().next()
    }

    fn eat(&mut self, expected: char) -> Result<()> {
        match self.peek() {
            Some(found) if found == expected => {
                self.rest = &self.rest[expected.len_utf8()..];
                Ok(())
            }
            Some(found) => Err(bad(format!("expected {expected}, found {found}"))),
            None => Err(bad(format!("expected {expected}, found the end"))),
        }
    }

    fn value(&mut self) -> Result<Value> {
        match self.peek() {
            Some('{') => self.object().map(Value::Obj),
            Some('[') => self.array().map(Value::Arr),
            Some('"') => self.string().map(Value::Str),
            Some(_) => self.number().map(Value::Num),
            None => Err(bad("expected a value, found the end")),
        }
    }

    fn object(&mut self) -> Result<BTreeMap<String, Value>> {
        self.eat('{')?;
        let mut map = BTreeMap::new();
        if self.peek() == Some('}') {
            self.eat('}')?;
            return Ok(map);
        }
        loop {
            let key = self.string()?;
            self.eat(':')?;
            map.insert(key, self.value()?);
            if self.peek() == Some(',') {
                self.eat(',')?;
                continue;
            }
            self.eat('}')?;
            return Ok(map);
        }
    }

    fn array(&mut self) -> Result<Vec<Value>> {
        self.eat('[')?;
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.eat(']')?;
            return Ok(items);
        }
        loop {
            items.push(self.value()?);
            if self.peek() == Some(',') {
                self.eat(',')?;
                continue;
            }
            self.eat(']')?;
            return Ok(items);
        }
    }

    /// No escapes beyond `\"` and `\\`.
    ///
    /// Nothing this file holds needs them: figure names are identifiers and
    /// skip reasons are fixed prose. Anything else is refused rather than
    /// silently mangled.
    fn string(&mut self) -> Result<String> {
        self.eat('"')?;
        let mut out = String::new();
        let mut chars = self.rest.char_indices();
        while let Some((at, c)) = chars.next() {
            match c {
                '"' => {
                    self.rest = &self.rest[at + 1..];
                    return Ok(out);
                }
                '\\' => match chars.next() {
                    Some((_, escaped @ ('"' | '\\'))) => out.push(escaped),
                    Some((_, other)) => return Err(bad(format!("unsupported escape \\{other}"))),
                    None => return Err(bad("a string ends in a backslash")),
                },
                other => out.push(other),
            }
        }
        Err(bad("an unterminated string"))
    }

    fn number(&mut self) -> Result<f64> {
        self.space();
        let end = self
            .rest
            .find(|c: char| !matches!(c, '0'..='9' | '-' | '+' | '.' | 'e' | 'E'))
            .unwrap_or(self.rest.len());
        let (text, rest) = self.rest.split_at(end);
        self.rest = rest;
        text.parse()
            .map_err(|_| bad(format!("not a number: {text}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::Figure;
    use crate::skip::Skip;

    fn conditions() -> Conditions {
        Conditions {
            scenes: [("reference", "r1"), ("reference-10x", "r1")]
                .into_iter()
                .collect(),
            platform: "linux",
            architecture: "x86_64",
            backend: "cuda".into(),
            engine: "0.39.0".into(),
            viewport: (1280, 800),
        }
    }

    fn run() -> Run {
        let mut run = Run::new(None);
        run.insert("dab.median", Figure::ms(2.4219, Some(50.0)));
        run.insert("locality.key_ratio", Figure::ratio(0.75, Some(2.0), 1.5));
        run.skip("brush.mesh", Skip::NoHeadlessGpu);
        run
    }

    #[test]
    fn what_is_written_is_what_is_read() {
        let read = parse(&render(&conditions(), None, &run())).expect("parses");
        assert_eq!(read.scenes["reference"], "r1");
        assert_eq!(read.scenes["reference-10x"], "r1");
        assert_eq!(read.platform, "linux");
        assert_eq!(read.architecture, "x86_64");
        assert_eq!(read.backend, "cuda");
        assert_eq!(read.engine, "0.39.0");
        assert_eq!(read.figures["dab.median"], 2.4219);
        assert_eq!(read.figures["locality.key_ratio"], 0.75);
        assert_eq!(read.skipped["brush.mesh"], "no headless GPU");
    }

    #[test]
    fn a_recorded_load_survives_the_round_trip() {
        let load = Load {
            one_minute: 3.0,
            cores: 24,
        };
        let read = parse(&render(&conditions(), Some(&load), &run())).expect("parses");
        assert_eq!(read.load_per_core, Some(0.125));
    }

    #[test]
    fn a_baseline_without_a_load_says_so_rather_than_claiming_quiet() {
        // Every baseline recorded before this field existed. Reading those as
        // 0.0 would assert a quiet machine nobody measured.
        let read = parse(&render(&conditions(), None, &run())).expect("parses");
        assert_eq!(read.load_per_core, None);
    }

    #[test]
    fn a_run_that_skipped_nothing_reads_back_empty() {
        let mut run = Run::new(None);
        run.insert("dab.median", Figure::ms(1.0, None));
        let read = parse(&render(&conditions(), None, &run)).expect("parses");
        assert!(read.skipped.is_empty());
    }

    #[test]
    fn a_baseline_without_a_skipped_section_still_reads() {
        let text = r#"{
  "conditions": {
    "scenes": { "reference": "r1" },
    "platform": "linux",
    "architecture": "x86_64",
    "backend": "cpu",
    "engine": "0.39.0",
    "viewport": [1280, 800]
  },
  "figures": { "dab.median": 3.5 }
}"#;
        let read = parse(text).expect("parses");
        assert_eq!(read.figures["dab.median"], 3.5);
        assert!(read.skipped.is_empty());
    }

    #[test]
    fn a_baseline_from_before_the_suite_reads_with_no_scenes() {
        let text = r#"{
  "conditions": {
    "scene": "reference-r1",
    "platform": "linux",
    "architecture": "x86_64",
    "backend": "cuda",
    "engine": "0.39.0",
    "viewport": [1280, 800]
  },
  "figures": { "dab.median": 2.4219 }
}"#;
        let read = parse(text).expect("parses");
        assert!(read.scenes.is_empty());
    }

    #[test]
    fn a_truncated_file_is_an_error_rather_than_half_a_baseline() {
        let text = r#"{ "conditions": { "scenes": { "reference": "r1" "#;
        assert!(parse(text).is_err());
    }

    #[test]
    fn a_figure_that_is_not_a_number_is_an_error() {
        let text = r#"{
  "conditions": {
    "scenes": { "reference": "r1" },
    "platform": "linux",
    "architecture": "x86_64",
    "backend": "cpu",
    "engine": "0.39.0",
    "viewport": [1280, 800]
  },
  "figures": { "dab.median": "fast" }
}"#;
        assert!(parse(text).is_err());
    }
}
