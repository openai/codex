//! Uname and other system-level functions.

#![allow(unsafe_code)]

use crate::backend;
#[cfg(target_os = "linux")]
use crate::backend::c;
use crate::ffi::CStr;
#[cfg(not(any(target_os = "espidf", target_os = "emscripten", target_os = "vita")))]
use crate::io;
use core::fmt;

#[cfg(linux_kernel)]
pub use backend::system::types::Sysinfo;

#[cfg(linux_kernel)]
use crate::fd::AsFd;
#[cfg(linux_kernel)]
use crate::ffi::c_int;

/// `uname()`—Returns high-level information about the runtime OS and
/// hardware.
///
/// For `gethostname()`, use [`Uname::nodename`] on the result.
///
/// # References
///  - [POSIX]
///  - [Linux]
///  - [Apple]
///  - [NetBSD]
///  - [FreeBSD]
///  - [OpenBSD]
///  - [DragonFly BSD]
///  - [illumos]
///  - [glibc]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/uname.html
/// [Linux]: https://man7.org/linux/man-pages/man2/uname.2.html
/// [Apple]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/uname.3.html
/// [NetBSD]: https://man.netbsd.org/uname.3
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=uname&sektion=3
/// [OpenBSD]: https://man.openbsd.org/uname.3
/// [DragonFly BSD]: https://man.dragonflybsd.org/?command=uname&section=3
/// [illumos]: https://illumos.org/man/2/uname
/// [glibc]: https://sourceware.org/glibc/manual/latest/html_node/Platform-Type.html
#[doc(alias = "gethostname")]
#[inline]
pub fn uname() -> Uname {
    Uname(backend::system::syscalls::uname())
}

/// `struct utsname`—Return type for [`uname`].
#[doc(alias = "utsname")]
pub struct Uname(backend::system::types::RawUname);

impl Uname {
    /// `sysname`—Operating system release name.
    #[inline]
    pub fn sysname(&self) -> &CStr {
        Self::to_cstr(self.0.sysname.as_ptr().cast())
    }

    /// `nodename`—Name with vague meaning.
    ///
    /// This is intended to be a network name, however it's unable to convey
    /// information about hosts that have multiple names, or any information
    /// about where the names are visible.
    ///
    /// This corresponds to the `gethostname` value.
    #[inline]
    pub fn nodename(&self) -> &CStr {
        Self::to_cstr(self.0.nodename.as_ptr().cast())
    }

    /// `release`—Operating system release version string.
    #[inline]
    pub fn release(&self) -> &CStr {
        Self::to_cstr(self.0.release.as_ptr().cast())
    }

    /// `version`—Operating system build identifiers.
    #[inline]
    pub fn version(&self) -> &CStr {
        Self::to_cstr(self.0.version.as_ptr().cast())
    }

    /// `machine`—Hardware architecture identifier.
    #[inline]
    pub fn machine(&self) -> &CStr {
        Self::to_cstr(self.0.machine.as_ptr().cast())
    }

    /// `domainname`—NIS or YP domain identifier.
    #[cfg(linux_kernel)]
    #[inline]
    pub fn domainname(&self) -> &CStr {
        Self::to_cstr(self.0.domainname.as_ptr().cast())
    }

    #[inline]
    fn to_cstr<'a>(ptr: *const u8) -> &'a CStr {
        // SAFETY: Strings returned from the kernel are always NUL-terminated.
        unsafe { CStr::from_ptr(ptr.cast()) }
    }
}

impl fmt::Debug for Uname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(not(linux_kernel))]
        {
            write!(
                f,
                "{:?} {:?} {:?} {:?} {:?}",
                self.sysname(),
                self.nodename(),
                self.release(),
                self.version(),
                self.machine(),
            )
        }
        #[cfg(linux_kernel)]
        {
            write!(
                f,
                "{:?} {:?} {:?} {:?} {:?} {:?}",
                self.sysname(),
                self.nodename(),
                self.release(),
                self.version(),
                self.machine(),
                self.domainname(),
            )
        }
    }
}

/// `sysinfo()`—Returns status information about the runtime OS.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/uname.2.html
#[cfg(linux_kernel)]
#[inline]
pub fn sysinfo() -> Sysinfo {
    backend::system::syscalls::sysinfo()
}

/// `sethostname(name)`—Sets the system host name.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/sethostname.2.html
#[cfg(not(any(
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "redox",
    target_os = "vita",
    target_os = "wasi"
)))]
#[inline]
pub fn sethostname(name: &[u8]) -> io::Result<()> {
    backend::system::syscalls::sethostname(name)
}

/// `setdomain(name)`—Sets the system NIS domain name.
///
/// # References
///  - [Linux]
///  - [FreeBSD]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/setdomainname.2.html
/// [FreeBSD]: https://man.freebsd.org/cgi/man.cgi?query=setdomainname&sektion=3
#[cfg(not(any(
    target_os = "cygwin",
    target_os = "emscripten",
    target_os = "espidf",
    target_os = "haiku",
    target_os = "horizon",
    target_os = "illumos",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
)))]
#[inline]
pub fn setdomainname(name: &[u8]) -> io::Result<()> {
    backend::system::syscalls::setdomainname(name)
}

