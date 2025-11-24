//! # Pretty Assertions
//!
//! When writing tests in Rust, you'll probably use `assert_eq!(a, b)` _a lot_.
//!
//! If such a test fails, it will present all the details of `a` and `b`.
//! But you have to spot the differences yourself, which is not always straightforward,
//! like here:
//!
//! ![standard assertion](https://raw.githubusercontent.com/rust-pretty-assertions/rust-pretty-assertions/2d2357ff56d22c51a86b2f1cfe6efcee9f5a8081/examples/standard_assertion.png)
//!
//! Wouldn't that task be _much_ easier with a colorful diff?
//!
//! ![pretty assertion](https://raw.githubusercontent.com/rust-pretty-assertions/rust-pretty-assertions/2d2357ff56d22c51a86b2f1cfe6efcee9f5a8081/examples/pretty_assertion.png)
//!
//! Yep — and you only need **one line of code** to make it happen:
//!
//! ```rust
//! use pretty_assertions::{assert_eq, assert_ne};
//! ```
//!
//! <details>
//! <summary>Show the example behind the screenshots above.</summary>
//!
//! ```rust,should_panic
//! // 1. add the `pretty_assertions` dependency to `Cargo.toml`.
//! // 2. insert this line at the top of each module, as needed
//! use pretty_assertions::{assert_eq, assert_ne};
//!
//! #[derive(Debug, PartialEq)]
//! struct Foo {
//!     lorem: &'static str,
//!     ipsum: u32,
//!     dolor: Result<String, String>,
//! }
//!
//! let x = Some(Foo { lorem: "Hello World!", ipsum: 42, dolor: Ok("hey".to_string())});
//! let y = Some(Foo { lorem: "Hello Wrold!", ipsum: 42, dolor: Ok("hey ho!".to_string())});
//!
//! assert_eq!(x, y);
//! ```
//! </details>
//!
//! ## Tip
//!
//! Specify it as [`[dev-dependencies]`](http://doc.crates.io/specifying-dependencies.html#development-dependencies)
//! and it will only be used for compiling tests, examples, and benchmarks.
//! This way the compile time of `cargo build` won't be affected!
//!
//! Also add `#[cfg(test)]` to your `use` statements, like this:
//!
//! ```rust
//! #[cfg(test)]
//! use pretty_assertions::{assert_eq, assert_ne};
//! ```
//!
//! ## Note
//!
//! * Since `Rust 2018` edition, you need to declare
//!   `use pretty_assertions::{assert_eq, assert_ne};` per module.
//!   Before you would write `#[macro_use] extern crate pretty_assertions;`.
//! * The replacement is only effective in your own crate, not in other libraries
//!   you include.
//! * `assert_ne` is also switched to multi-line presentation, but does _not_ show
//!   a diff.
//!
//! ## Features
//!
//! Features provided by the crate are:
//!
//! - `std`: Use the Rust standard library. Enabled by default.
//!   Exactly one of `std` and `alloc` is required.
//! - `alloc`: Use the `alloc` crate.
//!   Exactly one of `std` and `alloc` is required.
//! - `unstable`: opt-in to unstable features that may not follow Semantic Versioning.
//!   The implementation behind this feature is subject to change without warning between patch versions.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(clippy::all, missing_docs, unsafe_code)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;
use core::fmt::{self, Debug, Display};

mod printer;

/// A comparison of two values.
///
/// Where both values implement `Debug`, the comparison can be displayed as a pretty diff.
///
/// ```
/// use pretty_assertions::Comparison;
///
/// print!("{}", Comparison::new(&123, &134));
/// ```
///
/// The values may have different types, although in practice they are usually the same.
pub struct Comparison<'a, TLeft, TRight>
where
    TLeft: ?Sized,
    TRight: ?Sized,
{
    left: &'a TLeft,
    right: &'a TRight,
}

impl<'a, TLeft, TRight> Comparison<'a, TLeft, TRight>
where
    TLeft: ?Sized,
    TRight: ?Sized,
{
    /// Store two values to be compared in future.
    ///
    /// Expensive diffing is deferred until calling `Debug::fmt`.
    pub fn new(left: &'a TLeft, right: &'a TRight) -> Comparison<'a, TLeft, TRight> {
        Comparison { left, right }
    }
}

impl<'a, TLeft, TRight> Display for Comparison<'a, TLeft, TRight>
where
    TLeft: Debug + ?Sized,
    TRight: Debug + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // To diff arbitary types, render them as debug strings
        let left_debug = format!("{:#?}", self.left);
        let right_debug = format!("{:#?}", self.right);
        // And then diff the debug output
        printer::write_header(f)?;
        printer::write_lines(f, &left_debug, &right_debug)
    }
}

