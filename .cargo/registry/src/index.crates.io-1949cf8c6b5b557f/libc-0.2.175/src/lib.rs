//! libc - Raw FFI bindings to platforms' system libraries
#![crate_name = "libc"]
#![crate_type = "rlib"]
#![allow(
    renamed_and_removed_lints, // Keep this order.
    unknown_lints, // Keep this order.
    nonstandard_style,
    overflowing_literals,
    unused_macros,
    unused_macro_rules,
)]
// Prepare for a future upgrade
#![warn(rust_2024_compatibility)]
// Things missing for 2024 that are blocked on MSRV or breakage
#![allow(
    missing_unsafe_on_extern,
    edition_2024_expr_fragment_specifier,
    // Allowed globally, the warning is enabled in individual modules as we work through them
    unsafe_op_in_unsafe_fn
)]
#![cfg_attr(libc_deny_warnings, deny(warnings))]
// Attributes needed when building as part of the standard library
#![cfg_attr(feature = "rustc-dep-of-std", feature(link_cfg, no_core))]
#![cfg_attr(libc_thread_local, feature(thread_local))]
#![cfg_attr(feature = "rustc-dep-of-std", allow(internal_features))]
// DIFF(1.0): The thread local references that raise this lint were removed in 1.0
#![cfg_attr(feature = "rustc-dep-of-std", allow(static_mut_refs))]
// Enable extra lints:
#![cfg_attr(feature = "extra_traits", warn(missing_debug_implementations))]
#![warn(missing_copy_implementations, safe_packed_borrows)]
#![cfg_attr(not(feature = "rustc-dep-of-std"), no_std)]
#![cfg_attr(feature = "rustc-dep-of-std", no_core)]

#[macro_use]
mod macros;
mod new;

cfg_if! {
    if #[cfg(feature = "rustc-dep-of-std")] {
        extern crate rustc_std_workspace_core as core;
    }
}

pub use core::ffi::c_void;

#[allow(unused_imports)] // needed while the module is empty on some platforms
pub use new::*;

cfg_if! {
    if #[cfg(windows)] {
        mod primitives;
        pub use crate::primitives::*;

        mod windows;
        pub use crate::windows::*;

        prelude!();
    } else if #[cfg(target_os = "fuchsia")] {
        mod primitives;
        pub use crate::primitives::*;

        mod fuchsia;
        pub use crate::fuchsia::*;

        prelude!();
    } else if #[cfg(target_os = "switch")] {
        mod primitives;
        pub use primitives::*;

        mod switch;
        pub use switch::*;

        prelude!();
    } else if #[cfg(target_os = "psp")] {
        mod primitives;
        pub use primitives::*;

        mod psp;
        pub use crate::psp::*;

        prelude!();
    } else if #[cfg(target_os = "vxworks")] {
        mod primitives;
        pub use crate::primitives::*;

        mod vxworks;
        pub use crate::vxworks::*;

        prelude!();
    } else if #[cfg(target_os = "solid_asp3")] {
        mod primitives;
        pub use crate::primitives::*;

        mod solid;
        pub use crate::solid::*;

        prelude!();
    } else if #[cfg(unix)] {
        mod primitives;
        pub use crate::primitives::*;

        mod unix;
        pub use crate::unix::*;

        prelude!();
    } else if #[cfg(target_os = "hermit")] {
        mod primitives;
        pub use crate::primitives::*;

        mod hermit;
        pub use crate::hermit::*;

        prelude!();
    } else if #[cfg(target_os = "teeos")] {
        mod primitives;
        pub use primitives::*;

        mod teeos;
        pub use teeos::*;

        prelude!();
    } else if #[cfg(target_os = "trusty")] {
        mod primitives;
        pub use crate::primitives::*;

        mod trusty;
        pub use crate::trusty::*;

        prelude!();
    } else if #[cfg(all(target_env = "sgx", target_vendor = "fortanix"))] {
        mod primitives;
        pub use crate::primitives::*;

        mod sgx;
        pub use crate::sgx::*;

        prelude!();
    } else if #[cfg(any(target_env = "wasi", target_os = "wasi"))] {
        mod primitives;
        pub use crate::primitives::*;

        mod wasi;
        pub use crate::wasi::*;

        prelude!();
    } else if #[cfg(target_os = "xous")] {
        mod primitives;
        pub use crate::primitives::*;

        mod xous;
        pub use crate::xous::*;

        prelude!();
    } else {
        // non-supported targets: empty...
    }
}
