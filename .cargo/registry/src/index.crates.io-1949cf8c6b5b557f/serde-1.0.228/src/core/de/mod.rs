//! Generic data structure deserialization framework.
//!
//! The two most important traits in this module are [`Deserialize`] and
//! [`Deserializer`].
//!
//!  - **A type that implements `Deserialize` is a data structure** that can be
//!    deserialized from any data format supported by Serde, and conversely
//!  - **A type that implements `Deserializer` is a data format** that can
//!    deserialize any data structure supported by Serde.
//!
//! # The Deserialize trait
//!
//! Serde provides [`Deserialize`] implementations for many Rust primitive and
//! standard library types. The complete list is below. All of these can be
//! deserialized using Serde out of the box.
//!
//! Additionally, Serde provides a procedural macro called [`serde_derive`] to
//! automatically generate [`Deserialize`] implementations for structs and enums
//! in your program. See the [derive section of the manual] for how to use this.
//!
//! In rare cases it may be necessary to implement [`Deserialize`] manually for
//! some type in your program. See the [Implementing `Deserialize`] section of
//! the manual for more about this.
//!
//! Third-party crates may provide [`Deserialize`] implementations for types
//! that they expose. For example the [`linked-hash-map`] crate provides a
//! [`LinkedHashMap<K, V>`] type that is deserializable by Serde because the
//! crate provides an implementation of [`Deserialize`] for it.
//!
//! # The Deserializer trait
//!
//! [`Deserializer`] implementations are provided by third-party crates, for
//! example [`serde_json`], [`serde_yaml`] and [`postcard`].
//!
//! A partial list of well-maintained formats is given on the [Serde
//! website][data formats].
//!
//! # Implementations of Deserialize provided by Serde
//!
//! This is a slightly different set of types than what is supported for
//! serialization. Some types can be serialized by Serde but not deserialized.
//! One example is `OsStr`.
//!
//!  - **Primitive types**:
//!    - bool
//!    - i8, i16, i32, i64, i128, isize
//!    - u8, u16, u32, u64, u128, usize
//!    - f32, f64
//!    - char
//!  - **Compound types**:
//!    - \[T; 0\] through \[T; 32\]
//!    - tuples up to size 16
//!  - **Common standard library types**:
//!    - String
//!    - Option\<T\>
//!    - Result\<T, E\>
//!    - PhantomData\<T\>
//!  - **Wrapper types**:
//!    - Box\<T\>
//!    - Box\<\[T\]\>
//!    - Box\<str\>
//!    - Cow\<'a, T\>
//!    - Cell\<T\>
//!    - RefCell\<T\>
//!    - Mutex\<T\>
//!    - RwLock\<T\>
//!    - Rc\<T\>&emsp;*(if* features = \["rc"\] *is enabled)*
//!    - Arc\<T\>&emsp;*(if* features = \["rc"\] *is enabled)*
//!  - **Collection types**:
//!    - BTreeMap\<K, V\>
//!    - BTreeSet\<T\>
//!    - BinaryHeap\<T\>
//!    - HashMap\<K, V, H\>
//!    - HashSet\<T, H\>
//!    - LinkedList\<T\>
//!    - VecDeque\<T\>
//!    - Vec\<T\>
//!  - **Zero-copy types**:
//!    - &str
//!    - &\[u8\]
//!  - **FFI types**:
//!    - CString
//!    - Box\<CStr\>
//!    - OsString
//!  - **Miscellaneous standard library types**:
//!    - Duration
//!    - SystemTime
//!    - Path
//!    - PathBuf
//!    - Range\<T\>
//!    - RangeInclusive\<T\>
//!    - Bound\<T\>
//!    - num::NonZero*
//!    - `!` *(unstable)*
//!  - **Net types**:
//!    - IpAddr
//!    - Ipv4Addr
//!    - Ipv6Addr
//!    - SocketAddr
//!    - SocketAddrV4
//!    - SocketAddrV6
//!
//! [Implementing `Deserialize`]: https://serde.rs/impl-deserialize.html
//! [`Deserialize`]: crate::Deserialize
//! [`Deserializer`]: crate::Deserializer
//! [`LinkedHashMap<K, V>`]: https://docs.rs/linked-hash-map/*/linked_hash_map/struct.LinkedHashMap.html
//! [`postcard`]: https://github.com/jamesmunns/postcard
//! [`linked-hash-map`]: https://crates.io/crates/linked-hash-map
//! [`serde_derive`]: https://crates.io/crates/serde_derive
//! [`serde_json`]: https://github.com/serde-rs/json
//! [`serde_yaml`]: https://github.com/dtolnay/serde-yaml
//! [derive section of the manual]: https://serde.rs/derive.html
//! [data formats]: https://serde.rs/#data-formats

use crate::lib::*;

////////////////////////////////////////////////////////////////////////////////

pub mod value;

mod ignored_any;
mod impls;

pub use self::ignored_any::IgnoredAny;
#[cfg(all(not(feature = "std"), no_core_error))]
#[doc(no_inline)]
pub use crate::std_error::Error as StdError;
#[cfg(not(any(feature = "std", no_core_error)))]
#[doc(no_inline)]
pub use core::error::Error as StdError;
#[cfg(feature = "std")]
#[doc(no_inline)]
pub use std::error::Error as StdError;

////////////////////////////////////////////////////////////////////////////////

macro_rules! declare_error_trait {
    (Error: Sized $(+ $($supertrait:ident)::+)*) => {
        /// The `Error` trait allows `Deserialize` implementations to create descriptive
        /// error messages belonging to the `Deserializer` against which they are
        /// currently running.
        ///
        /// Every `Deserializer` declares an `Error` type that encompasses both
        /// general-purpose deserialization errors as well as errors specific to the
        /// particular deserialization format. For example the `Error` type of
        /// `serde_json` can represent errors like an invalid JSON escape sequence or an
        /// unterminated string literal, in addition to the error cases that are part of
        /// this trait.
        ///
        /// Most deserializers should only need to provide the `Error::custom` method
        /// and inherit the default behavior for the other methods.
        ///
        /// # Example implementation
        ///
        /// The [example data format] presented on the website shows an error
        /// type appropriate for a basic JSON data format.
        ///
        /// [example data format]: https://serde.rs/data-format.html
        #[cfg_attr(
            not(no_diagnostic_namespace),
            diagnostic::on_unimplemented(
                message = "the trait bound `{Self}: serde::de::Error` is not satisfied",
            )
        )]
        pub trait Error: Sized $(+ $($supertrait)::+)* {
            /// Raised when there is general error when deserializing a type.
            ///
            /// The message should not be capitalized and should not end with a period.
            ///
            /// ```edition2021
            /// # use std::str::FromStr;
            /// #
            /// # struct IpAddr;
            /// #
            /// # impl FromStr for IpAddr {
            /// #     type Err = String;
            /// #
            /// #     fn from_str(_: &str) -> Result<Self, String> {
            /// #         unimplemented!()
            /// #     }
            /// # }
            /// #
            /// use serde::de::{self, Deserialize, Deserializer};
            ///
            /// impl<'de> Deserialize<'de> for IpAddr {
            ///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            ///     where
            ///         D: Deserializer<'de>,
            ///     {
            ///         let s = String::deserialize(deserializer)?;
            ///         s.parse().map_err(de::Error::custom)
            ///     }
            /// }
            /// ```
            fn custom<T>(msg: T) -> Self
            where
                T: Display;

            /// Raised when a `Deserialize` receives a type different from what it was
            /// expecting.
            ///
            /// The `unexp` argument provides information about what type was received.
            /// This is the type that was present in the input file or other source data
            /// of the Deserializer.
            ///
            /// The `exp` argument provides information about what type was being
            /// expected. This is the type that is written in the program.
            ///
            /// For example if we try to deserialize a String out of a JSON file
            /// containing an integer, the unexpected type is the integer and the
            /// expected type is the string.
            #[cold]
            fn invalid_type(unexp: Unexpected, exp: &dyn Expected) -> Self {
                Error::custom(format_args!("invalid type: {}, expected {}", unexp, exp))
            }

            /// Raised when a `Deserialize` receives a value of the right type but that
            /// is wrong for some other reason.
            ///
            /// The `unexp` argument provides information about what value was received.
            /// This is the value that was present in the input file or other source
            /// data of the Deserializer.
            ///
            /// The `exp` argument provides information about what value was being
            /// expected. This is the type that is written in the program.
            ///
            /// For example if we try to deserialize a String out of some binary data
            /// that is not valid UTF-8, the unexpected value is the bytes and the
            /// expected value is a string.
            #[cold]
            fn invalid_value(unexp: Unexpected, exp: &dyn Expected) -> Self {
                Error::custom(format_args!("invalid value: {}, expected {}", unexp, exp))
            }

            /// Raised when deserializing a sequence or map and the input data contains
            /// too many or too few elements.
            ///
            /// The `len` argument is the number of elements encountered. The sequence
            /// or map may have expected more arguments or fewer arguments.
            ///
            /// The `exp` argument provides information about what data was being
            /// expected. For example `exp` might say that a tuple of size 6 was
            /// expected.
            #[cold]
            fn invalid_length(len: usize, exp: &dyn Expected) -> Self {
                Error::custom(format_args!("invalid length {}, expected {}", len, exp))
            }

            /// Raised when a `Deserialize` enum type received a variant with an
            /// unrecognized name.
            #[cold]
            fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self {
                if expected.is_empty() {
                    Error::custom(format_args!(
                        "unknown variant `{}`, there are no variants",
                        variant
                    ))
                } else {
                    Error::custom(format_args!(
                        "unknown variant `{}`, expected {}",
                        variant,
                        OneOf { names: expected }
                    ))
                }
            }

            /// Raised when a `Deserialize` struct type received a field with an
            /// unrecognized name.
            #[cold]
            fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self {
                if expected.is_empty() {
                    Error::custom(format_args!(
                        "unknown field `{}`, there are no fields",
                        field
                    ))
                } else {
                    Error::custom(format_args!(
                        "unknown field `{}`, expected {}",
                        field,
                        OneOf { names: expected }
                    ))
                }
            }

            /// Raised when a `Deserialize` struct type expected to receive a required
            /// field with a particular name but that field was not present in the
            /// input.
            #[cold]
            fn missing_field(field: &'static str) -> Self {
                Error::custom(format_args!("missing field `{}`", field))
            }

            /// Raised when a `Deserialize` struct type received more than one of the
            /// same field.
            #[cold]
            fn duplicate_field(field: &'static str) -> Self {
                Error::custom(format_args!("duplicate field `{}`", field))
            }
        }
    }
}

