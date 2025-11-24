//! is-terminal is a simple utility that answers one question:
//!
//! > Is this a terminal?
//!
//! A "terminal", also known as a "tty", is an I/O device which may be
//! interactive and may support color and other special features. This crate
//! doesn't provide any of those features; it just answers this one question.
//!
//! On Unix-family platforms, this is effectively the same as the [`isatty`]
//! function for testing whether a given stream is a terminal, though it
//! accepts high-level stream types instead of raw file descriptors.
//!
//! On Windows, it uses a variety of techniques to determine whether the
//! given stream is a terminal.
//!
//! # Example
//!
//! ```rust
//! use is_terminal::IsTerminal;
//!
//! if std::io::stdout().is_terminal() {
//!     println!("stdout is a terminal")
//! }
//! ```
//!
//! [`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html

#![cfg_attr(
    not(any(
        unix,
        windows,
        target_os = "wasi",
        target_os = "hermit",
        target_os = "unknown"
    )),
    no_std
)]

#[cfg(target_os = "wasi")]
use std::os::fd::{AsFd, AsRawFd};
#[cfg(target_os = "hermit")]
use std::os::hermit::io::AsFd;
#[cfg(unix)]
use std::os::unix::io::{AsFd, AsRawFd};
#[cfg(windows)]
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle};
#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;

/// Extension trait to check whether something is a terminal.
pub trait IsTerminal {
    /// Returns true if this is a terminal.
    ///
    /// # Example
    ///
    /// ```
    /// use is_terminal::IsTerminal;
    ///
    /// if std::io::stdout().is_terminal() {
    ///     println!("stdout is a terminal")
    /// }
    /// ```
    fn is_terminal(&self) -> bool;
}

/// Returns `true` if `this` is a terminal.
///
/// This is equivalent to calling `this.is_terminal()` and exists only as a
/// convenience to calling the trait method [`IsTerminal::is_terminal`]
/// without importing the trait.
///
/// # Example
///
/// ```
/// if is_terminal::is_terminal(&std::io::stdout()) {
///     println!("stdout is a terminal")
/// }
/// ```
pub fn is_terminal<T: IsTerminal>(this: T) -> bool {
    this.is_terminal()
}

#[cfg(not(any(windows, target_os = "unknown")))]
impl<Stream: AsFd> IsTerminal for Stream {
    #[inline]
    fn is_terminal(&self) -> bool {
        #[cfg(any(unix, target_os = "wasi"))]
        {
            let fd = self.as_fd();
            unsafe { libc::isatty(fd.as_raw_fd()) != 0 }
        }

        #[cfg(target_os = "hermit")]
        {
            use std::os::hermit::io::AsRawFd;
            hermit_abi::isatty(self.as_fd().as_fd().as_raw_fd())
        }
    }
}

#[cfg(windows)]
impl<Stream: AsHandle> IsTerminal for Stream {
    #[inline]
    fn is_terminal(&self) -> bool {
        handle_is_console(self.as_handle())
    }
}

// The Windows implementation here is copied from `handle_is_console` in
// library/std/src/sys/pal/windows/io.rs in Rust at revision
// e74c667a53c6368579867a74494e6fb7a7f17d13.

#[cfg(windows)]
fn handle_is_console(handle: BorrowedHandle<'_>) -> bool {
    use windows_sys::Win32::System::Console::GetConsoleMode;

    let handle = handle.as_raw_handle();

    // A null handle means the process has no console.
    if handle.is_null() {
        return false;
    }

    unsafe {
        let mut out = 0;
        if GetConsoleMode(handle as HANDLE, &mut out) != 0 {
            // False positives aren't possible. If we got a console then we definitely have a console.
            return true;
        }

        // Otherwise, we fall back to an msys hack to see if we can detect the presence of a pty.
        msys_tty_on(handle as HANDLE)
    }
}

