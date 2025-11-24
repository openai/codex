//! Implementation for Hermit
use crate::Error;
use core::mem::MaybeUninit;

extern "C" {
    fn sys_read_entropy(buffer: *mut u8, length: usize, flags: u32) -> isize;
    // Note that `sys_secure_rand32/64` are implemented using `sys_read_entropy`:
    // https://github.com/hermit-os/kernel/blob/430da84/src/syscalls/entropy.rs#L62-L104
    // But this may change in future and can depend on compilation target,
    // so to future-proof we use these "syscalls".
    fn sys_secure_rand32(value: *mut u32) -> i32;
    fn sys_secure_rand64(value: *mut u64) -> i32;
}

#[inline]
pub fn inner_u32() -> Result<u32, Error> {
    let mut res = MaybeUninit::uninit();
    let ret = unsafe { sys_secure_rand32(res.as_mut_ptr()) };
    match ret {
        0 => Ok(unsafe { res.assume_init() }),
        -1 => Err(Error::UNSUPPORTED),
        _ => Err(Error::UNEXPECTED),
    }
}

#[inline]
pub fn inner_u64() -> Result<u64, Error> {
    let mut res = MaybeUninit::uninit();
    let ret = unsafe { sys_secure_rand64(res.as_mut_ptr()) };
    match ret {
        0 => Ok(unsafe { res.assume_init() }),
        -1 => Err(Error::UNSUPPORTED),
        _ => Err(Error::UNEXPECTED),
    }
}

#[inline]
pub fn fill_inner(mut dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    while !dest.is_empty() {
        let res = unsafe { sys_read_entropy(dest.as_mut_ptr().cast::<u8>(), dest.len(), 0) };
        match res {
            res if res > 0 => {
                let len = usize::try_from(res).map_err(|_| Error::UNEXPECTED)?;
                dest = dest.get_mut(len..).ok_or(Error::UNEXPECTED)?;
            }
            code => {
                let code = i32::try_from(code).map_err(|_| Error::UNEXPECTED)?;
                return Err(Error::from_neg_error_code(code));
            }
        }
    }
    Ok(())
}