#[cfg(feature = "std")]
declare_error_trait!(Error: Sized + StdError);

#[cfg(not(feature = "std"))]
declare_error_trait!(Error: Sized + Debug + Display);

/// `Unexpected` represents an unexpected invocation of any one of the `Visitor`
/// trait methods.
///
/// This is used as an argument to the `invalid_type`, `invalid_value`, and
/// `invalid_length` methods of the `Error` trait to build error messages.
///
/// ```edition2021
/// # use std::fmt;
/// #
/// # use serde::de::{self, Unexpected, Visitor};
/// #
/// # struct Example;
/// #
/// # impl<'de> Visitor<'de> for Example {
/// #     type Value = ();
/// #
/// #     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
/// #         write!(formatter, "definitely not a boolean")
/// #     }
/// #
/// fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
/// where
///     E: de::Error,
/// {
///     Err(de::Error::invalid_type(Unexpected::Bool(v), &self))
/// }
/// # }
/// ```
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Unexpected<'a> {
    /// The input contained a boolean value that was not expected.
    Bool(bool),

    /// The input contained an unsigned integer `u8`, `u16`, `u32` or `u64` that
    /// was not expected.
    Unsigned(u64),

    /// The input contained a signed integer `i8`, `i16`, `i32` or `i64` that
    /// was not expected.
    Signed(i64),

    /// The input contained a floating point `f32` or `f64` that was not
    /// expected.
    Float(f64),

    /// The input contained a `char` that was not expected.
    Char(char),

    /// The input contained a `&str` or `String` that was not expected.
    Str(&'a str),

    /// The input contained a `&[u8]` or `Vec<u8>` that was not expected.
    Bytes(&'a [u8]),

    /// The input contained a unit `()` that was not expected.
    Unit,

    /// The input contained an `Option<T>` that was not expected.
    Option,

    /// The input contained a newtype struct that was not expected.
    NewtypeStruct,

    /// The input contained a sequence that was not expected.
    Seq,

    /// The input contained a map that was not expected.
    Map,

    /// The input contained an enum that was not expected.
    Enum,

    /// The input contained a unit variant that was not expected.
    UnitVariant,

    /// The input contained a newtype variant that was not expected.
    NewtypeVariant,

    /// The input contained a tuple variant that was not expected.
    TupleVariant,

    /// The input contained a struct variant that was not expected.
    StructVariant,

    /// A message stating what uncategorized thing the input contained that was
    /// not expected.
    ///
    /// The message should be a noun or noun phrase, not capitalized and without
    /// a period. An example message is "unoriginal superhero".
    Other(&'a str),
}

impl<'a> fmt::Display for Unexpected<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        use self::Unexpected::*;
        match *self {
            Bool(b) => write!(formatter, "boolean `{}`", b),
            Unsigned(i) => write!(formatter, "integer `{}`", i),
            Signed(i) => write!(formatter, "integer `{}`", i),
            Float(f) => write!(formatter, "floating point `{}`", WithDecimalPoint(f)),
            Char(c) => write!(formatter, "character `{}`", c),
            Str(s) => write!(formatter, "string {:?}", s),
            Bytes(_) => formatter.write_str("byte array"),
            Unit => formatter.write_str("unit value"),
            Option => formatter.write_str("Option value"),
            NewtypeStruct => formatter.write_str("newtype struct"),
            Seq => formatter.write_str("sequence"),
            Map => formatter.write_str("map"),
            Enum => formatter.write_str("enum"),
            UnitVariant => formatter.write_str("unit variant"),
            NewtypeVariant => formatter.write_str("newtype variant"),
            TupleVariant => formatter.write_str("tuple variant"),
            StructVariant => formatter.write_str("struct variant"),
            Other(other) => formatter.write_str(other),
        }
    }
}

