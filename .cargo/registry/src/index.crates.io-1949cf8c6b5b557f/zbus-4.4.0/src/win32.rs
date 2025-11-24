use std::{
    ffi::{CStr, OsStr},
    io::{Error, ErrorKind},
    net::SocketAddr,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
        prelude::OsStrExt,
    },
    ptr,
};

use windows_sys::Win32::{
    Foundation::{
        LocalFree, ERROR_INSUFFICIENT_BUFFER, FALSE, HANDLE, NO_ERROR, WAIT_ABANDONED,
        WAIT_OBJECT_0,
    },
    NetworkManagement::IpHelper::{GetTcpTable2, MIB_TCPTABLE2, MIB_TCP_STATE_ESTAB},
    Networking::WinSock::INADDR_LOOPBACK,
    Security::{
        Authorization::ConvertSidToStringSidA, GetTokenInformation, IsValidSid, TokenUser,
        TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        Memory::{MapViewOfFile, OpenFileMappingW, FILE_MAP_READ},
        Threading::{
            CreateMutexW, GetCurrentProcess, OpenProcess, OpenProcessToken, ReleaseMutex,
            WaitForSingleObject, INFINITE, PROCESS_ACCESS_RIGHTS,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

use crate::Address;
#[cfg(not(feature = "tokio"))]
use uds_windows::UnixStream;

struct Mutex(OwnedHandle);

impl Mutex {
    pub fn new(name: &str) -> Result<Self, crate::Error> {
        let name_wide = OsStr::new(name)
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();

        let handle = unsafe { CreateMutexW(ptr::null_mut(), FALSE, name_wide.as_ptr()) };

        // SAFETY: We have exclusive ownership over the mutex handle
        Ok(Self(unsafe {
            OwnedHandle::from_raw_handle(handle as RawHandle)
        }))
    }

    pub fn lock(&self) -> MutexGuard<'_> {
        match unsafe { WaitForSingleObject(self.0.as_raw_handle() as isize, INFINITE) } {
            WAIT_ABANDONED | WAIT_OBJECT_0 => MutexGuard(self),
            err => panic!("WaitForSingleObject() failed: return code {}", err),
        }
    }
}

struct MutexGuard<'a>(&'a Mutex);

impl Drop for MutexGuard<'_> {
    fn drop(&mut self) {
        unsafe { ReleaseMutex(self.0 .0.as_raw_handle() as isize) };
    }
}

// A process handle
pub struct ProcessHandle(OwnedHandle);

impl ProcessHandle {
    // Open the process associated with the process_id (if None, the current process)
    pub fn open(
        process_id: Option<u32>,
        desired_access: PROCESS_ACCESS_RIGHTS,
    ) -> Result<Self, Error> {
        let process = if let Some(process_id) = process_id {
            unsafe { OpenProcess(desired_access, false.into(), process_id) }
        } else {
            unsafe { GetCurrentProcess() }
        };

        if process == 0 {
            Err(Error::last_os_error())
        } else {
            // SAFETY: We have exclusive ownership over the process handle
            Ok(Self(unsafe {
                OwnedHandle::from_raw_handle(process as RawHandle)
            }))
        }
    }
}

// A process token
//
// See MSDN documentation:
// https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocesstoken
//
// Get the process security identifier with the `sid()` function.
pub struct ProcessToken(OwnedHandle);

impl ProcessToken {
    // Open the access token associated with the process_id (if None, the current process)
    pub fn open(process_id: Option<u32>) -> Result<Self, Error> {
        let mut process_token: HANDLE = HANDLE::default();
        let process = ProcessHandle::open(process_id, PROCESS_QUERY_LIMITED_INFORMATION)?;

        if unsafe {
            OpenProcessToken(
                process.0.as_raw_handle() as isize,
                TOKEN_QUERY,
                ptr::addr_of_mut!(process_token),
            ) == 0
        } {
            Err(Error::last_os_error())
        } else {
            // SAFETY: We have exclusive ownership over the process handle
            Ok(Self(unsafe {
                OwnedHandle::from_raw_handle(process_token as RawHandle)
            }))
        }
    }

    // Return the process SID (security identifier) as a string
    pub fn sid(&self) -> Result<String, Error> {
        let mut len = 256;
        let mut token_info;

        loop {
            token_info = vec![0u8; len as usize];

            let result = unsafe {
                GetTokenInformation(
                    self.0.as_raw_handle() as isize,
                    TokenUser,
                    token_info.as_mut_ptr().cast(),
                    len,
                    ptr::addr_of_mut!(len),
                )
            };

            if result != 0 {
                break;
            }

            let last_error = Error::last_os_error();
            if last_error.raw_os_error() == Some(ERROR_INSUFFICIENT_BUFFER as i32) {
                continue;
            }

            return Err(last_error);
        }

        let sid = unsafe { (*token_info.as_ptr().cast::<TOKEN_USER>()).User.Sid };

        if unsafe { IsValidSid(sid.cast()) == FALSE } {
            return Err(Error::new(ErrorKind::Other, "Invalid SID"));
        }

        let mut pstr = ptr::null_mut();
        if unsafe { ConvertSidToStringSidA(sid, ptr::addr_of_mut!(pstr)) == 0 } {
            return Err(Error::last_os_error());
        }

        let sid = unsafe { CStr::from_ptr(pstr.cast()) };
        let ret = sid
            .to_str()
            .map_err(|_| Error::new(ErrorKind::Other, "Invalid SID"))?
            .to_owned();
        unsafe {
            LocalFree(pstr.cast());
        }

        Ok(ret)
    }
}

