//! An implementation which calls out to an externally defined function.
use crate::{util::uninit_slice_fill_zero, Error};
use core::{mem::MaybeUninit, num::NonZeroU32};

/// Register a function to be invoked by `getrandom` on unsupported targets.
///
/// ## Writing a custom `getrandom` implementation
///
/// The function to register must have the same signature as
/// [`getrandom::getrandom`](crate::getrandom). The function can be defined
/// wherever you want, either in root crate or a dependent crate.
///
/// For example, if we wanted a `failure-getrandom` crate containing an
/// implementation that always fails, we would first depend on `getrandom`
/// (for the [`Error`] type) in `failure-getrandom/Cargo.toml`:
/// ```toml
/// [dependencies]
/// getrandom = "0.2"
/// ```
/// Note that the crate containing this function does **not** need to enable the
/// `"custom"` Cargo feature.
///
/// Next, in `failure-getrandom/src/lib.rs`, we define our function:
/// ```rust
/// use core::num::NonZeroU32;
/// use getrandom::Error;
///
/// // Some application-specific error code
/// const MY_CUSTOM_ERROR_CODE: u32 = Error::CUSTOM_START + 42;
/// pub fn always_fail(buf: &mut [u8]) -> Result<(), Error> {
///     let code = NonZeroU32::new(MY_CUSTOM_ERROR_CODE).unwrap();
///     Err(Error::from(code))
/// }
/// ```
///
/// ## Registering a custom `getrandom` implementation
///
/// Functions can only be registered in the root binary crate. Attempting to
/// register a function in a non-root crate will result in a linker error.
/// This is similar to
/// [`#[panic_handler]`](https://doc.rust-lang.org/nomicon/panic-handler.html) or
/// [`#[global_allocator]`](https://doc.rust-lang.org/edition-guide/rust-2018/platform-and-target-support/global-allocators.html),
/// where helper crates define handlers/allocators but only the binary crate
/// actually _uses_ the functionality.
///
/// To register the function, we first depend on `failure-getrandom` _and_
/// `getrandom` in `Cargo.toml`:
/// ```toml
/// [dependencies]
/// failure-getrandom = "0.1"
/// getrandom = { version = "0.2", features = ["custom"] }
/// ```
///
/// Then, we register the function in `src/main.rs`:
/// ```rust
/// # mod failure_getrandom { pub fn always_fail(_: &mut [u8]) -> Result<(), getrandom::Error> { unimplemented!() } }
/// use failure_getrandom::always_fail;
/// use getrandom::register_custom_getrandom;
///
/// register_custom_getrandom!(always_fail);
/// ```
///
/// Now any user of `getrandom` (direct or indirect) on this target will use the
/// registered function. As noted in the
/// [top-level documentation](index.html#custom-implementations) this
/// registration only has an effect on unsupported targets.
#[macro_export]
macro_rules! register_custom_getrandom {
    ($path:path) => {
        // TODO(MSRV 1.37): change to unnamed block
        const __GETRANDOM_INTERNAL: () = {
            // We use Rust ABI to be safe against potential panics in the passed function.
            #[no_mangle]
            unsafe fn __getrandom_custom(dest: *mut u8, len: usize) -> u32 {
                // Make sure the passed function has the type of getrandom::getrandom
                type F = fn(&mut [u8]) -> ::core::result::Result<(), $crate::Error>;
                let _: F = $crate::getrandom;
                let f: F = $path;
                let slice = ::core::slice::from_raw_parts_mut(dest, len);
                match f(slice) {
                    Ok(()) => 0,
                    Err(e) => e.code().get(),
                }
            }
        };
    };
}

#[allow(dead_code)]
pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    extern "Rust" {
        fn __getrandom_custom(dest: *mut u8, len: usize) -> u32;
    }
    // Previously we always passed a valid, initialized slice to
    // `__getrandom_custom`. Ensure `dest` has been initialized for backward
    // compatibility with implementations that rely on that (e.g. Rust
    // implementations that construct a `&mut [u8]` slice from `dest` and
    // `len`).
    let dest = uninit_slice_fill_zero(dest);
    let ret = unsafe { __getrandom_custom(dest.as_mut_ptr(), dest.len()) };
    match NonZeroU32::new(ret) {
        None => Ok(()),
        Some(code) => Err(Error::from(code)),
    }
}
