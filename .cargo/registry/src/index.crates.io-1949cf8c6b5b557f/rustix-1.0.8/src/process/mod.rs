//! Process-associated operations.

#[cfg(not(target_os = "wasi"))]
mod chdir;
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
mod chroot;
mod exit;
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
mod fcntl_getlk;
#[cfg(not(target_os = "wasi"))] // WASI doesn't have get[gpu]id.
mod id;
#[cfg(not(any(target_os = "aix", target_os = "espidf", target_os = "vita")))]
mod ioctl;
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
mod kill;
#[cfg(target_os = "linux")]
mod pidfd;
#[cfg(target_os = "linux")]
mod pidfd_getfd;
#[cfg(target_os = "linux")]
mod pivot_root;
#[cfg(linux_kernel)]
mod prctl;
#[cfg(not(any(target_os = "fuchsia", target_os = "vita", target_os = "wasi")))]
// WASI doesn't have [gs]etpriority.
mod priority;
#[cfg(freebsdlike)]
mod procctl;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
mod rlimit;
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
mod types;
#[cfg(not(target_os = "wasi"))] // WASI doesn't have umask.
mod umask;
#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
mod wait;

#[cfg(not(target_os = "wasi"))]
pub use chdir::*;
#[cfg(not(any(target_os = "fuchsia", target_os = "wasi")))]
pub use chroot::*;
pub use exit::*;
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub use fcntl_getlk::*;
#[cfg(not(target_os = "wasi"))]
pub use id::*;
#[cfg(not(any(target_os = "aix", target_os = "espidf", target_os = "vita")))]
pub use ioctl::*;
#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
pub use kill::*;
#[cfg(target_os = "linux")]
pub use pidfd::*;
#[cfg(target_os = "linux")]
pub use pidfd_getfd::*;
#[cfg(target_os = "linux")]
pub use pivot_root::*;
#[cfg(linux_kernel)]
pub use prctl::*;
#[cfg(not(any(target_os = "fuchsia", target_os = "vita", target_os = "wasi")))]
pub use priority::*;
#[cfg(freebsdlike)]
pub use procctl::*;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub use rlimit::*;
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub use types::*;
#[cfg(not(target_os = "wasi"))]
pub use umask::*;
#[cfg(not(any(target_os = "espidf", target_os = "vita", target_os = "wasi")))]
pub use wait::*;