/// `Expected` represents an explanation of what data a `Visitor` was expecting
/// to receive.
///
/// This is used as an argument to the `invalid_type`, `invalid_value`, and
/// `invalid_length` methods of the `Error` trait to build error messages. The
/// message should be a noun or noun phrase that completes the sentence "This
/// Visitor expects to receive ...", for example the message could be "an
/// integer between 0 and 64". The message should not be capitalized and should
/// not end with a period.
///
/// Within the context of a `Visitor` implementation, the `Visitor` itself
/// (`&self`) is an implementation of this trait.
///
/// ```edition2021
/// # use serde::de::{self, Unexpected, Visitor};
/// # use std::fmt;
/// #
/// # struct Example;
/// #
/// # impl<'de> Visitor<'de> for Example {
/// #     type Value = ();
/// #
/// #     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
/// #         write!(formatter, "definitely not a boolean")
/// #     }
/// #
/// fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
/// where
///     E: de::Error,
/// {
///     Err(de::Error::invalid_type(Unexpected::Bool(v), &self))
/// }
/// # }
/// ```
///
/// Outside of a `Visitor`, `&"..."` can be used.
///
/// ```edition2021
/// # use serde::de::{self, Unexpected};
/// #
/// # fn example<E>() -> Result<(), E>
/// # where
/// #     E: de::Error,
/// # {
/// #     let v = true;
/// return Err(de::Error::invalid_type(
///     Unexpected::Bool(v),
///     &"a negative integer",
/// ));
/// # }
/// ```
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::Expected` is not satisfied",
    )
)]
pub trait Expected {
    /// Format an explanation of what data was being expected. Same signature as
    /// the `Display` and `Debug` traits.
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result;
}

impl<'de, T> Expected for T
where
    T: Visitor<'de>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.expecting(formatter)
    }
}

impl Expected for &str {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self)
    }
}

impl Display for dyn Expected + '_ {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Expected::fmt(self, formatter)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A **data structure** that can be deserialized from any data format supported
/// by Serde.
///
/// Serde provides `Deserialize` implementations for many Rust primitive and
/// standard library types. The complete list is [here][crate::de]. All of these
/// can be deserialized using Serde out of the box.
///
/// Additionally, Serde provides a procedural macro called `serde_derive` to
/// automatically generate `Deserialize` implementations for structs and enums
/// in your program. See the [derive section of the manual][derive] for how to
/// use this.
///
/// In rare cases it may be necessary to implement `Deserialize` manually for
/// some type in your program. See the [Implementing
/// `Deserialize`][impl-deserialize] section of the manual for more about this.
///
/// Third-party crates may provide `Deserialize` implementations for types that
/// they expose. For example the `linked-hash-map` crate provides a
/// `LinkedHashMap<K, V>` type that is deserializable by Serde because the crate
/// provides an implementation of `Deserialize` for it.
///
/// [derive]: https://serde.rs/derive.html
/// [impl-deserialize]: https://serde.rs/impl-deserialize.html
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed by `Self` when deserialized. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        // Prevents `serde_core::de::Deserialize` appearing in the error message
        // in projects with no direct dependency on serde_core.
        message = "the trait bound `{Self}: serde::Deserialize<'de>` is not satisfied",
        note = "for local types consider adding `#[derive(serde::Deserialize)]` to your `{Self}` type",
        note = "for types from other crates check whether the crate offers a `serde` feature flag",
    )
)]
pub trait Deserialize<'de>: Sized {
    /// Deserialize this value from the given Serde deserializer.
    ///
    /// See the [Implementing `Deserialize`][impl-deserialize] section of the
    /// manual for more information about how to implement this method.
    ///
    /// [impl-deserialize]: https://serde.rs/impl-deserialize.html
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;

    /// Deserializes a value into `self` from the given Deserializer.
    ///
    /// The purpose of this method is to allow the deserializer to reuse
    /// resources and avoid copies. As such, if this method returns an error,
    /// `self` will be in an indeterminate state where some parts of the struct
    /// have been overwritten. Although whatever state that is will be
    /// memory-safe.
    ///
    /// This is generally useful when repeatedly deserializing values that
    /// are processed one at a time, where the value of `self` doesn't matter
    /// when the next deserialization occurs.
    ///
    /// If you manually implement this, your recursive deserializations should
    /// use `deserialize_in_place`.
    ///
    /// This method is stable and an official public API, but hidden from the
    /// documentation because it is almost never what newbies are looking for.
    /// Showing it in rustdoc would cause it to be featured more prominently
    /// than it deserves.
    #[doc(hidden)]
    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        // Default implementation just delegates to `deserialize` impl.
        *place = tri!(Deserialize::deserialize(deserializer));
        Ok(())
    }
}

/// A data structure that can be deserialized without borrowing any data from
/// the deserializer.
///
/// This is primarily useful for trait bounds on functions. For example a
/// `from_str` function may be able to deserialize a data structure that borrows
/// from the input string, but a `from_reader` function may only deserialize
/// owned data.
///
/// ```edition2021
/// # use serde::de::{Deserialize, DeserializeOwned};
/// # use std::io::{Read, Result};
/// #
/// # trait Ignore {
/// fn from_str<'a, T>(s: &'a str) -> Result<T>
/// where
///     T: Deserialize<'a>;
///
/// fn from_reader<R, T>(rdr: R) -> Result<T>
/// where
///     R: Read,
///     T: DeserializeOwned;
/// # }
/// ```
///
/// # Lifetime
///
/// The relationship between `Deserialize` and `DeserializeOwned` in trait
/// bounds is explained in more detail on the page [Understanding deserializer
/// lifetimes].
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::DeserializeOwned` is not satisfied",
    )
)]
pub trait DeserializeOwned: for<'de> Deserialize<'de> {}
impl<T> DeserializeOwned for T where T: for<'de> Deserialize<'de> {}

/// `DeserializeSeed` is the stateful form of the `Deserialize` trait. If you
/// ever find yourself looking for a way to pass data into a `Deserialize` impl,
/// this trait is the way to do it.
///
/// As one example of stateful deserialization consider deserializing a JSON
/// array into an existing buffer. Using the `Deserialize` trait we could
/// deserialize a JSON array into a `Vec<T>` but it would be a freshly allocated
/// `Vec<T>`; there is no way for `Deserialize` to reuse a previously allocated
/// buffer. Using `DeserializeSeed` instead makes this possible as in the
/// example code below.
///
/// The canonical API for stateless deserialization looks like this:
///
/// ```edition2021
/// # use serde::Deserialize;
/// #
/// # enum Error {}
/// #
/// fn func<'de, T: Deserialize<'de>>() -> Result<T, Error>
/// # {
/// #     unimplemented!()
/// # }
/// ```
///
/// Adjusting an API like this to support stateful deserialization is a matter
/// of accepting a seed as input:
///
/// ```edition2021
/// # use serde::de::DeserializeSeed;
/// #
/// # enum Error {}
/// #
/// fn func_seed<'de, T: DeserializeSeed<'de>>(seed: T) -> Result<T::Value, Error>
/// # {
/// #     let _ = seed;
/// #     unimplemented!()
/// # }
/// ```
///
/// In practice the majority of deserialization is stateless. An API expecting a
/// seed can be appeased by passing `std::marker::PhantomData` as a seed in the
/// case of stateless deserialization.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed by `Self::Value` when deserialized. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example
///
/// Suppose we have JSON that looks like `[[1, 2], [3, 4, 5], [6]]` and we need
/// to deserialize it into a flat representation like `vec![1, 2, 3, 4, 5, 6]`.
/// Allocating a brand new `Vec<T>` for each subarray would be slow. Instead we
/// would like to allocate a single `Vec<T>` and then deserialize each subarray
/// into it. This requires stateful deserialization using the `DeserializeSeed`
/// trait.
///
/// ```edition2021
/// use serde::de::{Deserialize, DeserializeSeed, Deserializer, SeqAccess, Visitor};
/// use std::fmt;
/// use std::marker::PhantomData;
///
/// // A DeserializeSeed implementation that uses stateful deserialization to
/// // append array elements onto the end of an existing vector. The preexisting
/// // state ("seed") in this case is the Vec<T>. The `deserialize` method of
/// // `ExtendVec` will be traversing the inner arrays of the JSON input and
/// // appending each integer into the existing Vec.
/// struct ExtendVec<'a, T: 'a>(&'a mut Vec<T>);
///
/// impl<'de, 'a, T> DeserializeSeed<'de> for ExtendVec<'a, T>
/// where
///     T: Deserialize<'de>,
/// {
///     // The return type of the `deserialize` method. This implementation
///     // appends onto an existing vector but does not create any new data
///     // structure, so the return type is ().
///     type Value = ();
///
///     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
///     where
///         D: Deserializer<'de>,
///     {
///         // Visitor implementation that will walk an inner array of the JSON
///         // input.
///         struct ExtendVecVisitor<'a, T: 'a>(&'a mut Vec<T>);
///
///         impl<'de, 'a, T> Visitor<'de> for ExtendVecVisitor<'a, T>
///         where
///             T: Deserialize<'de>,
///         {
///             type Value = ();
///
///             fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
///                 write!(formatter, "an array of integers")
///             }
///
///             fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
///             where
///                 A: SeqAccess<'de>,
///             {
///                 // Decrease the number of reallocations if there are many elements
///                 if let Some(size_hint) = seq.size_hint() {
///                     self.0.reserve(size_hint);
///                 }
///
///                 // Visit each element in the inner array and push it onto
///                 // the existing vector.
///                 while let Some(elem) = seq.next_element()? {
///                     self.0.push(elem);
///                 }
///                 Ok(())
///             }
///         }
///
///         deserializer.deserialize_seq(ExtendVecVisitor(self.0))
///     }
/// }
///
/// // Visitor implementation that will walk the outer array of the JSON input.
/// struct FlattenedVecVisitor<T>(PhantomData<T>);
///
/// impl<'de, T> Visitor<'de> for FlattenedVecVisitor<T>
/// where
///     T: Deserialize<'de>,
/// {
///     // This Visitor constructs a single Vec<T> to hold the flattened
///     // contents of the inner arrays.
///     type Value = Vec<T>;
///
///     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
///         write!(formatter, "an array of arrays")
///     }
///
///     fn visit_seq<A>(self, mut seq: A) -> Result<Vec<T>, A::Error>
///     where
///         A: SeqAccess<'de>,
///     {
///         // Create a single Vec to hold the flattened contents.
///         let mut vec = Vec::new();
///
///         // Each iteration through this loop is one inner array.
///         while let Some(()) = seq.next_element_seed(ExtendVec(&mut vec))? {
///             // Nothing to do; inner array has been appended into `vec`.
///         }
///
///         // Return the finished vec.
///         Ok(vec)
///     }
/// }
///
/// # fn example<'de, D>(deserializer: D) -> Result<(), D::Error>
/// # where
/// #     D: Deserializer<'de>,
/// # {
/// let visitor = FlattenedVecVisitor(PhantomData);
/// let flattened: Vec<u64> = deserializer.deserialize_seq(visitor)?;
/// #     Ok(())
/// # }
/// ```
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::DeserializeSeed<'de>` is not satisfied",
    )
)]
pub trait DeserializeSeed<'de>: Sized {
    /// The type produced by using this seed.
    type Value;

    /// Equivalent to the more common `Deserialize::deserialize` method, except
    /// with some initial piece of data (the seed) passed in.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>;
}

