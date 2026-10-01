//! Authenticate implicitly discovered Windows socket peers before sending data.
//! The socket's kernel-reported PID, rather than a PID file, identifies the token.

use std::io;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::os::windows::io::RawSocket;
use std::ptr;

use windows_sys::Win32::Networking::WinSock::SIO_AF_UNIX_GETPEERPID;
use windows_sys::Win32::Networking::WinSock::SOCKET_ERROR;
use windows_sys::Win32::Networking::WinSock::WSAGetLastError;
use windows_sys::Win32::Networking::WinSock::WSAIoctl;
use windows_sys::Win32::Security::EqualSid;
use windows_sys::Win32::Security::GetTokenInformation;
use windows_sys::Win32::Security::TOKEN_ELEVATION;
use windows_sys::Win32::Security::TOKEN_QUERY;
use windows_sys::Win32::Security::TOKEN_USER;
use windows_sys::Win32::Security::TokenElevation;
use windows_sys::Win32::System::Threading::GetCurrentProcess;
use windows_sys::Win32::System::Threading::OpenProcess;
use windows_sys::Win32::System::Threading::OpenProcessToken;
use windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;

pub(super) fn ensure_non_elevated_peer(socket: RawSocket) -> io::Result<()> {
    let mut pid = 0u32;
    let mut returned = 0;
    if unsafe {
        WSAIoctl(
            socket as _,
            SIO_AF_UNIX_GETPEERPID,
            ptr::null(),
            /*cbinbuffer*/ 0,
            ptr::addr_of_mut!(pid).cast(),
            std::mem::size_of_val(&pid) as u32,
            &mut returned,
            ptr::null_mut(),
            /*lpcompletionroutine*/ None,
        )
    } == SOCKET_ERROR
    {
        return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
    }
    // Older Windows versions can return a valid PID with zero bytes reported.
    if pid == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket peer PID unavailable",
        ));
    }
    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            /*binherithandle*/ 0,
            pid,
        )
    };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let _process = unsafe { OwnedHandle::from_raw_handle(process) };
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut current_token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut current_token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let _current_token = unsafe { OwnedHandle::from_raw_handle(current_token) };
    let current_user = crate::windows_security::token_user(current_token)?;
    let peer_user = crate::windows_security::token_user(token)?;
    let same_user = unsafe {
        EqualSid(
            (*(current_user.as_ptr().cast::<TOKEN_USER>())).User.Sid,
            (*(peer_user.as_ptr().cast::<TOKEN_USER>())).User.Sid,
        ) != 0
    };
    // Implicit reuse must not change either side's elevation context.
    for token in [current_token, token] {
        let mut elevation: TOKEN_ELEVATION = unsafe { std::mem::zeroed() };
        if unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                ptr::addr_of_mut!(elevation).cast(),
                std::mem::size_of_val(&elevation) as u32,
                &mut returned,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if !same_user || elevation.TokenIsElevated != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "implicit daemon connection requires non-elevated current-user tokens",
            ));
        }
    }
    Ok(())
}