/// A comparison of two strings.
///
/// In contrast to [`Comparison`], which uses the [`core::fmt::Debug`] representation,
/// `StrComparison` uses the string values directly, resulting in multi-line output for multiline strings.
///
/// ```
/// use pretty_assertions::StrComparison;
///
/// print!("{}", StrComparison::new("foo\nbar", "foo\nbaz"));
/// ```
///
/// ## Value type bounds
///
/// Any value that can be referenced as a [`str`] via [`AsRef`] may be used:
///
/// ```
/// use pretty_assertions::StrComparison;
///
/// #[derive(PartialEq)]
/// struct MyString(String);
///
/// impl AsRef<str> for MyString {
///     fn as_ref(&self) -> &str {
///         &self.0
///     }
/// }
///
/// print!(
///     "{}",
///     StrComparison::new(
///         &MyString("foo\nbar".to_owned()),
///         &MyString("foo\nbaz".to_owned()),
///     ),
/// );
/// ```
///
/// The values may have different types, although in practice they are usually the same.
pub struct StrComparison<'a, TLeft, TRight>
where
    TLeft: ?Sized,
    TRight: ?Sized,
{
    left: &'a TLeft,
    right: &'a TRight,
}

impl<'a, TLeft, TRight> StrComparison<'a, TLeft, TRight>
where
    TLeft: AsRef<str> + ?Sized,
    TRight: AsRef<str> + ?Sized,
{
    /// Store two values to be compared in future.
    ///
    /// Expensive diffing is deferred until calling `Debug::fmt`.
    pub fn new(left: &'a TLeft, right: &'a TRight) -> StrComparison<'a, TLeft, TRight> {
        StrComparison { left, right }
    }
}

impl<'a, TLeft, TRight> Display for StrComparison<'a, TLeft, TRight>
where
    TLeft: AsRef<str> + ?Sized,
    TRight: AsRef<str> + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        printer::write_header(f)?;
        printer::write_lines(f, self.left.as_ref(), self.right.as_ref())
    }
}

/// Asserts that two expressions are equal to each other (using [`PartialEq`]).
///
/// On panic, this macro will print a diff derived from [`Debug`] representation of
/// each value.
///
/// This is a drop in replacement for [`core::assert_eq!`].
/// You can provide a custom panic message if desired.
///
/// # Examples
///
/// ```
/// use pretty_assertions::assert_eq;
///
/// let a = 3;
/// let b = 1 + 2;
/// assert_eq!(a, b);
///
/// assert_eq!(a, b, "we are testing addition with {} and {}", a, b);
/// ```
#[macro_export]
macro_rules! assert_eq {
    ($left:expr, $right:expr$(,)?) => ({
        $crate::assert_eq!(@ $left, $right, "", "");
    });
    ($left:expr, $right:expr, $($arg:tt)*) => ({
        $crate::assert_eq!(@ $left, $right, ": ", $($arg)+);
    });
    (@ $left:expr, $right:expr, $maybe_colon:expr, $($arg:tt)*) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    use $crate::private::CreateComparison;
                    ::core::panic!("assertion failed: `(left == right)`{}{}\
                       \n\
                       \n{}\
                       \n",
                       $maybe_colon,
                       format_args!($($arg)*),
                       (left_val, right_val).create_comparison()
                    )
                }
            }
        }
    });
}

/// Asserts that two expressions are equal to each other (using [`PartialEq`]).
///
/// On panic, this macro will print a diff derived from each value's [`str`] representation.
/// See [`StrComparison`] for further details.
///
/// This is a drop in replacement for [`core::assert_eq!`].
/// You can provide a custom panic message if desired.
///
/// # Examples
///
/// ```
/// use pretty_assertions::assert_str_eq;
///
/// let a = "foo\nbar";
/// let b = ["foo", "bar"].join("\n");
/// assert_str_eq!(a, b);
///
/// assert_str_eq!(a, b, "we are testing concatenation with {} and {}", a, b);
/// ```
#[macro_export]
macro_rules! assert_str_eq {
    ($left:expr, $right:expr$(,)?) => ({
        $crate::assert_str_eq!(@ $left, $right, "", "");
    });
    ($left:expr, $right:expr, $($arg:tt)*) => ({
        $crate::assert_str_eq!(@ $left, $right, ": ", $($arg)+);
    });
    (@ $left:expr, $right:expr, $maybe_colon:expr, $($arg:tt)*) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    ::core::panic!("assertion failed: `(left == right)`{}{}\
                       \n\
                       \n{}\
                       \n",
                       $maybe_colon,
                       format_args!($($arg)*),
                       $crate::StrComparison::new(left_val, right_val)
                    )
                }
            }
        }
    });
}

