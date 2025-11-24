//! Implementation for VxWorks
use crate::{util_libc::last_os_error, Error};
use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

pub fn getrandom_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    static RNG_INIT: AtomicBool = AtomicBool::new(false);
    while !RNG_INIT.load(Relaxed) {
        let ret = unsafe { libc::randSecure() };
        if ret < 0 {
            return Err(Error::VXWORKS_RAND_SECURE);
        } else if ret > 0 {
            RNG_INIT.store(true, Relaxed);
            break;
        }
        unsafe { libc::usleep(10) };
    }

    // Prevent overflow of i32
    for chunk in dest.chunks_mut(i32::max_value() as usize) {
        let ret = unsafe { libc::randABytes(chunk.as_mut_ptr() as *mut u8, chunk.len() as i32) };
        if ret != 0 {
            return Err(last_os_error());
        }
    }
    Ok(())
}
