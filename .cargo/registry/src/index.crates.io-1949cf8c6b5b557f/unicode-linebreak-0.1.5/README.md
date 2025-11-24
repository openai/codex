# unicode-linebreak

Implementation of the Line Breaking Algorithm described in [Unicode Standard Annex #14][UAX14].

![test](https://github.com/axelf4/unicode-linebreak/workflows/test/badge.svg)
[![Documentation](https://docs.rs/unicode-linebreak/badge.svg)](https://docs.rs/unicode-linebreak)

Given an input text, locates "line break opportunities", or positions appropriate for wrapping
lines when displaying text.

## Example

```rust
use unicode_linebreak::{linebreaks, BreakOpportunity::{Mandatory, Allowed}};

let text = "a b \nc";
assert!(linebreaks(text).eq([
	(2, Allowed),   // May break after first space
	(5, Mandatory), // Must break after line feed
	(6, Mandatory)  // Must break at end of text, so that there always is at least one LB
]));
```

## Development

After cloning the repository or modifying `LineBreak.txt` the tables
have to be (re-)generated:

```sh
# Generate src/tables.rs
(cd gen-tables && cargo run)
# Run tests to make sure it was successful
cargo test
```

[UAX14]: https://www.unicode.org/reports/tr14/
