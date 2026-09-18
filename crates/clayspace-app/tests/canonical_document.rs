//! The canonical document records the ask, not what the engine computed.
//!
//! CI answers that by authoring the fixture on macOS and on Linux and
//! comparing digests, which is the only way to catch a value that differs
//! *between* toolchains — and it is also a signal that arrives once both legs
//! of a matrix have run, naming two hashes and nothing about which byte moved.
//!
//! This is the same property asked on one machine. A verb that computes where
//! it acts — Inflar sinking its dab centre along `clay_eval_gradients` before
//! the engine records it, which is what parted the digests on #229 — drops the
//! position it was *asked* for out of the file. So: every position the fixture
//! stamps has to be in the bytes verbatim, as the three little-endian floats
//! it was given. If one is not, the document is carrying something the engine
//! worked out, and that is the thing the matrix is about to disagree over.

#![forbid(unsafe_code)]

use clayspace_app::canonical;

/// Where the three floats of `at` sit in `bytes`, if they sit anywhere.
fn find(bytes: &[u8], at: [f32; 3]) -> Option<usize> {
    let needle: Vec<u8> = at.iter().flat_map(|c| c.to_le_bytes()).collect();
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
}

#[test]
fn every_stamped_position_is_in_the_file_verbatim() {
    let path = std::env::temp_dir().join(format!(
        "clayspace-canonical-{}.clayspace",
        std::process::id()
    ));
    canonical::write(&path).expect("the canonical document is authored and saved");
    let bytes = std::fs::read(&path).expect("the saved document reads back");
    let _ = std::fs::remove_file(&path);

    assert!(
        bytes.starts_with(b"CLAY"),
        "saved {} bytes and they are not a document",
        bytes.len()
    );
    for at in canonical::RECORDED {
        assert!(
            find(&bytes, at).is_some(),
            "{at:?} is not in the canonical document. The verb stamped there \
             records something it computed rather than what it was asked for, \
             and a computed float is not the same on every toolchain — see \
             `clayspace_app::canonical` for why that parts the digests."
        );
    }
}
