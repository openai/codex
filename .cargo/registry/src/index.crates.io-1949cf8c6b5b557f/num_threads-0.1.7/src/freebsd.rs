extern crate libc;

use std::num::NonZeroUsize;
use std::{mem, ptr};

pub(crate) fn num_threads() -> Option<NonZeroUsize> {
    // Safety: `sysctl` and `getpid` are both thread-safe.
    // `kip` is only accessed if sysctl() succeeds and agrees with the expected size,
    // and the data only trusted if both its embedded size and pid match expectations
    unsafe {
        let pid = libc::getpid();
        let mib: [libc::c_int; 4] = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
        let mut kip: libc::kinfo_proc = mem::zeroed();
        let expected_kip_len = mem::size_of_val(&kip);
        let mut kip_len = expected_kip_len;

        let ret = libc::sysctl(
            mib.as_ptr(),
            mib.len() as u32,
            &mut kip as *mut _ as *mut libc::c_void,
            &mut kip_len,
            ptr::null(),
            0,
        );

        if ret == 0
            && kip_len == expected_kip_len
            && kip.ki_structsize == expected_kip_len as i32
            && kip.ki_pid == pid
        {
            NonZeroUsize::new(kip.ki_numthreads as usize)
        } else {
            None
        }
    }
}
