//
// Doc comment
//
// Copyright (c) 2018 Guillaume Gomez
//

#![cfg_attr(feature = "no_core", feature(no_core))]
#![cfg_attr(feature = "no_core", no_core)]
#![cfg_attr(not(feature = "no_core"), no_std)]

//! The point of this (small) crate is to allow you to add doc comments from macros or
//! to test external markdown files' code blocks through `rustdoc`.
//!
//! ## Including file(s) for testing
//!
//! Let's assume you want to test code examples in your `README.md` file which
//! looks like this:
//!
//! ````text
//! # A crate
//!
//! Here is a code example:
//!
//! ```rust
//! let x = 2;
//! assert!(x != 0);
//! ```
//! ````
//!
//! You can use the `doc_comment!` macro to test it like this:
//!
//! ```
//! #[macro_use]
//! extern crate doc_comment;
//!
//! // When running `cargo test`, rustdoc will check this file as well.
//! doc_comment!(include_str!("../README.md"));
//! # fn main() {}
//! ```
//!
//! Please note that can also use the `doctest!` macro to have a shorter way to test an outer
//! file:
//!
//! ```no_run
//! #[macro_use]
//! extern crate doc_comment;
//!
//! doctest!("../README.md");
//! # fn main() {}
//! ```
//!
//! Please also note that you can use `#[cfg(doctest)]`:
//!
//! ```no_run
//! # #[macro_use]
//! # extern crate doc_comment;
//! #[cfg(doctest)]
//! doctest!("../README.md");
//! # fn main() {}
//! ```
//!
//! In this case, the examples in the `README.md` file will only be run on `cargo test`. You
//! can find more information about `#[cfg(doctest)]` in [this blogpost](https://blog.guillaume-gomez.fr/articles/2020-03-07+cfg%28doctest%29+is+stable+and+you+should+use+it).
//!
//! ## Generic documentation
//!
//! Now let's imagine you want to write documentation once for multiple types but
//! still having examples specific to each type:
//!
//! ```
//! // The macro which generates types
//! macro_rules! gen_types {
//!     ($tyname:ident) => {
//!         /// This is a wonderful generated struct!
//!         ///
//!         /// You can use it as follow:
//!         ///
//!         /// ```
//!         /// let x = FirstOne {
//!         ///     field1: 0,
//!         ///     field2: 0,
//!         ///     field3: 0,
//!         ///     field4: 0,
//!         /// };
//!         ///
//!         /// println!("Created a new instance of FirstOne: {:?}", x);
//!         /// ```
//!         #[derive(Debug)]
//!         pub struct $tyname {
//!             pub field1: u8,
//!             pub field2: u16,
//!             pub field3: u32,
//!             pub field4: u64,
//!         }
//!     }
//! }
//!
//! // Now let's actually generate types:
//! gen_types!(FirstOne);
//! gen_types!(SecondOne);
//! gen_types!(Another);
//! ```
//!
//! So now we have created three structs with different names, but they all have the exact same
//! documentation, which is an issue for any structs not called `FirstOne`. That's where
//! [`doc_comment!`] macro comes in handy!
//!
//! Let's rewrite the `gen_types!` macro:
//!
//!     // Of course, we need to import the `doc_comment` macro:
//!     #[macro_use]
//!     extern crate doc_comment;
//!
//!     macro_rules! gen_types {
//!         ($tyname:ident) => {
//!             doc_comment! {
//!     concat!("This is a wonderful generated struct!
//!
//!     You can use it as follow:
//!
//!     ```
//!     let x = ", stringify!($tyname), " {
//!         field1: 0,
//!         field2: 0,
//!         field3: 0,
//!         field4: 0,
//!     };
//!
//!     println!(\"Created a new instance of ", stringify!($tyname), ": {:?}\", x);
//!     ```"),
//!                 #[derive(Debug)]
//!                 pub struct $tyname {
//!                     pub field1: u8,
//!                     pub field2: u16,
//!                     pub field3: u32,
//!                     pub field4: u64,
//!                 }
//!             }
//!         }
//!     }
//!
//!     gen_types!(FirstOne);
//!     gen_types!(SecondOne);
//!     gen_types!(Another);
//!     # fn main() {}
//!
//! Now each struct has doc which match itself!

/// This macro can be used to generate documentation upon a type/item (or just to test outer
/// markdown file code examples).
///
/// # Example
///
/// ```
/// #[macro_use]
/// extern crate doc_comment;
///
/// // If you just want to test an outer markdown file:
/// doc_comment!(include_str!("../README.md"));
///
/// // If you want to document an item:
/// doc_comment!("fooo", pub struct Foo {});
/// # fn main() {}
/// ```
#[macro_export]
macro_rules! doc_comment {
    ($x:expr) => {
        #[doc = $x]
        extern {}
    };
    ($x:expr, $($tt:tt)*) => {
        #[doc = $x]
        $($tt)*
    };
}

/// This macro provides a simpler way to test an outer markdown file.
///
/// # Example
///
/// ```
/// extern crate doc_comment;
///
/// // The two next lines are doing exactly the same thing:
/// doc_comment::doc_comment!(include_str!("../README.md"));
/// doc_comment::doctest!("../README.md");
///
/// // If you want to have a name for your tests:
/// doc_comment::doctest!("../README.md", another);
/// # fn main() {}
/// ```
#[cfg(not(feature = "old_macros"))]
#[macro_export]
macro_rules! doctest {
    ($x:expr) => {
        doc_comment::doc_comment!(include_str!($x));
    };
    ($x:expr, $y:ident) => {
        doc_comment::doc_comment!(include_str!($x), mod $y {});
    };
}

/// This macro provides a simpler way to test an outer markdown file.
///
/// # Example
///
/// ```
/// #[macro_use]
/// extern crate doc_comment;
///
/// // The two next lines are doing exactly the same thing:
/// doc_comment!(include_str!("../README.md"));
/// doctest!("../README.md");
///
/// // If you want to have a name for your tests:
/// doctest!("../README.md", another);
/// # fn main() {}
/// ```
#[cfg(feature = "old_macros")]
#[macro_export]
macro_rules! doctest {
    ($x:expr) => {
        doc_comment!(include_str!($x));
    };
    ($x:expr, $y:ident) => {
        doc_comment!(include_str!($x), mod $y {});
    };
}
