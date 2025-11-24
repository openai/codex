#![doc(test(attr(
    allow(
        unknown_lints,
        // Problematic handling for foreign From<T> impls in tests
        // https://github.com/rust-lang/rust/issues/121621
        non_local_definitions,
        // Some tests use foo as name
        clippy::disallowed_names,
    ),
    deny(
        missing_debug_implementations,
        rust_2018_idioms,
        trivial_casts,
        trivial_numeric_casts,
        unused_extern_crates,
        unused_import_braces,
        unused_qualifications,
        warnings,
    ),
    forbid(unsafe_code),
)))]
// Not needed for 2018 edition and conflicts with `rust_2018_idioms`
#![doc(test(no_crate_inject))]
#![doc(html_root_url = "https://docs.rs/serde_with/3.14.0/")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

//! [![crates.io badge](https://img.shields.io/crates/v/serde_with.svg)](https://crates.io/crates/serde_with/)
//! [![Build Status](https://github.com/jonasbb/serde_with/actions/workflows/ci.yaml/badge.svg)](https://github.com/jonasbb/serde_with)
//! [![codecov](https://codecov.io/gh/jonasbb/serde_with/branch/master/graph/badge.svg)](https://codecov.io/gh/jonasbb/serde_with)
//! [![CII Best Practices](https://bestpractices.coreinfrastructure.org/projects/4322/badge)](https://bestpractices.coreinfrastructure.org/projects/4322)
//!
//! ---
//!
//! This crate provides custom de/serialization helpers to use in combination with [serde's `with` annotation][with-annotation] and with the improved [`serde_as`][as-annotation]-annotation.
//! Some common use cases are:
//!
//! * De/Serializing a type using the `Display` and `FromStr` traits, e.g., for `u8`, `url::Url`, or `mime::Mime`.
//!   Check [`DisplayFromStr`] for details.
//! * Support for arrays larger than 32 elements or using const generics.
//!   With `serde_as` large arrays are supported, even if they are nested in other types.
//!   `[bool; 64]`, `Option<[u8; M]>`, and `Box<[[u8; 64]; N]>` are all supported, as [this examples shows](#large-and-const-generic-arrays).
//! * Skip serializing all empty `Option` types with [`#[skip_serializing_none]`][skip_serializing_none].
//! * Apply a prefix / suffix to each field name of a struct, without changing the de/serialize implementations of the struct using [`with_prefix!`][] / [`with_suffix!`][].
//! * Deserialize a comma separated list like `#hash,#tags,#are,#great` into a `Vec<String>`.
//!   Check the documentation for [`serde_with::StringWithSeparator::<CommaSeparator, T>`][StringWithSeparator].
//!
//! ## Getting Help
//!
//! **Check out the [user guide][user guide] to find out more tips and tricks about this crate.**
//!
//! For further help using this crate you can [open a new discussion](https://github.com/jonasbb/serde_with/discussions/new) or ask on [users.rust-lang.org](https://users.rust-lang.org/).
//! For bugs, please open a [new issue](https://github.com/jonasbb/serde_with/issues/new) on GitHub.
//!
//! # Use `serde_with` in your Project
//!
//! ```bash
//! # Add the current version to your Cargo.toml
//! cargo add serde_with
//! ```
//!
//! The crate contains different features for integration with other common crates.
//! Check the [feature flags][] section for information about all available features.
//!
//! # Examples
//!
//! Annotate your struct or enum to enable the custom de/serializer.
//! The `#[serde_as]` attribute must be placed *before* the `#[derive]`.
//!
//! The `as` is analogous to the `with` attribute of serde.
//! You mirror the type structure of the field you want to de/serialize.
//! You can specify converters for the inner types of a field, e.g., `Vec<DisplayFromStr>`.
//! The default de/serialization behavior can be restored by using `_` as a placeholder, e.g., `BTreeMap<_, DisplayFromStr>`.
//!
//! ## `DisplayFromStr`
//!
//! ```rust
//! # #[cfg(all(feature = "macros", feature = "json"))] {
//! # use serde::{Deserialize, Serialize};
//! # use serde_with::{serde_as, DisplayFromStr};
//! #[serde_as]
//! # #[derive(Debug, Eq, PartialEq)]
//! #[derive(Deserialize, Serialize)]
//! struct Foo {
//!     // Serialize with Display, deserialize with FromStr
//!     #[serde_as(as = "DisplayFromStr")]
//!     bar: u8,
//! }
//!
//! // This will serialize
//! # let foo =
//! Foo {bar: 12}
//! # ;
//!
//! // into this JSON
//! # let json = r#"
//! {"bar": "12"}
//! # "#;
//! # assert_eq!(json.replace(" ", "").replace("\n", ""), serde_json::to_string(&foo).unwrap());
//! # assert_eq!(foo, serde_json::from_str(json).unwrap());
//! # }
//! ```
//!
//! ## Large and const-generic arrays
//!
//! serde does not support arrays with more than 32 elements or using const-generics.
//! The `serde_as` attribute allows circumventing this restriction, even for nested types and nested arrays.
//!
//! On top of it, `[u8; N]` (aka, bytes) can use the specialized `"Bytes"` for efficiency much like the `serde_bytes` crate.
//!
//! ```rust
//! # #[cfg(all(feature = "macros", feature = "json"))] {
//! # use serde::{Deserialize, Serialize};
//! # use serde_with::{serde_as, Bytes};
//! #[serde_as]
//! # #[derive(Debug, Eq, PartialEq)]
//! #[derive(Deserialize, Serialize)]
//! struct Arrays<const N: usize, const M: usize> {
//!     #[serde_as(as = "[_; N]")]
//!     constgeneric: [bool; N],
//!
//!     #[serde_as(as = "Box<[[_; 64]; N]>")]
//!     nested: Box<[[u8; 64]; N]>,
//!
//!     #[serde_as(as = "Option<[_; M]>")]
//!     optional: Option<[u8; M]>,
//!
//!     #[serde_as(as = "Bytes")]
//!     bytes: [u8; M],
//! }
//!
//! // This allows us to serialize a struct like this
//! let arrays: Arrays<100, 128> = Arrays {
//!     constgeneric: [true; 100],
//!     nested: Box::new([[111; 64]; 100]),
//!     optional: Some([222; 128]),
//!     bytes: [0x42; 128],
//! };
//! assert!(serde_json::to_string(&arrays).is_ok());
//! # }
//! ```
//!
//! ## `skip_serializing_none`
//!
//! This situation often occurs with JSON, but other formats also support optional fields.
//! If many fields are optional, putting the annotations on the structs can become tedious.
//! The `#[skip_serializing_none]` attribute must be placed *before* the `#[derive]`.
//!
//! ```rust
//! # #[cfg(all(feature = "macros", feature = "json"))] {
//! # use serde::{Deserialize, Serialize};
//! # use serde_with::skip_serializing_none;
//! #[skip_serializing_none]
//! # #[derive(Debug, Eq, PartialEq)]
//! #[derive(Deserialize, Serialize)]
//! struct Foo {
//!     a: Option<usize>,
//!     b: Option<usize>,
//!     c: Option<usize>,
//!     d: Option<usize>,
//!     e: Option<usize>,
//!     f: Option<usize>,
//!     g: Option<usize>,
//! }
//!
//! // This will serialize
//! # let foo =
//! Foo {a: None, b: None, c: None, d: Some(4), e: None, f: None, g: Some(7)}
//! # ;
//!
//! // into this JSON
//! # let json = r#"
//! {"d": 4, "g": 7}
//! # "#;
//! # assert_eq!(json.replace(" ", "").replace("\n", ""), serde_json::to_string(&foo).unwrap());
//! # assert_eq!(foo, serde_json::from_str(json).unwrap());
//! # }
//! ```
//!
//! ## Advanced `serde_as` usage
//!
//! This example is mainly supposed to highlight the flexibility of the `serde_as` annotation compared to [serde's `with` annotation][with-annotation].
//! More details about `serde_as` can be found in the [user guide].
//!
//! ```rust
//! # #[cfg(all(feature = "macros", feature = "hex"))]
//! # use {
//! #     serde::{Deserialize, Serialize},
//! #     serde_with::{serde_as, DisplayFromStr, DurationSeconds, hex::Hex, Map},
//! # };
//! # #[cfg(all(feature = "macros", feature = "hex"))]
//! use std::time::Duration;
//!
//! # #[cfg(all(feature = "macros", feature = "hex"))]
//! #[serde_as]
//! # #[derive(Debug, Eq, PartialEq)]
//! #[derive(Deserialize, Serialize)]
//! enum Foo {
//!     Durations(
//!         // Serialize them into a list of number as seconds
//!         #[serde_as(as = "Vec<DurationSeconds>")]
//!         Vec<Duration>,
//!     ),
//!     Bytes {
//!         // We can treat a Vec like a map with duplicates.
//!         // JSON only allows string keys, so convert i32 to strings
//!         // The bytes will be hex encoded
//!         #[serde_as(as = "Map<DisplayFromStr, Hex>")]
//!         bytes: Vec<(i32, Vec<u8>)>,
//!     }
//! }
//!
//! # #[cfg(all(feature = "macros", feature = "json", feature = "hex"))] {
//! // This will serialize
//! # let foo =
//! Foo::Durations(
//!     vec![Duration::new(5, 0), Duration::new(3600, 0), Duration::new(0, 0)]
//! )
//! # ;
//! // into this JSON
//! # let json = r#"
//! {
//!     "Durations": [5, 3600, 0]
//! }
//! # "#;
//! # assert_eq!(json.replace(" ", "").replace("\n", ""), serde_json::to_string(&foo).unwrap());
//! # assert_eq!(foo, serde_json::from_str(json).unwrap());
//!
//! // and serializes
//! # let foo =
//! Foo::Bytes {
//!     bytes: vec![
//!         (1, vec![0, 1, 2]),
//!         (-100, vec![100, 200, 255]),
//!         (1, vec![0, 111, 222]),
//!     ],
//! }
//! # ;
//! // into this JSON
//! # let json = r#"
//! {
//!     "Bytes": {
//!         "bytes": {
//!             "1": "000102",
//!             "-100": "64c8ff",
//!             "1": "006fde"
//!         }
//!     }
//! }
//! # "#;
//! # assert_eq!(json.replace(" ", "").replace("\n", ""), serde_json::to_string(&foo).unwrap());
//! # assert_eq!(foo, serde_json::from_str(json).unwrap());
//! # }
//! ```
//!
//! [`DisplayFromStr`]: https://docs.rs/serde_with/3.14.0/serde_with/struct.DisplayFromStr.html
//! [`with_prefix!`]: https://docs.rs/serde_with/3.14.0/serde_with/macro.with_prefix.html
//! [`with_suffix!`]: https://docs.rs/serde_with/3.14.0/serde_with/macro.with_suffix.html
//! [feature flags]: https://docs.rs/serde_with/3.14.0/serde_with/guide/feature_flags/index.html
//! [skip_serializing_none]: https://docs.rs/serde_with/3.14.0/serde_with/attr.skip_serializing_none.html
//! [StringWithSeparator]: https://docs.rs/serde_with/3.14.0/serde_with/struct.StringWithSeparator.html
//! [user guide]: https://docs.rs/serde_with/3.14.0/serde_with/guide/index.html
//! [with-annotation]: https://serde.rs/field-attrs.html#with
//! [as-annotation]: https://docs.rs/serde_with/3.14.0/serde_with/guide/serde_as/index.html

