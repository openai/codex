//! # Strum
//!
//! Strum is a set of macros and traits for working with
//! enums and strings easier in Rust.
//!
//! This crate only contains derive macros for use with the
//! [`strum`](https://docs.rs/strum)
//! crate.  The macros provied by this crate are also available by
//! enabling the `derive` feature in aforementioned `strum` crate.

#![recursion_limit = "128"]

extern crate proc_macro;

mod helpers;
mod macros;

use proc_macro2::TokenStream;
use std::env;
use syn::DeriveInput;

fn debug_print_generated(ast: &DeriveInput, toks: &TokenStream) {
    let debug = env::var("STRUM_DEBUG");
    if let Ok(s) = debug {
        if s == "1" {
            println!("{}", toks);
        }

        if ast.ident == s {
            println!("{}", toks);
        }
    }
}

/// Converts strings to enum variants based on their name.
///
/// auto-derives `std::str::FromStr` on the enum (for Rust 1.34 and above, `std::convert::TryFrom<&str>`
/// will be derived as well). Each variant of the enum will match on it's own name.
/// This can be overridden using `serialize="DifferentName"` or `to_string="DifferentName"`
/// on the attribute as shown below.
/// Multiple deserializations can be added to the same variant. If the variant contains additional data,
/// they will be set to their default values upon deserialization.
///
/// The `default` attribute can be applied to a tuple variant with a single data parameter. When a match isn't
/// found, the given variant will be returned and the input string will be captured in the parameter.
///
/// Note that the implementation of `FromStr` by default only matches on the name of the
/// variant. There is an option to match on different case conversions through the
/// `#[strum(serialize_all = "snake_case")]` type attribute.
///
/// See the [Additional Attributes](https://docs.rs/strum/latest/strum/additional_attributes/index.html)
/// Section for more information on using this feature.
///
/// If you have a large enum, you may want to consider using the `use_phf` attribute here. It leverages
/// perfect hash functions to parse much quicker than a standard `match`. (MSRV 1.46)
///
/// # Example howto use `EnumString`
/// ```
/// use std::str::FromStr;
/// use strum_macros::EnumString;
///
/// #[derive(Debug, PartialEq, EnumString)]
/// enum Color {
///     Red,
///     // The Default value will be inserted into range if we match "Green".
///     Green {
///         range: usize,
///     },
///
///     // We can match on multiple different patterns.
///     #[strum(serialize = "blue", serialize = "b")]
///     Blue(usize),
///
///     // Notice that we can disable certain variants from being found
///     #[strum(disabled)]
///     Yellow,
///
///     // We can make the comparison case insensitive (however Unicode is not supported at the moment)
///     #[strum(ascii_case_insensitive)]
///     Black,
/// }
///
/// /*
/// //The generated code will look like:
/// impl std::str::FromStr for Color {
///     type Err = ::strum::ParseError;
///
///     fn from_str(s: &str) -> ::core::result::Result<Color, Self::Err> {
///         match s {
///             "Red" => ::core::result::Result::Ok(Color::Red),
///             "Green" => ::core::result::Result::Ok(Color::Green { range:Default::default() }),
///             "blue" => ::core::result::Result::Ok(Color::Blue(Default::default())),
///             "b" => ::core::result::Result::Ok(Color::Blue(Default::default())),
///             s if s.eq_ignore_ascii_case("Black") => ::core::result::Result::Ok(Color::Black),
///             _ => ::core::result::Result::Err(::strum::ParseError::VariantNotFound),
///         }
///     }
/// }
/// */
///
/// // simple from string
/// let color_variant = Color::from_str("Red").unwrap();
/// assert_eq!(Color::Red, color_variant);
/// // short version works too
/// let color_variant = Color::from_str("b").unwrap();
/// assert_eq!(Color::Blue(0), color_variant);
/// // was disabled for parsing = returns parse-error
/// let color_variant = Color::from_str("Yellow");
/// assert!(color_variant.is_err());
/// // however the variant is still normally usable
/// println!("{:?}", Color::Yellow);
/// let color_variant = Color::from_str("bLACk").unwrap();
/// assert_eq!(Color::Black, color_variant);
/// ```
#[proc_macro_derive(EnumString, attributes(strum))]
pub fn from_string(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::from_string::from_string_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Converts enum variants to `&'a str`, where `'a` is the lifetime of the input enum reference.
///
/// Implements `AsRef<str>` on your enum using the same rules as
/// `Display` for determining what string is returned. The difference is that `as_ref()` returns
/// a `&str` instead of a `String` so you don't allocate any additional memory with each call.
///
/// If you require a `&'static str`, you can use
/// [`strum::IntoStaticStr`](crate::IntoStaticStr) instead.
///
/// ```
/// // You need to bring the AsRef trait into scope to use it
/// use std::convert::AsRef;
/// use strum_macros::AsRefStr;
///
/// #[derive(AsRefStr, Debug)]
/// enum Color {
///     #[strum(serialize = "redred")]
///     Red,
///     Green {
///         range: usize,
///     },
///     Blue(usize),
///     Yellow,
/// }
///
/// // uses the serialize string for Display
/// let red = Color::Red;
/// assert_eq!("redred", red.as_ref());
/// // by default the variants Name
/// let yellow = Color::Yellow;
/// assert_eq!("Yellow", yellow.as_ref());
/// // or for string formatting
/// println!(
///     "blue: {} green: {}",
///     Color::Blue(10).as_ref(),
///     Color::Green { range: 42 }.as_ref()
/// );
///
/// // With prefix on all variants
/// #[derive(AsRefStr, Debug)]
/// #[strum(prefix = "/")]
/// enum ColorWithPrefix {
///     #[strum(serialize = "redred")]
///     Red,
///     Green,
/// }
///
/// assert_eq!("/redred", ColorWithPrefix::Red.as_ref());
/// assert_eq!("/Green", ColorWithPrefix::Green.as_ref());
/// ```
#[proc_macro_derive(AsRefStr, attributes(strum))]
pub fn as_ref_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::as_ref_str::as_ref_str_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Implements `Strum::VariantNames` which adds an associated constant `VARIANTS` which is a `'static` slice of discriminant names.
///
/// Adds an `impl` block for the `enum` that adds a static `VARIANTS` array of `&'static str` that are the discriminant names.
/// This will respect the `serialize_all` attribute on the `enum` (like `#[strum(serialize_all = "snake_case")]`.
///
/// ```
/// // import the macros needed
/// use strum_macros::{EnumString};
/// // You need to import the trait, to have access to VARIANTS
/// use strum::VariantNames;
///
/// #[derive(Debug, EnumString, strum_macros::VariantNames)]
/// #[strum(serialize_all = "kebab-case")]
/// enum Color {
///     Red,
///     Blue,
///     Yellow,
///     RebeccaPurple,
/// }
/// assert_eq!(["red", "blue", "yellow", "rebecca-purple"], Color::VARIANTS);
/// ```
#[proc_macro_derive(VariantNames, attributes(strum))]
pub fn variant_names(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_variant_names::enum_variant_names_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

#[doc(hidden)]
#[proc_macro_derive(EnumVariantNames, attributes(strum))]
#[deprecated(
    since = "0.26.0",
    note = "please use `#[derive(VariantNames)]` instead"
)]
pub fn variant_names_deprecated(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_variant_names::enum_variant_names_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Adds a `'static` slice with all of the Enum's variants.
///
/// Implements `strum::VariantArray` which adds an associated constant `VARIANTS`.
/// This constant contains an array with all the variants of the enumerator.
///
/// This trait can only be autoderived if the enumerator is composed only of unit-type variants,
/// meaning that the variants must not have any data.
///
/// ```
/// use strum::VariantArray;
///
/// #[derive(VariantArray, Debug, PartialEq, Eq)]
/// enum Op {
///     Add,
///     Sub,
///     Mul,
///     Div,
/// }
///
/// assert_eq!(Op::VARIANTS, &[Op::Add, Op::Sub, Op::Mul, Op::Div]);
/// ```
#[proc_macro_derive(VariantArray, attributes(strum))]
pub fn static_variants_array(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_variant_array::static_variants_array_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

#[proc_macro_derive(AsStaticStr, attributes(strum))]
#[doc(hidden)]
#[deprecated(
    since = "0.22.0",
    note = "please use `#[derive(IntoStaticStr)]` instead"
)]
pub fn as_static_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::as_ref_str::as_static_str_inner(
        &ast,
        &macros::as_ref_str::GenerateTraitVariant::AsStaticStr,
    )
    .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Implements `From<MyEnum> for &'static str` on an enum.
