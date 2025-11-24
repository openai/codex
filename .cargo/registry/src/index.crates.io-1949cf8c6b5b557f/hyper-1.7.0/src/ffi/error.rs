use crate::ffi::size_t;

/// A more detailed error object returned by some hyper functions.
///
/// Compare with `hyper_code`, which is a simpler error returned from
/// some hyper functions.
///
/// Methods:
///
/// - hyper_error_code:  Get an equivalent hyper_code from this error.
/// - hyper_error_print: Print the details of this error to a buffer.
/// - hyper_error_free:  Frees a hyper_error.
pub struct hyper_error(crate::Error);

/// A return code for many of hyper's methods.
#[repr(C)]
pub enum hyper_code {
    /// All is well.
    HYPERE_OK,
    /// General error, details in the `hyper_error *`.
    HYPERE_ERROR,
    /// A function argument was invalid.
    HYPERE_INVALID_ARG,
    /// The IO transport returned an EOF when one wasn't expected.
    ///
    /// This typically means an HTTP request or response was expected, but the
    /// connection closed cleanly without sending (all of) it.
    HYPERE_UNEXPECTED_EOF,
    /// Aborted by a user supplied callback.
    HYPERE_ABORTED_BY_CALLBACK,
    /// An optional hyper feature was not enabled.
    #[cfg_attr(feature = "http2", allow(unused))]
    HYPERE_FEATURE_NOT_ENABLED,
    /// The peer sent an HTTP message that could not be parsed.
    HYPERE_INVALID_PEER_MESSAGE,
}

// ===== impl hyper_error =====

impl hyper_error {
    fn code(&self) -> hyper_code {
        use crate::error::Kind as ErrorKind;
        use crate::error::User;

        match self.0.kind() {
            ErrorKind::Parse(_) => hyper_code::HYPERE_INVALID_PEER_MESSAGE,
            ErrorKind::IncompleteMessage => hyper_code::HYPERE_UNEXPECTED_EOF,
            ErrorKind::User(User::AbortedByCallback) => hyper_code::HYPERE_ABORTED_BY_CALLBACK,
            // TODO: add more variants
            _ => hyper_code::HYPERE_ERROR,
        }
    }

    fn print_to(&self, dst: &mut [u8]) -> usize {
        use std::io::Write;

        let mut dst = std::io::Cursor::new(dst);

        // A write! error doesn't matter. As much as possible will have been
        // written, and the Cursor position will know how far that is (even
        // if that is zero).
        let _ = write!(dst, "{}", &self.0);
        dst.position() as usize
    }
}

ffi_fn! {
    /// Frees a `hyper_error`.
    ///
    /// This should be used for any error once it is no longer needed.
    fn hyper_error_free(err: *mut hyper_error) {
        drop(non_null!(Box::from_raw(err) ?= ()));
    }
}

ffi_fn! {
    /// Get an equivalent `hyper_code` from this error.
    fn hyper_error_code(err: *const hyper_error) -> hyper_code {
        non_null!(&*err ?= hyper_code::HYPERE_INVALID_ARG).code()
    }
}

ffi_fn! {
    /// Print the details of this error to a buffer.
    ///
    /// The `dst_len` value must be the maximum length that the buffer can
    /// store.
    ///
    /// The return value is number of bytes that were written to `dst`.
    fn hyper_error_print(err: *const hyper_error, dst: *mut u8, dst_len: size_t) -> size_t {
        let dst = unsafe {
            std::slice::from_raw_parts_mut(dst, dst_len)
        };
        non_null!(&*err ?= 0).print_to(dst)
    }
}
