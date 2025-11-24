//! Procedural macro for defining global constructor/destructor functions.
//!
//! This provides module initialization/teardown functions for Rust (like
//! `__attribute__((constructor))` in C/C++) for Linux, OSX, and Windows via
//! the `#[ctor]` and `#[dtor]` macros.
//!
//! This library works and is regularly tested on Linux, OSX and Windows, with both `+crt-static` and `-crt-static`.
//! Other platforms are supported but not tested as part of the automatic builds. This library will also work as expected in both
//! `bin` and `cdylib` outputs, ie: the `ctor` and `dtor` will run at executable or library
//! startup/shutdown respectively.
//!
//! This library currently requires Rust > `1.31.0` at a minimum for the
//! procedural macro support.

#![recursion_limit = "256"]

#[doc(hidden)]
#[allow(unused)]
pub use macros::__support;

mod macros;

/// Declarative forms of the `#[ctor]` and `#[dtor]` macros.
///
/// The declarative forms wrap and parse a proc_macro-like syntax like so, and
/// are identical in expansion to the undecorated procedural macros. The
/// declarative forms support the same attribute parameters as the procedural
/// macros.
///
/// ```rust
/// # mod test { use ctor::*; use libc_print::*;
/// ctor::declarative::ctor! {
///   #[ctor]
///   fn foo() {
///     libc_println!("Hello, world!");
///   }
/// }
/// # }
///
/// // ... the above is identical to:
///
/// # mod test_2 { use ctor::*; use libc_print::*;
/// #[ctor]
/// fn foo() {
///   libc_println!("Hello, world!");
/// }
/// # }
/// ```
pub mod declarative {
    #[doc(inline)]
    pub use crate::__support::ctor_parse as ctor;
    #[doc(inline)]
    #[cfg(feature = "dtor")]
    pub use crate::__support::dtor_parse as dtor;
}

/// Marks a function or static variable as a library/executable constructor.
/// This uses OS-specific linker sections to call a specific function at load
/// time.
///
/// # Important notes
///
/// Rust does not make any guarantees about stdlib support for life-before or
/// life-after main. This means that the `ctor` crate may not work as expected
/// in some cases, such as when used in an `async` runtime or making use of
/// stdlib services.
///
/// Multiple startup functions/statics are supported, but the invocation order
/// is not guaranteed.
///
/// The `ctor` crate assumes it is available as a direct dependency, with
/// `extern crate ctor`. If you re-export `ctor` items as part of your crate,
/// you can use the `crate_path` parameter to redirect the macro's output to the
/// correct crate.
///
/// # Attribute parameters
///
///  - `crate_path = ::path::to::ctor::crate`: The path to the `ctor` crate
///    containing the support macros. If you re-export `ctor` items as part of
///    your crate, you can use this to redirect the macro's output to the
///    correct crate.
///  - `used(linker)`: (Advanced) Mark the function as being used in the link
///    phase.
///  - `link_section = "section"`: The section to place the constructor in.
///  - `anonymous`: Do not give the constructor a name in the generated code
///    (allows for multiple constructors with the same name).
///
/// # Examples
///
/// Print a startup message (using `libc_print` for safety):
///
/// ```rust
/// # #![cfg_attr(feature="used_linker", feature(used_with_arg))]
/// # extern crate ctor;
/// # use ctor::*;
/// use libc_print::std_name::println;
///
/// #[ctor]
/// unsafe fn foo() {
///   // Using libc_print which is safe in `#[ctor]`
///   println!("Hello, world!");
/// }
///
/// # fn main() {
/// println!("main()");
/// # }
/// ```
///
/// Make changes to `static` variables:
///
/// ```rust
/// # #![cfg_attr(feature="used_linker", feature(used_with_arg))]
/// # extern crate ctor;
/// # mod test {
/// # use ctor::*;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// static INITED: AtomicBool = AtomicBool::new(false);
///
/// #[ctor]
/// unsafe fn set_inited() {
///   INITED.store(true, Ordering::SeqCst);
/// }
/// # }
/// ```
///
/// Initialize a `HashMap` at startup time:
///
/// ```rust
/// # #![cfg_attr(feature="used_linker", feature(used_with_arg))]
/// # extern crate ctor;
/// # mod test {
/// # use std::collections::HashMap;
/// # use ctor::*;
/// #[ctor]
/// pub static STATIC_CTOR: HashMap<u32, String> = unsafe {
///   let mut m = HashMap::new();
///   for i in 0..100 {
///     m.insert(i, format!("x*100={}", i*100));
///   }
///   m
/// };
/// # }
/// # pub fn main() {
/// #   assert_eq!(test::STATIC_CTOR.len(), 100);
/// #   assert_eq!(test::STATIC_CTOR[&20], "x*100=2000");
/// # }
/// ```
///
/// # Details
///
/// The `#[ctor]` macro makes use of linker sections to ensure that a function
/// is run at startup time.
///
/// ```rust
/// # #![cfg_attr(feature="used_linker", feature(used_with_arg))]
/// # extern crate ctor;
/// # mod test {
/// # use ctor::*;
/// #[ctor]
///
/// unsafe fn my_init_fn() {
///   /* ... */
/// }
/// # }
/// ```
///
/// The above example translates into the following Rust code (approximately):
///
/// ```rust
/// # fn my_init_fn() {}
/// #[used]
/// #[cfg_attr(target_os = "linux", link_section = ".init_array")]
/// #[cfg_attr(target_vendor = "apple", link_section = "__DATA,__mod_init_func,mod_init_funcs")]
/// #[cfg_attr(target_os = "windows", link_section = ".CRT$XCU")]
/// /* ... other platforms elided ... */
/// static INIT_FN: extern fn() = {
///     extern fn init_fn() { my_init_fn(); };
///     init_fn
/// };
/// ```
///
/// For `static` items, the macro generates a `std::sync::OnceLock` that is
/// initialized at startup time.
///
/// ```rust
/// # extern crate ctor;
/// # mod test {
/// # use ctor::*;
/// # use std::collections::HashMap;
/// #[ctor]
/// static FOO: HashMap<u32, String> = unsafe {
///   let mut m = HashMap::new();
///   for i in 0..100 {
///     m.insert(i, format!("x*100={}", i*100));
///   }
///   m
/// };
/// # }
/// ```
///
/// The above example translates into the following Rust code (approximately),
/// which eagerly initializes the `HashMap` inside a `OnceLock` at startup time:
///
/// ```rust
/// # extern crate ctor;
/// # mod test {
/// # use ctor::ctor;
/// # use std::collections::HashMap;
/// static FOO: FooStatic = FooStatic { value: ::std::sync::OnceLock::new() };
/// struct FooStatic {
///   value: ::std::sync::OnceLock<HashMap<u32, String>>,
/// }
///
/// impl ::std::ops::Deref for FooStatic {
///   type Target = HashMap<u32, String>;
///   fn deref(&self) -> &Self::Target {
///     self.value.get_or_init(|| unsafe {
///       let mut m = HashMap::new();
///       for i in 0..100 {
///         m.insert(i, format!("x*100={}", i*100));
///       }
///       m
///     })
///   }
/// }
///
/// #[ctor]
/// unsafe fn init_foo_ctor() {
///   _ = &*FOO;
/// }
/// # }
/// ```
#[doc(inline)]
#[cfg(feature = "proc_macro")]
pub use ctor_proc_macro::ctor;

/// Re-exported `#[dtor]` proc-macro from `dtor` crate.
///
/// See [`::dtor`] for more details.
#[doc(inline)]
#[cfg(feature = "dtor")]
pub use dtor::__dtor_from_ctor as dtor;
