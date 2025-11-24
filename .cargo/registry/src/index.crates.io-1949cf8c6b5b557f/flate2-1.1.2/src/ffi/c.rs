//! Implementation for C backends.
use std::cmp;
use std::fmt;
use std::marker;
use std::os::raw::{c_int, c_uint};
use std::ptr;

use super::*;
use crate::mem;

#[derive(Clone, Default)]
pub struct ErrorMessage(Option<&'static str>);

impl ErrorMessage {
    pub fn get(&self) -> Option<&str> {
        self.0
    }
}

pub struct StreamWrapper {
    // SAFETY: The field `inner` must always be accessed as a raw pointer,
    // since it points to a cyclic structure, and it must never be copied
    // by Rust.
    pub inner: *mut mz_stream,
}

impl fmt::Debug for StreamWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "StreamWrapper")
    }
}

impl Default for StreamWrapper {
    fn default() -> StreamWrapper {
        // SAFETY: The field `state` will be initialized across the FFI to
        // point to the opaque type `mz_internal_state`, which will contain a copy
        // of `inner`. This cyclic structure breaks the uniqueness invariant of
        // &mut mz_stream, so we must use a raw pointer instead of Box<mz_stream>.
        StreamWrapper {
            inner: Box::into_raw(Box::new(mz_stream {
                next_in: ptr::null_mut(),
                avail_in: 0,
                total_in: 0,
                next_out: ptr::null_mut(),
                avail_out: 0,
                total_out: 0,
                msg: ptr::null_mut(),
                adler: 0,
                data_type: 0,
                reserved: 0,
                opaque: ptr::null_mut(),
                state: ptr::null_mut(),

                #[cfg(any(
                    // zlib-ng
                    feature = "zlib-ng",
                    // libz-sys
                    all(not(feature = "cloudflare_zlib"), not(feature = "zlib-ng"), not(feature = "zlib-rs"))
                ))]
                zalloc: allocator::zalloc,
                #[cfg(any(
                    // zlib-ng
                    feature = "zlib-ng",
                    // libz-sys
                    all(not(feature = "cloudflare_zlib"), not(feature = "zlib-ng"), not(feature = "zlib-rs"))
                ))]
                zfree: allocator::zfree,

                #[cfg(
                    // cloudflare-zlib
                    all(feature = "cloudflare_zlib", not(feature = "zlib-rs"), not(feature = "zlib-ng")),
                )]
                zalloc: Some(allocator::zalloc),
                #[cfg(
                    // cloudflare-zlib
                    all(feature = "cloudflare_zlib", not(feature = "zlib-rs"), not(feature = "zlib-ng")),
                )]
                zfree: Some(allocator::zfree),

                // for zlib-rs, it is most efficient to have it provide the allocator.
                // The libz-rs-sys dependency is configured to use the rust system allocator
                #[cfg(all(feature = "zlib-rs", not(feature = "zlib-ng")))]
                zalloc: None,
                #[cfg(all(feature = "zlib-rs", not(feature = "zlib-ng")))]
                zfree: None,
            })),
        }
    }
}

impl Drop for StreamWrapper {
    fn drop(&mut self) {
        // SAFETY: At this point, every other allocation for struct has been freed by
        // `inflateEnd` or `deflateEnd`, and no copies of `inner` are retained by `C`,
        // so it is safe to drop the struct as long as the user respects the invariant that
        // `inner` must never be copied by Rust.
        drop(unsafe { Box::from_raw(self.inner) });
    }
}

#[cfg(any(
    // zlib-ng
    feature = "zlib-ng",
    // cloudflare-zlib
    all(feature = "cloudflare_zlib", not(feature = "zlib-rs"), not(feature = "zlib-ng")),
    // libz-sys
    all(not(feature = "cloudflare_zlib"), not(feature = "zlib-ng"), not(feature = "zlib-rs")),
))]
mod allocator {
    use super::*;

    use std::alloc::{self, Layout};
    use std::convert::TryFrom;
    use std::os::raw::c_void;

    const ALIGN: usize = std::mem::align_of::<usize>();

    fn align_up(size: usize, align: usize) -> usize {
        (size + align - 1) & !(align - 1)
    }