impl<'de, T> DeserializeSeed<'de> for PhantomData<T>
where
    T: Deserialize<'de>,
{
    type Value = T;

    #[inline]
    fn deserialize<D>(self, deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// A **data format** that can deserialize any data structure supported by
/// Serde.
///
/// The role of this trait is to define the deserialization half of the [Serde
/// data model], which is a way to categorize every Rust data type into one of
/// 29 possible types. Each method of the `Deserializer` trait corresponds to one
/// of the types of the data model.
///
/// Implementations of `Deserialize` map themselves into this data model by
/// passing to the `Deserializer` a `Visitor` implementation that can receive
/// these various types.
///
/// The types that make up the Serde data model are:
///
///  - **14 primitive types**
///    - bool
///    - i8, i16, i32, i64, i128
///    - u8, u16, u32, u64, u128
///    - f32, f64
///    - char
///  - **string**
///    - UTF-8 bytes with a length and no null terminator.
///    - When serializing, all strings are handled equally. When deserializing,
///      there are three flavors of strings: transient, owned, and borrowed.
///  - **byte array** - \[u8\]
///    - Similar to strings, during deserialization byte arrays can be
///      transient, owned, or borrowed.
///  - **option**
///    - Either none or some value.
///  - **unit**
///    - The type of `()` in Rust. It represents an anonymous value containing
///      no data.
///  - **unit_struct**
///    - For example `struct Unit` or `PhantomData<T>`. It represents a named
///      value containing no data.
///  - **unit_variant**
///    - For example the `E::A` and `E::B` in `enum E { A, B }`.
///  - **newtype_struct**
///    - For example `struct Millimeters(u8)`.
///  - **newtype_variant**
///    - For example the `E::N` in `enum E { N(u8) }`.
///  - **seq**
///    - A variably sized heterogeneous sequence of values, for example `Vec<T>`
///      or `HashSet<T>`. When serializing, the length may or may not be known
///      before iterating through all the data. When deserializing, the length
///      is determined by looking at the serialized data.
///  - **tuple**
///    - A statically sized heterogeneous sequence of values for which the
///      length will be known at deserialization time without looking at the
///      serialized data, for example `(u8,)` or `(String, u64, Vec<T>)` or
///      `[u64; 10]`.
///  - **tuple_struct**
///    - A named tuple, for example `struct Rgb(u8, u8, u8)`.
///  - **tuple_variant**
///    - For example the `E::T` in `enum E { T(u8, u8) }`.
///  - **map**
///    - A heterogeneous key-value pairing, for example `BTreeMap<K, V>`.
///  - **struct**
///    - A heterogeneous key-value pairing in which the keys are strings and
///      will be known at deserialization time without looking at the serialized
///      data, for example `struct S { r: u8, g: u8, b: u8 }`.
///  - **struct_variant**
///    - For example the `E::S` in `enum E { S { r: u8, g: u8, b: u8 } }`.
///
/// The `Deserializer` trait supports two entry point styles which enables
/// different kinds of deserialization.
///
/// 1. The `deserialize_any` method. Self-describing data formats like JSON are
///    able to look at the serialized data and tell what it represents. For
///    example the JSON deserializer may see an opening curly brace (`{`) and
///    know that it is seeing a map. If the data format supports
///    `Deserializer::deserialize_any`, it will drive the Visitor using whatever
///    type it sees in the input. JSON uses this approach when deserializing
///    `serde_json::Value` which is an enum that can represent any JSON
///    document. Without knowing what is in a JSON document, we can deserialize
///    it to `serde_json::Value` by going through
///    `Deserializer::deserialize_any`.
///
/// 2. The various `deserialize_*` methods. Non-self-describing formats like
///    Postcard need to be told what is in the input in order to deserialize it.
///    The `deserialize_*` methods are hints to the deserializer for how to
///    interpret the next piece of input. Non-self-describing formats are not
///    able to deserialize something like `serde_json::Value` which relies on
///    `Deserializer::deserialize_any`.
///
/// When implementing `Deserialize`, you should avoid relying on
/// `Deserializer::deserialize_any` unless you need to be told by the
/// Deserializer what type is in the input. Know that relying on
/// `Deserializer::deserialize_any` means your data type will be able to
/// deserialize from self-describing formats only, ruling out Postcard and many
/// others.
///
/// [Serde data model]: https://serde.rs/data-model.html
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed from the input when deserializing. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example implementation
///
/// The [example data format] presented on the website contains example code for
/// a basic JSON `Deserializer`.
///
/// [example data format]: https://serde.rs/data-format.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::Deserializer<'de>` is not satisfied",
    )
)]
pub trait Deserializer<'de>: Sized {
    /// The error type that can be returned if some error occurs during
    /// deserialization.
    type Error: Error;

    /// Require the `Deserializer` to figure out how to drive the visitor based
    /// on what data type is in the input.
    ///
    /// When implementing `Deserialize`, you should avoid relying on
    /// `Deserializer::deserialize_any` unless you need to be told by the
    /// Deserializer what type is in the input. Know that relying on
    /// `Deserializer::deserialize_any` means your data type will be able to
    /// deserialize from self-describing formats only, ruling out Postcard and
    /// many others.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a `bool` value.
    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an `i8` value.
    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an `i16` value.
    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an `i32` value.
    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an `i64` value.
    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an `i128` value.
    ///
    /// The default behavior unconditionally returns an error.
    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _ = visitor;
        Err(Error::custom("i128 is not supported"))
    }

    /// Hint that the `Deserialize` type is expecting a `u8` value.
    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a `u16` value.
    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a `u32` value.
    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a `u64` value.
    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an `u128` value.
    ///
    /// The default behavior unconditionally returns an error.
    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let _ = visitor;
        Err(Error::custom("u128 is not supported"))
    }

    /// Hint that the `Deserialize` type is expecting a `f32` value.
    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a `f64` value.
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a `char` value.
    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a string value and does
    /// not benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would benefit from taking ownership of `String` data,
    /// indicate this to the `Deserializer` by using `deserialize_string`
    /// instead.
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a string value and would
    /// benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would not benefit from taking ownership of `String`
    /// data, indicate that to the `Deserializer` by using `deserialize_str`
    /// instead.
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a byte array and does not
    /// benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would benefit from taking ownership of `Vec<u8>` data,
    /// indicate this to the `Deserializer` by using `deserialize_byte_buf`
    /// instead.
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a byte array and would
    /// benefit from taking ownership of buffered data owned by the
    /// `Deserializer`.
    ///
    /// If the `Visitor` would not benefit from taking ownership of `Vec<u8>`
    /// data, indicate that to the `Deserializer` by using `deserialize_bytes`
    /// instead.
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an optional value.
    ///
    /// This allows deserializers that encode an optional value as a nullable
    /// value to convert the null value into `None` and a regular value into
    /// `Some(value)`.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a unit value.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a unit struct with a
    /// particular name.
    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a newtype struct with a
    /// particular name.
    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a sequence of values.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a sequence of values and
    /// knows how many values there are without looking at the serialized data.
    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a tuple struct with a
    /// particular name and number of fields.
    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a map of key-value pairs.
    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting a struct with a particular
    /// name and fields.
    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting an enum value with a
    /// particular name and possible variants.
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type is expecting the name of a struct
    /// field or the discriminant of an enum variant.
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Hint that the `Deserialize` type needs to deserialize a value whose type
    /// doesn't matter because it is ignored.
    ///
    /// Deserializers for non-self-describing formats may not support this mode.
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Determine whether `Deserialize` implementations should expect to
    /// deserialize their human-readable form.
    ///
    /// Some types have a human-readable form that may be somewhat expensive to
    /// construct, as well as a binary form that is compact and efficient.
    /// Generally text-based formats like JSON and YAML will prefer to use the
    /// human-readable one and binary formats like Postcard will prefer the
    /// compact one.
    ///
    /// ```edition2021
    /// # use std::ops::Add;
    /// # use std::str::FromStr;
    /// #
    /// # struct Timestamp;
    /// #
    /// # impl Timestamp {
    /// #     const EPOCH: Timestamp = Timestamp;
    /// # }
    /// #
    /// # impl FromStr for Timestamp {
    /// #     type Err = String;
    /// #     fn from_str(_: &str) -> Result<Self, Self::Err> {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # struct Duration;
    /// #
    /// # impl Duration {
    /// #     fn seconds(_: u64) -> Self { unimplemented!() }
    /// # }
    /// #
    /// # impl Add<Duration> for Timestamp {
    /// #     type Output = Timestamp;
    /// #     fn add(self, _: Duration) -> Self::Output {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// use serde::de::{self, Deserialize, Deserializer};
    ///
    /// impl<'de> Deserialize<'de> for Timestamp {
    ///     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    ///     where
    ///         D: Deserializer<'de>,
    ///     {
    ///         if deserializer.is_human_readable() {
    ///             // Deserialize from a human-readable string like "2015-05-15T17:01:00Z".
    ///             let s = String::deserialize(deserializer)?;
    ///             Timestamp::from_str(&s).map_err(de::Error::custom)
    ///         } else {
    ///             // Deserialize from a compact binary representation, seconds since
    ///             // the Unix epoch.
    ///             let n = u64::deserialize(deserializer)?;
    ///             Ok(Timestamp::EPOCH + Duration::seconds(n))
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// The default implementation of this method returns `true`. Data formats
    /// may override this to `false` to request a compact form for types that
    /// support one. Note that modifying this method to change a format from
    /// human-readable to compact or vice versa should be regarded as a breaking
    /// change, as a value serialized in human-readable mode is not required to
    /// deserialize from the same data in compact mode.
    #[inline]
    fn is_human_readable(&self) -> bool {
        true
    }

    // Not public API.
    #[cfg(all(not(no_serde_derive), any(feature = "std", feature = "alloc")))]
    #[doc(hidden)]
    fn __deserialize_content_v1<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de, Value = crate::private::Content<'de>>,
    {
        self.deserialize_any(visitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// This trait represents a visitor that walks through a deserializer.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the requirement for lifetime of data
/// that may be borrowed by `Self::Value`. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example
///
/// ```edition2021
/// # use serde::de::{self, Unexpected, Visitor};
/// # use std::fmt;
/// #
/// /// A visitor that deserializes a long string - a string containing at least
/// /// some minimum number of bytes.
/// struct LongString {
///     min: usize,
/// }
///
/// impl<'de> Visitor<'de> for LongString {
///     type Value = String;
///
///     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
///         write!(formatter, "a string containing at least {} bytes", self.min)
///     }
///
///     fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
///     where
///         E: de::Error,
///     {
///         if s.len() >= self.min {
///             Ok(s.to_owned())
///         } else {
///             Err(de::Error::invalid_value(Unexpected::Str(s), &self))
///         }
///     }
/// }
/// ```
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::Visitor<'de>` is not satisfied",
    )
)]
pub trait Visitor<'de>: Sized {
    /// The value produced by this visitor.
    type Value;

    /// Format a message stating what data this Visitor expects to receive.
    ///
    /// This is used in error messages. The message should complete the sentence
    /// "This Visitor expects to receive ...", for example the message could be
    /// "an integer between 0 and 64". The message should not be capitalized and
    /// should not end with a period.
    ///
    /// ```edition2021
    /// # use std::fmt;
    /// #
    /// # struct S {
    /// #     max: usize,
    /// # }
    /// #
    /// # impl<'de> serde::de::Visitor<'de> for S {
    /// #     type Value = ();
    /// #
    /// fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    ///     write!(formatter, "an integer between 0 and {}", self.max)
    /// }
    /// # }
    /// ```
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result;

    /// The input contains a boolean.
    ///
    /// The default implementation fails with a type error.
    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Bool(v), &self))
    }

    /// The input contains an `i8`.
    ///
    /// The default implementation forwards to [`visit_i64`].
    ///
    /// [`visit_i64`]: #method.visit_i64
    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_i64(v as i64)
    }

    /// The input contains an `i16`.
    ///
    /// The default implementation forwards to [`visit_i64`].
    ///
    /// [`visit_i64`]: #method.visit_i64
    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_i64(v as i64)
    }

    /// The input contains an `i32`.
    ///
    /// The default implementation forwards to [`visit_i64`].
    ///
    /// [`visit_i64`]: #method.visit_i64
    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_i64(v as i64)
    }

    /// The input contains an `i64`.
    ///
    /// The default implementation fails with a type error.
    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Signed(v), &self))
    }

    /// The input contains a `i128`.
    ///
    /// The default implementation fails with a type error.
    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let mut buf = [0u8; 58];
        let mut writer = crate::format::Buf::new(&mut buf);
        fmt::Write::write_fmt(&mut writer, format_args!("integer `{}` as i128", v)).unwrap();
        Err(Error::invalid_type(
            Unexpected::Other(writer.as_str()),
            &self,
        ))
    }

    /// The input contains a `u8`.
    ///
    /// The default implementation forwards to [`visit_u64`].
    ///
    /// [`visit_u64`]: #method.visit_u64
    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_u64(v as u64)
    }

    /// The input contains a `u16`.
    ///
    /// The default implementation forwards to [`visit_u64`].
    ///
    /// [`visit_u64`]: #method.visit_u64
    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_u64(v as u64)
    }

    /// The input contains a `u32`.
    ///
    /// The default implementation forwards to [`visit_u64`].
    ///
    /// [`visit_u64`]: #method.visit_u64
    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_u64(v as u64)
    }

    /// The input contains a `u64`.
    ///
    /// The default implementation fails with a type error.
    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Unsigned(v), &self))
    }

    /// The input contains a `u128`.
    ///
    /// The default implementation fails with a type error.
    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let mut buf = [0u8; 57];
        let mut writer = crate::format::Buf::new(&mut buf);
        fmt::Write::write_fmt(&mut writer, format_args!("integer `{}` as u128", v)).unwrap();
        Err(Error::invalid_type(
            Unexpected::Other(writer.as_str()),
            &self,
        ))
    }

    /// The input contains an `f32`.
    ///
    /// The default implementation forwards to [`visit_f64`].
    ///
    /// [`visit_f64`]: #method.visit_f64
    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_f64(v as f64)
    }

    /// The input contains an `f64`.
    ///
    /// The default implementation fails with a type error.
    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Float(v), &self))
    }

    /// The input contains a `char`.
    ///
    /// The default implementation forwards to [`visit_str`] as a one-character
    /// string.
    ///
    /// [`visit_str`]: #method.visit_str
    #[inline]
    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_str(v.encode_utf8(&mut [0u8; 4]))
    }

    /// The input contains a string. The lifetime of the string is ephemeral and
    /// it may be destroyed after this method returns.
    ///
    /// This method allows the `Deserializer` to avoid a copy by retaining
    /// ownership of any buffered data. `Deserialize` implementations that do
    /// not benefit from taking ownership of `String` data should indicate that
    /// to the deserializer by using `Deserializer::deserialize_str` rather than
    /// `Deserializer::deserialize_string`.
    ///
    /// It is never correct to implement `visit_string` without implementing
    /// `visit_str`. Implement neither, both, or just `visit_str`.
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Str(v), &self))
    }

    /// The input contains a string that lives at least as long as the
    /// `Deserializer`.
    ///
    /// This enables zero-copy deserialization of strings in some formats. For
    /// example JSON input containing the JSON string `"borrowed"` can be
    /// deserialized with zero copying into a `&'a str` as long as the input
    /// data outlives `'a`.
    ///
    /// The default implementation forwards to `visit_str`.
    #[inline]
    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_str(v)
    }

    /// The input contains a string and ownership of the string is being given
    /// to the `Visitor`.
    ///
    /// This method allows the `Visitor` to avoid a copy by taking ownership of
    /// a string created by the `Deserializer`. `Deserialize` implementations
    /// that benefit from taking ownership of `String` data should indicate that
    /// to the deserializer by using `Deserializer::deserialize_string` rather
    /// than `Deserializer::deserialize_str`, although not every deserializer
    /// will honor such a request.
    ///
    /// It is never correct to implement `visit_string` without implementing
    /// `visit_str`. Implement neither, both, or just `visit_str`.
    ///
    /// The default implementation forwards to `visit_str` and then drops the
    /// `String`.
    #[inline]
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_str(&v)
    }

    /// The input contains a byte array. The lifetime of the byte array is
    /// ephemeral and it may be destroyed after this method returns.
    ///
    /// This method allows the `Deserializer` to avoid a copy by retaining
    /// ownership of any buffered data. `Deserialize` implementations that do
    /// not benefit from taking ownership of `Vec<u8>` data should indicate that
    /// to the deserializer by using `Deserializer::deserialize_bytes` rather
    /// than `Deserializer::deserialize_byte_buf`.
    ///
    /// It is never correct to implement `visit_byte_buf` without implementing
    /// `visit_bytes`. Implement neither, both, or just `visit_bytes`.
    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Bytes(v), &self))
    }

    /// The input contains a byte array that lives at least as long as the
    /// `Deserializer`.
    ///
    /// This enables zero-copy deserialization of bytes in some formats. For
    /// example Postcard data containing bytes can be deserialized with zero
    /// copying into a `&'a [u8]` as long as the input data outlives `'a`.
    ///
    /// The default implementation forwards to `visit_bytes`.
    #[inline]
    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_bytes(v)
    }

    /// The input contains a byte array and ownership of the byte array is being
    /// given to the `Visitor`.
    ///
    /// This method allows the `Visitor` to avoid a copy by taking ownership of
    /// a byte buffer created by the `Deserializer`. `Deserialize`
    /// implementations that benefit from taking ownership of `Vec<u8>` data
    /// should indicate that to the deserializer by using
    /// `Deserializer::deserialize_byte_buf` rather than
    /// `Deserializer::deserialize_bytes`, although not every deserializer will
    /// honor such a request.
    ///
    /// It is never correct to implement `visit_byte_buf` without implementing
    /// `visit_bytes`. Implement neither, both, or just `visit_bytes`.
    ///
    /// The default implementation forwards to `visit_bytes` and then drops the
    /// `Vec<u8>`.
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_bytes(&v)
    }

    /// The input contains an optional that is absent.
    ///
    /// The default implementation fails with a type error.
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Option, &self))
    }

    /// The input contains an optional that is present.
    ///
    /// The default implementation fails with a type error.
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _ = deserializer;
        Err(Error::invalid_type(Unexpected::Option, &self))
    }

    /// The input contains a unit `()`.
    ///
    /// The default implementation fails with a type error.
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Err(Error::invalid_type(Unexpected::Unit, &self))
    }

    /// The input contains a newtype struct.
    ///
    /// The content of the newtype struct may be read from the given
    /// `Deserializer`.
    ///
    /// The default implementation fails with a type error.
    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _ = deserializer;
        Err(Error::invalid_type(Unexpected::NewtypeStruct, &self))
    }

    /// The input contains a sequence of elements.
    ///
    /// The default implementation fails with a type error.
    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let _ = seq;
        Err(Error::invalid_type(Unexpected::Seq, &self))
    }

    /// The input contains a key-value map.
    ///
    /// The default implementation fails with a type error.
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let _ = map;
        Err(Error::invalid_type(Unexpected::Map, &self))
    }

    /// The input contains an enum.
    ///
    /// The default implementation fails with a type error.
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        let _ = data;
        Err(Error::invalid_type(Unexpected::Enum, &self))
    }

    // Used when deserializing a flattened Option field. Not public API.
    #[doc(hidden)]
    fn __private_visit_untagged_option<D>(self, _: D) -> Result<Self::Value, ()>
    where
        D: Deserializer<'de>,
    {
        Err(())
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Provides a `Visitor` access to each element of a sequence in the input.
///
/// This is a trait that a `Deserializer` passes to a `Visitor` implementation,
/// which deserializes each item in a sequence.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed by deserialized sequence elements. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example implementation
///
/// The [example data format] presented on the website demonstrates an
/// implementation of `SeqAccess` for a basic JSON data format.
///
/// [example data format]: https://serde.rs/data-format.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::SeqAccess<'de>` is not satisfied",
    )
)]
pub trait SeqAccess<'de> {
    /// The error type that can be returned if some error occurs during
    /// deserialization.
    type Error: Error;

    /// This returns `Ok(Some(value))` for the next value in the sequence, or
    /// `Ok(None)` if there are no more remaining items.
    ///
    /// `Deserialize` implementations should typically use
    /// `SeqAccess::next_element` instead.
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>;

    /// This returns `Ok(Some(value))` for the next value in the sequence, or
    /// `Ok(None)` if there are no more remaining items.
    ///
    /// This method exists as a convenience for `Deserialize` implementations.
    /// `SeqAccess` implementations should not override the default behavior.
    #[inline]
    fn next_element<T>(&mut self) -> Result<Option<T>, Self::Error>
    where
        T: Deserialize<'de>,
    {
        self.next_element_seed(PhantomData)
    }

    /// Returns the number of elements remaining in the sequence, if known.
    #[inline]
    fn size_hint(&self) -> Option<usize> {
        None
    }
}

