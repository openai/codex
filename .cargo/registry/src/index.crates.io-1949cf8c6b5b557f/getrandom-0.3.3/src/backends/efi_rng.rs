//! Implementation for UEFI using EFI_RNG_PROTOCOL
use crate::Error;
use core::{
    mem::MaybeUninit,
    ptr::{self, null_mut, NonNull},
    sync::atomic::{AtomicPtr, Ordering::Relaxed},
};
use r_efi::{
    efi::{BootServices, Handle},
    protocols::rng,
};

extern crate std;

pub use crate::util::{inner_u32, inner_u64};

#[cfg(not(target_os = "uefi"))]
compile_error!("`efi_rng` backend can be enabled only for UEFI targets!");

static RNG_PROTOCOL: AtomicPtr<rng::Protocol> = AtomicPtr::new(null_mut());

#[cold]
#[inline(never)]
fn init() -> Result<NonNull<rng::Protocol>, Error> {
    const HANDLE_SIZE: usize = size_of::<Handle>();

    let boot_services = std::os::uefi::env::boot_services()
        .ok_or(Error::BOOT_SERVICES_UNAVAILABLE)?
        .cast::<BootServices>();

    let mut handles = [ptr::null_mut(); 16];
    // `locate_handle` operates with length in bytes
    let mut buf_size = handles.len() * HANDLE_SIZE;
    let mut guid = rng::PROTOCOL_GUID;
    let ret = unsafe {
        ((*boot_services.as_ptr()).locate_handle)(
            r_efi::efi::BY_PROTOCOL,
            &mut guid,
            null_mut(),
            &mut buf_size,
            handles.as_mut_ptr(),
        )
    };

    if ret.is_error() {
        return Err(Error::from_uefi_code(ret.as_usize()));
    }

    let handles_len = buf_size / HANDLE_SIZE;
    let handles = handles.get(..handles_len).ok_or(Error::UNEXPECTED)?;

    let system_handle = std::os::uefi::env::image_handle();
    for &handle in handles {
        let mut protocol: MaybeUninit<*mut rng::Protocol> = MaybeUninit::uninit();

        let mut protocol_guid = rng::PROTOCOL_GUID;
        let ret = unsafe {
            ((*boot_services.as_ptr()).open_protocol)(
                handle,
                &mut protocol_guid,
                protocol.as_mut_ptr().cast(),
                system_handle.as_ptr(),
                ptr::null_mut(),
                r_efi::system::OPEN_PROTOCOL_GET_PROTOCOL,
            )
        };

        let protocol = if ret.is_error() {
            continue;
        } else {
            let protocol = unsafe { protocol.assume_init() };
            NonNull::new(protocol).ok_or(Error::UNEXPECTED)?
        };

        // Try to use the acquired protocol handle
        let mut buf = [0u8; 8];
        let mut alg_guid = rng::ALGORITHM_RAW;
        let ret = unsafe {
            ((*protocol.as_ptr()).get_rng)(
                protocol.as_ptr(),
                &mut alg_guid,
                buf.len(),
                buf.as_mut_ptr(),
            )
        };

        if ret.is_error() {
            continue;
        }

        RNG_PROTOCOL.store(protocol.as_ptr(), Relaxed);
        return Ok(protocol);
    }
    Err(Error::NO_RNG_HANDLE)
}

#[inline]
pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    let protocol = match NonNull::new(RNG_PROTOCOL.load(Relaxed)) {
        Some(p) => p,
        None => init()?,
    };

    let mut alg_guid = rng::ALGORITHM_RAW;
    let ret = unsafe {
        ((*protocol.as_ptr()).get_rng)(
            protocol.as_ptr(),
            &mut alg_guid,
            dest.len(),
            dest.as_mut_ptr().cast::<u8>(),
        )
    };

    if ret.is_error() {
        Err(Error::from_uefi_code(ret.as_usize()))
    } else {
        Ok(())
    }
}

impl Error {
    pub(crate) const BOOT_SERVICES_UNAVAILABLE: Error = Self::new_internal(10);
    pub(crate) const NO_RNG_HANDLE: Error = Self::new_internal(11);
}