#[cfg(feature = "alloc")]
extern crate alloc;
#[doc(hidden)]
pub extern crate core;
#[doc(hidden)]
pub extern crate serde;
#[doc(hidden)]
pub extern crate serde_derive;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "base64")]
#[cfg_attr(docsrs, doc(cfg(feature = "base64")))]
pub mod base64;
#[cfg(feature = "chrono_0_4")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono_0_4")))]
pub mod chrono_0_4;
/// Legacy export of the [`chrono_0_4`] module.
#[cfg(feature = "chrono")]
#[cfg_attr(docsrs, doc(cfg(feature = "chrono")))]
pub mod chrono {
    pub use crate::chrono_0_4::*;
    pub use chrono_0_4::*;
}
#[cfg(feature = "alloc")]
mod content;
pub mod de;
#[cfg(feature = "alloc")]
mod duplicate_key_impls;
#[cfg(feature = "alloc")]
mod enum_map;
#[cfg(feature = "std")]
mod flatten_maybe;
pub mod formats;
#[cfg(feature = "hex")]
#[cfg_attr(docsrs, doc(cfg(feature = "hex")))]
pub mod hex;
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub mod json;
#[cfg(feature = "alloc")]
mod key_value_map;
pub mod rust;
#[cfg(feature = "schemars_0_8")]
#[cfg_attr(docsrs, doc(cfg(feature = "schemars_0_8")))]
pub mod schemars_0_8;
#[cfg(feature = "schemars_0_9")]
#[cfg_attr(docsrs, doc(cfg(feature = "schemars_0_9")))]
pub mod schemars_0_9;
#[cfg(feature = "schemars_1")]
#[cfg_attr(docsrs, doc(cfg(feature = "schemars_1")))]
pub mod schemars_1;
pub mod ser;
mod serde_conv;
#[cfg(feature = "time_0_3")]
#[cfg_attr(docsrs, doc(cfg(feature = "time_0_3")))]
pub mod time_0_3;
mod utils;
#[cfg(feature = "std")]
#[doc(hidden)]
pub mod with_prefix;
#[cfg(feature = "std")]
#[doc(hidden)]
pub mod with_suffix;

// Taken from shepmaster/snafu
// Originally licensed as MIT+Apache 2
// https://github.com/shepmaster/snafu/blob/90991b609e8928ceebf7df1b040408539d21adda/src/lib.rs#L343-L376
#[cfg(feature = "guide")]
#[allow(unused_macro_rules)]
macro_rules! generate_guide {
    (pub mod $name:ident { $($children:tt)* } $($rest:tt)*) => {
        generate_guide!(@gen ".", pub mod $name { $($children)* } $($rest)*);
    };
    (@gen $prefix:expr, ) => {};
    (@gen $prefix:expr, pub mod $name:ident; $($rest:tt)*) => {
        generate_guide!(@gen $prefix, pub mod $name { } $($rest)*);
    };
    (@gen $prefix:expr, @code pub mod $name:ident; $($rest:tt)*) => {
        #[cfg(feature = "guide")]
        pub mod $name;

        #[cfg(not(feature = "guide"))]
        /// Not currently built; please add the `guide` feature flag.
        pub mod $name {}

        generate_guide!(@gen $prefix, $($rest)*);
    };
    (@gen $prefix:expr, pub mod $name:ident { $($children:tt)* } $($rest:tt)*) => {
        #[cfg(feature = "guide")]
        #[doc = include_str!(concat!($prefix, "/", stringify!($name), ".md"))]
        pub mod $name {
            generate_guide!(@gen concat!($prefix, "/", stringify!($name)), $($children)*);
        }
        #[cfg(not(feature = "guide"))]
        /// Not currently built; please add the `guide` feature flag.
        pub mod $name {
            generate_guide!(@gen concat!($prefix, "/", stringify!($name)), $($children)*);
        }

        generate_guide!(@gen $prefix, $($rest)*);
    };
}

#[cfg(feature = "guide")]
generate_guide! {
    pub mod guide {
        @code pub mod feature_flags;
        pub mod serde_as;
        pub mod serde_as_transformations;
    }
}

pub(crate) mod prelude {
    #![allow(unused_imports)]

    pub(crate) use crate::utils::duration::{DurationSigned, Sign};
    pub use crate::{de::*, ser::*, *};
    #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
    pub use alloc::sync::{Arc, Weak as ArcWeak};
    #[cfg(feature = "alloc")]
    pub use alloc::{
        borrow::{Cow, ToOwned},
        boxed::Box,
        collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque},
        rc::{Rc, Weak as RcWeak},
        string::{String, ToString},
        vec::Vec,
    };
    pub use core::{
        cell::{Cell, RefCell},
        convert::{TryFrom, TryInto},
        fmt::{self, Display},
        hash::{BuildHasher, Hash},
        marker::PhantomData,
        ops::{Bound, Range, RangeFrom, RangeInclusive, RangeTo},
        option::Option,
        pin::Pin,
        result::Result,
        str::FromStr,
        time::Duration,
    };
    pub use serde::{
        de::{
            Deserialize, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess,
            Error as DeError, Expected, IgnoredAny, IntoDeserializer, MapAccess, SeqAccess,
            Unexpected, VariantAccess, Visitor,
        },
        forward_to_deserialize_any,
        ser::{
            Error as SerError, Impossible, Serialize, SerializeMap, SerializeSeq, SerializeStruct,
            SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
            Serializer,
        },
    };
    #[cfg(feature = "std")]
    pub use std::{
        collections::{HashMap, HashSet},
        sync::{Mutex, RwLock},
        time::SystemTime,
    };
}

/// This module is not part of the public API
///
/// Do not rely on any exports.
#[doc(hidden)]
pub mod __private__ {
    pub use crate::prelude::*;
}

#[cfg(feature = "alloc")]
#[doc(inline)]
pub use crate::enum_map::EnumMap;
#[cfg(feature = "alloc")]
#[doc(inline)]
pub use crate::key_value_map::KeyValueMap;
#[doc(inline)]
pub use crate::{de::DeserializeAs, ser::SerializeAs};
use core::marker::PhantomData;
// Re-Export all proc_macros, as these should be seen as part of the serde_with crate
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
#[doc(inline)]
pub use serde_with_macros::*;

/// Adapter to convert from `serde_as` to the serde traits.
///
/// The `As` type adapter allows using types which implement [`DeserializeAs`] or [`SerializeAs`] in place of serde's `with` annotation.
/// The `with` annotation allows running custom code when de/serializing, however it is quite inflexible.
/// The traits [`DeserializeAs`]/[`SerializeAs`] are more flexible, as they allow composition and nesting of types to create more complex de/serialization behavior.
/// However, they are not directly compatible with serde, as they are not provided by serde.
/// The `As` type adapter makes them compatible, by forwarding the function calls to `serialize`/`deserialize` to the corresponding functions `serialize_as` and `deserialize_as`.
///
/// It is not required to use this type directly.
/// Instead, it is highly encouraged to use the [`#[serde_as]`][serde_as] attribute since it includes further usability improvements.
/// If the use of the use of the proc-macro is not acceptable, then `As` can be used directly with serde.
///
/// ```rust
/// # #[cfg(feature = "alloc")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{As, DisplayFromStr};
/// #
/// # #[allow(dead_code)]
/// #[derive(Deserialize, Serialize)]
/// # struct S {
/// // Serialize numbers as sequence of strings, using Display and FromStr
/// #[serde(with = "As::<Vec<DisplayFromStr>>")]
/// field: Vec<u8>,
/// # }
/// # }
/// ```
/// If the normal `Deserialize`/`Serialize` traits should be used, the placeholder type [`Same`] can be used.
/// It implements [`DeserializeAs`][]/[`SerializeAs`][], when the underlying type implements `Deserialize`/`Serialize`.
///
/// ```rust
/// # #[cfg(feature = "alloc")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{As, DisplayFromStr, Same};
/// # use std::collections::BTreeMap;
/// #
/// # #[allow(dead_code)]
/// #[derive(Deserialize, Serialize)]
/// # struct S {
/// // Serialize map, turn keys into strings but keep type of value
/// #[serde(with = "As::<BTreeMap<DisplayFromStr, Same>>")]
/// field: BTreeMap<u8, i32>,
/// # }
/// # }
/// ```
///
/// [serde_as]: https://docs.rs/serde_with/3.14.0/serde_with/attr.serde_as.html
pub struct As<T: ?Sized>(PhantomData<T>);

/// Adapter to convert from `serde_as` to the serde traits.
///
/// This is the counter-type to [`As`][].
/// It can be used whenever a type implementing [`DeserializeAs`]/[`SerializeAs`] is required but the normal [`Deserialize`](::serde::Deserialize)/[`Serialize`](::serde::Serialize) traits should be used.
/// Check [`As`] for an example.
pub struct Same;

/// De/Serialize using [`Display`] and [`FromStr`] implementation
///
/// This allows deserializing a string as a number.
/// It can be very useful for serialization formats like JSON, which do not support integer
/// numbers and have to resort to strings to represent them.
///
/// Another use case is types with [`Display`] and [`FromStr`] implementations, but without serde
/// support, which can be found in some crates.
///
/// If you control the type you want to de/serialize, you can instead use the two derive macros, [`SerializeDisplay`] and [`DeserializeFromStr`].
/// They properly implement the traits [`serde::Serialize`] and [`serde::Deserialize`] such that user of the type no longer have to use the `serde_as` system.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DisplayFromStr};
/// #
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "DisplayFromStr")]
///     mime: mime::Mime,
///     #[serde_as(as = "DisplayFromStr")]
///     number: u32,
/// }
///
/// let v: A = serde_json::from_value(json!({
///     "mime": "text/plain",
///     "number": "159",
/// })).unwrap();
/// assert_eq!(mime::TEXT_PLAIN, v.mime);
/// assert_eq!(159, v.number);
///
/// let x = A {
///     mime: mime::STAR_STAR,
///     number: 777,
/// };
/// assert_eq!(json!({ "mime": "*/*", "number": "777" }), serde_json::to_value(x).unwrap());
/// # }
/// ```
///
/// [`Display`]: std::fmt::Display
/// [`FromStr`]: std::str::FromStr
pub struct DisplayFromStr;