impl<'de, A> SeqAccess<'de> for &mut A
where
    A: ?Sized + SeqAccess<'de>,
{
    type Error = A::Error;

    #[inline]
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        (**self).next_element_seed(seed)
    }

    #[inline]
    fn next_element<T>(&mut self) -> Result<Option<T>, Self::Error>
    where
        T: Deserialize<'de>,
    {
        (**self).next_element()
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        (**self).size_hint()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Provides a `Visitor` access to each entry of a map in the input.
///
/// This is a trait that a `Deserializer` passes to a `Visitor` implementation.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed by deserialized map entries. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example implementation
///
/// The [example data format] presented on the website demonstrates an
/// implementation of `MapAccess` for a basic JSON data format.
///
/// [example data format]: https://serde.rs/data-format.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::MapAccess<'de>` is not satisfied",
    )
)]
pub trait MapAccess<'de> {
    /// The error type that can be returned if some error occurs during
    /// deserialization.
    type Error: Error;

    /// This returns `Ok(Some(key))` for the next key in the map, or `Ok(None)`
    /// if there are no more remaining entries.
    ///
    /// `Deserialize` implementations should typically use
    /// `MapAccess::next_key` or `MapAccess::next_entry` instead.
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>;

    /// This returns a `Ok(value)` for the next value in the map.
    ///
    /// `Deserialize` implementations should typically use
    /// `MapAccess::next_value` instead.
    ///
    /// # Panics
    ///
    /// Calling `next_value_seed` before `next_key_seed` is incorrect and is
    /// allowed to panic or return bogus results.
    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>;

    /// This returns `Ok(Some((key, value)))` for the next (key-value) pair in
    /// the map, or `Ok(None)` if there are no more remaining items.
    ///
    /// `MapAccess` implementations should override the default behavior if a
    /// more efficient implementation is possible.
    ///
    /// `Deserialize` implementations should typically use
    /// `MapAccess::next_entry` instead.
    #[inline]
    fn next_entry_seed<K, V>(
        &mut self,
        kseed: K,
        vseed: V,
    ) -> Result<Option<(K::Value, V::Value)>, Self::Error>
    where
        K: DeserializeSeed<'de>,
        V: DeserializeSeed<'de>,
    {
        match tri!(self.next_key_seed(kseed)) {
            Some(key) => {
                let value = tri!(self.next_value_seed(vseed));
                Ok(Some((key, value)))
            }
            None => Ok(None),
        }
    }

    /// This returns `Ok(Some(key))` for the next key in the map, or `Ok(None)`
    /// if there are no more remaining entries.
    ///
    /// This method exists as a convenience for `Deserialize` implementations.
    /// `MapAccess` implementations should not override the default behavior.
    #[inline]
    fn next_key<K>(&mut self) -> Result<Option<K>, Self::Error>
    where
        K: Deserialize<'de>,
    {
        self.next_key_seed(PhantomData)
    }

    /// This returns a `Ok(value)` for the next value in the map.
    ///
    /// This method exists as a convenience for `Deserialize` implementations.
    /// `MapAccess` implementations should not override the default behavior.
    ///
    /// # Panics
    ///
    /// Calling `next_value` before `next_key` is incorrect and is allowed to
    /// panic or return bogus results.
    #[inline]
    fn next_value<V>(&mut self) -> Result<V, Self::Error>
    where
        V: Deserialize<'de>,
    {
        self.next_value_seed(PhantomData)
    }

    /// This returns `Ok(Some((key, value)))` for the next (key-value) pair in
    /// the map, or `Ok(None)` if there are no more remaining items.
    ///
    /// This method exists as a convenience for `Deserialize` implementations.
    /// `MapAccess` implementations should not override the default behavior.
    #[inline]
    fn next_entry<K, V>(&mut self) -> Result<Option<(K, V)>, Self::Error>
    where
        K: Deserialize<'de>,
        V: Deserialize<'de>,
    {
        self.next_entry_seed(PhantomData, PhantomData)
    }

    /// Returns the number of entries remaining in the map, if known.
    #[inline]
    fn size_hint(&self) -> Option<usize> {
        None
    }
}

