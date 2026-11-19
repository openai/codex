//! Locate large string values in a serialized tool observation without decoding or copying them.

use crate::RawPayloadKind;
use std::ops::Range;

pub(crate) fn large_string_values(body: &[u8]) -> Vec<(Range<usize>, RawPayloadKind)> {
    const MIN_BYTES: usize = 64 << 10;
    let mut parts = Vec::new();
    if body.len() < MIN_BYTES {
        return parts;
    }

    // The writer supplies valid JSON. A quoted token followed by a colon is an object key.
    // Take slices of the original encoding so escapes, duplicate keys and order survive.
    let mut bytes = body.iter().copied().enumerate();
    while let Some((start, _)) = bytes.find(|(_, byte)| *byte == b'"') {
        while let Some((index, byte)) = bytes.next() {
            match byte {
                b'\\' => {
                    bytes.next();
                }
                b'"' => {
                    let end = index + 1;
                    if end - start >= MIN_BYTES
                        && body[end..].iter().find(|b| !b.is_ascii_whitespace()) != Some(&b':')
                    {
                        parts.push((start..end, RawPayloadKind::ContentValue));
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    parts
}