/// Use the first format if [`De/Serializer::is_human_readable`], otherwise use the second
///
/// If the second format is not specified, the normal
/// [`Deserialize`](::serde::Deserialize)/[`Serialize`](::serde::Serialize) traits are used.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DisplayFromStr, IfIsHumanReadable, DurationMilliSeconds, DurationSeconds};
/// use std::time::Duration;
///
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "IfIsHumanReadable<DisplayFromStr>")]
///     number: u32,
/// }
/// let x = A {
///     number: 777,
/// };
/// assert_eq!(json!({ "number": "777" }), serde_json::to_value(&x).unwrap());
/// assert_eq!(vec![145, 205, 3, 9], rmp_serde::to_vec(&x).unwrap());
///
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct B {
///     #[serde_as(as = "IfIsHumanReadable<DurationMilliSeconds, DurationSeconds>")]
///     duration: Duration,
/// }
/// let x = B {
///     duration: Duration::from_millis(1500),
/// };
/// assert_eq!(json!({ "duration": 1500 }), serde_json::to_value(&x).unwrap());
/// assert_eq!(vec![145, 2], rmp_serde::to_vec(&x).unwrap());
/// # }
/// ```
/// [`De/Serializer::is_human_readable`]: serde::Serializer::is_human_readable
/// [`is_human_readable`]: serde::Serializer::is_human_readable
pub struct IfIsHumanReadable<H, F = Same>(PhantomData<H>, PhantomData<F>);

/// De/Serialize a [`Option<String>`] type while transforming the empty string to [`None`]
///
/// Convert an [`Option<T>`] from/to string using [`FromStr`] and [`Display`](::core::fmt::Display) implementations.
/// An empty string is deserialized as [`None`] and a [`None`] vice versa.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, NoneAsEmptyString};
/// #
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "NoneAsEmptyString")]
///     tags: Option<String>,
/// }
///
/// let v: A = serde_json::from_value(json!({ "tags": "" })).unwrap();
/// assert_eq!(None, v.tags);
///
/// let v: A = serde_json::from_value(json!({ "tags": "Hi" })).unwrap();
/// assert_eq!(Some("Hi".to_string()), v.tags);
///
/// let x = A {
///     tags: Some("This is text".to_string()),
/// };
/// assert_eq!(json!({ "tags": "This is text" }), serde_json::to_value(x).unwrap());
///
/// let x = A {
///     tags: None,
/// };
/// assert_eq!(json!({ "tags": "" }), serde_json::to_value(x).unwrap());
/// # }
/// ```
///
/// [`FromStr`]: std::str::FromStr
pub struct NoneAsEmptyString;

/// Deserialize value and return [`Default`] on error
///
/// The main use case is ignoring error while deserializing.
/// Instead of erroring, it simply deserializes the [`Default`] variant of the type.
/// It is not possible to find the error location, i.e., which field had a deserialization error, with this method.
/// During serialization this wrapper does nothing.
/// The serialization behavior of the underlying type is preserved.
/// The type must implement [`Default`] for this conversion to work.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use serde_with::{serde_as, DefaultOnError};
/// #
/// #[serde_as]
/// #[derive(Deserialize, Debug)]
/// struct A {
///     #[serde_as(deserialize_as = "DefaultOnError")]
///     value: u32,
/// }
///
/// let a: A = serde_json::from_str(r#"{"value": 123}"#).unwrap();
/// assert_eq!(123, a.value);
///
/// // null is of invalid type
/// let a: A = serde_json::from_str(r#"{"value": null}"#).unwrap();
/// assert_eq!(0, a.value);
///
/// // String is of invalid type
/// let a: A = serde_json::from_str(r#"{"value": "123"}"#).unwrap();
/// assert_eq!(0, a.value);
///
/// // Map is of invalid type
/// let a: A = dbg!(serde_json::from_str(r#"{"value": {}}"#)).unwrap();
/// assert_eq!(0, a.value);
///
/// // Missing entries still cause errors
/// assert!(serde_json::from_str::<A>(r#"{  }"#).is_err());
/// # }
/// ```
///
/// Deserializing missing values can be supported by adding the `default` field attribute:
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use serde_with::{serde_as, DefaultOnError};
/// #
/// #[serde_as]
/// #[derive(Deserialize)]
/// struct B {
///     #[serde_as(deserialize_as = "DefaultOnError")]
///     #[serde(default)]
///     value: u32,
/// }
///
/// let b: B = serde_json::from_str(r#"{  }"#).unwrap();
/// assert_eq!(0, b.value);
/// # }
/// ```
///
/// `DefaultOnError` can be combined with other conversion methods.
/// In this example, we deserialize a `Vec`, each element is deserialized from a string.
/// If the string does not parse as a number, then we get the default value of 0.
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DefaultOnError, DisplayFromStr};
/// #
/// #[serde_as]
/// #[derive(Serialize, Deserialize)]
/// struct C {
///     #[serde_as(as = "Vec<DefaultOnError<DisplayFromStr>>")]
///     value: Vec<u32>,
/// }
///
/// let c: C = serde_json::from_value(json!({
///     "value": ["1", "2", "a3", "", {}, "6"]
/// })).unwrap();
/// assert_eq!(vec![1, 2, 0, 0, 0, 6], c.value);
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct DefaultOnError<T = Same>(PhantomData<T>);

/// Deserialize [`Default`] from `null` values
///
/// Instead of erroring on `null` values, it simply deserializes the [`Default`] variant of the type.
/// During serialization this wrapper does nothing.
/// The serialization behavior of the underlying type is preserved.
/// The type must implement [`Default`] for this conversion to work.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use serde_with::{serde_as, DefaultOnNull};
/// #
/// #[serde_as]
/// #[derive(Deserialize, Debug)]
/// struct A {
///     #[serde_as(deserialize_as = "DefaultOnNull")]
///     value: u32,
/// }
///
/// let a: A = serde_json::from_str(r#"{"value": 123}"#).unwrap();
/// assert_eq!(123, a.value);
///
/// // null values are deserialized into the default, here 0
/// let a: A = serde_json::from_str(r#"{"value": null}"#).unwrap();
/// assert_eq!(0, a.value);
/// # }
/// ```
///
/// `DefaultOnNull` can be combined with other conversion methods.
/// In this example, we deserialize a `Vec`, each element is deserialized from a string.
/// If we encounter null, then we get the default value of 0.
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DefaultOnNull, DisplayFromStr};
/// #
/// #[serde_as]
/// #[derive(Serialize, Deserialize)]
/// struct C {
///     #[serde_as(as = "Vec<DefaultOnNull<DisplayFromStr>>")]
///     value: Vec<u32>,
/// }
///
/// let c: C = serde_json::from_value(json!({
///     "value": ["1", "2", null, null, "5"]
/// })).unwrap();
/// assert_eq!(vec![1, 2, 0, 0, 5], c.value);
/// # }
/// ```
pub struct DefaultOnNull<T = Same>(PhantomData<T>);

/// Deserialize from bytes or string
///
/// Any Rust [`String`] can be converted into bytes, i.e., `Vec<u8>`.
/// Accepting both as formats while deserializing can be helpful while interacting with language
/// which have a looser definition of string than Rust.
///
/// # Example
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, BytesOrString};
/// #
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "BytesOrString")]
///     bytes_or_string: Vec<u8>,
/// }
///
/// // Here we deserialize from a byte array ...
/// let j = json!({
///   "bytes_or_string": [
///     0,
///     1,
///     2,
///     3
///   ]
/// });
///
/// let a: A = serde_json::from_value(j.clone()).unwrap();
/// assert_eq!(vec![0, 1, 2, 3], a.bytes_or_string);
///
/// // and serialization works too.
/// assert_eq!(j, serde_json::to_value(&a).unwrap());
///
/// // But we also support deserializing from a String
/// let j = json!({
///   "bytes_or_string": "✨Works!"
/// });
///
/// let a: A = serde_json::from_value(j).unwrap();
/// assert_eq!("✨Works!".as_bytes(), &*a.bytes_or_string);
/// # }
/// ```
/// [`String`]: std::string::String
#[cfg(feature = "alloc")]
pub struct BytesOrString;

