# toml_edit

[![Build Status](https://github.com/toml-rs/toml/workflows/Continuous%20integration/badge.svg)](https://github.com/toml-rs/toml/actions)
[![codecov](https://codecov.io/gh/toml-rs/toml/branch/master/graph/badge.svg)](https://codecov.io/gh/toml-rs/toml)
[![crates.io](https://img.shields.io/crates/v/toml_edit.svg)](https://crates.io/crates/toml_edit)
[![docs](https://docs.rs/toml_edit/badge.svg)](https://docs.rs/toml_edit)
[![Join the chat at https://gitter.im/toml_edit/Lobby](https://badges.gitter.im/a.svg)](https://gitter.im/toml_edit/Lobby)


This crate allows you to parse and modify toml
documents, while preserving comments, spaces *and
relative order* of items.

`toml_edit` is primarily tailored for [cargo-edit](https://github.com/killercup/cargo-edit/) needs.

## Example

```rust
use toml_edit::{DocumentMut, value};

fn main() {
    let toml = r#"
"hello" = 'toml!' # comment
['a'.b]
    "#;
    let mut doc = toml.parse::<DocumentMut>().expect("invalid doc");
    assert_eq!(doc.to_string(), toml);
    // let's add a new key/value pair inside a.b: c = {d = "hello"}
    doc["a"]["b"]["c"]["d"] = value("hello");
    // autoformat inline table a.b.c: { d = "hello" }
    doc["a"]["b"]["c"].as_inline_table_mut().map(|t| t.fmt());
    let expected = r#"
"hello" = 'toml!' # comment
['a'.b]
c = { d = "hello" }
    "#;
    assert_eq!(doc.to_string(), expected);
}
```

## Limitations

Things it does not preserve:

* Order of dotted keys, see [issue](https://github.com/toml-rs/toml/issues/163).

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms or
conditions.
