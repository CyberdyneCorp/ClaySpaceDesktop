//! JSON, written by hand — but written once.
//!
//! There is no serialiser in this project's dependency graph, and
//! `bench/json.rs` states why: *"a serialiser in the dependency graph is a
//! thing the audit has to consider forever for one file."* That file is now
//! two, which is the moment the reasoning stops being an argument for
//! `format!` at every call site and starts being an argument for this.
//!
//! What it buys is that well-formedness is a property of **the writer** rather
//! than of every place that writes. Commas, nesting and escaping are the three
//! things a hand-rendered document gets wrong, and none of the three is
//! reachable from outside here.
//!
//! Deliberately small, and deliberately write-only. Nothing in this
//! application parses what it writes, and a parser added to assert the absence
//! of a serialiser would be a joke at the audit's expense.

/// One open container, and whether it already holds a member.
#[derive(Debug, Clone, Copy)]
struct Container {
    closer: char,
    populated: bool,
}

/// A JSON document under construction.
///
/// Containers are opened and closed; the writer tracks the separator each one
/// owes, so a caller writes values and never punctuation.
#[derive(Debug)]
pub struct Json {
    out: String,
    open: Vec<Container>,
}

impl Default for Json {
    fn default() -> Self {
        Self::new()
    }
}

impl Json {
    /// A new document, with its outermost object already open.
    pub fn new() -> Self {
        let mut json = Self {
            out: String::new(),
            open: Vec::new(),
        };
        json.push_container('{');
        json
    }

    /// The document, with every container this writer opened closed.
    pub fn finish(mut self) -> String {
        while !self.open.is_empty() {
            self.end();
        }
        self.out.push('\n');
        self.out
    }

    /// How deeply nested the writer is. For tests, and for the debug assertion
    /// a caller may want that its sections balance.
    pub fn depth(&self) -> usize {
        self.open.len()
    }

    pub fn object(&mut self, key: &str) {
        self.key(key);
        self.push_container('{');
    }

    pub fn array(&mut self, key: &str) {
        self.key(key);
        self.push_container('[');
    }

    /// An object with no key of its own, for an array's elements.
    pub fn element(&mut self) {
        self.separate();
        self.push_container('{');
    }

    pub fn end(&mut self) {
        let Some(container) = self.open.pop() else {
            return;
        };
        if container.populated {
            self.out.push('\n');
            self.indent();
        }
        self.out.push(container.closer);
    }

    pub fn string(&mut self, key: &str, value: &str) {
        self.key(key);
        self.quoted(value);
    }

    /// A string, or `null` where there is nothing to say.
    ///
    /// `null` rather than an empty string, for the reason the whole file
    /// exists: *not known* and *known to be empty* are different claims and a
    /// reader must be able to tell them apart.
    pub fn maybe_string(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(value) => self.string(key, value),
            None => self.null(key),
        }
    }

    /// A measured quantity.
    ///
    /// Three decimals: these are milliseconds and nanoseconds, and a
    /// full-precision float invites a reader to treat a measurement as an
    /// equality.
    pub fn number(&mut self, key: &str, value: f64) {
        self.key(key);
        self.out.push_str(&format!("{value:.3}"));
    }

    /// A number, or `null` where the figure was never measured.
    ///
    /// Never a zero in that case. A backend that was not timed and a backend
    /// that costs nothing are different claims, and only one of them is ever
    /// true.
    pub fn maybe_number(&mut self, key: &str, value: Option<f64>) {
        match value {
            Some(value) => self.number(key, value),
            None => self.null(key),
        }
    }

    pub fn integer(&mut self, key: &str, value: u64) {
        self.key(key);
        self.out.push_str(&value.to_string());
    }

    pub fn boolean(&mut self, key: &str, value: bool) {
        self.key(key);
        self.out.push_str(if value { "true" } else { "false" });
    }

    pub fn null(&mut self, key: &str) {
        self.key(key);
        self.out.push_str("null");
    }

    /// A bare string, for an array of them.
    pub fn item(&mut self, value: &str) {
        self.separate();
        self.quoted(value);
    }

    fn key(&mut self, key: &str) {
        self.separate();
        self.quoted(key);
        self.out.push_str(": ");
    }

    fn push_container(&mut self, opener: char) {
        self.out.push(opener);
        self.open.push(Container {
            closer: if opener == '{' { '}' } else { ']' },
            populated: false,
        });
    }

    /// Writes the comma and the newline a new member owes.
    fn separate(&mut self) {
        if let Some(container) = self.open.last_mut() {
            if container.populated {
                self.out.push(',');
            }
            container.populated = true;
        }
        self.out.push('\n');
        self.indent();
    }

    fn indent(&mut self) {
        for _ in 0..self.open.len() {
            self.out.push_str("  ");
        }
    }

    /// A JSON string literal, escaped.
    ///
    /// The escapes JSON requires and no more: a quote, a backslash, and every
    /// control character below a space. A tool name or a backend name has no
    /// business carrying any of them, which is precisely why escaping them
    /// here rather than trusting that is the point.
    fn quoted(&mut self, value: &str) {
        self.out.push('"');
        for character in value.chars() {
            match character {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                c if (c as u32) < 0x20 => self.out.push_str(&format!("\\u{:04x}", c as u32)),
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_document_is_an_empty_object() {
        assert_eq!(Json::new().finish().trim(), "{}");
    }

    #[test]
    fn members_are_separated_and_containers_are_closed() {
        let mut json = Json::new();
        json.string("platform", "linux");
        json.array("backends");
        json.item("cpu");
        json.item("cuda");
        json.end();
        json.object("refill");
        json.maybe_number("cuda", None);
        json.end();
        let text = json.finish();

        assert!(text.contains("\"platform\": \"linux\""), "{text}");
        assert!(
            text.contains("\"cpu\",") && text.contains("\"cuda\""),
            "{text}"
        );
        assert!(text.contains("\"cuda\": null"), "{text}");
        assert_eq!(
            text.matches('{').count(),
            text.matches('}').count(),
            "unbalanced objects:\n{text}"
        );
        assert_eq!(
            text.matches('[').count(),
            text.matches(']').count(),
            "unbalanced arrays:\n{text}"
        );
    }

    #[test]
    fn a_container_left_open_is_closed_by_finishing() {
        // A caller that returns early must not be able to produce a document
        // that will not parse.
        let mut json = Json::new();
        json.array("tools");
        json.element();
        json.string("tool", "Padrão");
        assert_eq!(json.depth(), 3);
        let text = json.finish();
        assert!(text.trim_end().ends_with('}'), "{text}");
        assert_eq!(text.matches('[').count(), text.matches(']').count());
    }

    #[test]
    fn everything_json_forbids_in_a_string_is_escaped() {
        let mut json = Json::new();
        json.string("odd", "a \"quote\", a \\ and a \u{1}");
        let text = json.finish();
        // The control character must come out as its \u escape, not as itself:
        // a raw one in a JSON string is what makes a document unparseable.
        assert!(text.contains(r#"a \"quote\", a \\ and a \u0001"#), "{text}");
    }

    #[test]
    fn a_newline_never_reaches_the_document_raw() {
        let mut json = Json::new();
        json.string("note", "one\ntwo");
        let text = json.finish();
        assert!(text.contains(r"one\ntwo"), "{text}");
        // The only newlines are the writer's own formatting.
        assert_eq!(text.lines().count(), 3, "{text}");
    }

    #[test]
    fn a_measurement_is_not_written_to_full_precision() {
        let mut json = Json::new();
        json.number("ms", 1.0 / 3.0);
        assert!(json.finish().contains("0.333"));
    }
}