/// De/Serialize Durations as number of seconds.
///
/// De/serialize durations as number of seconds with sub-second precision.
/// Sub-second precision is *only* supported for [`DurationSecondsWithFrac`], but not for [`DurationSeconds`].
/// You can configure the serialization format between integers, floats, and stringified numbers with the `FORMAT` specifier and configure the deserialization with the `STRICTNESS` specifier.
///
/// The `STRICTNESS` specifier can either be [`formats::Strict`] or [`formats::Flexible`] and defaults to [`formats::Strict`].
/// [`formats::Strict`] means that deserialization only supports the type given in `FORMAT`, e.g., if `FORMAT` is `u64` deserialization from a `f64` will error.
/// [`formats::Flexible`] means that deserialization will perform a best effort to extract the correct duration and allows deserialization from any type.
/// For example, deserializing `DurationSeconds<f64, Flexible>` will discard any subsecond precision during deserialization from `f64` and will parse a `String` as an integer number.
/// Serialization of integers will round the duration to the nearest value.
///
/// This type also supports [`chrono::Duration`] with the `chrono_0_4`-[feature flag].
/// This type also supports [`time::Duration`][::time_0_3::Duration] with the `time_0_3`-[feature flag].
///
/// This table lists the available `FORMAT`s for the different duration types.
/// The `FORMAT` specifier defaults to `u64`/`f64`.
///
/// | Duration Type         | Converter                 | Available `FORMAT`s      |
/// | --------------------- | ------------------------- | ------------------------ |
/// | `std::time::Duration` | `DurationSeconds`         | *`u64`*, `f64`, `String` |
/// | `std::time::Duration` | `DurationSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::Duration`    | `DurationSeconds`         | `i64`, `f64`, `String`   |
/// | `chrono::Duration`    | `DurationSecondsWithFrac` | *`f64`*, `String`        |
/// | `time::Duration`      | `DurationSeconds`         | `i64`, `f64`, `String`   |
/// | `time::Duration`      | `DurationSecondsWithFrac` | *`f64`*, `String`        |
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DurationSeconds};
/// use std::time::Duration;
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Durations {
///     #[serde_as(as = "DurationSeconds<u64>")]
///     d_u64: Duration,
///     #[serde_as(as = "DurationSeconds<f64>")]
///     d_f64: Duration,
///     #[serde_as(as = "DurationSeconds<String>")]
///     d_string: Duration,
/// }
///
/// // Serialization
/// // See how the values get rounded, since subsecond precision is not allowed.
///
/// let d = Durations {
///     d_u64: Duration::new(12345, 0), // Create from seconds and nanoseconds
///     d_f64: Duration::new(12345, 500_000_000),
///     d_string: Duration::new(12345, 999_999_999),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "d_u64": 12345,
///     "d_f64": 12346.0,
///     "d_string": "12346",
/// });
/// assert_eq!(expected, serde_json::to_value(d).unwrap());
///
/// // Deserialization works too
/// // Subsecond precision in numbers will be rounded away
///
/// let json = json!({
///     "d_u64": 12345,
///     "d_f64": 12345.5,
///     "d_string": "12346",
/// });
/// let expected = Durations {
///     d_u64: Duration::new(12345, 0), // Create from seconds and nanoseconds
///     d_f64: Duration::new(12346, 0),
///     d_string: Duration::new(12346, 0),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`chrono::Duration`] is also supported when using the `chrono_0_4` feature.
/// It is a signed duration, thus can be de/serialized as an `i64` instead of a `u64`.
///
/// ```rust
/// # #[cfg(all(feature = "macros", feature = "chrono_0_4"))] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DurationSeconds};
/// # use chrono_0_4::Duration;
/// # /* Ugliness to make the docs look nicer since I want to hide the rename of the chrono crate
/// use chrono::Duration;
/// # */
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Durations {
///     #[serde_as(as = "DurationSeconds<i64>")]
///     d_i64: Duration,
///     #[serde_as(as = "DurationSeconds<f64>")]
///     d_f64: Duration,
///     #[serde_as(as = "DurationSeconds<String>")]
///     d_string: Duration,
/// }
///
/// // Serialization
/// // See how the values get rounded, since subsecond precision is not allowed.
///
/// let d = Durations {
///     d_i64: Duration::seconds(-12345),
///     d_f64: Duration::seconds(-12345) + Duration::milliseconds(500),
///     d_string: Duration::seconds(12345) + Duration::nanoseconds(999_999_999),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "d_i64": -12345,
///     "d_f64": -12345.0,
///     "d_string": "12346",
/// });
/// assert_eq!(expected, serde_json::to_value(d).unwrap());
///
/// // Deserialization works too
/// // Subsecond precision in numbers will be rounded away
///
/// let json = json!({
///     "d_i64": -12345,
///     "d_f64": -12345.5,
///     "d_string": "12346",
/// });
/// let expected = Durations {
///     d_i64: Duration::seconds(-12345),
///     d_f64: Duration::seconds(-12346),
///     d_string: Duration::seconds(12346),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`chrono::Duration`]: ::chrono_0_4::Duration
/// [feature flag]: https://docs.rs/serde_with/3.14.0/serde_with/guide/feature_flags/index.html
pub struct DurationSeconds<
    FORMAT: formats::Format = u64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// De/Serialize Durations as number of seconds.
///
/// De/serialize durations as number of seconds with subsecond precision.
/// Subsecond precision is *only* supported for [`DurationSecondsWithFrac`], but not for [`DurationSeconds`].
/// You can configure the serialization format between integers, floats, and stringified numbers with the `FORMAT` specifier and configure the deserialization with the `STRICTNESS` specifier.
/// Serialization of integers will round the duration to the nearest value.
///
/// The `STRICTNESS` specifier can either be [`formats::Strict`] or [`formats::Flexible`] and defaults to [`formats::Strict`].
/// [`formats::Strict`] means that deserialization only supports the type given in `FORMAT`, e.g., if `FORMAT` is `u64` deserialization from a `f64` will error.
/// [`formats::Flexible`] means that deserialization will perform a best effort to extract the correct duration and allows deserialization from any type.
/// For example, deserializing `DurationSeconds<f64, Flexible>` will discard any subsecond precision during deserialization from `f64` and will parse a `String` as an integer number.
///
/// This type also supports [`chrono::Duration`] with the `chrono`-[feature flag].
/// This type also supports [`time::Duration`][::time_0_3::Duration] with the `time_0_3`-[feature flag].
///
/// This table lists the available `FORMAT`s for the different duration types.
/// The `FORMAT` specifier defaults to `u64`/`f64`.
///
/// | Duration Type         | Converter                 | Available `FORMAT`s      |
/// | --------------------- | ------------------------- | ------------------------ |
/// | `std::time::Duration` | `DurationSeconds`         | *`u64`*, `f64`, `String` |
/// | `std::time::Duration` | `DurationSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::Duration`    | `DurationSeconds`         | `i64`, `f64`, `String`   |
/// | `chrono::Duration`    | `DurationSecondsWithFrac` | *`f64`*, `String`        |
/// | `time::Duration`      | `DurationSeconds`         | `i64`, `f64`, `String`   |
/// | `time::Duration`      | `DurationSecondsWithFrac` | *`f64`*, `String`        |
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DurationSecondsWithFrac};
/// use std::time::Duration;
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Durations {
///     #[serde_as(as = "DurationSecondsWithFrac<f64>")]
///     d_f64: Duration,
///     #[serde_as(as = "DurationSecondsWithFrac<String>")]
///     d_string: Duration,
/// }
///
/// // Serialization
/// // See how the values get rounded, since subsecond precision is not allowed.
///
/// let d = Durations {
///     d_f64: Duration::new(12345, 500_000_000), // Create from seconds and nanoseconds
///     d_string: Duration::new(12345, 999_999_000),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "d_f64": 12345.5,
///     "d_string": "12345.999999",
/// });
/// assert_eq!(expected, serde_json::to_value(d).unwrap());
///
/// // Deserialization works too
/// // Subsecond precision in numbers will be rounded away
///
/// let json = json!({
///     "d_f64": 12345.5,
///     "d_string": "12345.987654",
/// });
/// let expected = Durations {
///     d_f64: Duration::new(12345, 500_000_000), // Create from seconds and nanoseconds
///     d_string: Duration::new(12345, 987_654_000),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`chrono::Duration`] is also supported when using the `chrono_0_4` feature.
/// It is a signed duration, thus can be de/serialized as an `i64` instead of a `u64`.
///
/// ```rust
/// # #[cfg(all(feature = "macros", feature = "chrono_0_4"))] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DurationSecondsWithFrac};
/// # use chrono_0_4::Duration;
/// # /* Ugliness to make the docs look nicer since I want to hide the rename of the chrono crate
/// use chrono::Duration;
/// # */
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Durations {
///     #[serde_as(as = "DurationSecondsWithFrac<f64>")]
///     d_f64: Duration,
///     #[serde_as(as = "DurationSecondsWithFrac<String>")]
///     d_string: Duration,
/// }
///
/// // Serialization
///
/// let d = Durations {
///     d_f64: Duration::seconds(-12345) + Duration::milliseconds(500),
///     d_string: Duration::seconds(12345) + Duration::nanoseconds(999_999_000),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "d_f64": -12344.5,
///     "d_string": "12345.999999",
/// });
/// assert_eq!(expected, serde_json::to_value(d).unwrap());
///
/// // Deserialization works too
///
/// let json = json!({
///     "d_f64": -12344.5,
///     "d_string": "12345.987",
/// });
/// let expected = Durations {
///     d_f64: Duration::seconds(-12345) + Duration::milliseconds(500),
///     d_string: Duration::seconds(12345) + Duration::milliseconds(987),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`chrono::Duration`]: ::chrono_0_4::Duration
/// [feature flag]: https://docs.rs/serde_with/3.14.0/serde_with/guide/feature_flags/index.html
pub struct DurationSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`DurationSeconds`] with milli-seconds as base unit.
///
/// This type is equivalent to [`DurationSeconds`] except that each unit represents 1 milli-second instead of 1 second for [`DurationSeconds`].
pub struct DurationMilliSeconds<
    FORMAT: formats::Format = u64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`DurationSecondsWithFrac`] with milli-seconds as base unit.
///
/// This type is equivalent to [`DurationSecondsWithFrac`] except that each unit represents 1 milli-second instead of 1 second for [`DurationSecondsWithFrac`].
pub struct DurationMilliSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`DurationSeconds`] with micro-seconds as base unit.
///
/// This type is equivalent to [`DurationSeconds`] except that each unit represents 1 micro-second instead of 1 second for [`DurationSeconds`].
pub struct DurationMicroSeconds<
    FORMAT: formats::Format = u64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`DurationSecondsWithFrac`] with micro-seconds as base unit.
///
/// This type is equivalent to [`DurationSecondsWithFrac`] except that each unit represents 1 micro-second instead of 1 second for [`DurationSecondsWithFrac`].
pub struct DurationMicroSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`DurationSeconds`] with nano-seconds as base unit.
///
/// This type is equivalent to [`DurationSeconds`] except that each unit represents 1 nano-second instead of 1 second for [`DurationSeconds`].
pub struct DurationNanoSeconds<
    FORMAT: formats::Format = u64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`DurationSecondsWithFrac`] with nano-seconds as base unit.
