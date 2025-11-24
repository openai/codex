/*!
Search for regex matches in `&[u8]` haystacks.

This module provides a nearly identical API via [`Regex`] to the one found in
the top-level of this crate. There are two important differences:

1. Matching is done on `&[u8]` instead of `&str`. Additionally, `Vec<u8>`
is used where `String` would have been used in the top-level API.
2. Unicode support can be disabled even when disabling it would result in
matching invalid UTF-8 bytes.

# Example: match null terminated string

This shows how to find all null-terminated strings in a slice of bytes. This
works even if a C string contains invalid UTF-8.

```rust
use regex::bytes::Regex;

let re = Regex::new(r"(?-u)(?<cstr>[^\x00]+)\x00").unwrap();
let hay = b"foo\x00qu\xFFux\x00baz\x00";

// Extract all of the strings without the NUL terminator from each match.
// The unwrap is OK here since a match requires the `cstr` capture to match.
let cstrs: Vec<&[u8]> =
    re.captures_iter(hay)
      .map(|c| c.name("cstr").unwrap().as_bytes())
      .collect();
assert_eq!(cstrs, vec![&b"foo"[..], &b"qu\xFFux"[..], &b"baz"[..]]);
```

# Example: selectively enable Unicode support

This shows how to match an arbitrary byte pattern followed by a UTF-8 encoded
string (e.g., to extract a title from a Matroska file):

```rust
use regex::bytes::Regex;

let re = Regex::new(
    r"(?-u)\x7b\xa9(?:[\x80-\xfe]|[\x40-\xff].)(?u:(.*))"
).unwrap();
let hay = b"\x12\xd0\x3b\x5f\x7b\xa9\x85\xe2\x98\x83\x80\x98\x54\x76\x68\x65";

// Notice that despite the `.*` at the end, it will only match valid UTF-8
// because Unicode mode was enabled with the `u` flag. Without the `u` flag,
// the `.*` would match the rest of the bytes regardless of whether they were
// valid UTF-8.
let (_, [title]) = re.captures(hay).unwrap().extract();
assert_eq!(title, b"\xE2\x98\x83");
// We can UTF-8 decode the title now. And the unwrap here
// is correct because the existence of a match guarantees
// that `title` is valid UTF-8.
let title = std::str::from_utf8(title).unwrap();
assert_eq!(title, "☃");
```

In general, if the Unicode flag is enabled in a capture group and that capture
is part of the overall match, then the capture is *guaranteed* to be valid
UTF-8.

# Syntax

The supported syntax is pretty much the same as the syntax for Unicode
regular expressions with a few changes that make sense for matching arbitrary
bytes:

1. The `u` flag can be disabled even when disabling it might cause the regex to
match invalid UTF-8. When the `u` flag is disabled, the regex is said to be in
"ASCII compatible" mode.
2. In ASCII compatible mode, Unicode character classes are not allowed. Literal
Unicode scalar values outside of character classes are allowed.
3. In ASCII compatible mode, Perl character classes (`\w`, `\d` and `\s`)
revert to their typical ASCII definition. `\w` maps to `[[:word:]]`, `\d` maps
to `[[:digit:]]` and `\s` maps to `[[:space:]]`.
4. In ASCII compatible mode, word boundaries use the ASCII compatible `\w` to
determine whether a byte is a word byte or not.
5. Hexadecimal notation can be used to specify arbitrary bytes instead of
Unicode codepoints. For example, in ASCII compatible mode, `\xFF` matches the
literal byte `\xFF`, while in Unicode mode, `\xFF` is the Unicode codepoint
`U+00FF` that matches its UTF-8 encoding of `\xC3\xBF`. Similarly for octal
notation when enabled.
6. In ASCII compatible mode, `.` matches any *byte* except for `\n`. When the
`s` flag is additionally enabled, `.` matches any byte.

# Performance

In general, one should expect performance on `&[u8]` to be roughly similar to
performance on `&str`.
*/
pub use crate::{builders::bytes::*, regex::bytes::*, regexset::bytes::*};
