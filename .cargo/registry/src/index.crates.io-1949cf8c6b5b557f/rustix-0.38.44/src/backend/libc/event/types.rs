#[cfg(any(linux_kernel, target_os = "freebsd", target_os = "illumos"))]
use crate::backend::c;
#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf"
))]
use bitflags::bitflags;

#[cfg(any(
    linux_kernel,
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "espidf"
))]
bitflags! {
    /// `EFD_*` flags for use with [`eventfd`].
    ///
    /// [`eventfd`]: crate::event::eventfd
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct EventfdFlags: u32 {
        /// `EFD_CLOEXEC`
        #[cfg(not(target_os = "espidf"))]
        const CLOEXEC = bitcast!(c::EFD_CLOEXEC);
        /// `EFD_NONBLOCK`
        #[cfg(not(target_os = "espidf"))]
        const NONBLOCK = bitcast!(c::EFD_NONBLOCK);
        /// `EFD_SEMAPHORE`
        #[cfg(not(target_os = "espidf"))]
        const SEMAPHORE = bitcast!(c::EFD_SEMAPHORE);

        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}