///
/// This type is equivalent to [`DurationSecondsWithFrac`] except that each unit represents 1 nano-second instead of 1 second for [`DurationSecondsWithFrac`].
pub struct DurationNanoSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// De/Serialize timestamps as seconds since the UNIX epoch
///
/// De/serialize timestamps as seconds since the UNIX epoch.
/// Subsecond precision is *only* supported for [`TimestampSecondsWithFrac`], but not for [`TimestampSeconds`].
/// You can configure the serialization format between integers, floats, and stringified numbers with the `FORMAT` specifier and configure the deserialization with the `STRICTNESS` specifier.
/// Serialization of integers will round the timestamp to the nearest value.
///
/// The `STRICTNESS` specifier can either be [`formats::Strict`] or [`formats::Flexible`] and defaults to [`formats::Strict`].
/// [`formats::Strict`] means that deserialization only supports the type given in `FORMAT`, e.g., if `FORMAT` is `i64` deserialization from a `f64` will error.
/// [`formats::Flexible`] means that deserialization will perform a best effort to extract the correct timestamp and allows deserialization from any type.
/// For example, deserializing `TimestampSeconds<f64, Flexible>` will discard any subsecond precision during deserialization from `f64` and will parse a `String` as an integer number.
///
/// This type also supports [`chrono::DateTime`] with the `chrono_0_4`-[feature flag].
/// This type also supports [`time::OffsetDateTime`][::time_0_3::OffsetDateTime] and [`time::PrimitiveDateTime`][::time_0_3::PrimitiveDateTime] with the `time_0_3`-[feature flag].
///
/// This table lists the available `FORMAT`s for the different timestamp types.
/// The `FORMAT` specifier defaults to `i64` or `f64`.
///
/// | Timestamp Type            | Converter                  | Available `FORMAT`s      |
/// | ------------------------- | -------------------------- | ------------------------ |
/// | `std::time::SystemTime`   | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `std::time::SystemTime`   | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::DateTime<Utc>`   | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `chrono::DateTime<Utc>`   | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::DateTime<Local>` | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `chrono::DateTime<Local>` | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::NaiveDateTime`   | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `chrono::NaiveDateTime`   | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `time::OffsetDateTime`    | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `time::OffsetDateTime`    | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `time::PrimitiveDateTime` | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `time::PrimitiveDateTime` | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, TimestampSeconds};
/// use std::time::{Duration, SystemTime};
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Timestamps {
///     #[serde_as(as = "TimestampSeconds<i64>")]
///     st_i64: SystemTime,
///     #[serde_as(as = "TimestampSeconds<f64>")]
///     st_f64: SystemTime,
///     #[serde_as(as = "TimestampSeconds<String>")]
///     st_string: SystemTime,
/// }
///
/// // Serialization
/// // See how the values get rounded, since subsecond precision is not allowed.
///
/// let ts = Timestamps {
///     st_i64: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 0)).unwrap(),
///     st_f64: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 500_000_000)).unwrap(),
///     st_string: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 999_999_999)).unwrap(),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "st_i64": 12345,
///     "st_f64": 12346.0,
///     "st_string": "12346",
/// });
/// assert_eq!(expected, serde_json::to_value(ts).unwrap());
///
/// // Deserialization works too
/// // Subsecond precision in numbers will be rounded away
///
/// let json = json!({
///     "st_i64": 12345,
///     "st_f64": 12345.5,
///     "st_string": "12346",
/// });
/// let expected  = Timestamps {
///     st_i64: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 0)).unwrap(),
///     st_f64: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12346, 0)).unwrap(),
///     st_string: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12346, 0)).unwrap(),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`chrono::DateTime<Utc>`] and [`chrono::DateTime<Local>`] are also supported when using the `chrono` feature.
/// Like [`SystemTime`], it is a signed timestamp, thus can be de/serialized as an `i64`.
///
/// ```rust
/// # #[cfg(all(feature = "macros", feature = "chrono_0_4"))] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, TimestampSeconds};
/// # use chrono_0_4::{DateTime, Local, TimeZone, Utc};
/// # /* Ugliness to make the docs look nicer since I want to hide the rename of the chrono crate
/// use chrono::{DateTime, Local, TimeZone, Utc};
/// # */
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Timestamps {
///     #[serde_as(as = "TimestampSeconds<i64>")]
///     dt_i64: DateTime<Utc>,
///     #[serde_as(as = "TimestampSeconds<f64>")]
///     dt_f64: DateTime<Local>,
///     #[serde_as(as = "TimestampSeconds<String>")]
///     dt_string: DateTime<Utc>,
/// }
///
/// // Serialization
/// // See how the values get rounded, since subsecond precision is not allowed.
///
/// let ts = Timestamps {
///     dt_i64: Utc.timestamp_opt(-12345, 0).unwrap(),
///     dt_f64: Local.timestamp_opt(-12345, 500_000_000).unwrap(),
///     dt_string: Utc.timestamp_opt(12345, 999_999_999).unwrap(),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "dt_i64": -12345,
///     "dt_f64": -12345.0,
///     "dt_string": "12346",
/// });
/// assert_eq!(expected, serde_json::to_value(ts).unwrap());
///
/// // Deserialization works too
/// // Subsecond precision in numbers will be rounded away
///
/// let json = json!({
///     "dt_i64": -12345,
///     "dt_f64": -12345.5,
///     "dt_string": "12346",
/// });
/// let expected = Timestamps {
///     dt_i64: Utc.timestamp_opt(-12345, 0).unwrap(),
///     dt_f64: Local.timestamp_opt(-12346, 0).unwrap(),
///     dt_string: Utc.timestamp_opt(12346, 0).unwrap(),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`SystemTime`]: std::time::SystemTime
/// [`chrono::DateTime<Local>`]: ::chrono_0_4::DateTime
/// [`chrono::DateTime<Utc>`]: ::chrono_0_4::DateTime
/// [feature flag]: https://docs.rs/serde_with/3.14.0/serde_with/guide/feature_flags/index.html
pub struct TimestampSeconds<
    FORMAT: formats::Format = i64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// De/Serialize timestamps as seconds since the UNIX epoch
///
/// De/serialize timestamps as seconds since the UNIX epoch.
/// Subsecond precision is *only* supported for [`TimestampSecondsWithFrac`], but not for [`TimestampSeconds`].
/// You can configure the serialization format between integers, floats, and stringified numbers with the `FORMAT` specifier and configure the deserialization with the `STRICTNESS` specifier.
/// Serialization of integers will round the timestamp to the nearest value.
///
/// The `STRICTNESS` specifier can either be [`formats::Strict`] or [`formats::Flexible`] and defaults to [`formats::Strict`].
/// [`formats::Strict`] means that deserialization only supports the type given in `FORMAT`, e.g., if `FORMAT` is `i64` deserialization from a `f64` will error.
/// [`formats::Flexible`] means that deserialization will perform a best effort to extract the correct timestamp and allows deserialization from any type.
/// For example, deserializing `TimestampSeconds<f64, Flexible>` will discard any subsecond precision during deserialization from `f64` and will parse a `String` as an integer number.
///
/// This type also supports [`chrono::DateTime`] and [`chrono::NaiveDateTime`][NaiveDateTime] with the `chrono`-[feature flag].
/// This type also supports [`time::OffsetDateTime`][::time_0_3::OffsetDateTime] and [`time::PrimitiveDateTime`][::time_0_3::PrimitiveDateTime] with the `time_0_3`-[feature flag].
///
/// This table lists the available `FORMAT`s for the different timestamp types.
/// The `FORMAT` specifier defaults to `i64` or `f64`.
///
/// | Timestamp Type            | Converter                  | Available `FORMAT`s      |
/// | ------------------------- | -------------------------- | ------------------------ |
/// | `std::time::SystemTime`   | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `std::time::SystemTime`   | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::DateTime<Utc>`   | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `chrono::DateTime<Utc>`   | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::DateTime<Local>` | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `chrono::DateTime<Local>` | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `chrono::NaiveDateTime`   | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `chrono::NaiveDateTime`   | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `time::OffsetDateTime`    | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `time::OffsetDateTime`    | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
/// | `time::PrimitiveDateTime` | `TimestampSeconds`         | *`i64`*, `f64`, `String` |
/// | `time::PrimitiveDateTime` | `TimestampSecondsWithFrac` | *`f64`*, `String`        |
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, TimestampSecondsWithFrac};
/// use std::time::{Duration, SystemTime};
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Timestamps {
///     #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
///     st_f64: SystemTime,
///     #[serde_as(as = "TimestampSecondsWithFrac<String>")]
///     st_string: SystemTime,
/// }
///
/// // Serialization
/// // See how the values get rounded, since subsecond precision is not allowed.
///
/// let ts = Timestamps {
///     st_f64: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 500_000_000)).unwrap(),
///     st_string: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 999_999_000)).unwrap(),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "st_f64": 12345.5,
///     "st_string": "12345.999999",
/// });
/// assert_eq!(expected, serde_json::to_value(ts).unwrap());
///
/// // Deserialization works too
/// // Subsecond precision in numbers will be rounded away
///
/// let json = json!({
///     "st_f64": 12345.5,
///     "st_string": "12345.987654",
/// });
/// let expected = Timestamps {
///     st_f64: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 500_000_000)).unwrap(),
///     st_string: SystemTime::UNIX_EPOCH.checked_add(Duration::new(12345, 987_654_000)).unwrap(),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`chrono::DateTime<Utc>`] and [`chrono::DateTime<Local>`] are also supported when using the `chrono_0_4` feature.
/// Like [`SystemTime`], it is a signed timestamp, thus can be de/serialized as an `i64`.
///
/// ```rust
/// # #[cfg(all(feature = "macros", feature = "chrono_0_4"))] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, TimestampSecondsWithFrac};
/// # use chrono_0_4::{DateTime, Local, TimeZone, Utc};
/// # /* Ugliness to make the docs look nicer since I want to hide the rename of the chrono crate
/// use chrono::{DateTime, Local, TimeZone, Utc};
/// # */
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Timestamps {
///     #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
///     dt_f64: DateTime<Utc>,
///     #[serde_as(as = "TimestampSecondsWithFrac<String>")]
///     dt_string: DateTime<Local>,
/// }
///
/// // Serialization
///
/// let ts = Timestamps {
///     dt_f64: Utc.timestamp_opt(-12345, 500_000_000).unwrap(),
///     dt_string: Local.timestamp_opt(12345, 999_999_000).unwrap(),
/// };
/// // Observe the different data types
/// let expected = json!({
///     "dt_f64": -12344.5,
///     "dt_string": "12345.999999",
/// });
/// assert_eq!(expected, serde_json::to_value(ts).unwrap());
///
/// // Deserialization works too
///
/// let json = json!({
///     "dt_f64": -12344.5,
///     "dt_string": "12345.987",
/// });
/// let expected = Timestamps {
///     dt_f64: Utc.timestamp_opt(-12345, 500_000_000).unwrap(),
///     dt_string: Local.timestamp_opt(12345, 987_000_000).unwrap(),
/// };
/// assert_eq!(expected, serde_json::from_value(json).unwrap());
/// # }
/// ```
///
/// [`SystemTime`]: std::time::SystemTime
/// [`chrono::DateTime`]: ::chrono_0_4::DateTime
/// [`chrono::DateTime<Local>`]: ::chrono_0_4::DateTime
/// [`chrono::DateTime<Utc>`]: ::chrono_0_4::DateTime
/// [NaiveDateTime]: ::chrono_0_4::NaiveDateTime
/// [feature flag]: https://docs.rs/serde_with/3.14.0/serde_with/guide/feature_flags/index.html
pub struct TimestampSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`TimestampSeconds`] with milli-seconds as base unit.
///
/// This type is equivalent to [`TimestampSeconds`] except that each unit represents 1 milli-second instead of 1 second for [`TimestampSeconds`].
pub struct TimestampMilliSeconds<
    FORMAT: formats::Format = i64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`TimestampSecondsWithFrac`] with milli-seconds as base unit.
///
/// This type is equivalent to [`TimestampSecondsWithFrac`] except that each unit represents 1 milli-second instead of 1 second for [`TimestampSecondsWithFrac`].
pub struct TimestampMilliSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`TimestampSeconds`] with micro-seconds as base unit.
///
/// This type is equivalent to [`TimestampSeconds`] except that each unit represents 1 micro-second instead of 1 second for [`TimestampSeconds`].
pub struct TimestampMicroSeconds<
    FORMAT: formats::Format = i64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`TimestampSecondsWithFrac`] with micro-seconds as base unit.
///
/// This type is equivalent to [`TimestampSecondsWithFrac`] except that each unit represents 1 micro-second instead of 1 second for [`TimestampSecondsWithFrac`].
pub struct TimestampMicroSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`TimestampSeconds`] with nano-seconds as base unit.
///
/// This type is equivalent to [`TimestampSeconds`] except that each unit represents 1 nano-second instead of 1 second for [`TimestampSeconds`].
pub struct TimestampNanoSeconds<
    FORMAT: formats::Format = i64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Equivalent to [`TimestampSecondsWithFrac`] with nano-seconds as base unit.
///
/// This type is equivalent to [`TimestampSecondsWithFrac`] except that each unit represents 1 nano-second instead of 1 second for [`TimestampSecondsWithFrac`].
pub struct TimestampNanoSecondsWithFrac<
    FORMAT: formats::Format = f64,
    STRICTNESS: formats::Strictness = formats::Strict,
>(PhantomData<(FORMAT, STRICTNESS)>);

