use core::cmp::Ordering;

/// Decodes the next UTF-8 encoded codepoint from the given byte slice.
///
/// If no valid encoding of a codepoint exists at the beginning of the given
/// byte slice, then the first byte is returned instead.
///
/// This returns `None` if and only if `bytes` is empty.
///
/// This never panics.
///
/// *WARNING*: This is not designed for performance. If you're looking for a
/// fast UTF-8 decoder, this is not it. If you feel like you need one in this
/// crate, then please file an issue and discuss your use case.
pub(crate) fn decode(bytes: &[u8]) -> Option<Result<char, u8>> {
    crate::shared::util::utf8::decode(bytes)
}

/// Like std's `eq_ignore_ascii_case`, but returns a full `Ordering`.
#[inline]
pub(crate) fn cmp_ignore_ascii_case(s1: &str, s2: &str) -> Ordering {
    cmp_ignore_ascii_case_bytes(s1.as_bytes(), s2.as_bytes())
}

/// Like std's `eq_ignore_ascii_case`, but returns a full `Ordering` on
/// `&[u8]`.
#[inline]
pub(crate) fn cmp_ignore_ascii_case_bytes(s1: &[u8], s2: &[u8]) -> Ordering {
    // This function used to look like this:
    //
    //     let it1 = s1.iter().map(|&b| b.to_ascii_lowercase());
    //     let it2 = s2.iter().map(|&b| b.to_ascii_lowercase());
    //     it1.cmp(it2)
    //
    // But the code below seems to do better in microbenchmarks.
    let mut i = 0;
    loop {
        let b1 = s1.get(i).copied().map(|b| b.to_ascii_lowercase());
        let b2 = s2.get(i).copied().map(|b| b.to_ascii_lowercase());
        match (b1, b2) {
            (None, None) => return Ordering::Equal,
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (Some(b1), Some(b2)) if b1 == b2 => i += 1,
            (Some(b1), Some(b2)) => return b1.cmp(&b2),
        }
    }
}
