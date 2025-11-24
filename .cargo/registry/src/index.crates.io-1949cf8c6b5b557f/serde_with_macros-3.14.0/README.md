# Custom de/serialization functions for Rust's [serde](https://serde.rs)

[![crates.io badge](https://img.shields.io/crates/v/serde_with.svg)](https://crates.io/crates/serde_with/)
[![Build Status](https://github.com/jonasbb/serde_with/actions/workflows/ci.yaml/badge.svg)](https://github.com/jonasbb/serde_with)
[![codecov](https://codecov.io/gh/jonasbb/serde_with/branch/master/graph/badge.svg)](https://codecov.io/gh/jonasbb/serde_with)
[![CII Best Practices](https://bestpractices.coreinfrastructure.org/projects/4322/badge)](https://bestpractices.coreinfrastructure.org/projects/4322)

---

This crate provides custom de/serialization helpers to use in combination with [serde's `with` annotation][with-annotation] and with the improved [`serde_as`][as-annotation]-annotation.
Some common use cases are:

* De/Serializing a type using the `Display` and `FromStr` traits, e.g., for `u8`, `url::Url`, or `mime::Mime`.
     Check [`DisplayFromStr`] for details.
* Support for arrays larger than 32 elements or using const generics.
    With `serde_as` large arrays are supported, even if they are nested in other types.
    `[bool; 64]`, `Option<[u8; M]>`, and `Box<[[u8; 64]; N]>` are all supported, as [this examples shows](#large-and-const-generic-arrays).
* Skip serializing all empty `Option` types with [`#[skip_serializing_none]`][skip_serializing_none].
* Apply a prefix / suffix to each field name of a struct, without changing the de/serialize implementations of the struct using [`with_prefix!`][] / [`with_suffix!`][].
* Deserialize a comma separated list like `#hash,#tags,#are,#great` into a `Vec<String>`.
     Check the documentation for [`serde_with::StringWithSeparator::<CommaSeparator, T>`][StringWithSeparator].

### Getting Help

**Check out the [user guide][user guide] to find out more tips and tricks about this crate.**

For further help using this crate, you can [open a new discussion](https://github.com/jonasbb/serde_with/discussions/new) or ask on [users.rust-lang.org](https://users.rust-lang.org/).
For bugs, please open a [new issue](https://github.com/jonasbb/serde_with/issues/new) on GitHub.

## Use `serde_with` in your Project

```bash
# Add the current version to your Cargo.toml
cargo add serde_with
```

The crate contains different features for integration with other common crates.
Check the [feature flags][] section for information about all available features.

## Examples

Annotate your struct or enum to enable the custom de/serializer.
The `#[serde_as]` attribute must be placed *before* the `#[derive]`.

The `as` is analogous to the `with` attribute of serde.
You mirror the type structure of the field you want to de/serialize.
You can specify converters for the inner types of a field, e.g., `Vec<DisplayFromStr>`.
The default de/serialization behavior can be restored by using `_` as a placeholder, e.g., `BTreeMap<_, DisplayFromStr>`.

### `DisplayFromStr`

```rust
#[serde_as]
#[derive(Deserialize, Serialize)]
struct Foo {
    // Serialize with Display, deserialize with FromStr
    #[serde_as(as = "DisplayFromStr")]
    bar: u8,
}

// This will serialize
Foo {bar: 12}

// into this JSON
{"bar": "12"}
```

### Large and const-generic arrays

serde does not support arrays with more than 32 elements or using const-generics.
The `serde_as` attribute allows circumventing this restriction, even for nested types and nested arrays.

On top of it, `[u8; N]` (aka, bytes) can use the specialized `"Bytes"` for efficiency much like the `serde_bytes` crate.

```rust
#[serde_as]
#[derive(Deserialize, Serialize)]
struct Arrays<const N: usize, const M: usize> {
    #[serde_as(as = "[_; N]")]
    constgeneric: [bool; N],

    #[serde_as(as = "Box<[[_; 64]; N]>")]
    nested: Box<[[u8; 64]; N]>,

    #[serde_as(as = "Option<[_; M]>")]
    optional: Option<[u8; M]>,

    #[serde_as(as = "Bytes")]
    bytes: [u8; M],
}

// This allows us to serialize a struct like this
let arrays: Arrays<100, 128> = Arrays {
    constgeneric: [true; 100],
    nested: Box::new([[111; 64]; 100]),
    optional: Some([222; 128]),
    bytes: [0x42; 128],
};
assert!(serde_json::to_string(&arrays).is_ok());
```

### `skip_serializing_none`

This situation often occurs with JSON, but other formats also support optional fields.
If many fields are optional, putting the annotations on the structs can become tedious.
The `#[skip_serializing_none]` attribute must be placed *before* the `#[derive]`.

```rust
#[skip_serializing_none]
#[derive(Deserialize, Serialize)]
struct Foo {
    a: Option<usize>,
    b: Option<usize>,
    c: Option<usize>,
    d: Option<usize>,
    e: Option<usize>,
    f: Option<usize>,
    g: Option<usize>,
}

// This will serialize
Foo {a: None, b: None, c: None, d: Some(4), e: None, f: None, g: Some(7)}

// into this JSON
{"d": 4, "g": 7}
```

### Advanced `serde_as` usage

This example is mainly supposed to highlight the flexibility of the `serde_as` annotation compared to [serde's `with` annotation][with-annotation].
More details about `serde_as` can be found in the [user guide].

```rust
use std::time::Duration;

#[serde_as]
#[derive(Deserialize, Serialize)]
enum Foo {
    Durations(
        // Serialize them into a list of number as seconds
        #[serde_as(as = "Vec<DurationSeconds>")]
        Vec<Duration>,
    ),
    Bytes {
        // We can treat a Vec like a map with duplicates.
        // JSON only allows string keys, so convert i32 to strings
        // The bytes will be hex encoded
        #[serde_as(as = "Map<DisplayFromStr, Hex>")]
        bytes: Vec<(i32, Vec<u8>)>,
    }
}

// This will serialize
Foo::Durations(
    vec![Duration::new(5, 0), Duration::new(3600, 0), Duration::new(0, 0)]
)
// into this JSON
{
    "Durations": [5, 3600, 0]
}

// and serializes
Foo::Bytes {
    bytes: vec![
        (1, vec![0, 1, 2]),
        (-100, vec![100, 200, 255]),
        (1, vec![0, 111, 222]),
    ],
}
// into this JSON
{
    "Bytes": {
        "bytes": {
            "1": "000102",
            "-100": "64c8ff",
            "1": "006fde"
        }
    }
}
```

[`DisplayFromStr`]: https://docs.rs/serde_with/3.14.0/serde_with/struct.DisplayFromStr.html
[`with_prefix!`]: https://docs.rs/serde_with/3.14.0/serde_with/macro.with_prefix.html
[`with_suffix!`]: https://docs.rs/serde_with/3.14.0/serde_with/macro.with_suffix.html
[feature flags]: https://docs.rs/serde_with/3.14.0/serde_with/guide/feature_flags/index.html
[skip_serializing_none]: https://docs.rs/serde_with/3.14.0/serde_with/attr.skip_serializing_none.html
[StringWithSeparator]: https://docs.rs/serde_with/3.14.0/serde_with/struct.StringWithSeparator.html
[user guide]: https://docs.rs/serde_with/3.14.0/serde_with/guide/index.html
[with-annotation]: https://serde.rs/field-attrs.html#with
[as-annotation]: https://docs.rs/serde_with/3.14.0/serde_with/guide/serde_as/index.html

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

For detailed contribution instructions please read [`CONTRIBUTING.md`].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual-licensed as above, without any additional terms or conditions.

[`CONTRIBUTING.md`]: https://github.com/jonasbb/serde_with/blob/master/CONTRIBUTING.md
