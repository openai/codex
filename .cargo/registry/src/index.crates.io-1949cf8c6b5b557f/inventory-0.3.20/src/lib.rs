//! [![github]](https://github.com/dtolnay/inventory)&ensp;[![crates-io]](https://crates.io/crates/inventory)&ensp;[![docs-rs]](https://docs.rs/inventory)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! **Typed distributed plugin registration.**
//!
//! This crate provides a way to set up a plugin registry into which plugins
//! can be registered from any source file linked into your application. There
//! does not need to be a central list of all the plugins.
//!
//! # Examples
//!
//! Suppose we are writing a command line flags library and want to allow any
//! source file in the application to register command line flags that are
//! relevant to it.
//!
//! This is the flag registration style used by [gflags] and is better suited
//! for large scale development than maintaining a single central list of flags,
//! as the central list would become an endless source of merge conflicts in an
//! application developed simultaneously by thousands of developers.
//!
//! [gflags]: https://gflags.github.io/gflags/
//!
//! ## Instantiating the plugin registry
//!
//! Let's use a `struct Flag` as the plugin type, which will contain the short
//! name of the flag like `-v`, the full name like `--verbose`, and maybe other
//! information like argument type and help text. We instantiate a plugin
//! registry with an invocation of `inventory::collect!`.
//!
//! ```
//! pub struct Flag {
//!     short: char,
//!     name: &'static str,
//!     /* ... */
//! }
//!
//! impl Flag {
//!     pub const fn new(short: char, name: &'static str) -> Self {
//!         Flag { short, name }
//!     }
//! }
//!
//! inventory::collect!(Flag);
//! ```
//!
//! This `collect!` call must be in the same crate that defines the plugin type.
//! This macro does not "run" anything so place it outside of any function body.
//!
//! ## Registering plugins
//!
//! Now any crate with access to the `Flag` type can register flags as a plugin.
//! Plugins can be registered by the same crate that declares the plugin type,
//! or by any downstream crate.
//!
//! ```
//! # struct Flag;
//! #
//! # impl Flag {
//! #     const fn new(short: char, name: &'static str) -> Self {
//! #         Flag
//! #     }
//! # }
//! #
//! # inventory::collect!(Flag);
//! #
//! inventory::submit! {
//!     Flag::new('v', "verbose")
//! }
//! #
//! # fn main() {}
//! ```
//!
//! The `submit!` macro does not "run" anything so place it outside of any
//! function body. In particular, note that all `submit!` invocations across all
//! source files linked into your application all take effect simultaneously. A
//! `submit!` invocation is not a statement that needs to be called from `main`
//! in order to execute.
//!
//! ## Iterating over plugins
//!
//! The value `inventory::iter::<T>` is an iterator with element type `&'static
//! T` that iterates over all plugins registered of type `T`.
//!
//! ```
//! # struct Flag {
//! #     short: char,
//! #     name: &'static str,
//! # }
//! #
//! # inventory::collect!(Flag);
//! #
//! for flag in inventory::iter::<Flag> {
//!     println!("-{}, --{}", flag.short, flag.name);
//! }
//! ```
//!
//! There is no guarantee about the order that plugins of the same type are
//! visited by the iterator. They may be visited in any order.
//!
//! ## WebAssembly and constructors
//!
//! `inventory` supports all WebAssembly targets, including
//! `wasm*-unknown-unknown`. However, in unusual circumstances, ensuring that
//! constructors run may require some extra effort. The Wasm linker will
//! synthesize a function `extern "C" unsafe fn __wasm_call_ctors()` which calls
//! all constructors when invoked; this function will *not* be exported from the
//! module unless you do so explicitly. Depending on the result of a heuristic,
//! the linker may or may not insert a call to this function from the beginning
//! of every function that your module exports. Specifically, it regards a
//! module as having "command-style linkage" if:
//!
//! * it is not relocatable;
//! * it is not a position-independent executable;
//! * and it does not call `__wasm_call_ctors`, directly or indirectly, from any
//!   exported function.
//!
//! The linker expects that the embedder will call into a command-style module
//! only once per instantiation. Violation of this expectation can result in
//! `__wasm_call_ctors` being called multiple times. This is dangerous in
//! general, but safe and mostly harmless in the case of constructors generated
//! by `inventory`, which are idempotent.
//!
//! If you are building a module which relies on constructors and may be called
//! into multiple times per instance, you should export `__wasm_call_ctors` (or
//! a wrapper around it) and ensure that the embedder calls it immediately after
//! instantiation. Even though `inventory` may work fine without this, it is
//! still good practice, because it avoids unnecessary overhead from repeated
//! constructor invocation. It also can prevent unsoundness if some of your
//! constructors are generated by other crates or other programming languages.
//!
//! ```
//! #[cfg(target_family = "wasm")]
//! unsafe extern "C" {
//!     fn __wasm_call_ctors();
//! }
//!
//! fn main() {
//!     #[cfg(target_family = "wasm")]
//!     unsafe {
//!         __wasm_call_ctors();
//!     }
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/inventory/0.3.20")]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(
    clippy::doc_markdown,
    clippy::empty_enum,
    clippy::expl_impl_clone_on_copy,
    clippy::let_underscore_untyped,
    clippy::let_unit_value,
    clippy::must_use_candidate,
    clippy::new_without_default,
    clippy::semicolon_if_nothing_returned, // https://github.com/rust-lang/rust-clippy/issues/7324
)]