/// Returns true if there is an MSYS tty on the given handle.
#[cfg(windows)]
unsafe fn msys_tty_on(handle: HANDLE) -> bool {
    use std::ffi::c_void;
    use windows_sys::Win32::{
        Foundation::MAX_PATH,
        Storage::FileSystem::{
            FileNameInfo, GetFileInformationByHandleEx, GetFileType, FILE_TYPE_PIPE,
        },
    };

    // Early return if the handle is not a pipe.
    if GetFileType(handle) != FILE_TYPE_PIPE {
        return false;
    }

    /// Mirrors windows_sys::Win32::Storage::FileSystem::FILE_NAME_INFO, giving
    /// it a fixed length that we can stack allocate
    #[repr(C)]
    #[allow(non_snake_case)]
    struct FILE_NAME_INFO {
        FileNameLength: u32,
        FileName: [u16; MAX_PATH as usize],
    }
    let mut name_info = FILE_NAME_INFO {
        FileNameLength: 0,
        FileName: [0; MAX_PATH as usize],
    };
    // Safety: buffer length is fixed.
    let res = GetFileInformationByHandleEx(
        handle,
        FileNameInfo,
        &mut name_info as *mut _ as *mut c_void,
        std::mem::size_of::<FILE_NAME_INFO>() as u32,
    );
    if res == 0 {
        return false;
    }

    // Use `get` because `FileNameLength` can be out of range.
    let s = match name_info
        .FileName
        .get(..name_info.FileNameLength as usize / 2)
    {
        None => return false,
        Some(s) => s,
    };
    let name = String::from_utf16_lossy(s);
    // Get the file name only.
    let name = name.rsplit('\\').next().unwrap_or(&name);
    // This checks whether 'pty' exists in the file name, which indicates that
    // a pseudo-terminal is attached. To mitigate against false positives
    // (e.g., an actual file name that contains 'pty'), we also require that
    // the file name begins with either the strings 'msys-' or 'cygwin-'.)
    let is_msys = name.starts_with("msys-") || name.starts_with("cygwin-");
    let is_pty = name.contains("-pty");
    is_msys && is_pty
}

#[cfg(target_os = "unknown")]
impl IsTerminal for std::io::Stdin {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl IsTerminal for std::io::Stdout {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl IsTerminal for std::io::Stderr {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl<'a> IsTerminal for std::io::StdinLock<'a> {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl<'a> IsTerminal for std::io::StdoutLock<'a> {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl<'a> IsTerminal for std::io::StderrLock<'a> {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl<'a> IsTerminal for std::fs::File {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl IsTerminal for std::process::ChildStdin {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl IsTerminal for std::process::ChildStdout {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(target_os = "unknown")]
impl IsTerminal for std::process::ChildStderr {
    #[inline]
    fn is_terminal(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "unknown"))]
    use super::IsTerminal;

    #[test]
    #[cfg(windows)]
    fn stdin() {
        assert_eq!(
            atty::is(atty::Stream::Stdin),
            std::io::stdin().is_terminal()
        )
    }

    #[test]
    #[cfg(windows)]
    fn stdout() {
        assert_eq!(
            atty::is(atty::Stream::Stdout),
            std::io::stdout().is_terminal()
        )
    }

    #[test]
    #[cfg(windows)]
    fn stderr() {
        assert_eq!(
            atty::is(atty::Stream::Stderr),
            std::io::stderr().is_terminal()
        )
    }

    #[test]
    #[cfg(any(unix, target_os = "wasi"))]
    fn stdin() {
        assert_eq!(
            atty::is(atty::Stream::Stdin),
            rustix::stdio::stdin().is_terminal()
        )
    }

    #[test]
    #[cfg(any(unix, target_os = "wasi"))]
    fn stdout() {
        assert_eq!(
            atty::is(atty::Stream::Stdout),
            rustix::stdio::stdout().is_terminal()
        )
    }

    #[test]
    #[cfg(any(unix, target_os = "wasi"))]
    fn stderr() {
        assert_eq!(
            atty::is(atty::Stream::Stderr),
            rustix::stdio::stderr().is_terminal()
        )
    }

    #[test]
    #[cfg(any(unix, target_os = "wasi"))]
    fn stdin_vs_libc() {
        unsafe {
            assert_eq!(
                libc::isatty(libc::STDIN_FILENO) != 0,
                rustix::stdio::stdin().is_terminal()
            )
        }
    }

    #[test]
    #[cfg(any(unix, target_os = "wasi"))]
    fn stdout_vs_libc() {
        unsafe {
            assert_eq!(
                libc::isatty(libc::STDOUT_FILENO) != 0,
                rustix::stdio::stdout().is_terminal()
            )
        }
    }

    #[test]
    #[cfg(any(unix, target_os = "wasi"))]
    fn stderr_vs_libc() {
        unsafe {
            assert_eq!(
                libc::isatty(libc::STDERR_FILENO) != 0,
                rustix::stdio::stderr().is_terminal()
            )
        }
    }

    // Verify that the msys_tty_on function works with long path.
    #[test]
    #[cfg(windows)]
    fn msys_tty_on_path_length() {
        use std::{fs::File, os::windows::io::AsRawHandle};
        use windows_sys::Win32::Foundation::MAX_PATH;

        let dir = tempfile::tempdir().expect("Unable to create temporary directory");
        let file_path = dir.path().join("ten_chars_".repeat(25));
        // Ensure that the path is longer than MAX_PATH.
        assert!(file_path.to_string_lossy().len() > MAX_PATH as usize);
        let file = File::create(file_path).expect("Unable to create file");

        assert!(!unsafe { crate::msys_tty_on(file.as_raw_handle()) });
    }
}
