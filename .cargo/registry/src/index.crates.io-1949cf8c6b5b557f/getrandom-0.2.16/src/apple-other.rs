//! Implementation for iOS, tvOS, and watchOS where `getentropy` is unavailable.
use crate::Error;
use core::{ffi::c_void, mem::MaybeUninit};

// libsystem contains the libc of Darwin, and every binary ends up linked against it either way. This
// makes it a more lightweight choice compared to `Security.framework`.
extern "C" {
    // This RNG uses a thread-local CSPRNG to provide data, which is seeded by the operating system's root CSPRNG.
    // Its the best option after `getentropy` on modern Darwin-based platforms that also avoids the
    // high startup costs and linking of Security.framework.
    //
    // While its just an implementation detail, `Security.framework` just calls into this anyway.
    fn CCRandomGenerateBytes(bytes: *mut c_void, size: usize) -> i32;
}

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    let ret = unsafe { CCRandomGenerateBytes(dest.as_mut_ptr() as *mut c_void, dest.len()) };
    // kCCSuccess (from CommonCryptoError.h) is always zero.
    if ret != 0 {
        Err(Error::IOS_SEC_RANDOM)
    } else {
        Ok(())
    }
}