impl<'de, A> MapAccess<'de> for &mut A
where
    A: ?Sized + MapAccess<'de>,
{
    type Error = A::Error;

    #[inline]
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        (**self).next_key_seed(seed)
    }

    #[inline]
    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        (**self).next_value_seed(seed)
    }

    #[inline]
    fn next_entry_seed<K, V>(
        &mut self,
        kseed: K,
        vseed: V,
    ) -> Result<Option<(K::Value, V::Value)>, Self::Error>
    where
        K: DeserializeSeed<'de>,
        V: DeserializeSeed<'de>,
    {
        (**self).next_entry_seed(kseed, vseed)
    }

    #[inline]
    fn next_entry<K, V>(&mut self) -> Result<Option<(K, V)>, Self::Error>
    where
        K: Deserialize<'de>,
        V: Deserialize<'de>,
    {
        (**self).next_entry()
    }

    #[inline]
    fn next_key<K>(&mut self) -> Result<Option<K>, Self::Error>
    where
        K: Deserialize<'de>,
    {
        (**self).next_key()
    }

    #[inline]
    fn next_value<V>(&mut self) -> Result<V, Self::Error>
    where
        V: Deserialize<'de>,
    {
        (**self).next_value()
    }

    #[inline]
    fn size_hint(&self) -> Option<usize> {
        (**self).size_hint()
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Provides a `Visitor` access to the data of an enum in the input.
///
/// `EnumAccess` is created by the `Deserializer` and passed to the
/// `Visitor` in order to identify which variant of an enum to deserialize.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed by the deserialized enum variant. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example implementation
///
/// The [example data format] presented on the website demonstrates an
/// implementation of `EnumAccess` for a basic JSON data format.
///
/// [example data format]: https://serde.rs/data-format.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::EnumAccess<'de>` is not satisfied",
    )
)]
pub trait EnumAccess<'de>: Sized {
    /// The error type that can be returned if some error occurs during
    /// deserialization.
    type Error: Error;
    /// The `Visitor` that will be used to deserialize the content of the enum
    /// variant.
    type Variant: VariantAccess<'de, Error = Self::Error>;

    /// `variant` is called to identify which variant to deserialize.
    ///
    /// `Deserialize` implementations should typically use `EnumAccess::variant`
    /// instead.
    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>;

    /// `variant` is called to identify which variant to deserialize.
    ///
    /// This method exists as a convenience for `Deserialize` implementations.
    /// `EnumAccess` implementations should not override the default behavior.
    #[inline]
    fn variant<V>(self) -> Result<(V, Self::Variant), Self::Error>
    where
        V: Deserialize<'de>,
    {
        self.variant_seed(PhantomData)
    }
}

