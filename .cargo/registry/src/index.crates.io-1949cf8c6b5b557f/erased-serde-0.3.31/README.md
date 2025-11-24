Erased Serde
============

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/erased--serde-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/erased-serde)
[<img alt="crates.io" src="https://img.shields.io/crates/v/erased-serde.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/erased-serde)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-erased--serde-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/erased-serde)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/erased-serde/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/erased-serde/actions?query=branch%3Amaster)

This crate provides type-erased versions of Serde's `Serialize`, `Serializer`
and `Deserializer` traits that can be used as [trait objects].

[trait objects]: https://doc.rust-lang.org/book/first-edition/trait-objects.html

- [`erased_serde::Serialize`](https://docs.rs/erased-serde/0.3/erased_serde/trait.Serialize.html)
- [`erased_serde::Serializer`](https://docs.rs/erased-serde/0.3/erased_serde/trait.Serializer.html)
- [`erased_serde::Deserializer`](https://docs.rs/erased-serde/0.3/erased_serde/trait.Deserializer.html)

The usual Serde `Serialize`, `Serializer` and `Deserializer` traits cannot be
used as trait objects like `&dyn Serialize` or boxed trait objects like
`Box<dyn Serialize>` because of Rust's ["object safety" rules]. In particular,
all three traits contain generic methods which cannot be made into a trait
object.

["object safety" rules]: http://huonw.github.io/blog/2015/01/object-safety/

This library should be considered a low-level building block for interacting
with Serde APIs in an object-safe way. Most use cases will require higher level
functionality such as provided by [`typetag`] which uses this crate internally.

[`typetag`]: https://github.com/dtolnay/typetag

**The traits in this crate work seamlessly with any existing Serde `Serialize`
and `Deserialize` type and any existing Serde `Serializer` and `Deserializer`
format.**

```toml
[dependencies]
serde = "1.0"
erased-serde = "0.3"
```

## Serialization

```rust
use erased_serde::{Serialize, Serializer};
use std::collections::BTreeMap as Map;
use std::io;

fn main() {
    // Construct some serializers.
    let json = &mut serde_json::Serializer::new(io::stdout());
    let cbor = &mut serde_cbor::Serializer::new(serde_cbor::ser::IoWrite::new(io::stdout()));

    // The values in this map are boxed trait objects. Ordinarily this would not
    // be possible with serde::Serializer because of object safety, but type
    // erasure makes it possible with erased_serde::Serializer.
    let mut formats: Map<&str, Box<dyn Serializer>> = Map::new();
    formats.insert("json", Box::new(<dyn Serializer>::erase(json)));
    formats.insert("cbor", Box::new(<dyn Serializer>::erase(cbor)));

    // These are boxed trait objects as well. Same thing here - type erasure
    // makes this possible.
    let mut values: Map<&str, Box<dyn Serialize>> = Map::new();
    values.insert("vec", Box::new(vec!["a", "b"]));
    values.insert("int", Box::new(65536));

    // Pick a Serializer out of the formats map.
    let format = formats.get_mut("json").unwrap();

    // Pick a Serialize out of the values map.
    let value = values.get("vec").unwrap();

    // This line prints `["a","b"]` to stdout.
    value.erased_serialize(format).unwrap();
}
```

## Deserialization

```rust
use erased_serde::Deserializer;
use std::collections::BTreeMap as Map;

fn main() {
    static JSON: &[u8] = br#"{"A": 65, "B": 66}"#;
    static CBOR: &[u8] = &[162, 97, 65, 24, 65, 97, 66, 24, 66];

    // Construct some deserializers.
    let json = &mut serde_json::Deserializer::from_slice(JSON);
    let cbor = &mut serde_cbor::Deserializer::from_slice(CBOR);

    // The values in this map are boxed trait objects, which is not possible
    // with the normal serde::Deserializer because of object safety.
    let mut formats: Map<&str, Box<dyn Deserializer>> = Map::new();
    formats.insert("json", Box::new(<dyn Deserializer>::erase(json)));
    formats.insert("cbor", Box::new(<dyn Deserializer>::erase(cbor)));

    // Pick a Deserializer out of the formats map.
    let format = formats.get_mut("json").unwrap();

    let data: Map<String, usize> = erased_serde::deserialize(format).unwrap();

    println!("{}", data["A"] + data["B"]);
}
```

## How it works

This crate is based on a general technique for building trait objects of traits
that have generic methods (like all of Serde's traits). [This example code]
demonstrates the technique applied to a simplified case of a single generic
method. [Try it in the playground.]

[This example code]: https://github.com/dtolnay/erased-serde/blob/master/explanation/main.rs
[Try it in the playground.]: https://play.rust-lang.org/?gist=c1111875e7462ba3d0190aacb2fc2211

In erased-serde things are a bit more complicated than in the example for three
reasons but the idea is the same.

- We need to deal with trait methods that take `self` by value -- effectively by
  implementing the object-safe trait for `Option<T>` where `T` implements the
  real trait.
- We need to deal with traits that have associated types like `Serializer::Ok`
  and `Visitor::Value` -- by carefully short-term stashing things behind a
  pointer.
- We need to support trait methods that have a generic type in the return type
  but none of the argument types, like `SeqAccess::next_element` -- this can be
  flipped around into a callback style where the return value is instead passed
  on to a generic argument.

In the future maybe the Rust compiler will be able to apply this technique
automatically to any trait that is not already object safe by the current rules.

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