/// Asserts that two expressions are not equal to each other (using [`PartialEq`]).
///
/// On panic, this macro will print the values of the expressions with their
/// [`Debug`] representations.
///
/// This is a drop in replacement for [`core::assert_ne!`].
/// You can provide a custom panic message if desired.
///
/// # Examples
///
/// ```
/// use pretty_assertions::assert_ne;
///
/// let a = 3;
/// let b = 2;
/// assert_ne!(a, b);
///
/// assert_ne!(a, b, "we are testing that the values are not equal");
/// ```
#[macro_export]
macro_rules! assert_ne {
    ($left:expr, $right:expr$(,)?) => ({
        $crate::assert_ne!(@ $left, $right, "", "");
    });
    ($left:expr, $right:expr, $($arg:tt)+) => ({
        $crate::assert_ne!(@ $left, $right, ": ", $($arg)+);
    });
    (@ $left:expr, $right:expr, $maybe_colon:expr, $($arg:tt)+) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if *left_val == *right_val {
                    ::core::panic!("assertion failed: `(left != right)`{}{}\
                        \n\
                        \nBoth sides:\
                        \n{:#?}\
                        \n\
                        \n",
                        $maybe_colon,
                        format_args!($($arg)+),
                        left_val
                    )
                }
            }
        }
    });
}

/// Asserts that a value matches a pattern.
///
/// On panic, this macro will print a diff derived from [`Debug`] representation of
/// the value, and a string representation of the pattern.
///
/// This is a drop in replacement for [`core::assert_matches::assert_matches!`].
/// You can provide a custom panic message if desired.
///
/// # Examples
///
/// ```
/// use pretty_assertions::assert_matches;
///
/// let a = Some(3);
/// assert_matches!(a, Some(_));
///
/// assert_matches!(a, Some(value) if value > 2, "we are testing {:?} with a pattern", a);
/// ```
///
/// # Features
///
/// Requires the `unstable` feature to be enabled.
///
/// **Please note:** implementation under the `unstable` feature may be changed between
/// patch versions without warning.
#[cfg(feature = "unstable")]
#[macro_export]
macro_rules! assert_matches {
    ($left:expr, $( $pattern:pat )|+ $( if $guard: expr )? $(,)?) => ({
        match $left {
            $( $pattern )|+ $( if $guard )? => {}
            ref left_val => {
                $crate::assert_matches!(
                    @
                    left_val,
                    ::core::stringify!($($pattern)|+ $(if $guard)?),
                    "",
                    ""
                );
            }
        }
    });
    ($left:expr, $( $pattern:pat )|+ $( if $guard: expr )?, $($arg:tt)+) => ({
        match $left {
            $( $pattern )|+ $( if $guard )? => {}
            ref left_val => {
                $crate::assert_matches!(
                    @
                    left_val,
                    ::core::stringify!($($pattern)|+ $(if $guard)?),
                    ": ",
                    $($arg)+
                );
            }
        }

    });
    (@ $left:expr, $right:expr, $maybe_colon:expr, $($arg:tt)*) => ({
        match (&($left), &($right)) {
            (left_val, right_val) => {
                // Use the Display implementation to display the pattern,
                // as using Debug would add another layer of quotes to the output.
                struct Pattern<'a>(&'a str);
                impl ::core::fmt::Debug for Pattern<'_> {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        ::core::fmt::Display::fmt(self.0, f)
                    }
                }

                ::core::panic!("assertion failed: `(left matches right)`{}{}\
                   \n\
                   \n{}\
                   \n",
                   $maybe_colon,
                   format_args!($($arg)*),
                   $crate::Comparison::new(left_val, &Pattern(right_val))
                )
            }
        }
    });
}

// Not public API. Used by the expansion of this crate's assert macros.
#[doc(hidden)]
pub mod private {
    #[cfg(feature = "alloc")]
    use alloc::string::String;

    pub trait CompareAsStrByDefault: AsRef<str> {}
    impl CompareAsStrByDefault for str {}
    impl CompareAsStrByDefault for String {}
    impl<T: CompareAsStrByDefault + ?Sized> CompareAsStrByDefault for &T {}

    pub trait CreateComparison {
        type Comparison;
        fn create_comparison(self) -> Self::Comparison;
    }

    impl<'a, T, U> CreateComparison for &'a (T, U) {
        type Comparison = crate::Comparison<'a, T, U>;
        fn create_comparison(self) -> Self::Comparison {
            crate::Comparison::new(&self.0, &self.1)
        }
    }

    impl<'a, T, U> CreateComparison for (&'a T, &'a U)
    where
        T: CompareAsStrByDefault + ?Sized,
        U: CompareAsStrByDefault + ?Sized,
    {
        type Comparison = crate::StrComparison<'a, T, U>;
        fn create_comparison(self) -> Self::Comparison {
            crate::StrComparison::new(self.0, self.1)
        }
    }
}
