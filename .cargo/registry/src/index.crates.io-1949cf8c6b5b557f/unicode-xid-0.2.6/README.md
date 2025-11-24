# unicode-xid

Determine if a `char` is a valid identifier for a parser and/or lexer according to
[Unicode Standard Annex #31](https://www.unicode.org/reports/tr31/) rules.

[![Build Status](https://img.shields.io/github/actions/workflow/status/unicode-rs/unicode-xid/ci.yml?branch=master)](https://github.com/unicode-rs/unicode-xid/actions?query=branch%3Amaster)

[Documentation](https://unicode-rs.github.io/unicode-xid/unicode_xid/index.html)

```rust
extern crate unicode_xid;

use unicode_xid::UnicodeXID;

fn main() {
    let ch = 'a';
    println!("Is {} a valid start of an identifier? {}", ch, UnicodeXID::is_xid_start(ch));
}
```

# features

unicode-xid supports a `no_std` feature. This eliminates dependence
on std, and instead uses equivalent functions from core.


# changelog

## 0.2.6

- Update to Unicode 16.0.0.

## 0.2.5

- Update to Unicode 15.1.0.

## 0.2.4

- Update to Unicode 15.0.0.
- Replace `const` tables with `static` tables.

## 0.2.3

- Update to Unicode 14.0.0.

## 0.2.2

- Add an ASCII fast-path.

## 0.2.1

- Update to Unicode 13.0.0.
- Speed up lookup.

## 0.2.0

- Update to Unicode 12.1.0.

## 0.1.0

- Initial release.
