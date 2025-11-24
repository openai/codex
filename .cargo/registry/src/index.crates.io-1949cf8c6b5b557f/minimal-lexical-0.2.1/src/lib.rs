//! Fast, minimal float-parsing algorithm.
//!
//! minimal-lexical has a simple, high-level API with a single
//! exported function: [`parse_float`].
//!
//! [`parse_float`] expects a forward iterator for the integer
//! and fraction digits, as well as a parsed exponent as an [`i32`].
//!
//! For more examples, please see [simple-example](https://github.com/Alexhuszagh/minimal-lexical/blob/master/examples/simple.rs).
//!
//! EXAMPLES
//! --------
//!
//! ```
//! extern crate minimal_lexical;
//!
//! // Let's say we want to parse "1.2345".
//! // First, we need an external parser to extract the integer digits ("1"),
//! // the fraction digits ("2345"), and then parse the exponent to a 32-bit
//! // integer (0).
//! // Warning:
//! // --------
//! //  Please note that leading zeros must be trimmed from the integer,
//! //  and trailing zeros must be trimmed from the fraction. This cannot
//! //  be handled by minimal-lexical, since we accept iterators.
//! let integer = b"1";
//! let fraction = b"2345";
//! let float: f64 = minimal_lexical::parse_float(integer.iter(), fraction.iter(), 0);
//! println!("float={:?}", float);    // 1.235
//! ```
//!
//! [`parse_float`]: fn.parse_float.html
//! [`i32`]: https://doc.rust-lang.org/stable/std/primitive.i32.html

// FEATURES

// We want to have the same safety guarantees as Rust core,
// so we allow unused unsafe to clearly document safety guarantees.
#![allow(unused_unsafe)]
#![cfg_attr(feature = "lint", warn(unsafe_op_in_unsafe_fn))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(feature = "alloc", not(feature = "std")))]
extern crate alloc;

pub mod bellerophon;
pub mod bigint;
pub mod extended_float;
pub mod fpu;
pub mod heapvec;
pub mod lemire;
pub mod libm;
pub mod mask;
pub mod num;
pub mod number;
pub mod parse;
pub mod rounding;
pub mod slow;
pub mod stackvec;
pub mod table;

mod table_bellerophon;
mod table_lemire;
mod table_small;

// API
pub use self::num::Float;
pub use self::parse::parse_float;