///
/// Implements `From<YourEnum>` and `From<&'a YourEnum>` for `&'static str`. This is
/// useful for turning an enum variant into a static string.
/// The Rust `std` provides a blanket impl of the reverse direction - i.e. `impl Into<&'static str> for YourEnum`.
///
/// ```
/// use strum_macros::IntoStaticStr;
///
/// #[derive(IntoStaticStr)]
/// enum State<'a> {
///     Initial(&'a str),
///     Finished,
/// }
///
/// fn verify_state<'a>(s: &'a str) {
///     let mut state = State::Initial(s);
///     // The following won't work because the lifetime is incorrect:
///     // let wrong: &'static str = state.as_ref();
///     // using the trait implemented by the derive works however:
///     let right: &'static str = state.into();
///     assert_eq!("Initial", right);
///     state = State::Finished;
///     let done: &'static str = state.into();
///     assert_eq!("Finished", done);
/// }
///
/// verify_state(&"hello world".to_string());
/// ```
#[proc_macro_derive(IntoStaticStr, attributes(strum))]
pub fn into_static_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::as_ref_str::as_static_str_inner(
        &ast,
        &macros::as_ref_str::GenerateTraitVariant::From,
    )
    .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// implements `std::string::ToString` on an enum
///
/// ```
/// // You need to bring the ToString trait into scope to use it
/// use std::string::ToString;
/// use strum_macros;
///
/// #[derive(strum_macros::ToString, Debug)]
/// enum Color {
///     #[strum(serialize = "redred")]
///     Red,
///     Green {
///         range: usize,
///     },
///     Blue(usize),
///     Yellow,
/// }
///
/// // uses the serialize string for Display
/// let red = Color::Red;
/// assert_eq!(String::from("redred"), red.to_string());
/// // by default the variants Name
/// let yellow = Color::Yellow;
/// assert_eq!(String::from("Yellow"), yellow.to_string());
/// ```
#[deprecated(
    since = "0.22.0",
    note = "please use `#[derive(Display)]` instead. See issue https://github.com/Peternator7/strum/issues/132"
)]
#[doc(hidden)]
#[proc_macro_derive(ToString, attributes(strum))]
pub fn to_string(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::to_string::to_string_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Converts enum variants to strings.
///
/// Deriving `Display` on an enum prints out the given enum. This enables you to perform round
/// trip style conversions from enum into string and back again for unit style variants. `Display`
/// choose which serialization to used based on the following criteria:
///
/// 1. If there is a `to_string` property, this value will be used. There can only be one per variant.
/// 2. Of the various `serialize` properties, the value with the longest length is chosen. If that
///    behavior isn't desired, you should use `to_string`.
/// 3. The name of the variant will be used if there are no `serialize` or `to_string` attributes.
/// 4. If the enum has a `strum(prefix = "some_value_")`, every variant will have that prefix prepended
///    to the serialization.
/// 5. Enums with fields support string interpolation.
///    Note this means the variant will not "round trip" if you then deserialize the string.
///
///    ```rust
///    #[derive(strum_macros::Display)]
///    pub enum Color {
///        #[strum(to_string = "saturation is {sat}")]
///        Red { sat: usize },
///        #[strum(to_string = "hue is {1}, saturation is {0}")]
///        Blue(usize, usize),
///    }
///    ```
///
/// ```
/// // You need to bring the ToString trait into scope to use it
/// use std::string::ToString;
/// use strum_macros::Display;
///
/// #[derive(Display, Debug)]
/// enum Color {
///     #[strum(serialize = "redred")]
///     Red,
///     Green {
///         range: usize,
///     },
///     Blue(usize),
///     Yellow,
///     #[strum(to_string = "purple with {sat} saturation")]
///     Purple {
///         sat: usize,
///     },
/// }
///
/// // uses the serialize string for Display
/// let red = Color::Red;
/// assert_eq!(String::from("redred"), format!("{}", red));
/// // by default the variants Name
/// let yellow = Color::Yellow;
/// assert_eq!(String::from("Yellow"), yellow.to_string());
/// // or for string formatting
/// println!(
///     "blue: {} green: {}",
///     Color::Blue(10),
///     Color::Green { range: 42 }
/// );
/// // you can also use named fields in message
/// let purple = Color::Purple { sat: 10 };
/// assert_eq!(String::from("purple with 10 saturation"), purple.to_string());
/// ```
#[proc_macro_derive(Display, attributes(strum))]
pub fn display(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::display::display_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Creates a new type that iterates of the variants of an enum.
///
/// Iterate over the variants of an Enum. Any additional data on your variants will be set to `Default::default()`.
/// The macro implements [`strum::IntoEnumIterator`](https://docs.rs/strum/latest/strum/trait.IntoEnumIterator.html) on your enum and creates a new type called `YourEnumIter` that is the iterator object.
/// You cannot derive `EnumIter` on any type with a lifetime bound (`<'a>`) because the iterator would surely
/// create [unbounded lifetimes](https://doc.rust-lang.org/nightly/nomicon/unbounded-lifetimes.html).
///
/// ```
///
/// // You need to bring the trait into scope to use it!
/// use strum::IntoEnumIterator;
/// use strum_macros::EnumIter;
///
/// #[derive(EnumIter, Debug, PartialEq)]
/// enum Color {
///     Red,
///     Green { range: usize },
///     Blue(usize),
///     Yellow,
/// }
///
/// // It's simple to iterate over the variants of an enum.
/// for color in Color::iter() {
///     println!("My favorite color is {:?}", color);
/// }
///
/// let mut ci = Color::iter();
/// assert_eq!(Some(Color::Red), ci.next());
/// assert_eq!(Some(Color::Green {range: 0}), ci.next());
/// assert_eq!(Some(Color::Blue(0)), ci.next());
/// assert_eq!(Some(Color::Yellow), ci.next());
/// assert_eq!(None, ci.next());
/// ```
#[proc_macro_derive(EnumIter, attributes(strum))]
pub fn enum_iter(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::enum_iter::enum_iter_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Generated `is_*()` methods for each variant.
/// E.g. `Color.is_red()`.
///
/// ```
///
/// use strum_macros::EnumIs;
///
/// #[derive(EnumIs, Debug)]
/// enum Color {
///     Red,
///     Green { range: usize },
/// }
///
/// assert!(Color::Red.is_red());
/// assert!(Color::Green{range: 0}.is_green());
/// ```
#[proc_macro_derive(EnumIs, attributes(strum))]
pub fn enum_is(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_is::enum_is_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Generated `try_as_*()` methods for all tuple-style variants.
/// E.g. `Message.try_as_write()`.
///
/// These methods will only be generated for tuple-style variants, not for named or unit variants.
///
/// ```
/// use strum_macros::EnumTryAs;
///
/// #[derive(EnumTryAs, Debug)]
/// enum Message {
///     Quit,
///     Move { x: i32, y: i32 },
///     Write(String),
///     ChangeColor(i32, i32, i32),
/// }
///
/// assert_eq!(
///     Message::Write(String::from("Hello")).try_as_write(),
///     Some(String::from("Hello"))
/// );
/// assert_eq!(
///     Message::ChangeColor(1, 2, 3).try_as_change_color(),
///     Some((1, 2, 3))
/// );
/// ```
#[proc_macro_derive(EnumTryAs, attributes(strum))]
pub fn enum_try_as(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::enum_try_as::enum_try_as_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Creates a new type that maps all the variants of an enum to another generic value.
///
/// This macro only supports enums with unit type variants.A new type called `YourEnumTable<T>`. Essentially, it's a wrapper
/// `[T; YourEnum::Count]` where gets/sets are infallible. Some important caveats to note:
///
/// * The size of `YourEnumTable<T>` increases with the number of variants, not the number of values because it's always
///   fully populated. This means it may not be a good choice for sparsely populated maps.
///
/// * Lookups are basically constant time since it's functionally an array index.
///
/// * Your variants cannot have associated data. You can use `EnumDiscriminants` to generate an Enum with the same
///   names to work around this.
///
/// # Stability
///
/// Several people expressed interest in a data structure like this and pushed the PR through to completion, but the api
/// seems incomplete, and I reserve the right to deprecate it in the future if it becomes clear the design is flawed.
///
/// # Example
/// ```rust
/// use strum_macros::EnumTable;
///
/// #[derive(EnumTable)]
/// enum Color {
///     Red,
///     Yellow,
///     Green,
///     Blue,
/// }
///
/// assert_eq!(ColorTable::default(), ColorTable::new(0, 0, 0, 0));
/// assert_eq!(ColorTable::filled(2), ColorTable::new(2, 2, 2, 2));
/// assert_eq!(ColorTable::from_closure(|_| 3), ColorTable::new(3, 3, 3, 3));
/// assert_eq!(ColorTable::default().transform(|_, val| val + 2), ColorTable::new(2, 2, 2, 2));
///
/// let mut complex_map = ColorTable::from_closure(|color| match color {
///     Color::Red => 0,
///     _ => 3
/// });
///
/// complex_map[Color::Green] = complex_map[Color::Red];
/// assert_eq!(complex_map, ColorTable::new(0, 3, 0, 3));
/// ```
#[doc(hidden)]
#[proc_macro_derive(EnumTable, attributes(strum))]
pub fn enum_table(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::enum_table::enum_table_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Add a function to enum that allows accessing variants by its discriminant
///
/// This macro adds a standalone function to obtain an enum variant by its discriminant. The macro adds
/// `from_repr(discriminant: usize) -> Option<YourEnum>` as a standalone function on the enum. For
/// variants with additional data, the returned variant will use the `Default` trait to fill the
/// data. The discriminant follows the same rules as `rustc`. The first discriminant is zero and each
/// successive variant has a discriminant of one greater than the previous variant, except where an
/// explicit discriminant is specified. The type of the discriminant will match the `repr` type if
/// it is specified.
///
/// When the macro is applied using rustc >= 1.46 and when there is no additional data on any of
/// the variants, the `from_repr` function is marked `const`. rustc >= 1.46 is required
/// to allow `match` statements in `const fn`. The no additional data requirement is due to the
/// inability to use `Default::default()` in a `const fn`.
///
/// You cannot derive `FromRepr` on any type with a lifetime bound (`<'a>`) because the function would surely
/// create [unbounded lifetimes](https://doc.rust-lang.org/nightly/nomicon/unbounded-lifetimes.html).
///
/// ```
///
/// use strum_macros::FromRepr;
///
/// #[derive(FromRepr, Debug, PartialEq)]
/// enum Color {
///     Red,
///     Green { range: usize },
///     Blue(usize),
///     Yellow,
/// }
///
/// assert_eq!(Some(Color::Red), Color::from_repr(0));
/// assert_eq!(Some(Color::Green {range: 0}), Color::from_repr(1));
/// assert_eq!(Some(Color::Blue(0)), Color::from_repr(2));
/// assert_eq!(Some(Color::Yellow), Color::from_repr(3));
/// assert_eq!(None, Color::from_repr(4));
///
/// // Custom discriminant tests
/// #[derive(FromRepr, Debug, PartialEq)]
/// #[repr(u8)]
/// enum Vehicle {
///     Car = 1,
///     Truck = 3,
/// }
///
/// assert_eq!(None, Vehicle::from_repr(0));
/// ```
///
/// On versions of rust >= 1.46, the `from_repr` function is marked `const`.
///
/// ```rust
/// use strum_macros::FromRepr;
///
/// #[derive(FromRepr, Debug, PartialEq)]
/// #[repr(u8)]
/// enum Number {
///     One = 1,
///     Three = 3,
/// }
///
/// # #[rustversion::since(1.46)]
/// const fn number_from_repr(d: u8) -> Option<Number> {
///     Number::from_repr(d)
/// }
///
/// # #[rustversion::before(1.46)]
/// # fn number_from_repr(d: u8) -> Option<Number> {
/// #     Number::from_repr(d)
/// # }
/// assert_eq!(None, number_from_repr(0));
/// assert_eq!(Some(Number::One), number_from_repr(1));
/// assert_eq!(None, number_from_repr(2));
/// assert_eq!(Some(Number::Three), number_from_repr(3));
/// assert_eq!(None, number_from_repr(4));
/// ```
#[proc_macro_derive(FromRepr, attributes(strum))]
pub fn from_repr(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks =
        macros::from_repr::from_repr_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Add a verbose message to an enum variant.
///
/// Encode strings into the enum itself. The `strum_macros::EmumMessage` macro implements the `strum::EnumMessage` trait.
/// `EnumMessage` looks for `#[strum(message="...")]` attributes on your variants.
/// You can also provided a `detailed_message="..."` attribute to create a separate more detailed message than the first.
///
/// `EnumMessage` also exposes the variants doc comments through `get_documentation()`. This is useful in some scenarios,
/// but `get_message` should generally be preferred. Rust doc comments are intended for developer facing documentation,
/// not end user messaging.
///
/// ```
/// // You need to bring the trait into scope to use it
/// use strum::EnumMessage;
/// use strum_macros;
///
/// #[derive(strum_macros::EnumMessage, Debug)]
/// #[allow(dead_code)]
/// enum Color {
///     /// Danger color.
///     #[strum(message = "Red", detailed_message = "This is very red")]
///     Red,
///     #[strum(message = "Simply Green")]
///     Green { range: usize },
///     #[strum(serialize = "b", serialize = "blue")]
///     Blue(usize),
/// }
///
/// // Generated code looks like more or less like this:
/// /*
/// impl ::strum::EnumMessage for Color {
///     fn get_message(&self) -> ::core::option::Option<&'static str> {
///         match self {
///             &Color::Red => ::core::option::Option::Some("Red"),
///             &Color::Green {..} => ::core::option::Option::Some("Simply Green"),
///             _ => None
///         }
///     }
///
///     fn get_detailed_message(&self) -> ::core::option::Option<&'static str> {
///         match self {
///             &Color::Red => ::core::option::Option::Some("This is very red"),
///             &Color::Green {..}=> ::core::option::Option::Some("Simply Green"),
///             _ => None
///         }
///     }
///
///     fn get_documentation(&self) -> ::std::option::Option<&'static str> {
///         match self {
///             &Color::Red => ::std::option::Option::Some("Danger color."),
///             _ => None
///         }
///     }
///
///     fn get_serializations(&self) -> &'static [&'static str] {
///         match self {
///             &Color::Red => {
///                 static ARR: [&'static str; 1] = ["Red"];
///                 &ARR
///             },
///             &Color::Green {..}=> {
///                 static ARR: [&'static str; 1] = ["Green"];
///                 &ARR
///             },
///             &Color::Blue (..) => {
///                 static ARR: [&'static str; 2] = ["b", "blue"];
///                 &ARR
///             },
///         }
///     }
/// }
/// */
///
/// let c = Color::Red;
/// assert_eq!("Red", c.get_message().unwrap());
/// assert_eq!("This is very red", c.get_detailed_message().unwrap());
/// assert_eq!("Danger color.", c.get_documentation().unwrap());
/// assert_eq!(["Red"], c.get_serializations());
/// ```
#[proc_macro_derive(EnumMessage, attributes(strum))]
pub fn enum_messages(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_messages::enum_message_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Add custom properties to enum variants.
///
/// Enables the encoding of arbitrary constants into enum variants. This method
/// currently only supports adding additional string values. Other types of literals are still
/// experimental in the rustc compiler. The generated code works by nesting match statements.
/// The first match statement matches on the type of the enum, and the inner match statement
/// matches on the name of the property requested. This design works well for enums with a small
/// number of variants and properties, but scales linearly with the number of variants so may not
/// be the best choice in all situations.
///
/// ```
///
/// use strum_macros;
/// // bring the trait into scope
/// use strum::EnumProperty;
///
/// #[derive(strum_macros::EnumProperty, Debug)]
/// #[allow(dead_code)]
/// enum Color {
///     #[strum(props(Red = "255", Blue = "255", Green = "255"))]
///     White,
///     #[strum(props(Red = "0", Blue = "0", Green = "0"))]
///     Black,
///     #[strum(props(Red = "0", Blue = "255", Green = "0"))]
///     Blue,
///     #[strum(props(Red = "255", Blue = "0", Green = "0"))]
///     Red,
///     #[strum(props(Red = "0", Blue = "0", Green = "255"))]
///     Green,
/// }
///
/// let my_color = Color::Red;
/// let display = format!(
///     "My color is {:?}. It's RGB is {},{},{}",
///     my_color,
///     my_color.get_str("Red").unwrap(),
///     my_color.get_str("Green").unwrap(),
///     my_color.get_str("Blue").unwrap()
/// );
/// assert_eq!("My color is Red. It\'s RGB is 255,0,0", &display);
/// ```

#[proc_macro_derive(EnumProperty, attributes(strum))]
pub fn enum_properties(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_properties::enum_properties_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Generate a new type with only the discriminant names.
///
/// Given an enum named `MyEnum`, generates another enum called `MyEnumDiscriminants` with the same
/// variants but without any data fields. This is useful when you wish to determine the variant of
/// an `enum` but one or more of the variants contains a non-`Default` field. `From`
/// implementations are generated so that you can easily convert from `MyEnum` to
/// `MyEnumDiscriminants`.
///
/// By default, the generated enum has the following derives: `Clone, Copy, Debug, PartialEq, Eq`.
/// You can add additional derives using the `#[strum_discriminants(derive(AdditionalDerive))]`
/// attribute.
///
/// Note, the variant attributes passed to the discriminant enum are filtered to avoid compilation
/// errors due to the derives mismatches, thus only `#[doc]`, `#[cfg]`, `#[allow]`, and `#[deny]`
/// are passed through by default. If you want to specify a custom attribute on the discriminant
/// variant, wrap it with `#[strum_discriminants(...)]` attribute.
///
/// ```
/// // Bring trait into scope
/// use std::str::FromStr;
/// use strum::{IntoEnumIterator, EnumMessage};
/// use strum_macros::{EnumDiscriminants, EnumIter, EnumString};
///
/// #[derive(Debug)]
/// struct NonDefault;
///
/// // simple example
/// # #[allow(dead_code)]
/// #[derive(Debug, EnumDiscriminants)]
/// #[strum_discriminants(derive(EnumString, EnumMessage))]
/// enum MyEnum {
///     #[strum_discriminants(strum(message = "Variant zero"))]
///     Variant0(NonDefault),
///     Variant1 { a: NonDefault },
/// }
///
/// // You can rename the generated enum using the `#[strum_discriminants(name(OtherName))]` attribute:
/// # #[allow(dead_code)]
/// #[derive(Debug, EnumDiscriminants)]
/// #[strum_discriminants(derive(EnumIter))]
/// #[strum_discriminants(name(MyVariants))]
/// enum MyEnumR {
///     Variant0(bool),
///     Variant1 { a: bool },
/// }
///
/// // test simple example
/// assert_eq!(
///     MyEnumDiscriminants::Variant0,
///     MyEnumDiscriminants::from_str("Variant0").unwrap()
/// );
/// // test rename example combined with EnumIter
/// assert_eq!(
///     vec![MyVariants::Variant0, MyVariants::Variant1],
///     MyVariants::iter().collect::<Vec<_>>()
/// );
///
/// // Make use of the auto-From conversion to check whether an instance of `MyEnum` matches a
/// // `MyEnumDiscriminants` discriminant.
/// assert_eq!(
///     MyEnumDiscriminants::Variant0,
///     MyEnum::Variant0(NonDefault).into()
/// );
/// assert_eq!(
///     MyEnumDiscriminants::Variant0,
///     MyEnumDiscriminants::from(MyEnum::Variant0(NonDefault))
/// );
///
/// // Make use of the EnumMessage on the `MyEnumDiscriminants` discriminant.
/// assert_eq!(
///     MyEnumDiscriminants::Variant0.get_message(),
///     Some("Variant zero")
/// );
/// ```
///
/// It is also possible to specify the visibility (e.g. `pub`/`pub(crate)`/etc.)
/// of the generated enum. By default, the generated enum inherits the
/// visibility of the parent enum it was generated from.
///
/// ```
/// use strum_macros::EnumDiscriminants;
///
/// // You can set the visibility of the generated enum using the `#[strum_discriminants(vis(..))]` attribute:
/// mod inner {
///     use strum_macros::EnumDiscriminants;
///
///     # #[allow(dead_code)]
///     #[derive(Debug, EnumDiscriminants)]
///     #[strum_discriminants(vis(pub))]
///     #[strum_discriminants(name(PubDiscriminants))]
///     enum PrivateEnum {
///         Variant0(bool),
///         Variant1 { a: bool },
///     }
/// }
///
/// // test visibility example, `PrivateEnum` should not be accessible here
/// assert_ne!(
///     inner::PubDiscriminants::Variant0,
///     inner::PubDiscriminants::Variant1,
/// );
/// ```
#[proc_macro_derive(EnumDiscriminants, attributes(strum, strum_discriminants))]
pub fn enum_discriminants(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let toks = macros::enum_discriminants::enum_discriminants_inner(&ast)
        .unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}

/// Add a constant `usize` equal to the number of variants.
///
/// For a given enum generates implementation of `strum::EnumCount`,
/// which adds a static property `COUNT` of type usize that holds the number of variants.
///
/// ```
/// use strum::{EnumCount, IntoEnumIterator};
/// use strum_macros::{EnumCount as EnumCountMacro, EnumIter};
///
/// #[derive(Debug, EnumCountMacro, EnumIter)]
/// enum Week {
///     Sunday,
///     Monday,
///     Tuesday,
///     Wednesday,
///     Thursday,
///     Friday,
///     Saturday,
/// }
///
/// assert_eq!(7, Week::COUNT);
/// assert_eq!(Week::iter().count(), Week::COUNT);
///
/// ```
#[proc_macro_derive(EnumCount, attributes(strum))]
pub fn enum_count(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);
    let toks =
        macros::enum_count::enum_count_inner(&ast).unwrap_or_else(|err| err.to_compile_error());
    debug_print_generated(&ast, &toks);
    toks.into()
}
