//! Implementation for WASI Preview 1
use crate::Error;
use core::mem::MaybeUninit;

pub use crate::util::{inner_u32, inner_u64};

// This linking is vendored from the wasi crate:
// https://docs.rs/wasi/0.11.0+wasi-snapshot-preview1/src/wasi/lib_generated.rs.html#2344-2350
#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn random_get(arg0: i32, arg1: i32) -> i32;
}

/// WASI p1 uses `u16` for error codes in its witx definitions:
/// https://github.com/WebAssembly/WASI/blob/38454e9e/legacy/preview1/witx/typenames.witx#L34-L39
const MAX_ERROR_CODE: i32 = u16::MAX as i32;

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    // Based on the wasi code:
    // https://docs.rs/wasi/0.11.0+wasi-snapshot-preview1/src/wasi/lib_generated.rs.html#2046-2062
    // Note that size of an allocated object can not be bigger than isize::MAX bytes.
    // WASI 0.1 supports only 32-bit WASM, so casting length to `i32` is safe.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let ret = unsafe { random_get(dest.as_mut_ptr() as i32, dest.len() as i32) };
    match ret {
        0 => Ok(()),
        // WASI functions should return positive error codes which are smaller than `MAX_ERROR_CODE`
        code if code <= MAX_ERROR_CODE => Err(Error::from_neg_error_code(-code)),
        _ => Err(Error::UNEXPECTED),
    }
}
