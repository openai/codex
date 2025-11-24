//! Implementation for VxWorks
use crate::Error;
use core::{
    cmp::Ordering::{Equal, Greater, Less},
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

#[path = "../util_libc.rs"]
mod util_libc;

pub use crate::util::{inner_u32, inner_u64};

static RNG_INIT: AtomicBool = AtomicBool::new(false);

#[cold]
fn init() -> Result<(), Error> {
    let ret = unsafe { libc::randSecure() };
    match ret.cmp(&0) {
        Greater => RNG_INIT.store(true, Relaxed),
        Equal => unsafe {
            libc::usleep(10);
        },
        Less => return Err(Error::VXWORKS_RAND_SECURE),
    }
    Ok(())
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    while !RNG_INIT.load(Relaxed) {
        init()?;
    }

    // Prevent overflow of i32
    let chunk_size = usize::try_from(i32::MAX).expect("VxWorks does not support 16-bit targets");
    for chunk in dest.chunks_mut(chunk_size) {
        let chunk_len: libc::c_int = chunk
            .len()
            .try_into()
            .expect("chunk size is bounded by i32::MAX");
        let p: *mut libc::c_uchar = chunk.as_mut_ptr().cast();
        let ret = unsafe { libc::randABytes(p, chunk_len) };
        if ret != 0 {
            return Err(util_libc::last_os_error());
        }
    }
    Ok(())
}

impl Error {
    /// On VxWorks, call to `randSecure` failed (random number generator is not yet initialized).
    pub(crate) const VXWORKS_RAND_SECURE: Error = Self::new_internal(10);
}