/// `VariantAccess` is a visitor that is created by the `Deserializer` and
/// passed to the `Deserialize` to deserialize the content of a particular enum
/// variant.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed by the deserialized enum variant. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example implementation
///
/// The [example data format] presented on the website demonstrates an
/// implementation of `VariantAccess` for a basic JSON data format.
///
/// [example data format]: https://serde.rs/data-format.html
#[cfg_attr(
    not(no_diagnostic_namespace),
    diagnostic::on_unimplemented(
        message = "the trait bound `{Self}: serde::de::VariantAccess<'de>` is not satisfied",
    )
)]
pub trait VariantAccess<'de>: Sized {
    /// The error type that can be returned if some error occurs during
    /// deserialization. Must match the error type of our `EnumAccess`.
    type Error: Error;

    /// Called when deserializing a variant with no values.
    ///
    /// If the data contains a different type of variant, the following
    /// `invalid_type` error should be constructed:
    ///
    /// ```edition2021
    /// # use serde::de::{self, value, DeserializeSeed, Visitor, VariantAccess, Unexpected};
    /// #
    /// # struct X;
    /// #
    /// # impl<'de> VariantAccess<'de> for X {
    /// #     type Error = value::Error;
    /// #
    /// fn unit_variant(self) -> Result<(), Self::Error> {
    ///     // What the data actually contained; suppose it is a tuple variant.
    ///     let unexp = Unexpected::TupleVariant;
    ///     Err(de::Error::invalid_type(unexp, &"unit variant"))
    /// }
    /// #
    /// #     fn newtype_variant_seed<T>(self, _: T) -> Result<T::Value, Self::Error>
    /// #     where
    /// #         T: DeserializeSeed<'de>,
    /// #     { unimplemented!() }
    /// #
    /// #     fn tuple_variant<V>(self, _: usize, _: V) -> Result<V::Value, Self::Error>
    /// #     where
    /// #         V: Visitor<'de>,
    /// #     { unimplemented!() }
    /// #
    /// #     fn struct_variant<V>(self, _: &[&str], _: V) -> Result<V::Value, Self::Error>
    /// #     where
    /// #         V: Visitor<'de>,
    /// #     { unimplemented!() }
    /// # }
    /// ```
    fn unit_variant(self) -> Result<(), Self::Error>;

    /// Called when deserializing a variant with a single value.
    ///
    /// `Deserialize` implementations should typically use
    /// `VariantAccess::newtype_variant` instead.
    ///
    /// If the data contains a different type of variant, the following
    /// `invalid_type` error should be constructed:
    ///
    /// ```edition2021
    /// # use serde::de::{self, value, DeserializeSeed, Visitor, VariantAccess, Unexpected};
    /// #
    /// # struct X;
    /// #
    /// # impl<'de> VariantAccess<'de> for X {
    /// #     type Error = value::Error;
    /// #
    /// #     fn unit_variant(self) -> Result<(), Self::Error> {
    /// #         unimplemented!()
    /// #     }
    /// #
    /// fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
    /// where
    ///     T: DeserializeSeed<'de>,
    /// {
    ///     // What the data actually contained; suppose it is a unit variant.
    ///     let unexp = Unexpected::UnitVariant;
    ///     Err(de::Error::invalid_type(unexp, &"newtype variant"))
    /// }
    /// #
    /// #     fn tuple_variant<V>(self, _: usize, _: V) -> Result<V::Value, Self::Error>
    /// #     where
    /// #         V: Visitor<'de>,
    /// #     { unimplemented!() }
    /// #
    /// #     fn struct_variant<V>(self, _: &[&str], _: V) -> Result<V::Value, Self::Error>
    /// #     where
    /// #         V: Visitor<'de>,
    /// #     { unimplemented!() }
    /// # }
    /// ```
    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>;

    /// Called when deserializing a variant with a single value.
    ///
    /// This method exists as a convenience for `Deserialize` implementations.
    /// `VariantAccess` implementations should not override the default
    /// behavior.
    #[inline]
    fn newtype_variant<T>(self) -> Result<T, Self::Error>
    where
        T: Deserialize<'de>,
    {
        self.newtype_variant_seed(PhantomData)
    }

    /// Called when deserializing a tuple-like variant.
    ///
    /// The `len` is the number of fields expected in the tuple variant.
    ///
    /// If the data contains a different type of variant, the following
    /// `invalid_type` error should be constructed:
    ///
    /// ```edition2021
    /// # use serde::de::{self, value, DeserializeSeed, Visitor, VariantAccess, Unexpected};
    /// #
    /// # struct X;
    /// #
    /// # impl<'de> VariantAccess<'de> for X {
    /// #     type Error = value::Error;
    /// #
    /// #     fn unit_variant(self) -> Result<(), Self::Error> {
    /// #         unimplemented!()
    /// #     }
    /// #
    /// #     fn newtype_variant_seed<T>(self, _: T) -> Result<T::Value, Self::Error>
    /// #     where
    /// #         T: DeserializeSeed<'de>,
    /// #     { unimplemented!() }
    /// #
    /// fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    /// where
    ///     V: Visitor<'de>,
    /// {
    ///     // What the data actually contained; suppose it is a unit variant.
    ///     let unexp = Unexpected::UnitVariant;
    ///     Err(de::Error::invalid_type(unexp, &"tuple variant"))
    /// }
    /// #
    /// #     fn struct_variant<V>(self, _: &[&str], _: V) -> Result<V::Value, Self::Error>
    /// #     where
    /// #         V: Visitor<'de>,
    /// #     { unimplemented!() }
    /// # }
    /// ```
    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;

    /// Called when deserializing a struct-like variant.
    ///
    /// The `fields` are the names of the fields of the struct variant.
    ///
    /// If the data contains a different type of variant, the following
    /// `invalid_type` error should be constructed:
    ///
    /// ```edition2021
    /// # use serde::de::{self, value, DeserializeSeed, Visitor, VariantAccess, Unexpected};
    /// #
    /// # struct X;
    /// #
    /// # impl<'de> VariantAccess<'de> for X {
    /// #     type Error = value::Error;
    /// #
    /// #     fn unit_variant(self) -> Result<(), Self::Error> {
    /// #         unimplemented!()
    /// #     }
    /// #
    /// #     fn newtype_variant_seed<T>(self, _: T) -> Result<T::Value, Self::Error>
    /// #     where
    /// #         T: DeserializeSeed<'de>,
    /// #     { unimplemented!() }
    /// #
    /// #     fn tuple_variant<V>(self, _: usize, _: V) -> Result<V::Value, Self::Error>
    /// #     where
    /// #         V: Visitor<'de>,
    /// #     { unimplemented!() }
    /// #
    /// fn struct_variant<V>(
    ///     self,
    ///     _fields: &'static [&'static str],
    ///     _visitor: V,
    /// ) -> Result<V::Value, Self::Error>
    /// where
    ///     V: Visitor<'de>,
    /// {
    ///     // What the data actually contained; suppose it is a unit variant.
    ///     let unexp = Unexpected::UnitVariant;
    ///     Err(de::Error::invalid_type(unexp, &"struct variant"))
    /// }
    /// # }
    /// ```
    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>;
}