// Not public API.
#[doc(hidden)]
pub extern crate core;

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr;
#[cfg(target_family = "wasm")]
use core::sync::atomic::AtomicBool;
use core::sync::atomic::{AtomicPtr, Ordering};

// Not public API. Used by generated code.
#[doc(hidden)]
pub struct Registry {
    head: AtomicPtr<Node>,
}

// Not public API. Used by generated code.
#[doc(hidden)]
pub struct Node {
    pub value: &'static dyn ErasedNode,
    pub next: UnsafeCell<Option<&'static Node>>,
    #[cfg(target_family = "wasm")]
    pub initialized: AtomicBool,
}

// The `value` is Sync, and `next` is only mutated during submit, which is prior
// to any reads.
unsafe impl Sync for Node {}

// Not public API. Used by generated code.
#[doc(hidden)]
pub trait ErasedNode: Sync {
    // SAFETY: requires *node.value is of type Self.
    unsafe fn submit(&self, node: &'static Node);
}

impl<T: Collect> ErasedNode for T {
    unsafe fn submit(&self, node: &'static Node) {
        unsafe {
            T::registry().submit(node);
        }
    }
}

/// Trait bound corresponding to types that can be iterated by inventory::iter.
///
/// This trait cannot be implemented manually. Instead use the [`collect`] macro
/// which expands to an implementation of this trait for the given type.
///
/// # Examples
///
/// ```
/// use inventory::Collect;
///
/// fn count_plugins<T: Collect>() -> usize {
///     inventory::iter::<T>.into_iter().count()
/// }
/// ```
pub trait Collect: Sync + Sized + 'static {
    #[doc(hidden)]
    fn registry() -> &'static Registry;
}