/// Optimized handling of owned and borrowed byte representations.
///
/// Serialization of byte sequences like `&[u8]` or `Vec<u8>` is quite inefficient since each value will be serialized individually.
/// This converter type optimizes the serialization and deserialization.
///
/// This is a port of the [`serde_bytes`] crate making it compatible with the `serde_as` annotation, which allows it to be used in more cases than provided by [`serde_bytes`].
///
/// The type provides de/serialization for these types:
///
/// * `[u8; N]`, not possible using `serde_bytes`
/// * `&[u8; N]`, not possible using `serde_bytes`
/// * `&[u8]`
/// * `Box<[u8; N]>`, not possible using `serde_bytes`
/// * `Box<[u8]>`
/// * `Vec<u8>`
/// * `Cow<'_, [u8]>`
/// * `Cow<'_, [u8; N]>`, not possible using `serde_bytes`
///
/// [`serde_bytes`]: https://crates.io/crates/serde_bytes
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, Bytes};
/// # use std::borrow::Cow;
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Test<'a> {
///     #[serde_as(as = "Bytes")]
///     array: [u8; 15],
///     #[serde_as(as = "Bytes")]
///     boxed: Box<[u8]>,
///     #[serde_as(as = "Bytes")]
///     #[serde(borrow)]
///     cow: Cow<'a, [u8]>,
///     #[serde_as(as = "Bytes")]
///     #[serde(borrow)]
///     cow_array: Cow<'a, [u8; 15]>,
///     #[serde_as(as = "Bytes")]
///     vec: Vec<u8>,
/// }
///
/// let value = Test {
///     array: *b"0123456789ABCDE",
///     boxed: b"...".to_vec().into_boxed_slice(),
///     cow: Cow::Borrowed(b"FooBar"),
///     cow_array: Cow::Borrowed(&[42u8; 15]),
///     vec: vec![0x41, 0x61, 0x21],
/// };
/// let expected = r#"(
///     array: b"0123456789ABCDE",
///     boxed: b"...",
///     cow: b"FooBar",
///     cow_array: b"***************",
///     vec: b"Aa!",
/// )"#;
///
/// # let pretty_config = ron::ser::PrettyConfig::new().new_line("\n");
/// assert_eq!(expected, ron::ser::to_string_pretty(&value, pretty_config).unwrap());
/// assert_eq!(value, ron::from_str(expected).unwrap());
/// # }
/// ```
///
/// Fully borrowed types can also be used but you'll need a Deserializer that
/// supports Serde's 0-copy deserialization:
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, Bytes};
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct TestBorrows<'a> {
///     #[serde_as(as = "Bytes")]
///     #[serde(borrow)]
///     array_buf: &'a [u8; 15],
///     #[serde_as(as = "Bytes")]
///     #[serde(borrow)]
///     buf: &'a [u8],
/// }
///
/// let value = TestBorrows {
///     array_buf: &[10u8; 15],
///     buf: &[20u8, 21u8, 22u8],
/// };
/// let expected = r#"(
///     array_buf: b"\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n",
///     buf: b"\x14\x15\x16",
/// )"#;
///
/// # let pretty_config = ron::ser::PrettyConfig::new().new_line("\n");
/// assert_eq!(expected, ron::ser::to_string_pretty(&value, pretty_config).unwrap());
/// // RON doesn't support borrowed deserialization of byte arrays
/// # }
/// ```
///
/// ## Alternative to [`BytesOrString`]
///
/// The [`Bytes`] can replace [`BytesOrString`].
/// [`Bytes`] is implemented for more types, which makes it better.
/// The serialization behavior of [`Bytes`] differs from [`BytesOrString`], therefore only `deserialize_as` should be used.
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use serde_json::json;
/// # use serde_with::{serde_as, Bytes};
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, serde::Serialize)]
/// struct Test {
///     #[serde_as(deserialize_as = "Bytes")]
///     from_bytes: Vec<u8>,
///     #[serde_as(deserialize_as = "Bytes")]
///     from_str: Vec<u8>,
/// }
///
/// // Different serialized values ...
/// let j = json!({
///     "from_bytes": [70,111,111,45,66,97,114],
///     "from_str": "Foo-Bar",
/// });
///
/// // can be deserialized ...
/// let test = Test {
///     from_bytes: b"Foo-Bar".to_vec(),
///     from_str: b"Foo-Bar".to_vec(),
/// };
/// assert_eq!(test, serde_json::from_value(j).unwrap());
///
/// // and serialization will always be a byte sequence
/// # assert_eq!(json!(
/// {
///     "from_bytes": [70,111,111,45,66,97,114],
///     "from_str": [70,111,111,45,66,97,114],
/// }
/// # ), serde_json::to_value(&test).unwrap());
/// # }
/// ```
pub struct Bytes;

/// Deserialize one or many elements
///
/// Sometimes it is desirable to have a shortcut in writing 1-element lists in a config file.
/// Usually, this is done by either writing a list or the list element itself.
/// This distinction is not semantically important on the Rust side, thus both forms should deserialize into the same `Vec`.
///
/// The `OneOrMany` adapter achieves exactly this use case.
/// The serialization behavior can be tweaked to either always serialize as a list using [`PreferMany`] or to serialize as the inner element if possible using [`PreferOne`].
/// By default, [`PreferOne`] is assumed, which can also be omitted like `OneOrMany<_>`.
///
/// [`PreferMany`]: crate::formats::PreferMany
/// [`PreferOne`]: crate::formats::PreferOne
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use serde_json::json;
/// # use serde_with::{serde_as, OneOrMany};
/// # use serde_with::formats::{PreferOne, PreferMany};
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, serde::Serialize)]
/// struct Data {
///     #[serde_as(as = "OneOrMany<_, PreferOne>")]
///     countries: Vec<String>,
///     #[serde_as(as = "OneOrMany<_, PreferMany>")]
///     cities: Vec<String>,
/// }
///
/// // The adapter allows deserializing a `Vec` from either
/// // a single element
/// let j = json!({
///     "countries": "Spain",
///     "cities": "Berlin",
/// });
/// assert!(serde_json::from_value::<Data>(j).is_ok());
///
/// // or from a list.
/// let j = json!({
///     "countries": ["Germany", "France"],
///     "cities": ["Amsterdam"],
/// });
/// assert!(serde_json::from_value::<Data>(j).is_ok());
///
/// // For serialization you can choose how a single element should be encoded.
/// // Either directly, with `PreferOne` (default), or as a list with `PreferMany`.
/// let data = Data {
///     countries: vec!["Spain".to_string()],
///     cities: vec!["Berlin".to_string()],
/// };
/// let j = json!({
///     "countries": "Spain",
///     "cities": ["Berlin"],
/// });
/// assert_eq!(serde_json::to_value(data).unwrap(), j);
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct OneOrMany<T, FORMAT: formats::Format = formats::PreferOne>(PhantomData<(T, FORMAT)>);

