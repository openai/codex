#![recursion_limit = "256"]

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

// Code note:

// You might wonder why we don't use `__attribute__((destructor))`/etc for
// dtor. Unfortunately mingw doesn't appear to properly support section-based
// hooks for shutdown, ie:

// https://github.com/Alexpux/mingw-w64/blob/d0d7f784833bbb0b2d279310ddc6afb52fe47a46/mingw-w64-crt/crt/crtdll.c

// In addition, OSX has removed support for section-based shutdown hooks after
// warning about it for a number of years:

// https://reviews.llvm.org/D45578

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::{TokenStream};

/// Attributes required to mark a function as a constructor. This may be exposed in the future if we determine
/// it to be stable.
#[doc(hidden)]
macro_rules! ctor_attributes {
    () => {
        quote!(
            #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".init_array")]
            #[cfg_attr(target_os = "freebsd", link_section = ".init_array")]
            #[cfg_attr(target_os = "netbsd", link_section = ".init_array")]
            #[cfg_attr(target_os = "openbsd", link_section = ".init_array")]
            #[cfg_attr(target_os = "dragonfly", link_section = ".init_array")]
            #[cfg_attr(target_os = "illumos", link_section = ".init_array")]
            #[cfg_attr(target_os = "haiku", link_section = ".init_array")]
            #[cfg_attr(any(target_os = "macos", target_os = "ios"), link_section = "__DATA,__mod_init_func")]
            #[cfg_attr(windows, link_section = ".CRT$XCU")]
        )
    };
}

/// Marks a function or static variable as a library/executable constructor.
/// This uses OS-specific linker sections to call a specific function at
/// load time.
///
/// Multiple startup functions/statics are supported, but the invocation order is not
/// guaranteed.
///
/// # Examples
///
/// Print a startup message (using `libc_print` for safety):
///
/// ```rust
/// # extern crate ctor;
/// # use ctor::*;
/// use libc_print::std_name::println;
///
/// #[ctor]
/// fn foo() {
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
/// # extern crate ctor;
/// # use ctor::*;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// static INITED: AtomicBool = AtomicBool::new(false);
///
/// #[ctor]
/// fn foo() {
///   INITED.store(true, Ordering::SeqCst);
/// }
/// ```
///
/// Initialize a `HashMap` at startup time:
///
/// ```rust
/// # extern crate ctor;
/// # use std::collections::HashMap;
/// # use ctor::*;
/// #[ctor]
/// static STATIC_CTOR: HashMap<u32, String> = {
///   let mut m = HashMap::new();
///   for i in 0..100 {
///     m.insert(i, format!("x*100={}", i*100));
///   }
///   m
/// };
///
/// # pub fn main() {
/// #   assert_eq!(STATIC_CTOR.len(), 100);
/// #   assert_eq!(STATIC_CTOR[&20], "x*100=2000");
/// # }
/// ```
///
/// # Details
///
/// The `#[ctor]` macro makes use of linker sections to ensure that a
/// function is run at startup time.
///
/// The above example translates into the following Rust code (approximately):
///
///```rust
/// #[used]
/// #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".init_array")]
/// #[cfg_attr(target_os = "freebsd", link_section = ".init_array")]
/// #[cfg_attr(target_os = "netbsd", link_section = ".init_array")]
/// #[cfg_attr(target_os = "openbsd", link_section = ".init_array")]
/// #[cfg_attr(target_os = "illumos", link_section = ".init_array")]
/// #[cfg_attr(any(target_os = "macos", target_os = "ios"), link_section = "__DATA,__mod_init_func")]
/// #[cfg_attr(target_os = "windows", link_section = ".CRT$XCU")]
/// static FOO: extern fn() = {
///   #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
///   extern fn foo() { /* ... */ };
///   foo
/// };
/// ```
#[proc_macro_attribute]
pub fn ctor(_attribute: TokenStream, function: TokenStream) -> TokenStream {
    let item: syn::Item = syn::parse_macro_input!(function);
    if let syn::Item::Fn(function) = item {
        validate_item("ctor", &function);

        let syn::ItemFn {
            attrs,
            block,
            vis,
            sig:
                syn::Signature {
                    ident,
                    unsafety,
                    constness,
                    abi,
                    ..
                },
            ..
        } = function;

        // Linux/ELF: https://www.exploit-db.com/papers/13234

        // Mac details: https://blog.timac.org/2016/0716-constructor-and-destructor-attributes/

        // Why .CRT$XCU on Windows? https://www.cnblogs.com/sunkang/archive/2011/05/24/2055635.html
        // 'I'=C init, 'C'=C++ init, 'P'=Pre-terminators and 'T'=Terminators

        let ctor_ident =
            syn::parse_str::<syn::Ident>(format!("{}___rust_ctor___ctor", ident).as_ref())
                .expect("Unable to create identifier");

        let tokens = ctor_attributes!();
        let output = quote!(
            #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "dragonfly", target_os = "illumos", target_os = "haiku", target_os = "macos", target_os = "ios", windows)))]
            compile_error!("#[ctor] is not supported on the current target");

            #(#attrs)*
            #vis #unsafety extern #abi #constness fn #ident() #block

            #[used]
            #[allow(non_upper_case_globals)]
            #[doc(hidden)]
            #tokens
            static #ctor_ident
            :
            unsafe extern "C" fn() =
            {
                #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
                unsafe extern "C" fn #ctor_ident() { #ident() };
                #ctor_ident
            }
            ;
        );

        // eprintln!("{}", output);

        output.into()
    } else if let syn::Item::Static(var) = item {
        let syn::ItemStatic {
            ident,
            mutability,
            expr,
            attrs,
            ty,
            vis,
            ..
        } = var;

        if mutability.is_some() {
            panic!("#[ctor]-annotated static objects must not be mutable");
        }

        if attrs.iter().any(|attr| {
            attr.path
                .segments
                .iter()
                .any(|segment| segment.ident == "no_mangle")
        }) {
            panic!("#[ctor]-annotated static objects do not support #[no_mangle]");
        }

        let ctor_ident =
            syn::parse_str::<syn::Ident>(format!("{}___rust_ctor___ctor", ident).as_ref())
                .expect("Unable to create identifier");
        let storage_ident =
            syn::parse_str::<syn::Ident>(format!("{}___rust_ctor___storage", ident).as_ref())
                .expect("Unable to create identifier");

        let tokens = ctor_attributes!();
        let output = quote!(
            #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "dragonfly", target_os = "illumos", target_os = "haiku", target_os = "macos", target_os = "ios", windows)))]
            compile_error!("#[ctor] is not supported on the current target");

            // This is mutable, but only by this macro code!
            static mut #storage_ident: Option<#ty> = None;

            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #vis struct #ident<T> {
                _data: core::marker::PhantomData<T>
            }

            #(#attrs)*
            #vis static #ident: #ident<#ty> = #ident {
                _data: core::marker::PhantomData::<#ty>
            };

            impl core::ops::Deref for #ident<#ty> {
                type Target = #ty;
                fn deref(&self) -> &'static #ty {
                    unsafe {
                        #storage_ident.as_ref().unwrap()
                    }
                }
            }

            #[used]
            #[allow(non_upper_case_globals)]
            #tokens
            static #ctor_ident
            :
            unsafe extern "C" fn() = {
                #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
                unsafe extern "C" fn initer() {
                    #storage_ident = Some(#expr);
                }; initer }
            ;
        );

        // eprintln!("{}", output);

        output.into()
    } else {
        panic!("#[ctor] items must be functions or static globals");
    }
}

