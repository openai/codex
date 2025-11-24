#[doc(hidden)]
#[allow(unused)]
pub mod __support {
    /// Return type for the constructor. Why is this needed?
    ///
    /// On Windows, `.CRT$XIA` … `.CRT$XIZ` constructors are required to return a `usize` value. We don't know
    /// if the user is putting this function into a retval-requiring section or a non-retval section, so we
    /// just return a `usize` value which is always valid and just ignored if not needed.
    ///
    /// Miri is pedantic about this, so we just return `()` if we're running under miri.
    ///
    /// See <https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/initterm-initterm-e?view=msvc-170>
    #[cfg(all(windows, not(miri)))]
    pub type CtorRetType = usize;
    #[cfg(any(not(windows), miri))]
    pub type CtorRetType = ();

    pub use crate::__ctor_call as ctor_call;
    pub use crate::__ctor_entry as ctor_entry;
    pub use crate::__ctor_link_section as ctor_link_section;
    pub use crate::__ctor_link_section_attr as ctor_link_section_attr;
    pub use crate::__ctor_parse as ctor_parse;
    pub use crate::__dtor_entry as dtor_entry;
    pub use crate::__dtor_parse as dtor_parse;
    pub use crate::__if_has_feature as if_has_feature;
    pub use crate::__if_unsafe as if_unsafe;
    pub use crate::__include_no_warn_on_missing_unsafe_feature as include_no_warn_on_missing_unsafe_feature;
    pub use crate::__include_used_linker_feature as include_used_linker_feature;
    pub use crate::__unify_features as unify_features;
}