    pub extern "C" fn zalloc(_ptr: *mut c_void, items: uInt, item_size: uInt) -> *mut c_void {
        // We need to multiply `items` and `item_size` to get the actual desired
        // allocation size. Since `zfree` doesn't receive a size argument we
        // also need to allocate space for a `usize` as a header so we can store
        // how large the allocation is to deallocate later.
        let size = match items
            .checked_mul(item_size)
            .and_then(|i| usize::try_from(i).ok())
            .map(|size| align_up(size, ALIGN))
            .and_then(|i| i.checked_add(std::mem::size_of::<usize>()))
        {
            Some(i) => i,
            None => return ptr::null_mut(),
        };

        // Make sure the `size` isn't too big to fail `Layout`'s restrictions
        let layout = match Layout::from_size_align(size, ALIGN) {
            Ok(layout) => layout,
            Err(_) => return ptr::null_mut(),
        };

        unsafe {
            // Allocate the data, and if successful store the size we allocated
            // at the beginning and then return an offset pointer.
            let ptr = alloc::alloc(layout) as *mut usize;
            if ptr.is_null() {
                return ptr as *mut c_void;
            }
            *ptr = size;
            ptr.add(1) as *mut c_void
        }
    }

    pub extern "C" fn zfree(_ptr: *mut c_void, address: *mut c_void) {
        unsafe {
            // Move our address being freed back one pointer, read the size we
            // stored in `zalloc`, and then free it using the standard Rust
            // allocator.
            let ptr = (address as *mut usize).offset(-1);
            let size = *ptr;
            let layout = Layout::from_size_align_unchecked(size, ALIGN);
            alloc::dealloc(ptr as *mut u8, layout)
        }
    }
}

unsafe impl<D: Direction> Send for Stream<D> {}
unsafe impl<D: Direction> Sync for Stream<D> {}

/// Trait used to call the right destroy/end function on the inner
/// stream object on drop.
pub trait Direction {
    unsafe fn destroy(stream: *mut mz_stream) -> c_int;
}

#[derive(Debug)]
pub enum DirCompress {}
#[derive(Debug)]
pub enum DirDecompress {}

#[derive(Debug)]
pub struct Stream<D: Direction> {
    pub stream_wrapper: StreamWrapper,
    pub total_in: u64,
    pub total_out: u64,
    pub _marker: marker::PhantomData<D>,
}

impl<D: Direction> Stream<D> {
    pub fn msg(&self) -> ErrorMessage {
        // SAFETY: The field `inner` must always be accessed as a raw pointer,
        // since it points to a cyclic structure. No copies of `inner` can be
        // retained for longer than the lifetime of `self`.
        let msg = unsafe { (*self.stream_wrapper.inner).msg };
        ErrorMessage(if msg.is_null() {
            None
        } else {
            let s = unsafe { std::ffi::CStr::from_ptr(msg) };
            std::str::from_utf8(s.to_bytes()).ok()
        })
    }
}

impl<D: Direction> Drop for Stream<D> {
    fn drop(&mut self) {
        unsafe {
            let _ = D::destroy(self.stream_wrapper.inner);
        }
    }
}

impl Direction for DirCompress {
    unsafe fn destroy(stream: *mut mz_stream) -> c_int {
        mz_deflateEnd(stream)
    }
}
impl Direction for DirDecompress {
    unsafe fn destroy(stream: *mut mz_stream) -> c_int {
        mz_inflateEnd(stream)
    }
}

#[derive(Debug)]
pub struct Inflate {
    pub inner: Stream<DirDecompress>,
}

impl InflateBackend for Inflate {
    fn make(zlib_header: bool, window_bits: u8) -> Self {
        unsafe {
            let state = StreamWrapper::default();
            let ret = mz_inflateInit2(
                state.inner,
                if zlib_header {
                    window_bits as c_int
                } else {
                    -(window_bits as c_int)
                },
            );
            assert_eq!(ret, 0);
            Inflate {
                inner: Stream {
                    stream_wrapper: state,
                    total_in: 0,
                    total_out: 0,
                    _marker: marker::PhantomData,
                },
            }
        }
    }

    fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        let raw = self.inner.stream_wrapper.inner;
        // SAFETY: The field `inner` must always be accessed as a raw pointer,
        // since it points to a cyclic structure. No copies of `inner` can be
        // retained for longer than the lifetime of `self`.
        unsafe {
            (*raw).msg = ptr::null_mut();
            (*raw).next_in = input.as_ptr() as *mut u8;
            (*raw).avail_in = cmp::min(input.len(), c_uint::MAX as usize) as c_uint;
            (*raw).next_out = output.as_mut_ptr();
            (*raw).avail_out = cmp::min(output.len(), c_uint::MAX as usize) as c_uint;

            let rc = mz_inflate(raw, flush as c_int);

            // Unfortunately the total counters provided by zlib might be only
            // 32 bits wide and overflow while processing large amounts of data.
            self.inner.total_in += ((*raw).next_in as usize - input.as_ptr() as usize) as u64;
            self.inner.total_out += ((*raw).next_out as usize - output.as_ptr() as usize) as u64;

            // reset these pointers so we don't accidentally read them later
            (*raw).next_in = ptr::null_mut();
            (*raw).avail_in = 0;
            (*raw).next_out = ptr::null_mut();
            (*raw).avail_out = 0;

            match rc {
                MZ_DATA_ERROR | MZ_STREAM_ERROR | MZ_MEM_ERROR => {
                    mem::decompress_failed(self.inner.msg())
                }
                MZ_OK => Ok(Status::Ok),
                MZ_BUF_ERROR => Ok(Status::BufError),
                MZ_STREAM_END => Ok(Status::StreamEnd),
                #[allow(clippy::unnecessary_cast)]
                MZ_NEED_DICT => mem::decompress_need_dict((*raw).adler as u32),
                c => panic!("unknown return code: {}", c),
            }
        }
    }

    fn reset(&mut self, zlib_header: bool) {
        let bits = if zlib_header {
            MZ_DEFAULT_WINDOW_BITS
        } else {
            -MZ_DEFAULT_WINDOW_BITS
        };
        unsafe {
            inflateReset2(self.inner.stream_wrapper.inner, bits);
        }
        self.inner.total_out = 0;
        self.inner.total_in = 0;
    }
}

impl Backend for Inflate {
    #[inline]
    fn total_in(&self) -> u64 {
        self.inner.total_in
    }

    #[inline]
    fn total_out(&self) -> u64 {
        self.inner.total_out
    }
}

#[derive(Debug)]
pub struct Deflate {
    pub inner: Stream<DirCompress>,
}

impl DeflateBackend for Deflate {
    fn make(level: Compression, zlib_header: bool, window_bits: u8) -> Self {
        unsafe {
            let state = StreamWrapper::default();
            let ret = mz_deflateInit2(
                state.inner,
                level.0 as c_int,
                MZ_DEFLATED,
                if zlib_header {
                    window_bits as c_int
                } else {
                    -(window_bits as c_int)
                },
                8,
                MZ_DEFAULT_STRATEGY,
            );
            assert_eq!(ret, 0);
            Deflate {
                inner: Stream {
                    stream_wrapper: state,
                    total_in: 0,
                    total_out: 0,
                    _marker: marker::PhantomData,
                },
            }
        }
    }
    fn compress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        let raw = self.inner.stream_wrapper.inner;
        // SAFETY: The field `inner` must always be accessed as a raw pointer,
        // since it points to a cyclic structure. No copies of `inner` can be
        // retained for longer than the lifetime of `self`.
        unsafe {
            (*raw).msg = ptr::null_mut();
            (*raw).next_in = input.as_ptr() as *mut _;
            (*raw).avail_in = cmp::min(input.len(), c_uint::MAX as usize) as c_uint;
            (*raw).next_out = output.as_mut_ptr();
            (*raw).avail_out = cmp::min(output.len(), c_uint::MAX as usize) as c_uint;

            let rc = mz_deflate(raw, flush as c_int);

            // Unfortunately the total counters provided by zlib might be only
            // 32 bits wide and overflow while processing large amounts of data.

            self.inner.total_in += ((*raw).next_in as usize - input.as_ptr() as usize) as u64;
            self.inner.total_out += ((*raw).next_out as usize - output.as_ptr() as usize) as u64;
            // reset these pointers so we don't accidentally read them later
            (*raw).next_in = ptr::null_mut();
            (*raw).avail_in = 0;
            (*raw).next_out = ptr::null_mut();
            (*raw).avail_out = 0;

            match rc {
                MZ_OK => Ok(Status::Ok),
                MZ_BUF_ERROR => Ok(Status::BufError),
                MZ_STREAM_END => Ok(Status::StreamEnd),
                MZ_STREAM_ERROR => mem::compress_failed(self.inner.msg()),
                c => panic!("unknown return code: {}", c),
            }
        }
    }

    fn reset(&mut self) {
        self.inner.total_in = 0;
        self.inner.total_out = 0;
        let rc = unsafe { mz_deflateReset(self.inner.stream_wrapper.inner) };
        assert_eq!(rc, MZ_OK);
    }
}

