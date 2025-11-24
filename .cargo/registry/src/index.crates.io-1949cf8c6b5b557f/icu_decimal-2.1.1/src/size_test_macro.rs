// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/*************************************************************************************
 * NOTE: PLEASE KEEP THIS FILE IN SYNC WITH ALL OTHER FILES NAMED size_test_macro.rs *
 * TODO(#4467): Copy this file automatically                                         *
 *************************************************************************************/

/// Generates a test that checks the stack size of an item and a macro
/// that should be used with `#[doc]` to document it.
///
/// ```text
/// size_test!(MyType, my_type_size, 32);
///
/// // Add this annotation to the type's docs:
/// #[doc = my_type_size!()]
/// ```
///
/// The size should correspond to the Rust version in rust-toolchain.toml.
///
/// If the size on latest beta differs from rust-toolchain.toml, use the
/// named arguments version of this macro to specify both sizes:
///
/// ```text
/// size_test!(MyType, my_type_size, pinned = 32, beta = 24, nightly = 24);
/// ```
///
/// The test is ignored by default but runs in CI. To run the test locally,
/// run `cargo test -- --include-ignored`
macro_rules! size_test {
    ($ty:ty, $id:ident, pinned = $pinned:literal, beta = $beta:literal, nightly = $nightly:literal) => {
        macro_rules! $id {
            () => {
                concat!(
                    "\n\n",
                    "📏 This item has a stack size of <b>",
                    stringify!($pinned),
                    " bytes</b> on the stable toolchain and <b>",
                    stringify!($beta),
                    " bytes</b> on beta toolchain at release date."
                )
            };
        }
        #[test]
        #[cfg_attr(not(icu4x_run_size_tests), ignore)] // Doesn't work on arbitrary Rust versions
        fn $id() {
            let size = core::mem::size_of::<$ty>();
            let success = match option_env!("CI_TOOLCHAIN") {
                Some("nightly") => size == $nightly,
                Some("beta") => size == $beta,
                Some("pinned-stable") => size == $pinned,
                // Manual invocation: match either size
                _ => matches!(size, $pinned | $beta | $nightly),
            };
            assert!(
                success,
                "size_of {} = {}.\n** To reproduce this failure, run `cargo test -- --ignored` **",
                stringify!($ty),
                size,
            );
        }
    };
    ($ty:ty, $id:ident, $size:literal) => {
        macro_rules! $id {
            () => {
                concat!(
                    "\n\n",
                    "📏 This item has a stack size of <b>",
                    stringify!($size),
                    " bytes</b> on the stable toolchain at release date."
                )
            };
        }
        #[test]
        #[cfg_attr(not(icu4x_run_size_tests), ignore)] // Doesn't work on arbitrary Rust versions
        fn $id() {
            let size = core::mem::size_of::<$ty>();
            let expected = $size;
            assert_eq!(
                size,
                expected,
                "size_of {} = {}.\n** To reproduce this failure, run `cargo test -- --ignored` **",
                stringify!($ty),
                size,
            );
        }
    };
}

pub(crate) use size_test;