/// Parse a `#[ctor]`-annotated item as if it were a proc-macro.
///
/// This macro supports both the `fn` and `static` forms of the `#[ctor]`
/// attribute, including attribute parameters.
///
/// ```rust
/// # #[cfg(any())] // disabled due to code sharing between ctor/dtor
/// # mod test {
/// # use ctor::declarative::ctor;
/// ctor! {
///   /// Create a ctor with a link section
/// # #[cfg(any())]
///   #[ctor(link_section = ".ctors")]
///   unsafe fn foo() { /* ... */ }
/// }
///
/// ctor! {
///   #[ctor]
/// # #[cfg(any())]
///   static FOO: std::collections::HashMap<u32, String> = unsafe {
///     let mut m = std::collections::HashMap::new();
///     m.insert(1, "foo".to_string());
///     m
///   };
/// }
/// # }
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! __ctor_parse {
    (#[ctor $(($($meta:tt)*))?] $(#[$imeta:meta])* pub ( $($extra:tt)* ) $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::ctor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[pub($($extra)*)], item=$($item)*);
    };
    (#[ctor $(($($meta:tt)*))?] $(#[$imeta:meta])* pub $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::ctor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[pub], item=$($item)*);
    };
    (#[ctor $(($($meta:tt)*))?] $(#[$imeta:meta])* fn $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::ctor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[], item=fn $($item)*);
    };
    (#[ctor $(($($meta:tt)*))?] $(#[$imeta:meta])* unsafe $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::ctor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[], item=unsafe $($item)*);
    };
    (#[ctor $(($($meta:tt)*))?] $(#[$imeta:meta])* static $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::ctor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[], item=static $($item)*);
    };
    // Reorder attributes that aren't `#[ctor]`
    (#[$imeta:meta] $($rest:tt)*) => {
        $crate::__support::ctor_parse!(__reorder__(#[$imeta],), $($rest)*);
    };
    (__reorder__($(#[$imeta:meta],)*), #[ctor $(($($meta:tt)*))?] $($rest:tt)*) => {
        $crate::__support::ctor_parse!(#[ctor $(($($meta)*))?] $(#[$imeta])* $($rest)*);
    };
    (__reorder__($(#[$imeta:meta],)*), #[$imeta2:meta] $($rest:tt)*) => {
        $crate::__support::ctor_parse!(__reorder__($(#[$imeta],)*#[$imeta2],), $($rest)*);
    };
}

/// Parse a `#[dtor]`-annotated item as if it were a proc-macro.
///
/// ```rust
/// # #[cfg(any())] mod test {
/// dtor! {
///   #[dtor]
///   unsafe fn foo() { /* ... */ }
/// }
/// # }
#[doc(hidden)]
#[macro_export]
macro_rules! __dtor_parse {
    (#[dtor $(($($meta:tt)*))?] $(#[$imeta:meta])* pub ( $($extra:tt)* ) $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::dtor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[pub($($extra)*)], item=$($item)*);
    };
    (#[dtor $(($($meta:tt)*))?] $(#[$imeta:meta])* pub $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::dtor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[pub], item=$($item)*);
    };
    (#[dtor $(($($meta:tt)*))?] $(#[$imeta:meta])* fn $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::dtor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[], item=fn $($item)*);
    };
    (#[dtor $(($($meta:tt)*))?] $(#[$imeta:meta])* unsafe $($item:tt)*) => {
        $crate::__support::unify_features!(next=$crate::__support::dtor_entry, meta=[$($($meta)*)?], features=[], imeta=$(#[$imeta])*, vis=[], item=unsafe $($item)*);
    };
    // Reorder attributes that aren't `#[dtor]`
    (#[$imeta:meta] $($rest:tt)*) => {
        $crate::__support::dtor_parse!(__reorder__(#[$imeta],), $($rest)*);
    };
    (__reorder__($(#[$imeta:meta],)*), #[dtor $(($($meta:tt)*))?] $($rest:tt)*) => {
        $crate::__support::dtor_parse!(#[dtor $(($($meta)*))?] $(#[$imeta])* $($rest)*);
    };
    (__reorder__($(#[$imeta:meta],)*), #[$imeta2:meta] $($rest:tt)*) => {
        $crate::__support::dtor_parse!(__reorder__($(#[$imeta],)*#[$imeta2],), $($rest)*);
    };
}

/// Extract #[ctor/dtor] attribute parameters and crate features and turn them
/// into a unified feature array.
///
/// Supported attributes:
///
/// - `used(linker)` -> feature: `used_linker`
/// - `link_section = ...` -> feature: `(link_section = ...)`
/// - `crate_path = ...` -> feature: `(crate_path = ...)`
#[doc(hidden)]
#[macro_export]
macro_rules! __unify_features {
    // Entry
    (next=$next_macro:path, meta=[$($meta:tt)*], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::unify_features!(used_linker, next=$next_macro, meta=[$($meta)*], features=[$($features)*], $($rest)*);
    };

    // Add used_linker feature if cfg(feature="used_linker")
    (used_linker, next=$next_macro:path, meta=[$($meta:tt)*], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::include_used_linker_feature!(
            $crate::__support::unify_features!(__no_warn_on_missing_unsafe, next=$next_macro, meta=[$($meta)*], features=[used_linker,$($features)*], $($rest)*);
            $crate::__support::unify_features!(__no_warn_on_missing_unsafe, next=$next_macro, meta=[$($meta)*], features=[$($features)*], $($rest)*);
        );
    };
    // Add __no_warn_on_missing_unsafe feature if cfg(feature="__no_warn_on_missing_unsafe")
    (__no_warn_on_missing_unsafe, next=$next_macro:path, meta=[$($meta:tt)*], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::include_used_linker_feature!(
            $crate::__support::unify_features!(continue, next=$next_macro, meta=[$($meta)*], features=[__no_warn_on_missing_unsafe,$($features)*], $($rest)*);
            $crate::__support::unify_features!(continue, next=$next_macro, meta=[$($meta)*], features=[$($features)*], $($rest)*);
        );
    };

    // Parse meta into features
    (continue, next=$next_macro:path, meta=[used(linker) $(, $($meta:tt)* )?], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::unify_features!(continue, next=$next_macro, meta=[$($($meta)*)?], features=[used_linker,$($features)*], $($rest)*);
    };
    (continue, next=$next_macro:path, meta=[link_section = $section:tt $(, $($meta:tt)* )?], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::unify_features!(continue, next=$next_macro, meta=[$($($meta)*)?], features=[(link_section=$section),$($features)*], $($rest)*);
    };
    (continue, next=$next_macro:path, meta=[crate_path = $path:path $(, $($meta:tt)* )?], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::unify_features!(continue, next=$next_macro, meta=[$($($meta)*)?], features=[(crate_path=$path),$($features)*], $($rest)*);
    };
    (continue, next=$next_macro:path, meta=[anonymous $(, $($meta:tt)* )?], features=[$($features:tt)*], $($rest:tt)*) => {
        $crate::__support::unify_features!(continue, next=$next_macro, meta=[$($($meta)*)?], features=[anonymous,$($features)*], $($rest)*);
    };
    (continue, next=$next_macro:path, meta=[$unknown_meta:meta $($meta:tt)*], features=[$($features:tt)*], $($rest:tt)*) => {
        compile_error!(concat!("Unknown attribute parameter: ", stringify!($unknown_meta)));
    };

    (continue, next=$next_macro:path, meta=[], features=[$($features:tt)*], $($rest:tt)*) => {
        $next_macro!(features=[$($features)*], $($rest)*);
    };
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "used_linker")]
macro_rules! __include_used_linker_feature {
    ($true:item $false:item) => {
        $true
    };
}

#[doc(hidden)]
#[macro_export]
#[cfg(not(feature = "used_linker"))]
macro_rules! __include_used_linker_feature {
    ($true:item $false:item) => {
        $false
    };
}

#[doc(hidden)]
#[macro_export]
#[cfg(feature = "__no_warn_on_missing_unsafe")]
macro_rules! __include_no_warn_on_missing_unsafe_feature {
    ($true:item $false:item) => {
        $true
    };
}

#[doc(hidden)]
#[macro_export]
#[cfg(not(feature = "__no_warn_on_missing_unsafe"))]
macro_rules! __include_no_warn_on_missing_unsafe_feature {
    ($true:item $false:item) => {
        $false
    };
}

/// If the features array contains the requested feature, generates `if_true`, else `if_false`.
///
/// This macro matches the features recursively.
///
/// Example: `[(link_section = ".ctors") , used_linker , __warn_on_missing_unsafe ,]`
#[doc(hidden)]
#[macro_export]
#[allow(unknown_lints, edition_2024_expr_fragment_specifier)]
macro_rules! __if_has_feature {
    (used_linker,                 [used_linker,                     $($rest:tt)*], {$($if_true:tt)*}, {$($if_false:tt)*}) => { $($if_true)* };
    (__no_warn_on_missing_unsafe, [__no_warn_on_missing_unsafe,     $($rest:tt)*], {$($if_true:tt)*}, {$($if_false:tt)*}) => { $($if_true)* };
    (anonymous,                   [anonymous,                       $($rest:tt)*], {$($if_true:tt)*}, {$($if_false:tt)*}) => { $($if_true)* };
    ((link_section(c)),           [(link_section=$section:literal), $($rest:tt)*], {$($if_true:tt)*}, {$($if_false:tt)*}) => { #[link_section = $section] $($if_true)* };

    // Fallback rules
    ($anything:tt, [$x:ident, $($rest:tt)*], {$($if_true:tt)*}, {$($if_false:tt)*}) => { $crate::__support::if_has_feature!($anything, [$($rest)*], {$($if_true)*}, {$($if_false)*}); };
    ($anything:tt, [($x:ident=$y:expr), $($rest:tt)*], {$($if_true:tt)*}, {$($if_false:tt)*}) => { $crate::__support::if_has_feature!($anything, [$($rest)*], {$($if_true)*}, {$($if_false)*}); };
    ($anything:tt, [], {$($if_true:tt)*}, {$($if_false:tt)*}) => { $($if_false)* };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __if_unsafe {
    (, {$($if_unsafe:tt)*}, {$($if_safe:tt)*}) => { $($if_safe)* };
    (unsafe, {$($if_unsafe:tt)*}, {$($if_safe:tt)*}) => { $($if_unsafe)* };
}

#[doc(hidden)]
#[macro_export]
#[allow(unknown_lints, edition_2024_expr_fragment_specifier)]
macro_rules! __ctor_entry {
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=unsafe fn $($item:tt)*) => {
        $crate::__support::ctor_entry!(features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=unsafe, item=fn $($item)*);
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=fn $($item:tt)*) => {
        $crate::__support::ctor_entry!(features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=, item=fn $($item)*);
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=static $ident:ident : $ty:ty = $(unsafe)? $({ $lit:literal })? $($lit2:literal)? ;) => {
        compile_error!(concat!("Use `const ", stringify!($ident), " = ", stringify!($($lit)?$($lit2)?), ";` or `static ", stringify!($ident), ": ", stringify!($ty), " = ", stringify!($($lit)?$($lit2)?), ";` instead"));
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=static $ident:ident : $ty:ty = unsafe $($item:tt)*) => {
        $crate::__support::ctor_entry!(features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=unsafe, item=static $ident: $ty = $($item)*);
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=static $ident:ident : $ty:ty = $($item:tt)*) => {
        $crate::__support::ctor_entry!(features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=, item=static $ident: $ty = $($item)*);
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=fn $ident:ident() $block:block) => {
        $crate::__support::if_has_feature!(anonymous, $features, {
            $crate::__support::ctor_entry!(unnamed, features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=$($unsafe)?, item=fn $ident() $block);
        }, {
            $crate::__support::ctor_entry!(named, features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=$($unsafe)?, item=fn $ident() $block);
        });
    };
    (unnamed,features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=fn $ident:ident() $block:block) => {
        const _: () = {
            $crate::__support::ctor_entry!(named, features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=$($unsafe)?, item=fn $ident() $block);
        };
    };
    (named, features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=fn $ident:ident() $block:block) => {
        $(#[$fnmeta])*
        #[allow(unused)]
        $($vis)* $($unsafe)? fn $ident() {
            #[allow(unsafe_code)]
            {
                $crate::__support::if_unsafe!($($unsafe)?, {}, {
                    $crate::__support::if_has_feature!( __warn_on_missing_unsafe, $features, {
                        #[deprecated="ctor deprecation note:\n\n \
                        Use of #[ctor] without `unsafe fn` is deprecated. As code execution before main\n\
                        is unsupported by most Rust runtime functions, these functions must be marked\n\
                        `unsafe`."]
                            const fn ctor_without_unsafe_is_deprecated() {}
                            #[allow(unused)]
                            static UNSAFE_WARNING: () = ctor_without_unsafe_is_deprecated();
                    }, {});
                });

                $crate::__support::ctor_call!(
                    features=$features,
                    { unsafe { $ident(); } }
                );
            }

            #[cfg(target_family = "wasm")]
            {
                static __CTOR__INITILIZED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
                if __CTOR__INITILIZED.swap(true, core::sync::atomic::Ordering::Relaxed) {
                    return;
                }
            }

            $block
        }
    };
    (features=$features:tt, imeta=$(#[$imeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=static $ident:ident : $ty:ty = $block:block;) => {
        $(#[$imeta])*
        $($vis)* static $ident: $ident::Static<$ty> = $ident::Static::<$ty> {
            _storage: {
                $crate::__support::ctor_call!(
                    features=$features,
                    { _ = &*$ident; }
                );

                ::std::sync::OnceLock::new()
            }
        };

        impl ::core::ops::Deref for $ident::Static<$ty> {
            type Target = $ty;
            fn deref(&self) -> &$ty {
                fn init() -> $ty $block

                self._storage.get_or_init(move || {
                    init()
                })
            }
        }

        #[doc(hidden)]
        #[allow(non_upper_case_globals, non_snake_case)]
        #[allow(unsafe_code)]
        mod $ident {
            $crate::__support::if_unsafe!($($unsafe)?, {}, {
                $crate::__support::if_has_feature!( __warn_on_missing_unsafe, $features, {
                    #[deprecated="ctor deprecation note:\n\n \
                    Use of #[ctor] without `unsafe { ... }` is deprecated. As code execution before main\n\
                    is unsupported by most Rust runtime functions, these functions must be marked\n\
                    `unsafe`."]
                        const fn ctor_without_unsafe_is_deprecated() {}
                        #[allow(unused)]
                        static UNSAFE_WARNING: () = ctor_without_unsafe_is_deprecated();
                }, {});
            });

            #[allow(non_camel_case_types, unreachable_pub)]
            pub struct Static<T> {
                pub _storage: ::std::sync::OnceLock<T>
            }
        }
    };
}

// Code note:

// You might wonder why we don't use `__attribute__((destructor))`/etc for
// dtor. Unfortunately mingw doesn't appear to properly support section-based
// hooks for shutdown, ie:

// https://github.com/Alexpux/mingw-w64/blob/d0d7f784833bbb0b2d279310ddc6afb52fe47a46/mingw-w64-crt/crt/crtdll.c

// In addition, OSX has removed support for section-based shutdown hooks after
// warning about it for a number of years:

// https://reviews.llvm.org/D45578

#[doc(hidden)]
#[macro_export]
macro_rules! __dtor_entry {
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=fn $ident:ident() $block:block) => {
        $crate::__support::dtor_entry!(features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=, item=fn $ident() $block);
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], item=unsafe fn $ident:ident() $block:block) => {
        $crate::__support::dtor_entry!(features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=unsafe, item=fn $ident() $block);
    };
    (features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=fn $ident:ident() $block:block) => {
        $crate::__support::if_has_feature!(anonymous, $features, {
            $crate::__support::dtor_entry!(unnamed, features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=$($unsafe)?, item=fn $ident() $block);
        }, {
            $crate::__support::dtor_entry!(named, features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=$($unsafe)?, item=fn $ident() $block);
        });
    };
    (unnamed, features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=fn $ident:ident() $block:block) => {
        const _: () = {
            $crate::__support::dtor_entry!(named, features=$features, imeta=$(#[$fnmeta])*, vis=[$($vis)*], unsafe=$($unsafe)?, item=fn $ident() $block);
        };
    };
    (named, features=$features:tt, imeta=$(#[$fnmeta:meta])*, vis=[$($vis:tt)*], unsafe=$($unsafe:ident)?, item=fn $ident:ident() $block:block) => {
        $(#[$fnmeta])*
        #[allow(unused)]
        $($vis)* $($unsafe)? fn $ident() {
            #[allow(unsafe_code)]
            {
                $crate::__support::if_unsafe!($($unsafe)?, {}, {
                    $crate::__support::if_has_feature!( __warn_on_missing_unsafe, $features, {
                        #[deprecated="dtor deprecation note:\n\n \
                        Use of #[dtor] without `unsafe fn` is deprecated. As code execution after main\n\
                        is unsupported by most Rust runtime functions, these functions must be marked\n\
                        `unsafe`."]
                        const fn dtor_without_unsafe_is_deprecated() {}
                        #[allow(unused)]
                        static UNSAFE_WARNING: () = dtor_without_unsafe_is_deprecated();
                    }, {});
                });

                $crate::__support::ctor_call!(
                    features=$features,
                    { unsafe { do_atexit(__dtor); } }
                );

                $crate::__support::ctor_link_section!(
                    exit,
                    features=$features,

                    /*unsafe*/ extern "C" fn __dtor(
                        #[cfg(target_vendor = "apple")] _: *const u8
                    ) { unsafe { $ident() } }
                );

                #[cfg(not(target_vendor = "apple"))]
                #[inline(always)]
                unsafe fn do_atexit(cb: unsafe extern fn()) {
                    /*unsafe*/ extern "C" {
                        fn atexit(cb: unsafe extern fn());
                    }
                    unsafe {
                        atexit(cb);
                    }
                }

                // For platforms that have __cxa_atexit, we register the dtor as scoped to dso_handle
                #[cfg(target_vendor = "apple")]
                #[inline(always)]
                unsafe fn do_atexit(cb: /*unsafe*/ extern "C" fn(_: *const u8)) {
                    /*unsafe*/ extern "C" {
                        static __dso_handle: *const u8;
                        fn __cxa_atexit(cb: /*unsafe*/ extern "C" fn(_: *const u8), arg: *const u8, dso_handle: *const u8);
                    }
                    unsafe {
                        __cxa_atexit(cb, core::ptr::null(), __dso_handle);
                    }
                }
            }

            $block
        }
    };
}

/// Annotate a block with its appropriate link section.
#[doc(hidden)]
#[macro_export]
macro_rules! __ctor_call {
    (features=$features:tt, { $($block:tt)+ } ) => {
        $crate::__support::ctor_link_section!(
            array,
            features=$features,

            #[allow(non_upper_case_globals, non_snake_case)]
            #[doc(hidden)]
            static f: /*unsafe*/ extern "C" fn() -> $crate::__support::CtorRetType =
            {
                $crate::__support::ctor_link_section!(
                    startup,
                    features=$features,

                    #[allow(non_snake_case)]
                    /*unsafe*/ extern "C" fn f() -> $crate::__support::CtorRetType {
                        $($block)+;
                        core::default::Default::default()
                    }
                );

                f
            };
        );
    }
}

/// Annotate a block with its appropriate link section.
#[doc(hidden)]
#[macro_export]
macro_rules! __ctor_link_section {
    ($section:ident, features=$features:tt, $($block:tt)+) => {
        $crate::__support::if_has_feature!(used_linker, $features, {
            $crate::__support::ctor_link_section_attr!($section, $features, used(linker), $($block)+);
        }, {
            $crate::__support::ctor_link_section_attr!($section, $features, used, $($block)+);
        });

        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "dragonfly",
            target_os = "illumos",
            target_os = "haiku",
            target_vendor = "apple",
            target_family = "wasm",
            windows
        )))]
        compile_error!("#[ctor]/#[dtor] is not supported on the current target");
    }
}

/// Apply either the default cfg-based link section attributes, or
/// the overridden link_section attribute.
#[doc(hidden)]
#[macro_export]
macro_rules! __ctor_link_section_attr {
    (array, $features:tt, $used:meta, $item:item) => {
        $crate::__support::if_has_feature!((link_section(c)), $features, {
            #[allow(unsafe_code)]
            #[$used]
            $item
        }, {
            #[allow(unsafe_code)]
            $crate::__support::ctor_link_section_attr!(
                [[any(
                    target_os = "linux",
                    target_os = "android",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "dragonfly",
                    target_os = "illumos",
                    target_os = "haiku",
                    target_family = "wasm"
                ), ".init_array"],
                [target_vendor = "apple", "__DATA,__mod_init_func,mod_init_funcs"],
                [windows, ".CRT$XCU"]],
                #[$used]
                $item
            );
        });
    };
    (startup, $features:tt, $used:meta, $item:item) => {
        #[cfg(not(clippy))]
        $crate::__support::ctor_link_section_attr!([[any(target_os = "linux", target_os = "android"), ".text.startup"]], $item);

        #[cfg(clippy)]
        $item
    };
    (exit, $features:tt, $used:meta, $item:item) => {
        #[cfg(not(clippy))]
        $crate::__support::ctor_link_section_attr!([[any(target_os = "linux", target_os = "android"), ".text.exit"]], $item);

        #[cfg(clippy)]
        $item
    };
    ([$( [$cond:meta, $literal:literal ] ),+], $item:item) => {
        $( #[cfg_attr($cond, link_section = $literal)] )+
        $item
    };
}
