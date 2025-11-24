//! Process execution domains
use crate::errno::Errno;
use crate::Result;

use libc::{self, c_int, c_ulong};

libc_bitflags! {
    /// Flags used and returned by [`get()`](fn.get.html) and
    /// [`set()`](fn.set.html).
    pub struct Persona: c_int {
        /// Provide the legacy virtual address space layout.
        ADDR_COMPAT_LAYOUT;
        /// Disable address-space-layout randomization.
        ADDR_NO_RANDOMIZE;
        /// Limit the address space to 32 bits.
        ADDR_LIMIT_32BIT;
        /// Use `0xc0000000` as the offset at which to search a virtual memory
        /// chunk on [`mmap(2)`], otherwise use `0xffffe000`.
        ///
        /// [`mmap(2)`]: https://man7.org/linux/man-pages/man2/mmap.2.html
        ADDR_LIMIT_3GB;
        /// User-space function pointers to signal handlers point to descriptors.
        #[cfg(not(any(target_env = "musl", target_env = "uclibc")))]
        FDPIC_FUNCPTRS;
        /// Map page 0 as read-only.
        MMAP_PAGE_ZERO;
        /// `PROT_READ` implies `PROT_EXEC` for [`mmap(2)`].
        ///
        /// [`mmap(2)`]: https://man7.org/linux/man-pages/man2/mmap.2.html
        READ_IMPLIES_EXEC;
        /// No effects.
        SHORT_INODE;
        /// [`select(2)`], [`pselect(2)`], and [`ppoll(2)`] do not modify the
        /// returned timeout argument when interrupted by a signal handler.
        ///
        /// [`select(2)`]: https://man7.org/linux/man-pages/man2/select.2.html
        /// [`pselect(2)`]: https://man7.org/linux/man-pages/man2/pselect.2.html
        /// [`ppoll(2)`]: https://man7.org/linux/man-pages/man2/ppoll.2.html
        STICKY_TIMEOUTS;
        /// Have [`uname(2)`] report a 2.6.40+ version number rather than a 3.x
        /// version number.
        ///
        /// [`uname(2)`]: https://man7.org/linux/man-pages/man2/uname.2.html
        #[cfg(not(any(target_env = "musl", target_env = "uclibc")))]
        UNAME26;
        /// No effects.
        WHOLE_SECONDS;
    }
}

/// Retrieve the current process personality.
///
/// Returns a Result containing a Persona instance.
///
/// Example:
///
/// ```
/// # use nix::sys::personality::{self, Persona};
/// let pers = personality::get().unwrap();
/// assert!(!pers.contains(Persona::WHOLE_SECONDS));
/// ```
pub fn get() -> Result<Persona> {
    let res = unsafe { libc::personality(0xFFFFFFFF) };

    Errno::result(res).map(Persona::from_bits_truncate)
}

/// Set the current process personality.
///
/// Returns a Result containing the *previous* personality for the
/// process, as a Persona.
///
/// For more information, see [personality(2)](https://man7.org/linux/man-pages/man2/personality.2.html)
///
/// **NOTE**: This call **replaces** the current personality entirely.
/// To **update** the personality, first call `get()` and then `set()`
/// with the modified persona.
///
/// Example:
///
// Disable test on aarch64 until we know why it fails.
// https://github.com/nix-rust/nix/issues/2060
#[cfg_attr(target_arch = "aarch64", doc = " ```no_run")]
#[cfg_attr(not(target_arch = "aarch64"), doc = " ```")]
/// # use nix::sys::personality::{self, Persona};
/// let mut pers = personality::get().unwrap();
/// assert!(!pers.contains(Persona::ADDR_NO_RANDOMIZE));
/// personality::set(pers | Persona::ADDR_NO_RANDOMIZE).unwrap();
/// ```
pub fn set(persona: Persona) -> Result<Persona> {
    let res = unsafe { libc::personality(persona.bits() as c_ulong) };

    Errno::result(res).map(Persona::from_bits_truncate)
}
