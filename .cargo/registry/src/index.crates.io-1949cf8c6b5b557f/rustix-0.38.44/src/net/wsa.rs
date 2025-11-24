use crate::io;
use core::mem::MaybeUninit;
use windows_sys::Win32::Networking::WinSock::{WSACleanup, WSAGetLastError, WSAStartup, WSADATA};

/// `WSAStartup()`—Initialize process-wide Windows support for sockets.
///
/// On Windows, it's necessary to initialize the sockets subsystem before
/// using sockets APIs. The function performs the necessary initialization.
///
/// # References
///  - [Winsock]
///
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsastartup
pub fn wsa_startup() -> io::Result<WSADATA> {
    // Request version 2.2, which has been the latest version since far older
    // versions of Windows than we support here. For more information about
    // the version, see [here].
    //
    // [here]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsastartup#remarks
    let version = 0x202;
    let mut data = MaybeUninit::uninit();
    unsafe {
        let ret = WSAStartup(version, data.as_mut_ptr());
        if ret == 0 {
            Ok(data.assume_init())
        } else {
            Err(io::Errno::from_raw_os_error(WSAGetLastError()))
        }
    }
}

/// `WSACleanup()`—Clean up process-wide Windows support for sockets.
///
/// In a program where `init` is called, if sockets are no longer necessary,
/// this function releases associated resources.
///
/// # References
///  - [Winsock]
///
/// [Winsock]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsacleanup
pub fn wsa_cleanup() -> io::Result<()> {
    unsafe {
        if WSACleanup() == 0 {
            Ok(())
        } else {
            Err(io::Errno::from_raw_os_error(WSAGetLastError()))
        }
    }
}
