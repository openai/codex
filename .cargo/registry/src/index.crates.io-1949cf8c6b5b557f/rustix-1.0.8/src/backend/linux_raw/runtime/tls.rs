//! TLS utilities.

/// For use with [`set_thread_area`].
///
/// [`set_thread_area`]: crate::runtime::set_thread_area
#[cfg(target_arch = "x86")]
pub type UserDesc = linux_raw_sys::general::user_desc;
