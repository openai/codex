#![doc = include_str!("../README.md")]

mod macros;

#[doc(hidden)]
#[allow(unused)]
pub use macros::__support;

/// Marks a function as a library/executable destructor. This uses OS-specific
/// linker sections to call a specific function at termination time.
///
/// Multiple shutdown functions are supported, but the invocation order is not
/// guaranteed.
///
/// # Attribute parameters
///
///  - `crate_path = ::path::to::dtor::crate`: The path to the `dtor` crate
///    containing the support macros. If you re-export `dtor` items as part of
///    your crate, you can use this to redirect the macro's output to the
///    correct crate.
///  - `used(linker)`: (Advanced) Mark the function as being used in the link
///    phase.
///  - `link_section = "section"`: The section to place the dtor's code in.
///  - `anonymous`: Do not give the destructor a name in the generated code
///    (allows for multiple destructors with the same name).
///
/// ```rust
/// # #![cfg_attr(feature="used_linker", feature(used_with_arg))]
/// # extern crate dtor;
/// # use dtor::dtor;
/// # fn main() {}
///
/// #[dtor]
/// fn shutdown() {
///   /* ... */
/// }
/// ```
#[doc(inline)]
#[cfg(feature = "proc_macro")]
pub use dtor_proc_macro::dtor;

#[doc(hidden)]
#[cfg(feature = "proc_macro")]
pub use dtor_proc_macro::__dtor_from_ctor;

/// Declarative forms of the `#[dtor]` macro.
///
/// The declarative forms wrap and parse a proc_macro-like syntax like so, and
/// are identical in expansion to the undecorated procedural macros. The
/// declarative forms support the same attribute parameters as the procedural
/// macros.
///
/// ```rust
/// # #[cfg(any())] mod test { use dtor::*; use libc_print::*;
/// dtor::declarative::dtor! {
///   #[dtor]
///   fn foo() {
///     libc_println!("Goodbye, world!");
///   }
/// }
/// # }
///
/// // ... the above is identical to:
///
/// # #[cfg(any())] mod test_2 { use dtor::*; use libc_print::*;
/// #[dtor]
/// fn foo() {
///   libc_println!("Goodbye, world!");
/// }
/// # }
/// ```
pub mod declarative {
    #[doc(inline)]
    pub use crate::__support::dtor_parse as dtor;
}