// Get the process ID of the local socket address
// TODO: add ipv6 support
pub fn socket_addr_get_pid(addr: &SocketAddr) -> Result<u32, Error> {
    let mut len = 4096;
    let mut tcp_table = vec![];
    let res = loop {
        tcp_table.resize(len as usize, 0);
        let res = unsafe {
            GetTcpTable2(
                tcp_table.as_mut_ptr().cast::<MIB_TCPTABLE2>(),
                ptr::addr_of_mut!(len),
                0,
            )
        };
        if res != ERROR_INSUFFICIENT_BUFFER {
            break res;
        }
    };
    if res != NO_ERROR {
        return Err(Error::last_os_error());
    }

    let tcp_table = tcp_table.as_mut_ptr().cast::<MIB_TCPTABLE2>();
    let entries = unsafe {
        std::slice::from_raw_parts(
            (*tcp_table).table.as_ptr(),
            (*tcp_table).dwNumEntries as usize,
        )
    };
    for entry in entries {
        let port = (entry.dwLocalPort & 0xFFFF) as u16;
        let port = u16::from_be(port);

        if entry.dwState == MIB_TCP_STATE_ESTAB as u32
            && u32::from_be(entry.dwLocalAddr) == INADDR_LOOPBACK
            && u32::from_be(entry.dwRemoteAddr) == INADDR_LOOPBACK
            && port == addr.port()
        {
            return Ok(entry.dwOwningPid);
        }
    }

    Err(Error::new(ErrorKind::Other, "PID of TCP address not found"))
}

// Get the process ID of the connected peer
#[cfg(any(test, not(feature = "tokio")))]
pub fn tcp_stream_get_peer_pid(stream: &std::net::TcpStream) -> Result<u32, Error> {
    let peer_addr = stream.peer_addr()?;

    socket_addr_get_pid(&peer_addr)
}

#[cfg(not(feature = "tokio"))]
fn last_err() -> std::io::Error {
    use windows_sys::Win32::Networking::WinSock::WSAGetLastError;

    let err = unsafe { WSAGetLastError() };
    std::io::Error::from_raw_os_error(err)
}

// Get the process ID of the connected peer
#[cfg(not(feature = "tokio"))]
pub fn unix_stream_get_peer_pid(stream: &UnixStream) -> Result<u32, Error> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{WSAIoctl, IOC_OUT, IOC_VENDOR, SOCKET_ERROR};

    macro_rules! _WSAIOR {
        ($x:expr, $y:expr) => {
            IOC_OUT | $x | $y
        };
    }

    let socket = stream.as_raw_socket();
    const SIO_AF_UNIX_GETPEERPID: u32 = _WSAIOR!(IOC_VENDOR, 256);
    let mut ret = 0;
    let mut bytes = 0;

    let r = unsafe {
        WSAIoctl(
            socket as _,
            SIO_AF_UNIX_GETPEERPID,
            ptr::null_mut(),
            0,
            ptr::addr_of_mut!(ret).cast(),
            std::mem::size_of_val(&ret) as u32,
            ptr::addr_of_mut!(bytes),
            ptr::null_mut(),
            None,
        )
    };

    if r == SOCKET_ERROR {
        return Err(last_err());
    }

    Ok(ret)
}

fn read_shm(name: &str) -> Result<Vec<u8>, crate::Error> {
    let handle = {
        let wide_name = OsStr::new(name)
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();

        let res = unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, wide_name.as_ptr()) };

        if res != 0 {
            // SAFETY: We have exclusive ownership over the file mapping handle
            unsafe { OwnedHandle::from_raw_handle(res as RawHandle) }
        } else {
            return Err(crate::Error::Address(
                "Unable to open shared memory".to_owned(),
            ));
        }
    };

    let addr = unsafe { MapViewOfFile(handle.as_raw_handle() as isize, FILE_MAP_READ, 0, 0, 0) };

    if addr.Value.is_null() {
        return Err(crate::Error::Address("MapViewOfFile() failed".to_owned()));
    }

    let data = unsafe { CStr::from_ptr(addr.Value.cast()) };
    Ok(data.to_bytes().to_owned())
}

pub fn autolaunch_bus_address() -> Result<Address, crate::Error> {
    let mutex = Mutex::new("DBusAutolaunchMutex")?;
    let _guard = mutex.lock();

    let addr = read_shm("DBusDaemonAddressInfo")?;
    let addr = String::from_utf8(addr)
        .map_err(|e| crate::Error::Address(format!("Unable to parse address as UTF-8: {}", e)))?;

    addr.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_pid_and_sid() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = std::net::TcpStream::connect(addr).unwrap();
        let _server = listener.incoming().next().unwrap().unwrap();

        let pid = tcp_stream_get_peer_pid(&client).unwrap();
        let process_token = ProcessToken::open(if pid != 0 { Some(pid) } else { None }).unwrap();
        let sid = process_token.sid().unwrap();
        assert!(!sid.is_empty());
    }
}
