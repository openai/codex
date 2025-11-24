# unicode-truncate

Unicode-aware algorithm to pad or truncate `str` in terms of displayed width.

[![crates.io](https://img.shields.io/crates/v/unicode-truncate.svg)](https://crates.io/crates/unicode-truncate)
[![Documentation](https://docs.rs/unicode-truncate/badge.svg)](https://docs.rs/unicode-truncate)
[![Build Status](https://github.com/aetf/unicode-truncate/actions/workflows/rust.yml/badge.svg)](https://github.com/Aetf/unicode-truncate/actions)

## Examples

Safely truncate string to display width even not at character boundaries.

```rust
use unicode_truncate::UnicodeTruncateStr;

fn main() {
    assert_eq!("你好吗".unicode_truncate(5), ("你好", 4));
}
```

Making sure the string is displayed in exactly number of columns by combining padding and
truncating.

```rust
use unicode_truncate::UnicodeTruncateStr;
use unicode_truncate::Alignment;
use unicode_width::UnicodeWidthStr;

fn main() {
    let str = "你好吗".unicode_pad(5, Alignment::Left, true);
    assert_eq!(str, "你好 ");
    assert_eq!(str.width(), 5);
}
```

## Features

`unicode-truncate` can be built without `std` by disabling the default feature `std`. However, in
that case `unicode_truncate::UnicodeTruncateStr::unicode_pad` won't be available because it depends
on `std::string::String` and `std::borrow::Cow`.
