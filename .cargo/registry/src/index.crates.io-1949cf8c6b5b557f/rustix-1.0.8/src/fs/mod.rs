//! Filesystem operations.

mod abs;
#[cfg(not(target_os = "redox"))]
mod at;
mod constants;
#[cfg(linux_kernel)]
mod copy_file_range;
#[cfg(all(feature = "alloc", not(any(target_os = "espidf", target_os = "redox"))))]
mod dir;
#[cfg(not(any(
    apple,
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
mod fadvise;
pub(crate) mod fcntl;
#[cfg(apple)]
mod fcntl_apple;
#[cfg(apple)]
mod fcopyfile;
pub(crate) mod fd;
#[cfg(all(apple, feature = "alloc"))]
mod getpath;
#[cfg(not(target_os = "wasi"))] // WASI doesn't have get[gpu]id.
mod id;
#[cfg(linux_kernel)]
pub mod inotify;
#[cfg(linux_kernel)]
mod ioctl;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
mod makedev;
#[cfg(any(linux_kernel, target_os = "freebsd"))]
mod memfd_create;
#[cfg(linux_kernel)]
mod openat2;
#[cfg(linux_kernel)]
mod raw_dir;
mod seek_from;
#[cfg(target_os = "linux")]
mod sendfile;
#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
mod special;
#[cfg(linux_kernel)]
mod statx;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
mod sync;
#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
mod xattr;

pub use abs::*;
#[cfg(not(target_os = "redox"))]
pub use at::*;
pub use constants::*;
#[cfg(linux_kernel)]
pub use copy_file_range::copy_file_range;
#[cfg(all(feature = "alloc", not(any(target_os = "espidf", target_os = "redox"))))]
pub use dir::{Dir, DirEntry};
#[cfg(not(any(
    apple,
    netbsdlike,
    target_os = "dragonfly",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
)))]
pub use fadvise::fadvise;
pub use fcntl::*;
#[cfg(apple)]
pub use fcntl_apple::*;
#[cfg(apple)]
pub use fcopyfile::*;
pub use fd::*;
#[cfg(all(apple, feature = "alloc"))]
pub use getpath::getpath;
#[cfg(not(target_os = "wasi"))]
pub use id::*;
#[cfg(linux_kernel)]
pub use ioctl::*;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub use makedev::*;
#[cfg(any(linux_kernel, target_os = "freebsd"))]
pub use memfd_create::memfd_create;
#[cfg(linux_kernel)]
pub use openat2::openat2;
#[cfg(linux_kernel)]
pub use raw_dir::{RawDir, RawDirEntry};
pub use seek_from::SeekFrom;
#[cfg(target_os = "linux")]
pub use sendfile::sendfile;
#[cfg(not(any(target_os = "espidf", target_os = "redox")))]
pub use special::*;
#[cfg(linux_kernel)]
pub use statx::*;
#[cfg(not(any(
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
pub use sync::sync;
#[cfg(any(apple, linux_kernel, target_os = "hurd"))]
pub use xattr::*;

/// Re-export types common to POSIX-ish platforms.
#[cfg(feature = "std")]
#[cfg(unix)]
pub use std::os::unix::fs::{DirEntryExt, FileExt, FileTypeExt, MetadataExt, OpenOptionsExt};
#[cfg(feature = "std")]
#[cfg(all(wasi_ext, target_os = "wasi"))]
pub use std::os::wasi::fs::{DirEntryExt, FileExt, FileTypeExt, MetadataExt, OpenOptionsExt};