/// Marks a function as a library/executable destructor. This uses OS-specific
/// linker sections to call a specific function at termination time.
///
/// Multiple shutdown functions are supported, but the invocation order is not
/// guaranteed.
///
/// `sys_common::at_exit` is usually a better solution for shutdown handling, as
/// it allows you to use `stdout` in your handlers.
///
/// ```rust
/// # extern crate ctor;
/// # use ctor::*;
/// # fn main() {}
///
/// #[dtor]
/// fn shutdown() {
///   /* ... */
/// }
/// ```
#[proc_macro_attribute]
pub fn dtor(_attribute: TokenStream, function: TokenStream) -> TokenStream {
    let function: syn::ItemFn = syn::parse_macro_input!(function);
    validate_item("dtor", &function);

    let syn::ItemFn {
        attrs,
        block,
        vis,
        sig:
            syn::Signature {
                ident,
                unsafety,
                constness,
                abi,
                ..
            },
        ..
    } = function;

    let mod_ident = syn::parse_str::<syn::Ident>(format!("{}___rust_dtor___mod", ident).as_ref())
        .expect("Unable to create identifier");

    let dtor_ident = syn::parse_str::<syn::Ident>(format!("{}___rust_dtor___dtor", ident).as_ref())
        .expect("Unable to create identifier");

    let tokens = ctor_attributes!();
    let output = quote!(
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "dragonfly", target_os = "illumos", target_os = "haiku", target_os = "macos", target_os = "ios", windows)))]
        compile_error!("#[dtor] is not supported on the current target");

        #(#attrs)*
        #vis #unsafety extern #abi #constness fn #ident() #block

        mod #mod_ident {
            use super::#ident;

            // Note that we avoid a dep on the libc crate by linking directly to atexit functions

            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            #[inline(always)]
            unsafe fn do_atexit(cb: unsafe extern fn()) {
                extern "C" {
                    fn atexit(cb: unsafe extern fn());
                }
                atexit(cb);
            }

            // For platforms that have __cxa_atexit, we register the dtor as scoped to dso_handle
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            #[inline(always)]
            unsafe fn do_atexit(cb: unsafe extern fn()) {
                extern "C" {
                    static __dso_handle: *const u8;
                    fn __cxa_atexit(cb: unsafe extern fn(), arg: *const u8, dso_handle: *const u8);
                }
                __cxa_atexit(cb, std::ptr::null(), __dso_handle);
            }

            #[used]
            #[allow(non_upper_case_globals)]
            #tokens
            static __dtor_export
            :
            unsafe extern "C" fn() =
            {
                #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.exit")]
                unsafe extern "C" fn #dtor_ident() { #ident() };
                #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
                unsafe extern fn __dtor_atexit() {
                    do_atexit(#dtor_ident);
                };
                __dtor_atexit
            };
        }
    );

    // eprintln!("{}", output);

    output.into()
}

fn validate_item(typ: &str, item: &syn::ItemFn) {
    let syn::ItemFn { vis, sig, .. } = item;

    // Ensure that visibility modifier is not present
    match vis {
        syn::Visibility::Inherited => {}
        _ => panic!("#[{}] methods must not have visibility modifiers", typ),
    }

    // No parameters allowed
    if !sig.inputs.is_empty() {
        panic!("#[{}] methods may not have parameters", typ);
    }

    // No return type allowed
    match sig.output {
        syn::ReturnType::Default => {}
        _ => panic!("#[{}] methods must not have return types", typ),
    }
}
