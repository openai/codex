#[cfg(unix)]
pub(crate) const FDS_MAX: usize = 1024; // this is hardcoded in sdbus - nothing in the spec

pub(crate) fn padding_for_8_bytes(value: usize) -> usize {
    padding_for_n_bytes(value, 8)
}

pub(crate) fn padding_for_n_bytes(value: usize, align: usize) -> usize {
    let len_rounded_up = value.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);

    len_rounded_up.wrapping_sub(value)
}

/// Helper trait for macro-generated code.
///
/// This trait allows macros to refer to the `Ok` and `Err` types of a [Result] that is behind a
/// type alias.  This is currently required because the macros for properties expect a Result
/// return value, but the macro-generated `receive_` functions need to refer to the actual
/// type without the associated error.
#[doc(hidden)]
pub trait ResultAdapter {
    type Ok;
    type Err;
}

impl<T, E> ResultAdapter for Result<T, E> {
    type Ok = T;
    type Err = E;
}

#[cfg(not(feature = "tokio"))]
#[doc(hidden)]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    async_io::block_on(future)
}

#[cfg(feature = "tokio")]
#[doc(hidden)]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::sync::OnceLock;
    static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = TOKIO_RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .expect("launch of single-threaded tokio runtime")
    });
    runtime.block_on(future)
}

// If we're running inside a Flatpak sandbox.
pub(crate) fn is_flatpak() -> bool {
    std::env::var("FLATPAK_ID").is_ok()
}