impl Registry {
    // Not public API. Used by generated code.
    pub const fn new() -> Self {
        Registry {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    // SAFETY: requires type of *new.value matches the $ty surrounding the
    // declaration of this registry in inventory::collect macro.
    unsafe fn submit(&'static self, new: &'static Node) {
        // The WebAssembly linker uses an unreliable heuristic to determine
        // whether a module is a "command-style" linkage, for which it will
        // insert a call to  `__wasm_call_ctors` at the top of every exported
        // function. It expects that the embedder will call into such modules
        // only once per instantiation. If this heuristic goes wrong, we can end
        // up having our constructors invoked multiple times, which without this
        // safeguard would lead to our registry's linked list becoming circular.
        // On non-Wasm platforms, this check is unnecessary, so we skip it.
        #[cfg(target_family = "wasm")]
        if new.initialized.swap(true, Ordering::Relaxed) {
            return;
        }

        let mut head = self.head.load(Ordering::Relaxed);
        loop {
            unsafe {
                *new.next.get() = head.as_ref();
            }
            let new_ptr = new as *const Node as *mut Node;
            match self
                .head
                .compare_exchange(head, new_ptr, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => return,
                Err(prev) => head = prev,
            }
        }
    }
}

/// An iterator over plugins registered of a given type.
///
/// The value `inventory::iter::<T>` is an iterator with element type `&'static
/// T`.
///
/// There is no guarantee about the order that plugins of the same type are
/// visited by the iterator. They may be visited in any order.
///
/// # Examples
///
/// ```
/// # struct Flag {
/// #     short: char,
/// #     name: &'static str,
/// # }
/// #
/// # inventory::collect!(Flag);
/// #
/// # const IGNORE: &str = stringify! {
/// use my_flags::Flag;
/// # };
///
/// fn main() {
///     for flag in inventory::iter::<Flag> {
///         println!("-{}, --{}", flag.short, flag.name);
///     }
/// }
/// ```
///
/// Refer to the [crate level documentation](index.html) for a complete example
/// of instantiating a plugin registry and submitting plugins.
#[allow(non_camel_case_types)]
pub type iter<T> = private::iter<T>;

mod void_iter {
    enum Void {}

    #[repr(C, packed)]
    pub struct Iter<T>([*const T; 0], Void);

    unsafe impl<T> Send for Iter<T> {}
    unsafe impl<T> Sync for Iter<T> {}
}

mod value_iter {
    #[doc(hidden)]
    pub use crate::private::iter::iter;
}

mod private {
    // Based on https://github.com/dtolnay/ghost
    #[allow(non_camel_case_types)]
    pub enum iter<T> {
        __Phantom(crate::void_iter::Iter<T>),
        iter,
    }

    #[doc(hidden)]
    pub use crate::value_iter::*;
}

#[doc(hidden)]
pub use crate::private::*;

const _: () = {
    fn into_iter<T: Collect>() -> Iter<T> {
        let head = T::registry().head.load(Ordering::Acquire);
        Iter {
            // Head pointer is always null or valid &'static Node.
            node: unsafe { head.as_ref() },
            marker: PhantomData,
        }
    }

    impl<T: Collect> IntoIterator for iter<T> {
        type Item = &'static T;
        type IntoIter = Iter<T>;

        fn into_iter(self) -> Self::IntoIter {
            into_iter()
        }
    }

    #[doc(hidden)]
    impl<T: Collect> Deref for iter<T> {
        type Target = fn() -> Iter<T>;
        fn deref(&self) -> &Self::Target {
            &(into_iter as fn() -> Iter<T>)
        }
    }

    pub struct Iter<T: 'static> {
        node: Option<&'static Node>,
        marker: PhantomData<T>,
    }

    impl<T: 'static> Iterator for Iter<T> {
        type Item = &'static T;

        fn next(&mut self) -> Option<Self::Item> {
            let node = self.node?;
            unsafe {
                let value_ptr = (node.value as *const dyn ErasedNode).cast::<T>();
                self.node = *node.next.get();
                Some(&*value_ptr)
            }
        }
    }

    impl<T> Clone for Iter<T> {
        fn clone(&self) -> Self {
            Self {
                node: self.node,
                marker: PhantomData,
            }
        }
    }
};

/// Associate a plugin registry with the specified type.
///
/// This call must be in the same crate that defines the plugin type. This macro
/// does not "run" anything so place it outside of any function body.
///
/// # Examples
///
/// Suppose we are writing a command line flags library and want to allow any
/// source file in the application to register command line flags that are
/// relevant to it.
///
/// This is the flag registration style used by [gflags] and is better suited
/// for large scale development than maintaining a single central list of flags,
/// as the central list would become an endless source of merge conflicts.
///
/// [gflags]: https://gflags.github.io/gflags/
///
/// ```
/// pub struct Flag {
///     short: char,
///     name: &'static str,
///     /* ... */
/// }
///
/// inventory::collect!(Flag);
/// ```
///
/// Refer to the [crate level documentation](index.html) for a complete example
/// of submitting plugins and iterating a plugin registry.
#[macro_export]
macro_rules! collect {
    ($ty:ty) => {
        impl $crate::Collect for $ty {
            #[inline]
            fn registry() -> &'static $crate::Registry {
                static REGISTRY: $crate::Registry = $crate::Registry::new();
                &REGISTRY
            }
        }
    };
}

