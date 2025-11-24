# tinystr [![crates.io](https://img.shields.io/crates/v/tinystr)](https://crates.io/crates/tinystr)

<!-- cargo-rdme start -->

`tinystr` is a utility crate of the [`ICU4X`] project.

It includes [`TinyAsciiStr`], a core API for representing small ASCII-only bounded length strings.

It is optimized for operations on strings of size 8 or smaller. When use cases involve comparison
and conversion of strings for lowercase/uppercase/titlecase, or checking
numeric/alphabetic/alphanumeric, `TinyAsciiStr` is the edge performance library.

## Examples

```rust
use tinystr::TinyAsciiStr;

let s1: TinyAsciiStr<4> = "tEsT".parse().expect("Failed to parse.");

assert_eq!(s1, "tEsT");
assert_eq!(s1.to_ascii_uppercase(), "TEST");
assert_eq!(s1.to_ascii_lowercase(), "test");
assert_eq!(s1.to_ascii_titlecase(), "Test");
assert!(s1.is_ascii_alphanumeric());
assert!(!s1.is_ascii_numeric());

let s2 = TinyAsciiStr::<8>::try_from_raw(*b"New York")
    .expect("Failed to parse.");

assert_eq!(s2, "New York");
assert_eq!(s2.to_ascii_uppercase(), "NEW YORK");
assert_eq!(s2.to_ascii_lowercase(), "new york");
assert_eq!(s2.to_ascii_titlecase(), "New york");
assert!(!s2.is_ascii_alphanumeric());
```

## Details

When strings are of size 8 or smaller, the struct transforms the strings as `u32`/`u64` and uses
bitmasking to provide basic string manipulation operations:
* `is_ascii_numeric`
* `is_ascii_alphabetic`
* `is_ascii_alphanumeric`
* `to_ascii_lowercase`
* `to_ascii_uppercase`
* `to_ascii_titlecase`
* `PartialEq`

`TinyAsciiStr` will fall back to `u8` character manipulation for strings of length greater than 8.

[`ICU4X`]: ../icu/index.html

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
