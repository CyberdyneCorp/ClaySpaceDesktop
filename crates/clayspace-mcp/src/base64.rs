//! Standard base64, because an image content block is base64 and nothing else
//! in this workspace is.
//!
//! Thirty lines against RFC 4648's own vectors, rather than a dependency to
//! audit, license and carry in `ATTRIBUTION.md` for one encode per screenshot.
//! Encoding only: nothing here ever decodes one.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// RFC 4648 §4, with padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[(triple >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(triple >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(triple & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    /// RFC 4648 §10. The whole point of a hand-written encoder is that its
    /// vectors are published, so the test is the specification's own table.
    #[test]
    fn the_published_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn every_byte_of_the_alphabet_is_reachable() {
        // 0..=255 covers both non-alphanumeric output characters and the high
        // bits a signed byte would get wrong.
        let all: Vec<u8> = (0..=255u8).collect();
        let encoded = encode(&all);
        assert!(encoded.contains('+'), "{encoded}");
        assert!(encoded.contains('/'), "{encoded}");
        assert_eq!(encoded.len(), 344);
    }

    #[test]
    fn padding_follows_the_remainder() {
        assert!(encode(&[0u8; 3]).ends_with("AAAA"));
        assert!(encode(&[0u8; 4]).ends_with("=="));
        assert!(encode(&[0u8; 5]).ends_with('='));
    }
}
