//! Load and unload kernel modules.
//!
//! For more details see

use std::ffi::CStr;
use std::os::unix::io::{AsFd, AsRawFd};

use crate::errno::Errno;
use crate::Result;

/// Loads a kernel module from a buffer.
///
/// It loads an ELF image into kernel space,
/// performs any necessary symbol relocations,
/// initializes module parameters to values provided by the caller,
/// and then runs the module's init function.
///
/// This function requires `CAP_SYS_MODULE` privilege.
///
/// The `module_image` argument points to a buffer containing the binary image
/// to be loaded. The buffer should contain a valid ELF image
/// built for the running kernel.
///
/// The `param_values` argument is a string containing space-delimited specifications
/// of the values for module parameters.
/// Each of the parameter specifications has the form:
///
/// `name[=value[,value...]]`
///
/// # Example
///
/// ```no_run
/// use std::fs::File;
/// use std::io::Read;
/// use std::ffi::CString;
/// use nix::kmod::init_module;
///
/// let mut f = File::open("mykernel.ko").unwrap();
/// let mut contents: Vec<u8> = Vec::new();
/// f.read_to_end(&mut contents).unwrap();
/// init_module(&mut contents, &CString::new("who=Rust when=Now,12").unwrap()).unwrap();
/// ```
///
/// See [`man init_module(2)`](https://man7.org/linux/man-pages/man2/init_module.2.html) for more information.
pub fn init_module(module_image: &[u8], param_values: &CStr) -> Result<()> {
    let res = unsafe {
        libc::syscall(
            libc::SYS_init_module,
            module_image.as_ptr(),
            module_image.len(),
            param_values.as_ptr(),
        )
    };

    Errno::result(res).map(drop)
}

libc_bitflags!(
    /// Flags used by the `finit_module` function.
    pub struct ModuleInitFlags: libc::c_uint {
        /// Ignore symbol version hashes.
        MODULE_INIT_IGNORE_MODVERSIONS;
        /// Ignore kernel version magic.
        MODULE_INIT_IGNORE_VERMAGIC;
    }
);

/// Loads a kernel module from a given file descriptor.
///
/// # Example
///
/// ```no_run
/// use std::fs::File;
/// use std::ffi::CString;
/// use nix::kmod::{finit_module, ModuleInitFlags};
///
/// let f = File::open("mymod.ko").unwrap();
/// finit_module(&f, &CString::new("").unwrap(), ModuleInitFlags::empty()).unwrap();
/// ```
///
/// See [`man init_module(2)`](https://man7.org/linux/man-pages/man2/init_module.2.html) for more information.
pub fn finit_module<Fd: AsFd>(
    fd: Fd,
    param_values: &CStr,
    flags: ModuleInitFlags,
) -> Result<()> {
    let res = unsafe {
        libc::syscall(
            libc::SYS_finit_module,
            fd.as_fd().as_raw_fd(),
            param_values.as_ptr(),
            flags.bits(),
        )
    };

    Errno::result(res).map(drop)
}

libc_bitflags!(
    /// Flags used by `delete_module`.
    ///
    /// See [`man delete_module(2)`](https://man7.org/linux/man-pages/man2/delete_module.2.html)
    /// for a detailed description how these flags work.
    pub struct DeleteModuleFlags: libc::c_int {
        /// `delete_module` will return immediately, with an error, if the module has a nonzero
        /// reference count.
        O_NONBLOCK;
        /// `delete_module` will unload the module immediately, regardless of whether it has a
        /// nonzero reference count.
        O_TRUNC;
    }
);

/// Unloads the kernel module with the given name.
///
/// # Example
///
/// ```no_run
/// use std::ffi::CString;
/// use nix::kmod::{delete_module, DeleteModuleFlags};
///
/// delete_module(&CString::new("mymod").unwrap(), DeleteModuleFlags::O_NONBLOCK).unwrap();
/// ```
///
/// See [`man delete_module(2)`](https://man7.org/linux/man-pages/man2/delete_module.2.html) for more information.
pub fn delete_module(name: &CStr, flags: DeleteModuleFlags) -> Result<()> {
    let res = unsafe {
        libc::syscall(libc::SYS_delete_module, name.as_ptr(), flags.bits())
    };

    Errno::result(res).map(drop)
}
