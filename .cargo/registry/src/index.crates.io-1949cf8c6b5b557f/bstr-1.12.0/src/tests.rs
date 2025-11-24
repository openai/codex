/// A sequence of tests for checking whether lossy decoding uses the maximal
/// subpart strategy correctly. Namely, if a sequence of otherwise invalid
/// UTF-8 bytes is a valid prefix of a valid UTF-8 sequence, then the entire
/// prefix is replaced by a single replacement codepoint. In all other cases,
/// each invalid byte is replaced by a single replacement codepoint.
///
/// The first element in each tuple is the expected result of lossy decoding,
/// while the second element is the input given.
pub(crate) const LOSSY_TESTS: &[(&str, &[u8])] = &[
    ("a", b"a"),
    ("\u{FFFD}", b"\xFF"),
    ("\u{FFFD}\u{FFFD}", b"\xFF\xFF"),
    ("β\u{FFFD}", b"\xCE\xB2\xFF"),
    ("☃\u{FFFD}", b"\xE2\x98\x83\xFF"),
    ("𝝱\u{FFFD}", b"\xF0\x9D\x9D\xB1\xFF"),
    ("\u{FFFD}\u{FFFD}", b"\xCE\xF0"),
    ("\u{FFFD}\u{FFFD}", b"\xCE\xFF"),
    ("\u{FFFD}\u{FFFD}", b"\xE2\x98\xF0"),
    ("\u{FFFD}\u{FFFD}", b"\xE2\x98\xFF"),
    ("\u{FFFD}", b"\xF0\x9D\x9D"),
    ("\u{FFFD}\u{FFFD}", b"\xF0\x9D\x9D\xF0"),
    ("\u{FFFD}\u{FFFD}", b"\xF0\x9D\x9D\xFF"),
    ("\u{FFFD}", b"\xCE"),
    ("a\u{FFFD}", b"a\xCE"),
    ("\u{FFFD}", b"\xE2\x98"),
    ("a\u{FFFD}", b"a\xE2\x98"),
    ("\u{FFFD}", b"\xF0\x9D\x9C"),
    ("a\u{FFFD}", b"a\xF0\x9D\x9C"),
    ("a\u{FFFD}\u{FFFD}\u{FFFD}z", b"a\xED\xA0\x80z"),
    ("☃βツ\u{FFFD}", b"\xe2\x98\x83\xce\xb2\xe3\x83\x84\xFF"),
    ("a\u{FFFD}\u{FFFD}\u{FFFD}b", b"\x61\xF1\x80\x80\xE1\x80\xC2\x62"),
];