impl Backend for Deflate {
    #[inline]
    fn total_in(&self) -> u64 {
        self.inner.total_in
    }

    #[inline]
    fn total_out(&self) -> u64 {
        self.inner.total_out
    }
}

pub use self::c_backend::*;

/// For backwards compatibility, we provide symbols as `mz_` to mimic the miniz API
#[allow(bad_style)]
#[allow(unused_imports)]
mod c_backend {
    use std::mem;
    use std::os::raw::{c_char, c_int};

    #[cfg(feature = "zlib-ng")]
    use libz_ng_sys as libz;

    #[cfg(all(feature = "zlib-rs", not(feature = "zlib-ng")))]
    use libz_rs_sys as libz;

    #[cfg(
        // cloudflare-zlib
        all(feature = "cloudflare_zlib", not(feature = "zlib-rs"), not(feature = "zlib-ng")),
    )]
    use cloudflare_zlib_sys as libz;

    #[cfg(
        // libz-sys
        all(not(feature = "cloudflare_zlib"), not(feature = "zlib-ng"), not(feature = "zlib-rs")),
    )]
    use libz_sys as libz;

    pub use libz::deflate as mz_deflate;
    pub use libz::deflateEnd as mz_deflateEnd;
    pub use libz::deflateReset as mz_deflateReset;
    pub use libz::inflate as mz_inflate;
    pub use libz::inflateEnd as mz_inflateEnd;
    pub use libz::z_stream as mz_stream;
    pub use libz::*;

    pub use libz::Z_BLOCK as MZ_BLOCK;
    pub use libz::Z_BUF_ERROR as MZ_BUF_ERROR;
    pub use libz::Z_DATA_ERROR as MZ_DATA_ERROR;
    pub use libz::Z_DEFAULT_STRATEGY as MZ_DEFAULT_STRATEGY;
    pub use libz::Z_DEFLATED as MZ_DEFLATED;
    pub use libz::Z_FINISH as MZ_FINISH;
    pub use libz::Z_FULL_FLUSH as MZ_FULL_FLUSH;
    pub use libz::Z_MEM_ERROR as MZ_MEM_ERROR;
    pub use libz::Z_NEED_DICT as MZ_NEED_DICT;
    pub use libz::Z_NO_FLUSH as MZ_NO_FLUSH;
    pub use libz::Z_OK as MZ_OK;
    pub use libz::Z_PARTIAL_FLUSH as MZ_PARTIAL_FLUSH;
    pub use libz::Z_STREAM_END as MZ_STREAM_END;
    pub use libz::Z_STREAM_ERROR as MZ_STREAM_ERROR;
    pub use libz::Z_SYNC_FLUSH as MZ_SYNC_FLUSH;

    pub const MZ_DEFAULT_WINDOW_BITS: c_int = 15;

    #[cfg(feature = "zlib-ng")]
    const ZLIB_VERSION: &str = "2.1.0.devel\0";
    #[cfg(all(not(feature = "zlib-ng"), feature = "zlib-rs"))]
    const ZLIB_VERSION: &str = "1.3.0-zlib-rs-0.5.1\0";
    #[cfg(not(any(feature = "zlib-ng", feature = "zlib-rs")))]
    const ZLIB_VERSION: &str = "1.2.8\0";

    pub unsafe extern "C" fn mz_deflateInit2(
        stream: *mut mz_stream,
        level: c_int,
        method: c_int,
        window_bits: c_int,
        mem_level: c_int,
        strategy: c_int,
    ) -> c_int {
        libz::deflateInit2_(
            stream,
            level,
            method,
            window_bits,
            mem_level,
            strategy,
            ZLIB_VERSION.as_ptr() as *const c_char,
            mem::size_of::<mz_stream>() as c_int,
        )
    }
    pub unsafe extern "C" fn mz_inflateInit2(stream: *mut mz_stream, window_bits: c_int) -> c_int {
        libz::inflateInit2_(
            stream,
            window_bits,
            ZLIB_VERSION.as_ptr() as *const c_char,
            mem::size_of::<mz_stream>() as c_int,
        )
    }
}