////////////////////////////////////////////////////////////////////////////////

/// Converts an existing value into a `Deserializer` from which other values can
/// be deserialized.
///
/// # Lifetime
///
/// The `'de` lifetime of this trait is the lifetime of data that may be
/// borrowed from the resulting `Deserializer`. See the page [Understanding
/// deserializer lifetimes] for a more detailed explanation of these lifetimes.
///
/// [Understanding deserializer lifetimes]: https://serde.rs/lifetimes.html
///
/// # Example
///
/// ```edition2021
/// use serde::de::{value, Deserialize, IntoDeserializer};
/// use serde_derive::Deserialize;
/// use std::str::FromStr;
///
/// #[derive(Deserialize)]
/// enum Setting {
///     On,
///     Off,
/// }
///
/// impl FromStr for Setting {
///     type Err = value::Error;
///
///     fn from_str(s: &str) -> Result<Self, Self::Err> {
///         Self::deserialize(s.into_deserializer())
///     }
/// }
/// ```
pub trait IntoDeserializer<'de, E: Error = value::Error> {
    /// The type of the deserializer being converted into.
    type Deserializer: Deserializer<'de, Error = E>;

    /// Convert this value into a deserializer.
    fn into_deserializer(self) -> Self::Deserializer;
}

////////////////////////////////////////////////////////////////////////////////

/// Used in error messages.
///
/// - expected `a`
/// - expected `a` or `b`
/// - expected one of `a`, `b`, `c`
///
/// The slice of names must not be empty.
struct OneOf {
    names: &'static [&'static str],
}

impl Display for OneOf {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.names.len() {
            0 => panic!(), // special case elsewhere
            1 => write!(formatter, "`{}`", self.names[0]),
            2 => write!(formatter, "`{}` or `{}`", self.names[0], self.names[1]),
            _ => {
                tri!(formatter.write_str("one of "));
                for (i, alt) in self.names.iter().enumerate() {
                    if i > 0 {
                        tri!(formatter.write_str(", "));
                    }
                    tri!(write!(formatter, "`{}`", alt));
                }
                Ok(())
            }
        }
    }
}

struct WithDecimalPoint(f64);

impl Display for WithDecimalPoint {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        struct LookForDecimalPoint<'f, 'a> {
            formatter: &'f mut fmt::Formatter<'a>,
            has_decimal_point: bool,
        }

        impl<'f, 'a> fmt::Write for LookForDecimalPoint<'f, 'a> {
            fn write_str(&mut self, fragment: &str) -> fmt::Result {
                self.has_decimal_point |= fragment.contains('.');
                self.formatter.write_str(fragment)
            }

            fn write_char(&mut self, ch: char) -> fmt::Result {
                self.has_decimal_point |= ch == '.';
                self.formatter.write_char(ch)
            }
        }

        if self.0.is_finite() {
            let mut writer = LookForDecimalPoint {
                formatter,
                has_decimal_point: false,
            };
            tri!(write!(writer, "{}", self.0));
            if !writer.has_decimal_point {
                tri!(formatter.write_str(".0"));
            }
        } else {
            tri!(write!(formatter, "{}", self.0));
        }
        Ok(())
    }
}