/// Reboot command for use with [`reboot`].
#[cfg(target_os = "linux")]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
#[non_exhaustive]
pub enum RebootCommand {
    /// Disables the Ctrl-Alt-Del keystroke.
    ///
    /// When disabled, the keystroke will send a [`Signal::INT`] to
    /// [`Pid::INIT`].
    ///
    /// [`Signal::INT`]: crate::process::Signal::INT
    /// [`Pid::INIT`]: crate::process::Pid::INIT
    CadOff = c::LINUX_REBOOT_CMD_CAD_OFF,
    /// Enables the Ctrl-Alt-Del keystroke.
    ///
    /// When enabled, the keystroke will trigger a [`Restart`].
    ///
    /// [`Restart`]: Self::Restart
    CadOn = c::LINUX_REBOOT_CMD_CAD_ON,
    /// Prints the message "System halted" and halts the system
    Halt = c::LINUX_REBOOT_CMD_HALT,
    /// Execute a kernel that has been loaded earlier with [`kexec_load`].
    ///
    /// [`kexec_load`]: https://man7.org/linux/man-pages/man2/kexec_load.2.html
    Kexec = c::LINUX_REBOOT_CMD_KEXEC,
    /// Prints the message "Power down.", stops the system, and tries to remove
    /// all power
    PowerOff = c::LINUX_REBOOT_CMD_POWER_OFF,
    /// Prints the message "Restarting system." and triggers a restart
    Restart = c::LINUX_REBOOT_CMD_RESTART,
    /// Hibernate the system by suspending to disk
    SwSuspend = c::LINUX_REBOOT_CMD_SW_SUSPEND,
}

/// `reboot`—Reboot the system or enable/disable Ctrl-Alt-Del.
///
/// The reboot syscall, despite the name, can actually do much more than
/// reboot.
///
/// Among other things, it can:
///  - Restart, Halt, Power Off, and Suspend the system
///  - Enable and disable the Ctrl-Alt-Del keystroke
///  - Execute other kernels
///  - Terminate init inside PID namespaces
///
/// It is highly recommended to carefully read the kernel documentation before
/// calling this function.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/reboot.2.html
#[cfg(target_os = "linux")]
pub fn reboot(cmd: RebootCommand) -> io::Result<()> {
    backend::system::syscalls::reboot(cmd)
}

/// `init_module`—Load a kernel module.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/init_module.2.html
#[inline]
#[cfg(linux_kernel)]
pub fn init_module(image: &[u8], param_values: &CStr) -> io::Result<()> {
    backend::system::syscalls::init_module(image, param_values)
}

/// `finit_module`—Load a kernel module from a file descriptor.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/finit_module.2.html
#[inline]
#[cfg(linux_kernel)]
pub fn finit_module<Fd: AsFd>(fd: Fd, param_values: &CStr, flags: c_int) -> io::Result<()> {
    backend::system::syscalls::finit_module(fd.as_fd(), param_values, flags)
}

/// `delete_module`—Unload a kernel module.
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/delete_module.2.html
#[inline]
#[cfg(linux_kernel)]
pub fn delete_module(name: &CStr, flags: c_int) -> io::Result<()> {
    backend::system::syscalls::delete_module(name, flags)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::backend::c;

    #[cfg(linux_kernel)]
    #[test]
    fn test_sysinfo_layouts() {
        // Don't assert the size for `Sysinfo` because `c::sysinfo` has a
        // computed-size padding field at the end that bindgen doesn't support,
        // and `c::sysinfo` may add fields over time.
        assert_eq!(
            core::mem::align_of::<Sysinfo>(),
            core::mem::align_of::<c::sysinfo>()
        );
        check_renamed_struct_field!(Sysinfo, sysinfo, uptime);
        check_renamed_struct_field!(Sysinfo, sysinfo, loads);
        check_renamed_struct_field!(Sysinfo, sysinfo, totalram);
        check_renamed_struct_field!(Sysinfo, sysinfo, freeram);
        check_renamed_struct_field!(Sysinfo, sysinfo, sharedram);
        check_renamed_struct_field!(Sysinfo, sysinfo, bufferram);
        check_renamed_struct_field!(Sysinfo, sysinfo, totalswap);
        check_renamed_struct_field!(Sysinfo, sysinfo, freeswap);
        check_renamed_struct_field!(Sysinfo, sysinfo, procs);
        check_renamed_struct_field!(Sysinfo, sysinfo, totalhigh);
        check_renamed_struct_field!(Sysinfo, sysinfo, freehigh);
        check_renamed_struct_field!(Sysinfo, sysinfo, mem_unit);
    }
}
