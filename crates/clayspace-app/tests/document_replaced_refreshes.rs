//! Every view model that *can* be refreshed *is*, when the document changes.
//!
//! Reported from a session: after using the deformation cage or a tube along a
//! curve, choosing **New** left both still up — the cage raised, the curve's
//! points and settings intact — over a document that had never had either.
//! `LatticeViewModel::refresh` and `CurveViewModel::refresh` both existed and
//! did exactly the right thing. Neither was called.
//!
//! So the defect is an **omission from a list**, and the list is unusual in
//! that nothing reads it: `after_document_replaced` names seven view models by
//! hand and there were ten that could have been named. A test that drove the
//! cage and asserted it came down would prove the view model works, which was
//! never in doubt.
//!
//! This asks the structural question instead — which is the same instrument
//! `tools/check_layering.py` points at the crate graph, and the same shape as
//! ClayCore's own device-coverage and shard checks. A view model that grows a
//! `refresh` and is not wired here fails on the row that was added.
//!
//! `App` lives in the binary rather than the library, so this reads the source
//! rather than calling the function. That is a real weakness — a rename would
//! confuse it — and it is still worth more than a test that cannot fail.

use std::collections::{BTreeMap, BTreeSet};

/// The body of `App::after_document_replaced`, as text.
fn after_document_replaced() -> String {
    let source = std::fs::read_to_string("src/main.rs").expect("the app's own source");
    let start = source
        .find("fn after_document_replaced(&mut self)")
        .expect("after_document_replaced is where a replaced document is settled");
    // To the next function at the same indentation, which is where the body
    // ends. Naive, and it fails loudly rather than silently: an empty body
    // would fail every row below.
    let rest = &source[start..];
    let end = rest[1..]
        .find("\n    fn ")
        .or_else(|| rest[1..].find("\n    pub fn "))
        .map(|at| at + 1)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Which view-model type each `App` field holds.
fn view_model_fields() -> BTreeMap<String, String> {
    let source = std::fs::read_to_string("src/main.rs").expect("the app's own source");
    let start = source
        .find("struct App {")
        .expect("App is the composition root");
    let body = &source[start..start + source[start..].find("\n}").expect("App ends")];
    body.lines()
        .filter_map(|line| {
            let line = line.trim().trim_end_matches(',');
            let (name, kind) = line.split_once(": ")?;
            let kind = kind.rsplit("::").next()?;
            kind.ends_with("ViewModel")
                .then(|| (kind.to_string(), name.to_string()))
        })
        .collect()
}

/// The view models that offer a plain `refresh`, from the crate itself.
fn refreshable() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let dir = std::path::Path::new("../clayspace-vm/src");
    for entry in std::fs::read_dir(dir).expect("the view-model crate") {
        let path = entry.expect("a directory entry").path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(name) = stem.strip_suffix("_vm") else {
            continue;
        };
        let source = std::fs::read_to_string(&path).expect("a view model's source");
        if source.contains("pub fn refresh(&mut self)") {
            // `object_vm.rs` holds `ObjectViewModel`: the file is named for
            // the noun and the type for the same noun, so the mapping is the
            // stem in camel case rather than anything to look up.
            let camel: String = name
                .split('_')
                .map(|part| {
                    let mut chars = part.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                        None => String::new(),
                    }
                })
                .collect();
            found.insert(format!("{camel}ViewModel"));
        }
    }
    assert!(
        found.len() >= 6,
        "only {} refreshable view models were found, so the scan is broken \
         rather than the wiring",
        found.len()
    );
    found
}

/// A view model whose refresh deliberately does not belong here, and why.
///
/// Empty, and kept as the place a deliberate exclusion is argued rather than
/// left as a gap someone fills silently.
const NOT_ON_A_REPLACED_DOCUMENT: &[(&str, &str)] = &[];

#[test]
fn every_refreshable_view_model_is_refreshed_when_the_document_is_replaced() {
    let body = after_document_replaced();
    let fields = view_model_fields();
    let mut missed = Vec::new();

    for kind in refreshable() {
        let Some(field) = fields.get(&kind) else {
            // Held by something other than `App`, so this is not the list it
            // belongs to.
            continue;
        };
        if let Some((_, why)) = NOT_ON_A_REPLACED_DOCUMENT.iter().find(|(k, _)| *k == kind) {
            println!("  {kind} deliberately not refreshed: {why}");
            continue;
        }
        // Either the plain refresh or a named one — `sculpt` has
        // `refresh_after_open`, which is its version of the same thing.
        let called = body.contains(&format!("self.{field}.refresh()"))
            || body.contains(&format!("self.{field}.refresh_"));
        if !called {
            missed.push(format!("  {kind:<22} as self.{field}"));
        }
    }

    assert!(
        missed.is_empty(),
        "these view models offer a refresh and are not refreshed when the \
         document is replaced, so they keep describing the document that was \
         thrown away:\n{}\n\nAdd the call to `after_document_replaced`, or add \
         it to NOT_ON_A_REPLACED_DOCUMENT with the reason.",
        missed.join("\n")
    );
}