/// Enter an element into the plugin registry corresponding to its type.
///
/// This call may be in the same crate that defines the type, or downstream in
/// any crate that depends on that crate.
///
/// This macro does not "run" anything so place it outside of any function body.
/// In particular, note that all `submit!` invocations across all source files
/// linked into your application all take effect simultaneously. A `submit!`
/// invocation is not a statement that needs to be called from `main` in order
/// to execute.
///
/// # Examples
///
/// Put `submit!` invocations outside of any function body.
///
/// ```
/// # struct Flag;
/// #
/// # impl Flag {
/// #     const fn new(short: char, name: &'static str) -> Self {
/// #         Flag
/// #     }
/// # }
/// #
/// # inventory::collect!(Flag);
/// #
/// inventory::submit! {
///     Flag::new('v', "verbose")
/// }
/// #
/// # fn main() {}
/// ```
///
/// Do not try to invoke `submit!` from inside of a function body as it does not
/// do what you want.
///
/// ```compile_fail
/// // Do not do this.
/// fn submit_flags(has_verbose_flag: bool) {
///     if has_verbose_flag {
///         inventory::submit! {
///             Flag::new('v', "verbose")
///         }
///     }
/// }
/// ```
///
/// Refer to the [crate level documentation](index.html) for a complete example
/// of instantiating and iterating a plugin registry.
#[macro_export]
macro_rules! submit {
    ($($value:tt)*) => {
        $crate::__do_submit! {
            { $($value)* }
            { $($value)* }
        }
    };
}

// Not public API.
#[cfg(target_family = "wasm")]
#[doc(hidden)]
pub mod __private {
    #[doc(hidden)]
    pub use rustversion::attr;
}

// Not public API.
#[doc(hidden)]
#[macro_export]
macro_rules! __do_submit {
    (used={ $($used:tt)+ } $($value:tt)*) => {
        #[allow(non_upper_case_globals)]
        const _: () = {
            static __INVENTORY: $crate::Node = $crate::Node {
                value: &{ $($value)* },
                next: $crate::core::cell::UnsafeCell::new($crate::core::option::Option::None),
                #[cfg(target_family = "wasm")]
                initialized: $crate::core::sync::atomic::AtomicBool::new(false),
            };

            #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
            unsafe extern "C" fn __ctor() {
                unsafe { $crate::ErasedNode::submit(__INVENTORY.value, &__INVENTORY) }
            }

            // Linux/ELF: https://www.exploit-db.com/papers/13234
            //
            // macOS: https://blog.timac.org/2016/0716-constructor-and-destructor-attributes/
            //
            // Why .CRT$XCU on Windows? https://www.cnblogs.com/sunkang/archive/2011/05/24/2055635.html
            // 'I'=C init, 'C'=C++ init, 'P'=Pre-terminators and 'T'=Terminators
            $($used)+
            #[cfg_attr(
                all(
                    not(target_family = "wasm"),
                    any(
                        target_os = "linux",
                        target_os = "android",
                        target_os = "dragonfly",
                        target_os = "freebsd",
                        target_os = "haiku",
                        target_os = "illumos",
                        target_os = "netbsd",
                        target_os = "openbsd",
                        target_os = "none",
                    )
                ),
                link_section = ".init_array",
            )]
            #[cfg_attr(
                target_family = "wasm",
                $crate::__private::attr(
                    any(all(stable, since(1.85)), since(2024-12-18)),
                    link_section = ".init_array",
                ),
            )]
            #[cfg_attr(
                any(target_os = "macos", target_os = "ios"),
                link_section = "__DATA,__mod_init_func",
            )]
            #[cfg_attr(windows, link_section = ".CRT$XCU")]
            static __CTOR: unsafe extern "C" fn() = __ctor;
        };
    };

    ({ #![used($($used:tt)+)] $($value:tt)* } { $pound:tt $bang:tt $brackets:tt $($dup:tt)* }) => {
        $crate::__do_submit! {
            used={ $pound $brackets }
            $($value)*
        }
    };

    ({ $($value:tt)* } { $($dup:tt)* }) => {
        $crate::__do_submit! {
            used={ #[used] }
            $($value)*
        }
    };
}