/// Try multiple deserialization options until one succeeds.
///
/// This adapter allows you to specify a list of deserialization options.
/// They are tried in order and the first one working is applied.
/// Serialization always picks the first option.
///
/// `PickFirst` has one type parameter which must be instantiated with a tuple of two, three, or four elements.
/// For example, `PickFirst<(_, DisplayFromStr)>` on a field of type `u32` allows deserializing from a number or from a string via the `FromStr` trait.
/// The value will be serialized as a number, since that is what the first type `_` indicates.
///
/// # Examples
///
/// Deserialize a number from either a number or a string.
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, DisplayFromStr, PickFirst};
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Data {
///     #[serde_as(as = "PickFirst<(_, DisplayFromStr)>")]
///     as_number: u32,
///     #[serde_as(as = "PickFirst<(DisplayFromStr, _)>")]
///     as_string: u32,
/// }
/// let data = Data {
///     as_number: 123,
///     as_string: 456
/// };
///
/// // Both fields can be deserialized from numbers:
/// let j = json!({
///     "as_number": 123,
///     "as_string": 456,
/// });
/// assert_eq!(data, serde_json::from_value(j).unwrap());
///
/// // or from a string:
/// let j = json!({
///     "as_number": "123",
///     "as_string": "456",
/// });
/// assert_eq!(data, serde_json::from_value(j).unwrap());
///
/// // For serialization the first type in the tuple determines the behavior.
/// // The `as_number` field will use the normal `Serialize` behavior and produce a number,
/// // while `as_string` used `Display` to produce a string.
/// let expected = json!({
///     "as_number": 123,
///     "as_string": "456",
/// });
/// assert_eq!(expected, serde_json::to_value(&data).unwrap());
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct PickFirst<T>(PhantomData<T>);

/// Serialize value by converting to/from a proxy type with serde support.
///
/// This adapter serializes a type `O` by converting it into a second type `T` and serializing `T`.
/// Deserializing works analogue, by deserializing a `T` and then converting into `O`.
///
/// ```rust
/// # #[cfg(any())] {
/// struct S {
///     #[serde_as(as = "FromInto<T>")]
///     value: O,
/// }
/// # }
/// ```
///
/// For serialization `O` needs to be `O: Into<T> + Clone`.
/// For deserialization the opposite `T: Into<O>` is required.
/// The `Clone` bound is required since `serialize` operates on a reference but `Into` implementations on references are uncommon.
///
/// **Note**: [`TryFromInto`] is the more generalized version of this adapter which uses the [`TryInto`] trait instead.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, FromInto};
/// #
/// #[derive(Clone, Debug, PartialEq)]
/// struct Rgb {
///     red: u8,
///     green: u8,
///     blue: u8,
/// }
///
/// # /*
/// impl From<(u8, u8, u8)> for Rgb { ... }
/// impl From<Rgb> for (u8, u8, u8) { ... }
/// # */
/// #
/// # impl From<(u8, u8, u8)> for Rgb {
/// #     fn from(v: (u8, u8, u8)) -> Self {
/// #         Rgb {
/// #             red: v.0,
/// #             green: v.1,
/// #             blue: v.2,
/// #         }
/// #     }
/// # }
/// #
/// # impl From<Rgb> for (u8, u8, u8) {
/// #     fn from(v: Rgb) -> Self {
/// #         (v.red, v.green, v.blue)
/// #     }
/// # }
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Color {
///     #[serde_as(as = "FromInto<(u8, u8, u8)>")]
///     rgb: Rgb,
/// }
/// let color = Color {
///     rgb: Rgb {
///         red: 128,
///         green: 64,
///         blue: 32,
///     },
/// };
///
/// // Define our expected JSON form
/// let j = json!({
///     "rgb": [128, 64, 32],
/// });
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(j, serde_json::to_value(&color).unwrap());
/// assert_eq!(color, serde_json::from_value(j).unwrap());
/// # }
/// ```
pub struct FromInto<T>(PhantomData<T>);

/// Serialize a reference value by converting to/from a proxy type with serde support.
///
/// This adapter serializes a type `O` by converting it into a second type `T` and serializing `T`.
/// Deserializing works analogue, by deserializing a `T` and then converting into `O`.
///
/// ```rust
/// # #[cfg(any())] {
/// struct S {
///     #[serde_as(as = "FromIntoRef<T>")]
///     value: O,
/// }
/// # }
/// ```
///
/// For serialization `O` needs to be `for<'a> &'a O: Into<T>`.
/// For deserialization the opposite `T: Into<O>` is required.
///
/// **Note**: [`TryFromIntoRef`] is the more generalized version of this adapter which uses the [`TryInto`] trait instead.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, FromIntoRef};
/// #
/// #[derive(Debug, PartialEq)]
/// struct Rgb {
///     red: u8,
///     green: u8,
///     blue: u8,
/// }
///
/// # /*
/// impl From<(u8, u8, u8)> for Rgb { ... }
/// impl From<Rgb> for (u8, u8, u8) { ... }
/// # */
/// #
/// # impl From<(u8, u8, u8)> for Rgb {
/// #     fn from(v: (u8, u8, u8)) -> Self {
/// #         Rgb {
/// #             red: v.0,
/// #             green: v.1,
/// #             blue: v.2,
/// #         }
/// #     }
/// # }
/// #
/// # impl<'a> From<&'a Rgb> for (u8, u8, u8) {
/// #     fn from(v: &'a Rgb) -> Self {
/// #         (v.red, v.green, v.blue)
/// #     }
/// # }
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Color {
///     #[serde_as(as = "FromIntoRef<(u8, u8, u8)>")]
///     rgb: Rgb,
/// }
/// let color = Color {
///     rgb: Rgb {
///         red: 128,
///         green: 64,
///         blue: 32,
///     },
/// };
///
/// // Define our expected JSON form
/// let j = json!({
///     "rgb": [128, 64, 32],
/// });
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(j, serde_json::to_value(&color).unwrap());
/// assert_eq!(color, serde_json::from_value(j).unwrap());
/// # }
/// ```
pub struct FromIntoRef<T>(PhantomData<T>);

/// Serialize value by converting to/from a proxy type with serde support.
///
/// This adapter serializes a type `O` by converting it into a second type `T` and serializing `T`.
/// Deserializing works analogue, by deserializing a `T` and then converting into `O`.
///
/// ```rust
/// # #[cfg(any())] {
/// struct S {
///     #[serde_as(as = "TryFromInto<T>")]
///     value: O,
/// }
/// # }
/// ```
///
/// For serialization `O` needs to be `O: TryInto<T> + Clone`.
/// For deserialization the opposite `T: TryInto<O>` is required.
/// The `Clone` bound is required since `serialize` operates on a reference but `TryInto` implementations on references are uncommon.
/// In both cases the `TryInto::Error` type must implement [`Display`](std::fmt::Display).
///
/// **Note**: [`FromInto`] is the more specialized version of this adapter which uses the infallible [`Into`] trait instead.
/// [`TryFromInto`] is strictly more general and can also be used where [`FromInto`] is applicable.
/// The example shows a use case, when only the deserialization behavior is fallible, but not serializing.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, TryFromInto};
/// #
/// #[derive(Clone, Debug, PartialEq)]
/// enum Boollike {
///     True,
///     False,
/// }
///
/// # /*
/// impl From<Boollike> for u8 { ... }
/// # */
/// #
/// impl TryFrom<u8> for Boollike {
///     type Error = String;
///     fn try_from(v: u8) -> Result<Self, Self::Error> {
///         match v {
///             0 => Ok(Boollike::False),
///             1 => Ok(Boollike::True),
///             _ => Err(format!("Boolikes can only be constructed from 0 or 1 but found {}", v))
///         }
///     }
/// }
/// #
/// # impl From<Boollike> for u8 {
/// #     fn from(v: Boollike) -> Self {
/// #        match v {
/// #            Boollike::True => 1,
/// #            Boollike::False => 0,
/// #        }
/// #     }
/// # }
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Data {
///     #[serde_as(as = "TryFromInto<u8>")]
///     b: Boollike,
/// }
/// let data = Data {
///     b: Boollike::True,
/// };
///
/// // Define our expected JSON form
/// let j = json!({
///     "b": 1,
/// });
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(j, serde_json::to_value(&data).unwrap());
/// assert_eq!(data, serde_json::from_value(j).unwrap());
///
/// // Numbers besides 0 or 1 should be an error
/// let j = json!({
///     "b": 2,
/// });
/// assert_eq!("Boolikes can only be constructed from 0 or 1 but found 2", serde_json::from_value::<Data>(j).unwrap_err().to_string());
/// # }
/// ```
pub struct TryFromInto<T>(PhantomData<T>);

/// Serialize a reference value by converting to/from a proxy type with serde support.
///
/// This adapter serializes a type `O` by converting it into a second type `T` and serializing `T`.
/// Deserializing works analogue, by deserializing a `T` and then converting into `O`.
///
/// ```rust
/// # #[cfg(any())] {
/// struct S {
///     #[serde_as(as = "TryFromIntoRef<T>")]
///     value: O,
/// }
/// # }
/// ```
///
/// For serialization `O` needs to be `for<'a> &'a O: TryInto<T>`.
/// For deserialization the opposite `T: TryInto<O>` is required.
/// In both cases the `TryInto::Error` type must implement [`Display`](std::fmt::Display).
///
/// **Note**: [`FromIntoRef`] is the more specialized version of this adapter which uses the infallible [`Into`] trait instead.
/// [`TryFromIntoRef`] is strictly more general and can also be used where [`FromIntoRef`] is applicable.
/// The example shows a use case, when only the deserialization behavior is fallible, but not serializing.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, TryFromIntoRef};
/// #
/// #[derive(Debug, PartialEq)]
/// enum Boollike {
///     True,
///     False,
/// }
///
/// # /*
/// impl From<Boollike> for u8 { ... }
/// # */
/// #
/// impl TryFrom<u8> for Boollike {
///     type Error = String;
///     fn try_from(v: u8) -> Result<Self, Self::Error> {
///         match v {
///             0 => Ok(Boollike::False),
///             1 => Ok(Boollike::True),
///             _ => Err(format!("Boolikes can only be constructed from 0 or 1 but found {}", v))
///         }
///     }
/// }
/// #
/// # impl<'a> From<&'a Boollike> for u8 {
/// #     fn from(v: &'a Boollike) -> Self {
/// #        match v {
/// #            Boollike::True => 1,
/// #            Boollike::False => 0,
/// #        }
/// #     }
/// # }
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Data {
///     #[serde_as(as = "TryFromIntoRef<u8>")]
///     b: Boollike,
/// }
/// let data = Data {
///     b: Boollike::True,
/// };
///
/// // Define our expected JSON form
/// let j = json!({
///     "b": 1,
/// });
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(j, serde_json::to_value(&data).unwrap());
/// assert_eq!(data, serde_json::from_value(j).unwrap());
///
/// // Numbers besides 0 or 1 should be an error
/// let j = json!({
///     "b": 2,
/// });
/// assert_eq!("Boolikes can only be constructed from 0 or 1 but found 2", serde_json::from_value::<Data>(j).unwrap_err().to_string());
/// # }
/// ```
pub struct TryFromIntoRef<T>(PhantomData<T>);

/// Borrow `Cow` data during deserialization when possible.
///
/// The types `Cow<'a, [u8]>`, `Cow<'a, [u8; N]>`, and `Cow<'a, str>` can borrow from the input data during deserialization.
/// serde supports this, by annotating the fields with `#[serde(borrow)]`. but does not support borrowing on nested types.
/// This gap is filled by this `BorrowCow` adapter.
///
/// Using this adapter with `Cow<'a, [u8]>`/`Cow<'a, [u8; N]>` will serialize the value as a sequence of `u8` values.
/// This *might* not allow to borrow the data during deserialization.
/// For a different format, which is also more efficient, use the [`Bytes`] adapter, which is also implemented for `Cow`.
///
/// When combined with the [`serde_as`] attribute, the `#[serde(borrow)]` annotation will be added automatically.
/// If the annotation is wrong or too broad, for example because of multiple lifetime parameters, a manual annotation is required.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, BorrowCow};
/// # use std::borrow::Cow;
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Data<'a, 'b, 'c> {
///     #[serde_as(as = "BorrowCow")]
///     str: Cow<'a, str>,
///     #[serde_as(as = "BorrowCow")]
///     slice: Cow<'b, [u8]>,
///
///     #[serde_as(as = "Option<[BorrowCow; 1]>")]
///     nested: Option<[Cow<'c, str>; 1]>,
/// }
/// let data = Data {
///     str: "foobar".into(),
///     slice: b"foobar"[..].into(),
///     nested: Some(["HelloWorld".into()]),
/// };
///
/// // Define our expected JSON form
/// let j = r#"{
///   "str": "foobar",
///   "slice": [
///     102,
///     111,
///     111,
///     98,
///     97,
///     114
///   ],
///   "nested": [
///     "HelloWorld"
///   ]
/// }"#;
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(j, serde_json::to_string_pretty(&data).unwrap());
/// assert_eq!(data, serde_json::from_str(j).unwrap());
///
/// // Cow borrows from the input data
/// let deserialized: Data<'_, '_, '_> = serde_json::from_str(j).unwrap();
/// assert!(matches!(deserialized.str, Cow::Borrowed(_)));
/// assert!(matches!(deserialized.nested, Some([Cow::Borrowed(_)])));
/// // JSON does not allow borrowing bytes, so `slice` does not borrow
/// assert!(matches!(deserialized.slice, Cow::Owned(_)));
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct BorrowCow;

/// Deserialize a sequence into `Vec<T>`, skipping elements which fail to deserialize.
///
/// The serialization behavior is identical to `Vec<T>`. This is an alternative to `Vec<T>`
/// which is resilient against unexpected data.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, VecSkipError};
/// #
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// # #[non_exhaustive]
/// enum Color {
///     Red,
///     Green,
///     Blue,
/// }
/// # use Color::*;
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Palette(#[serde_as(as = "VecSkipError<_>")] Vec<Color>);
///
/// let data = Palette(vec![Blue, Green,]);
/// let source_json = r#"["Blue", "Yellow", "Green"]"#;
/// let data_json = r#"["Blue","Green"]"#;
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(data_json, serde_json::to_string(&data).unwrap());
/// assert_eq!(data, serde_json::from_str(source_json).unwrap());
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct VecSkipError<T>(PhantomData<T>);

/// Deserialize a map, skipping keys and values which fail to deserialize.
///
/// By default serde terminates if it fails to deserialize a key or a value when deserializing
/// a map. Sometimes a map has heterogeneous keys or values but we only care about some specific
/// types, and it is desirable to skip entries on errors.
///
/// It is especially useful in conjunction to `#[serde(flatten)]` to capture a map mixed in with
/// other entries which we don't want to exhaust in the type definition.
///
/// The serialization behavior is identical to the underlying map.
///
/// The implementation supports both the [`HashMap`] and the [`BTreeMap`] from the standard library.
///
/// [`BTreeMap`]: std::collections::BTreeMap
/// [`HashMap`]: std::collections::HashMap
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use std::collections::BTreeMap;
/// # use serde_with::{serde_as, DisplayFromStr, MapSkipError};
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct VersionNames {
///     yanked: Vec<u16>,
///     #[serde_as(as = "MapSkipError<DisplayFromStr, _>")]
///     #[serde(flatten)]
///     names: BTreeMap<u16, String>,
/// }
///
/// let data = VersionNames {
///     yanked: vec![2, 5],
///     names: BTreeMap::from_iter([
///         (0u16, "v0".to_string()),
///         (1, "v1".to_string()),
///         (4, "v4".to_string())
///     ]),
/// };
/// let source_json = r#"{
///   "0": "v0",
///   "1": "v1",
///   "4": "v4",
///   "yanked": [2, 5],
///   "last_updated": 1704085200
/// }"#;
/// let data_json = r#"{"yanked":[2,5],"0":"v0","1":"v1","4":"v4"}"#;
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(data_json, serde_json::to_string(&data).unwrap());
/// assert_eq!(data, serde_json::from_str(source_json).unwrap());
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct MapSkipError<K, V>(PhantomData<(K, V)>);

/// Deserialize a boolean from a number
///
/// Deserialize a number (of `u8`) and turn it into a boolean.
/// The adapter supports a [`Strict`](crate::formats::Strict) and [`Flexible`](crate::formats::Flexible) format.
/// In `Strict` mode, the number must be `0` or `1`.
/// All other values produce an error.
/// In `Flexible` mode, the number any non-zero value is converted to `true`.
///
/// During serialization only `0` or `1` are ever emitted.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, BoolFromInt};
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Data(#[serde_as(as = "BoolFromInt")] bool);
///
/// let data = Data(true);
/// let j = json!(1);
/// // Ensure serialization and deserialization produce the expected results
/// assert_eq!(j, serde_json::to_value(&data).unwrap());
/// assert_eq!(data, serde_json::from_value(j).unwrap());
///
/// // false maps to 0
/// let data = Data(false);
/// let j = json!(0);
/// assert_eq!(j, serde_json::to_value(&data).unwrap());
/// assert_eq!(data, serde_json::from_value(j).unwrap());
//
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Flexible(#[serde_as(as = "BoolFromInt<serde_with::formats::Flexible>")] bool);
///
/// // Flexible turns any non-zero number into true
/// let data = Flexible(true);
/// let j = json!(100);
/// assert_eq!(data, serde_json::from_value(j).unwrap());
/// # }
/// ```
pub struct BoolFromInt<S: formats::Strictness = formats::Strict>(PhantomData<S>);

/// De/Serialize a delimited collection using [`Display`] and [`FromStr`] implementation
///
/// `StringWithSeparator` takes a second type, which needs to implement [`Display`]+[`FromStr`] and constitutes the inner type of the collection.
/// You can define an arbitrary separator, by specifying a type which implements [`Separator`].
/// Some common ones, like space and comma are already predefined and you can find them [here][`Separator`].
///
/// An empty string deserializes as an empty collection.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// #
/// # use serde_with::{serde_as, StringWithSeparator};
/// use serde_with::formats::{CommaSeparator, SpaceSeparator};
/// use std::collections::BTreeSet;
///
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "StringWithSeparator::<SpaceSeparator, String>")]
///     tags: Vec<String>,
///     #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
///     more_tags: BTreeSet<String>,
/// }
///
/// let v: A = serde_json::from_str(r##"{
///     "tags": "#hello #world",
///     "more_tags": "foo,bar,bar"
/// }"##).unwrap();
/// assert_eq!(vec!["#hello", "#world"], v.tags);
/// assert_eq!(2, v.more_tags.len());
///
/// let x = A {
///     tags: vec!["1".to_string(), "2".to_string(), "3".to_string()],
///     more_tags: BTreeSet::new(),
/// };
/// assert_eq!(
///     r#"{"tags":"1 2 3","more_tags":""}"#,
///     serde_json::to_string(&x).unwrap()
/// );
/// # }
/// ```
///
/// [`Display`]: core::fmt::Display
/// [`FromStr`]: core::str::FromStr
/// [`Separator`]: crate::formats::Separator
/// [`serde_as`]: crate::guide::serde_as
pub struct StringWithSeparator<Sep, T>(PhantomData<(Sep, T)>);

/// This serializes a list of tuples into a map
///
/// Normally, you want to use a [`HashMap`] or a [`BTreeMap`] when deserializing a map.
/// However, sometimes this is not possible due to type constraints, e.g., if the type implements neither [`Hash`] nor [`Ord`].
/// Another use case is deserializing a map with duplicate keys.
///
/// # Examples
///
/// `Wrapper` does not implement [`Hash`] nor [`Ord`], thus prohibiting the use [`HashMap`] or [`BTreeMap`].
/// The JSON also contains a duplicate key.
///
/// [`BTreeMap`]: std::collections::BTreeMap
/// [`HashMap`]: std::collections::HashMap
/// [`Vec`]: std::vec::Vec
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, Map};
/// #
/// #[serde_as]
/// #[derive(Debug, Deserialize, Serialize, Default)]
/// struct S {
///     #[serde_as(as = "Map<_, _>")]
///     s: Vec<(Wrapper<i32>, String)>,
/// }
///
/// #[derive(Clone, Debug, Serialize, Deserialize)]
/// #[serde(transparent)]
/// struct Wrapper<T>(T);
///
/// let data = S {
///     s: vec![
///         (Wrapper(1), "a".to_string()),
///         (Wrapper(2), "b".to_string()),
///         (Wrapper(3), "c".to_string()),
///         (Wrapper(2), "d".to_string()),
///     ],
/// };
///
/// let json = r#"{
///   "s": {
///     "1": "a",
///     "2": "b",
///     "3": "c",
///     "2": "d"
///   }
/// }"#;
/// assert_eq!(json, serde_json::to_string_pretty(&data).unwrap());
/// # }
/// ```
pub struct Map<K, V>(PhantomData<(K, V)>);

/// De/Serialize a Map into a list of tuples
///
/// Some formats, like JSON, have limitations on the types of keys for maps.
/// In case of JSON, keys are restricted to strings.
/// Rust features more powerful keys, for example tuples, which can not be serialized to JSON.
///
/// This helper serializes the Map into a list of tuples, which do not have the same type restrictions.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::{serde_as, Seq};
/// # use std::collections::BTreeMap;
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "Seq<(_, _)>")]
///     s: BTreeMap<(String, u32), u32>,
/// }
///
/// // This converts the Rust type
/// let data = A {
///     s: BTreeMap::from([
///         (("Hello".to_string(), 123), 0),
///         (("World".to_string(), 456), 1),
///     ]),
/// };
///
/// // into this JSON
/// let value = json!({
///     "s": [
///         [["Hello", 123], 0],
///         [["World", 456], 1]
///     ]
/// });
///
/// assert_eq!(value, serde_json::to_value(&data).unwrap());
/// assert_eq!(data, serde_json::from_value(value).unwrap());
/// # }
/// ```
pub struct Seq<V>(PhantomData<V>);

/// Ensure no duplicate keys exist in a map.
///
/// By default serde has a last-value-wins implementation, if duplicate keys for a map exist.
/// Sometimes it is desirable to know when such an event happens, as the first value is overwritten
/// and it can indicate an error in the serialized data.
///
/// This helper returns an error if two identical keys exist in a map.
///
/// The implementation supports both the [`HashMap`] and the [`BTreeMap`] from the standard library.
///
/// [`BTreeMap`]: std::collections::BTreeMap
/// [`HashMap`]: std::collections::HashMap
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::Deserialize;
/// # use std::collections::HashMap;
/// # use serde_with::{serde_as, MapPreventDuplicates};
/// #
/// #[serde_as]
/// # #[derive(Debug, Eq, PartialEq)]
/// #[derive(Deserialize)]
/// struct Doc {
///     #[serde_as(as = "MapPreventDuplicates<_, _>")]
///     map: HashMap<usize, usize>,
/// }
///
/// // Maps are serialized normally,
/// let s = r#"{"map": {"1": 1, "2": 2, "3": 3}}"#;
/// let mut v = Doc {
///     map: HashMap::new(),
/// };
/// v.map.insert(1, 1);
/// v.map.insert(2, 2);
/// v.map.insert(3, 3);
/// assert_eq!(v, serde_json::from_str(s).unwrap());
///
/// // but create an error if duplicate keys, like the `1`, exist.
/// let s = r#"{"map": {"1": 1, "2": 2, "1": 3}}"#;
/// let res: Result<Doc, _> = serde_json::from_str(s);
/// assert!(res.is_err());
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct MapPreventDuplicates<K, V>(PhantomData<(K, V)>);

/// Ensure that the first key is taken, if duplicate keys exist
///
/// By default serde has a last-key-wins implementation, if duplicate keys for a map exist.
/// Sometimes the opposite strategy is desired. This helper implements a first-key-wins strategy.
///
/// The implementation supports both the [`HashMap`] and the [`BTreeMap`] from the standard library.
///
/// [`BTreeMap`]: std::collections::BTreeMap
/// [`HashMap`]: std::collections::HashMap
#[cfg(feature = "alloc")]
pub struct MapFirstKeyWins<K, V>(PhantomData<(K, V)>);

/// Ensure no duplicate values exist in a set.
///
/// By default serde has a last-value-wins implementation, if duplicate values for a set exist.
/// Sometimes it is desirable to know when such an event happens, as the first value is overwritten
/// and it can indicate an error in the serialized data.
///
/// This helper returns an error if two identical values exist in a set.
///
/// The implementation supports both the [`HashSet`] and the [`BTreeSet`] from the standard library.
///
/// [`BTreeSet`]: std::collections::BTreeSet
/// [`HashSet`]: std::collections::HashSet
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use std::collections::HashSet;
/// # use serde::Deserialize;
/// # use serde_with::{serde_as, SetPreventDuplicates};
/// #
/// #[serde_as]
/// # #[derive(Debug, Eq, PartialEq)]
/// #[derive(Deserialize)]
/// struct Doc {
///     #[serde_as(as = "SetPreventDuplicates<_>")]
///     set: HashSet<usize>,
/// }
///
/// // Sets are serialized normally,
/// let s = r#"{"set": [1, 2, 3, 4]}"#;
/// let v = Doc {
///     set: HashSet::from_iter(vec![1, 2, 3, 4]),
/// };
/// assert_eq!(v, serde_json::from_str(s).unwrap());
///
/// // but create an error if duplicate values, like the `1`, exist.
/// let s = r#"{"set": [1, 2, 3, 4, 1]}"#;
/// let res: Result<Doc, _> = serde_json::from_str(s);
/// assert!(res.is_err());
/// # }
/// ```
#[cfg(feature = "alloc")]
pub struct SetPreventDuplicates<T>(PhantomData<T>);

/// Ensure that the last value is taken, if duplicate values exist
///
/// By default serde has a first-value-wins implementation, if duplicate keys for a set exist.
/// Sometimes the opposite strategy is desired. This helper implements a first-value-wins strategy.
///
/// The implementation supports both the [`HashSet`] and the [`BTreeSet`] from the standard library.
///
/// [`BTreeSet`]: std::collections::BTreeSet
/// [`HashSet`]: std::collections::HashSet
#[cfg(feature = "alloc")]
pub struct SetLastValueWins<T>(PhantomData<T>);

/// Helper for implementing [`JsonSchema`] on serializers whose output depends
/// on the type of the concrete field.
///
/// It is added implicitly by the [`#[serde_as]`](crate::serde_as) macro when any `schemars`
/// feature is enabled.
///
/// [`JsonSchema`]: ::schemars_1::JsonSchema
#[cfg(any(
    feature = "schemars_0_8",
    feature = "schemars_0_9",
    feature = "schemars_1"
))]
pub struct Schema<T: ?Sized, TA>(PhantomData<T>, PhantomData<TA>);
