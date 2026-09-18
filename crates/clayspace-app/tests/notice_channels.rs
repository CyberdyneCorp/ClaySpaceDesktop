//! Every channel a refusal can arrive on is one the door reads.
//!
//! Whether a command was refused is decided in one place: the composition root
//! samples a list of notice channels before the command and again after, and
//! answers `isError` when one of them was written. A ViewModel that owns a
//! notice channel and is *not* in that list therefore refuses into a void —
//! the sentence is written, the agent is told the command succeeded, and the
//! document is left exactly as it was.
//!
//! That is not a hypothetical. The cage, the curve, the boolean and the rig
//! each carried a notice nobody read: a boolean over a hierarchy answered
//! success and produced no layer, a scale the brick cache refused answered
//! success and left the readout showing a size that does not exist, and a
//! refused cage apply threw away every drag the sculptor had made without a
//! word. Each was found by hand, one at a time, months apart.
//!
//! So the list is checked against the ViewModels rather than trusted. This
//! reads the composition root's own `struct App` for the ViewModels it holds,
//! reads each of those ViewModels for the notice channels it owns, and fails
//! naming any channel the registry does not read. Adding a panel that can
//! refuse and forgetting to register it fails here, which is the only reason
//! the next one will not be found the way the last four were.
//!
//! Source text rather than types, because `App` lives in `main.rs` and a test
//! cannot construct one without a window and a GPU device — and the rule being
//! held is about a list a person maintains, which is a property of the text.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Accessors that carry a standing condition rather than an event.
///
/// `unavailable` is what greys a button out — "there is nothing here to bake"
/// — and it is written by every refresh of the panel, not by a command. The
/// same write that sets it also sets the panel's `notice`, so a door that read
/// both would answer one refused bake with the same sentence twice, and a
/// panel merely refreshing with nothing to do would answer a refusal to a
/// command that was never about it.
const NOT_AN_EVENT: &[&str] = &["unavailable"];

/// Where the registry begins and ends in the composition root.
///
/// `notice_occurrences` is the first thing after the two lists, and it reads
/// them rather than naming channels itself — so the slice between these is
/// exactly the registry and nothing else.
const REGISTRY_BEGINS: &str = "fn refusal_channels(";
const REGISTRY_ENDS: &str = "fn notice_occurrences(";

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `ObjectViewModel` as the module that holds it is spelled: `object_vm`.
fn module_of(kind: &str) -> String {
    let mut module = String::new();
    for (position, letter) in kind.chars().enumerate() {
        if letter.is_ascii_uppercase() && position > 0 {
            module.push('_');
        }
        module.extend(letter.to_lowercase());
    }
    module.push_str("_vm");
    module
}

/// The ViewModels the composition root holds, as the field it holds each in
/// and the module the ViewModel is written in.
///
/// Keyed on the *type* rather than the field, because the two differ where it
/// reads better — `references: ReferenceViewModel` — and it is the type that
/// says which file to look in.
fn view_models_of_the_shell(main: &str) -> Vec<(String, String)> {
    let body = main
        .split_once("\nstruct App {")
        .expect("the composition root's own struct")
        .1
        .split_once("\n}\n")
        .expect("the end of it")
        .0;

    let mut held = Vec::new();
    for line in body.lines() {
        let Some((field, kind)) = line.trim().split_once(": ") else {
            continue;
        };
        let named = |text: &str| {
            !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        let kind = kind.trim_end_matches(',');
        let Some(kind) = kind
            .rsplit("::")
            .next()
            .and_then(|k| k.strip_suffix("ViewModel"))
        else {
            continue;
        };
        if named(field) {
            held.push((field.to_string(), module_of(kind)));
        }
    }
    held
}

/// The notice channels one ViewModel owns, by the accessor that reaches them.
fn channels_of(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let (name, signature) = line.trim().strip_prefix("pub fn ")?.split_once('(')?;
            signature
                .contains("&Observable<Option<String>>")
                .then(|| name.to_string())
        })
        .filter(|name| !NOT_AN_EVENT.contains(&name.as_str()))
        .collect()
}

#[test]
fn every_notice_channel_is_read() {
    let main = read(&crate_root().join("src/main.rs"));
    let registry = main
        .split_once(REGISTRY_BEGINS)
        .expect("the refusal channel registry")
        .1
        .split_once(REGISTRY_ENDS)
        .expect("the end of the registry")
        .0;

    let held = view_models_of_the_shell(&main);
    assert!(
        held.len() > 10,
        "only {} ViewModels were found in `struct App`, so this test is \
         checking almost nothing — the struct was reshaped and the reading \
         above no longer finds its fields",
        held.len()
    );

    let mut unread = BTreeSet::new();
    for (field, module) in held {
        let source = crate_root().join(format!("../clayspace-vm/src/{module}.rs"));
        if !source.exists() {
            // A ViewModel written somewhere other than `clayspace-vm`. There
            // are none today; skipping rather than failing keeps this test
            // about unread channels instead of about where a file lives.
            continue;
        }
        for channel in channels_of(&read(&source)) {
            let call = format!("self.{field}.{channel}()");
            if !registry.contains(&call) {
                unread.insert(call);
            }
        }
    }

    assert!(
        unread.is_empty(),
        "these notice channels are written and never read, so the refusals on \
         them reach nobody — an agent is answered success and the sculptor is \
         shown nothing. Add each to `App::refusal_channels` (or, for something \
         that did happen, `App::remark_channels`): {unread:?}"
    );
}
