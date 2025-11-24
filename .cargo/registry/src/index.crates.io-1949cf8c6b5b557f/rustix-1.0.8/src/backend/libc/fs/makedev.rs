// TODO: Remove the unsafe blocks. libc 0.2.171 removed `unsafe` from several
// of these functions. Eventually we should depend on that version and remove
// the `unsafe` blocks in the code, but for now, disable that warning so that
// we're compatible with older libc versions.
#![allow(unused_unsafe)]

#[cfg(not(all(target_os = "android", target_pointer_width = "32")))]
use crate::backend::c;
use crate::fs::Dev;

#[cfg(not(any(
    apple,
    solarish,
    target_os = "aix",
    target_os = "android",
    target_os = "emscripten",
)))]
#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    c::makedev(maj, min)
}

#[cfg(solarish)]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    // SAFETY: Solarish's `makedev` is marked unsafe but it isn't doing
    // anything unsafe.
    unsafe { c::makedev(maj, min) }
}

#[cfg(all(target_os = "android", not(target_pointer_width = "32")))]
#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    c::makedev(maj, min)
}

#[cfg(all(target_os = "android", target_pointer_width = "32"))]
#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    // 32-bit Android's `dev_t` is 32-bit, but its `st_dev` is 64-bit, so we do
    // it ourselves.
    ((u64::from(maj) & 0xffff_f000_u64) << 32)
        | ((u64::from(maj) & 0x0000_0fff_u64) << 8)
        | ((u64::from(min) & 0xffff_ff00_u64) << 12)
        | (u64::from(min) & 0x0000_00ff_u64)
}

#[cfg(target_os = "emscripten")]
#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    // Emscripten's `makedev` has a 32-bit return value.
    Dev::from(c::makedev(maj, min))
}

#[cfg(apple)]
#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    // Apple's `makedev` oddly has signed argument types and is `unsafe`.
    unsafe { c::makedev(maj as i32, min as i32) }
}

#[cfg(target_os = "aix")]
#[inline]
pub(crate) fn makedev(maj: u32, min: u32) -> Dev {
    // AIX's `makedev` oddly is `unsafe`.
    unsafe { c::makedev(maj, min) }
}

#[cfg(not(any(
    apple,
    freebsdlike,
    target_os = "android",
    target_os = "emscripten",
    target_os = "netbsd"
)))]
#[inline]
pub(crate) fn major(dev: Dev) -> u32 {
    unsafe { c::major(dev) }
}

#[cfg(any(
    apple,
    freebsdlike,
    target_os = "netbsd",
    all(target_os = "android", not(target_pointer_width = "32")),
))]
#[inline]
pub(crate) fn major(dev: Dev) -> u32 {
    // On some platforms `major` oddly has signed return types.
    (unsafe { c::major(dev) }) as u32
}

#[cfg(all(target_os = "android", target_pointer_width = "32"))]
#[inline]
pub(crate) fn major(dev: Dev) -> u32 {
    // 32-bit Android's `dev_t` is 32-bit, but its `st_dev` is 64-bit, so we do
    // it ourselves.
    (((dev >> 31 >> 1) & 0xffff_f000) | ((dev >> 8) & 0x0000_0fff)) as u32
}

#[cfg(target_os = "emscripten")]
#[inline]
pub(crate) fn major(dev: Dev) -> u32 {
    // Emscripten's `major` has a 32-bit argument value.
    unsafe { c::major(dev as u32) }
}

#[cfg(not(any(
    apple,
    freebsdlike,
    target_os = "android",
    target_os = "emscripten",
    target_os = "netbsd"
)))]
#[inline]
pub(crate) fn minor(dev: Dev) -> u32 {
    unsafe { c::minor(dev) }
}

#[cfg(any(
    apple,
    freebsdlike,
    target_os = "netbsd",
    all(target_os = "android", not(target_pointer_width = "32"))
))]
#[inline]
pub(crate) fn minor(dev: Dev) -> u32 {
    // On some platforms, `minor` oddly has signed return types.
    (unsafe { c::minor(dev) }) as u32
}

#[cfg(all(target_os = "android", target_pointer_width = "32"))]
#[inline]
pub(crate) fn minor(dev: Dev) -> u32 {
    // 32-bit Android's `dev_t` is 32-bit, but its `st_dev` is 64-bit, so we do
    // it ourselves.
    (((dev >> 12) & 0xffff_ff00) | (dev & 0x0000_00ff)) as u32
}

#[cfg(target_os = "emscripten")]
#[inline]
pub(crate) fn minor(dev: Dev) -> u32 {
    // Emscripten's `minor` has a 32-bit argument value.
    unsafe { c::minor(dev as u32) }
}
