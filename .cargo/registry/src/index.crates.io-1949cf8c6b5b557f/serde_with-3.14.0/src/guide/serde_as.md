# `serde_as` Annotation

This is an alternative to serde's `with` annotation.
It is more flexible and composable, but works with fewer types.

The scheme is based on two new traits, [`SerializeAs`] and [`DeserializeAs`], which need to be implemented by all types which want to be compatible with `serde_as`.
The proc-macro attribute [`#[serde_as]`][crate::serde_as] exists as a usability boost for users.
The basic design of `serde_as` was developed by [@markazmierczak](https://github.com/markazmierczak).

This page contains some general advice on the usage of `serde_as` and on implementing the necessary traits.  
[**A list of all supported transformations enabled by `serde_as` is available on this page.**](crate::guide::serde_as_transformations)

1. [Switching from serde's with to `serde_as`](#switching-from-serdes-with-to-serde_as)
    1. [Deserializing Optional Fields](#deserializing-optional-fields)
    2. [Gating `serde_as` on Features](#gating-serde_as-on-features)
2. [Implementing `SerializeAs` / `DeserializeAs`](#implementing-serializeas--deserializeas)
    1. [Using `#[serde_as]` on types without `SerializeAs` and `Serialize` implementations](#using-serde_as-on-types-without-serializeas-and-serialize-implementations)
    2. [Using `#[serde_as]` with serde's remote derives](#using-serde_as-with-serdes-remote-derives)
3. [Re-exporting `serde_as`](#re-exporting-serde_as)

## Switching from serde's with to `serde_as`

For the user, the main difference is that instead of

```rust,ignore
#[serde(with = "...")]
```

you now have to write

```rust,ignore
#[serde_as(as = "...")]
```

and place the `#[serde_as]` attribute *before* the `#[derive]` attribute.
You still need the `#[derive(Serialize, Deserialize)]` on the struct/enum.
You mirror the type structure of the field you want to de/serialize.
You can specify converters for the inner types of a field, e.g., `Vec<DisplayFromStr>`.
The default de/serialization behavior can be restored by using `_` as a placeholder, e.g., `BTreeMap<_, DisplayFromStr>`.

Combined, this looks like:

```rust
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

# #[allow(dead_code)]
#[serde_as]
#[derive(Serialize, Deserialize)]
struct A {
    #[serde_as(as = "DisplayFromStr")]
    mime: mime::Mime,
}
```

The main advantage is that you can compose `serde_as` stuff, which is impossible with the `with` annotation.
For example, the `mime` field from above could be nested in one or more data structures:

```rust
# use std::collections::BTreeMap;
# use serde::{Deserialize, Serialize};
# use serde_with::{serde_as, DisplayFromStr};
#
# #[allow(dead_code)]
#[serde_as]
#[derive(Serialize, Deserialize)]
struct A {
    #[serde_as(as = "Option<BTreeMap<_, Vec<DisplayFromStr>>>")]
    mime: Option<BTreeMap<String, Vec<mime::Mime>>>,
}
```

### Deserializing Optional Fields

In many cases, using `serde_as` on a field of type `Option` should behave as expected.
This means the field can still be missing during deserialization and will be filled with the value `None`.

This "magic" can break in some cases. Then it becomes necessary to apply `#[serde(default)]` on the field in question.
If the field is of type `Option<T>` and the conversion type is of `Option<S>`, the default attribute is automatically applied.
These variants are detected as `Option`.

* `Option`
* `std::option::Option`, with or without leading `::`
* `core::option::Option`, with or without leading `::`

Any renaming will interfere with the detection, such as `use std::option::Option as StdOption;`.
For more information, you can inspect the documentation of the `serde_as` macro.

```rust
# use serde::{Deserialize, Serialize};
# use serde_with::{serde_as, DisplayFromStr};
#
# #[allow(dead_code)]
#[serde_as]
#[derive(Serialize, Deserialize)]
struct A {
    #[serde_as(as = "Option<DisplayFromStr>")]
    // In this situation both `Option`s will be correctly identified and
    // `#[serde(default)]` will be applied on this field.
    val: Option<u32>,
}
```

In the future, this behavior might change and `default` would be applied on `Option<T>` fields.
You can add your feedback at [serde_with#185].

### Gating `serde_as` on Features

Gating `serde_as` behind optional features is possible using the `cfg_eval` attribute.
The attribute is available via the [`cfg_eval`-crate](https://docs.rs/cfg_eval) on stable or using the [Rust attribute](https://doc.rust-lang.org/1.70.0/core/prelude/v1/attr.cfg_eval.html) on unstable nightly.

The `cfg_eval` attribute must be placed **before** the struct-level `serde_as` attribute.
You can combine them in a single `cfg_attr`, as long as the order is preserved.

```rust,ignore
#[cfg_attr(feature="serde", cfg_eval::cfg_eval, serde_as)]
#[cfg_attr(feature="serde", derive(Serialize, Deserialize))]
struct Struct {
    #[cfg_attr(feature="serde", serde_as(as = "Vec<(_, _)>"))]
    map: HashMap<(i32,i32), i32>,
}
```

## Implementing `SerializeAs` / `DeserializeAs`

You can support [`SerializeAs`] / [`DeserializeAs`] on your own types too.
Most "leaf" types do not need to implement these traits, since they are supported implicitly.
"Leaf" types refer to types which directly serialize, like plain data types.
[`SerializeAs`] / [`DeserializeAs`] is essential for collection types, like `Vec` or `BTreeMap`, since they need special handling for the key/value de/serialization such that the conversions can be done on the key/values.
You also find them implemented on the conversion types, such as the [`DisplayFromStr`] type.
These comprise the bulk of this crate and allow you to perform all the nice conversions to [hex strings], the [bytes to string converter], or [duration to UNIX epoch].

In many cases, conversion is only required from one serializable type to another one, without requiring the full power of the `Serialize` or `Deserialize` traits.
In these cases, the [`serde_conv!`] macro conveniently allows defining conversion types without the boilerplate.
The documentation of [`serde_conv!`] contains more details how to use it.

The trait documentations for [`SerializeAs`] and [`DeserializeAs`] describe in details how to implement them for container types like `Box` or `Vec` and other types.

### Using `#[serde_as]` on types without `SerializeAs` and `Serialize` implementations

The `SerializeAs` and `DeserializeAs` traits can easily be used together with types from other crates without running into orphan rule problems.
This is a distinct advantage of the `serde_as` system.
For this example, we assume we have a type `RemoteType` from a dependency which does not implement `Serialize` nor `SerializeAs`.
We assume we have a module containing a `serialize` and a `deserialize` function, which can be used in the `#[serde(with = "MODULE")]` annotation.
You find an example in the [official serde documentation](https://serde.rs/custom-date-format.html).

Our goal is to serialize this `Data` struct.
Currently, we do not have anything we can use to replace `???` with, since `_` only works if `RemoteType` would implement `Serialize`, which it does not.

```rust
# #[cfg(any())] {
#[serde_as]
#[derive(serde::Serialize)]
struct Data {
    #[serde_as(as = "Vec<???>")]
    vec: Vec<RemoteType>,
}
# }
```

We need to create a new type for which we can implement `SerializeAs`, to replace the `???`.
The `SerializeAs` implementation is **always** written for a local type.
This allows it to seamlessly work with types from dependencies without running into orphan rule problems.

```rust
# #[cfg(any())] {
struct LocalType;

impl SerializeAs<RemoteType> for LocalType {
    fn serialize_as<S>(value: &RemoteType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {  
        MODULE::serialize(value, serializer)
    }
}

impl<'de> DeserializeAs<'de, RemoteType> for LocalType {
    fn deserialize_as<D>(deserializer: D) -> Result<RemoteType, D::Error>
    where
        D: Deserializer<'de>,
    {  
        MODULE::deserialize(deserializer)
    }
}
# }
```

This is what the final implementation looks like.
We assumed we already have a module `MODULE` with a `serialize` function, which we use here to provide the implementation.
As can be seen, this is mostly boilerplate, since the most part is encapsulated in `$module::serialize`.
The final `Data` struct will now look like:

```rust
# #[cfg(any())] {
#[serde_as]
#[derive(serde::Serialize)]
struct Data {
    #[serde_as(as = "Vec<LocalType>")]
    vec: Vec<RemoteType>,
}
# }
```

### Using `#[serde_as]` with serde's remote derives

A special case of the above section is using it on remote derives.
This is a special functionality of serde, where it derives the de/serialization code for a type from another crate if all fields are `pub`.
You can find all the details in the [official serde documentation](https://serde.rs/remote-derive.html).

```rust
# #[cfg(any())] {
// Pretend that this is somebody else's crate, not a module.
mod other_crate {
    // Neither Serde nor the other crate provides Serialize and Deserialize
    // impls for this struct.
    pub struct Duration {
        pub secs: i64,
        pub nanos: i32,
    }
}

////////////////////////////////////////////////////////////////////////////////

use other_crate::Duration;

// Serde calls this the definition of the remote type. It is just a copy of the
// remote data structure. The `remote` attribute gives the path to the actual
// type we intend to derive code for.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Duration")]
struct DurationDef {
    secs: i64,
    nanos: i32,
}
# }
```

Our goal is now to use `Duration` within `serde_as`.
We use the existing `DurationDef` type and its `serialize` and `deserialize` functions.
We can write this implementation.
The implementation for `DeserializeAs` works analogue.

```rust
# #[cfg(any())] {
impl SerializeAs<Duration> for DurationDef {
    fn serialize_as<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {  
        DurationDef::serialize(value, serializer)
    }
}
# }
```

This now allows us to use `Duration` for serialization.

```rust
# #[cfg(any())] {
use other_crate::Duration;

#[serde_as]
#[derive(serde::Serialize)]
struct Data {
    #[serde_as(as = "Vec<DurationDef>")]
    vec: Vec<Duration>,
}
# }
```

## Re-exporting `serde_as`

If `serde_as` is being used in a context where the `serde_with` crate is not available from the root
path, but is re-exported at some other path, the `crate = "..."` attribute argument should be used
to specify its path. This may be the case if `serde_as` is being used in a procedural macro -
otherwise, users of that macro would need to add `serde_with` to their own Cargo manifest.

The `crate` argument will generally be used in conjunction with [`serde`'s own `crate` argument].

For example, a type definition may be defined in a procedural macro:

```rust,ignore
// some_other_lib_derive/src/lib.rs

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn define_some_type(_item: TokenStream) -> TokenStream {
    let def = quote! {
        #[serde(crate = "::some_other_lib::serde")]
        #[::some_other_lib::serde_with::serde_as(crate = "::some_other_lib::serde_with")]
        #[derive(::some_other_lib::serde::Deserialize)]
        struct Data {
            #[serde_as(as = "_")]
            a: u32,
        }
    };

    TokenStream::from(def)
}
```

This can be re-exported through a library which also re-exports `serde` and `serde_with`:

```rust,ignore
// some_other_lib/src/lib.rs

pub use serde;
pub use serde_with;
pub use some_other_lib_derive::define_some_type;
```

The procedural macro can be used by other crates without any additional imports:

```rust,ignore
// consuming_crate/src/main.rs

some_other_lib::define_some_type!();
```

[`DeserializeAs`]: crate::DeserializeAs
[`DisplayFromStr`]: crate::DisplayFromStr
[`serde_as`]: crate::serde_as
[`serde_conv!`]: crate::serde_conv!
[`serde`'s own `crate` argument]: https://serde.rs/container-attrs.html#crate
[`SerializeAs`]: crate::SerializeAs
[bytes to string converter]: crate::BytesOrString
[duration to UNIX epoch]: crate::DurationSeconds
[hex strings]: crate::hex::Hex
[serde_with#185]: https://github.com/jonasbb/serde_with/issues/185
