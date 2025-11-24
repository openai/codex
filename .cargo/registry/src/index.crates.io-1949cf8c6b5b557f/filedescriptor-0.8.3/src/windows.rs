use crate::{
    AsRawFileDescriptor, AsRawSocketDescriptor, Error, FileDescriptor, FromRawFileDescriptor,
    FromRawSocketDescriptor, IntoRawFileDescriptor, IntoRawSocketDescriptor, OwnedHandle, Pipe,
    Result, StdioDescriptor,
};
use std::io::{self, Error as IoError};
use std::os::windows::prelude::*;
use std::ptr;
use std::sync::Once;
use std::time::Duration;
use winapi::shared::ws2def::{AF_INET, INADDR_LOOPBACK, SOCKADDR_IN};
use winapi::um::fileapi::*;
use winapi::um::handleapi::*;
use winapi::um::minwinbase::SECURITY_ATTRIBUTES;
use winapi::um::namedpipeapi::{CreatePipe, GetNamedPipeInfo};
use winapi::um::processenv::{GetStdHandle, SetStdHandle};
use winapi::um::processthreadsapi::*;
use winapi::um::winbase::{FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE};
use winapi::um::winnt::HANDLE;
use winapi::um::winsock2::{
    accept, bind, closesocket, connect, getsockname, getsockopt, htonl, ioctlsocket, listen, recv,
    send, WSAGetLastError, WSAPoll, WSASocketW, WSAStartup, INVALID_SOCKET, SOCKET, SOCK_STREAM,
    SOL_SOCKET, SO_ERROR, WSADATA, WSAENOTSOCK, WSA_FLAG_NO_HANDLE_INHERIT,
};
pub use winapi::um::winsock2::{POLLERR, POLLHUP, POLLIN, POLLOUT, WSAPOLLFD as pollfd};

/// `RawFileDescriptor` is a platform independent type alias for the
/// underlying platform file descriptor type.  It is primarily useful
/// for avoiding using `cfg` blocks in platform independent code.
pub type RawFileDescriptor = RawHandle;

/// `SocketDescriptor` is a platform independent type alias for the
/// underlying platform socket descriptor type.  It is primarily useful
/// for avoiding using `cfg` blocks in platform independent code.
pub type SocketDescriptor = SOCKET;

const STD_INPUT_HANDLE: u32 = 4294967286; // -10
const STD_OUTPUT_HANDLE: u32 = 4294967285; // -11
const STD_ERROR_HANDLE: u32 = 4294967284; // -12

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HandleType {
    Char,
    Disk,
    Pipe,
    Socket,
    Unknown,
}

impl Default for HandleType {
    fn default() -> Self {
        HandleType::Unknown
    }
}

impl<T: AsRawHandle> AsRawFileDescriptor for T {
    fn as_raw_file_descriptor(&self) -> RawFileDescriptor {
        self.as_raw_handle()
    }
}

impl<T: IntoRawHandle> IntoRawFileDescriptor for T {
    fn into_raw_file_descriptor(self) -> RawFileDescriptor {
        self.into_raw_handle()
    }
}

impl<T: FromRawHandle> FromRawFileDescriptor for T {
    unsafe fn from_raw_file_descriptor(handle: RawHandle) -> Self {
        Self::from_raw_handle(handle)
    }
}

impl<T: AsRawSocket> AsRawSocketDescriptor for T {
    fn as_socket_descriptor(&self) -> SocketDescriptor {
        self.as_raw_socket() as SocketDescriptor
    }
}

impl<T: IntoRawSocket> IntoRawSocketDescriptor for T {
    fn into_socket_descriptor(self) -> SocketDescriptor {
        self.into_raw_socket() as SocketDescriptor
    }
}

impl<T: FromRawSocket> FromRawSocketDescriptor for T {
    unsafe fn from_socket_descriptor(handle: SocketDescriptor) -> Self {
        Self::from_raw_socket(handle as _)
    }
}

unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    fn probe_handle_type_if_unknown(handle: RawHandle, handle_type: HandleType) -> HandleType {
        match handle_type {
            HandleType::Unknown => Self::probe_handle_type(handle),
            t => t,
        }
    }

    pub(crate) fn probe_handle_type(handle: RawHandle) -> HandleType {
        let handle = handle as HANDLE;
        match unsafe { GetFileType(handle) } {
            FILE_TYPE_CHAR => HandleType::Char,
            FILE_TYPE_DISK => HandleType::Disk,
            FILE_TYPE_PIPE => {
                // Could be a pipe or a socket.  Test if for pipeness
                let mut flags = 0;
                let mut out_buf = 0;
                let mut in_buf = 0;
                let mut inst = 0;
                if unsafe {
                    GetNamedPipeInfo(handle, &mut flags, &mut out_buf, &mut in_buf, &mut inst)
                } != 0
                {
                    HandleType::Pipe
                } else {
                    // It's probably a socket, but it may be a special device used
                    // when piping between WSL and native win32 apps.
                    let mut err = 0;
                    let mut errsize = std::mem::size_of_val(&err) as _;
                    if unsafe {
                        getsockopt(
                            handle as _,
                            SOL_SOCKET,
                            SO_ERROR,
                            &mut err as *mut _ as *mut i8,
                            &mut errsize,
                        ) != 0
                            && WSAGetLastError() == WSAENOTSOCK
                    } {
                        HandleType::Pipe
                    } else {
                        HandleType::Socket
                    }
                }
            }
            _ => HandleType::Unknown,
        }
    }

    fn is_socket_handle(&self) -> bool {
        match self.handle_type {
            HandleType::Socket => true,
            HandleType::Unknown => Self::probe_handle_type(self.handle) == HandleType::Socket,
            _ => false,
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.handle != INVALID_HANDLE_VALUE as _ && !self.handle.is_null() {
            unsafe {
                if self.is_socket_handle() {
                    closesocket(self.handle as _);
                } else {
                    CloseHandle(self.handle as _);
                }
            };
        }
    }
}

impl FromRawHandle for OwnedHandle {
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        OwnedHandle {
            handle,
            handle_type: Self::probe_handle_type(handle),
        }
    }
}

impl OwnedHandle {
    #[inline]
    pub(crate) fn dup_impl<F: AsRawFileDescriptor>(f: &F, handle_type: HandleType) -> Result<Self> {
        let handle = f.as_raw_file_descriptor();
        if handle == INVALID_HANDLE_VALUE as _ || handle.is_null() {
            return Ok(OwnedHandle {
                handle,
                handle_type,
            });
        }

        let handle_type = Self::probe_handle_type_if_unknown(handle, handle_type);

        let proc = unsafe { GetCurrentProcess() };
        let mut duped = INVALID_HANDLE_VALUE;
        let ok = unsafe {
            DuplicateHandle(
                proc,
                handle as *mut _,
                proc,
                &mut duped,
                0,
                0, // not inheritable
                winapi::um::winnt::DUPLICATE_SAME_ACCESS,
            )
        };
        if ok == 0 {
            Err(IoError::last_os_error().into())
        } else {
            Ok(OwnedHandle {
                handle: duped as *mut _,
                handle_type,
            })
        }
    }
}

impl AsRawHandle for OwnedHandle {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle
    }
}

impl IntoRawHandle for OwnedHandle {
    fn into_raw_handle(self) -> RawHandle {
        let handle = self.handle;
        std::mem::forget(self);
        handle
    }
}

impl FileDescriptor {
    #[inline]
    pub(crate) fn as_stdio_impl(&self) -> Result<std::process::Stdio> {
        let duped = self.handle.try_clone()?;
        let handle = duped.into_raw_handle();
        let stdio = unsafe { std::process::Stdio::from_raw_handle(handle) };
        Ok(stdio)
    }

    #[inline]
    pub(crate) fn as_file_impl(&self) -> Result<std::fs::File> {
        let duped = self.handle.try_clone()?;
        let handle = duped.into_raw_handle();
        let stdio = unsafe { std::fs::File::from_raw_handle(handle) };
        Ok(stdio)
    }

    #[inline]
    pub(crate) fn set_non_blocking_impl(&mut self, non_blocking: bool) -> Result<()> {
        if !self.handle.is_socket_handle() {
            return Err(Error::OnlySocketsNonBlocking);
        }

        let mut on = if non_blocking { 1 } else { 0 };
        let res = unsafe {
            ioctlsocket(
                self.as_raw_socket() as SOCKET,
                winapi::um::winsock2::FIONBIO,
                &mut on,
            )
        };
        if res != 0 {
            Err(Error::FionBio(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }

    pub(crate) fn redirect_stdio_impl<F: AsRawFileDescriptor>(
        f: &F,
        stdio: StdioDescriptor,
    ) -> Result<Self> {
        let std_handle = match stdio {
            StdioDescriptor::Stdin => STD_INPUT_HANDLE,
            StdioDescriptor::Stdout => STD_OUTPUT_HANDLE,
            StdioDescriptor::Stderr => STD_ERROR_HANDLE,
        };

        let raw_std_handle = unsafe { GetStdHandle(std_handle) } as *mut _;
        let std_original = unsafe { FileDescriptor::from_raw_handle(raw_std_handle) };

        let cloned_handle = OwnedHandle::dup(f)?;
        if unsafe { SetStdHandle(std_handle, cloned_handle.into_raw_handle() as *mut _) } == 0 {
            Err(Error::SetStdHandle(std::io::Error::last_os_error()))
        } else {
            Ok(std_original)
        }
    }
}

impl IntoRawHandle for FileDescriptor {
    fn into_raw_handle(self) -> RawHandle {
        self.handle.into_raw_handle()
    }
}

impl AsRawHandle for FileDescriptor {
    fn as_raw_handle(&self) -> RawHandle {
        self.handle.as_raw_handle()
    }
}

impl FromRawHandle for FileDescriptor {
    unsafe fn from_raw_handle(handle: RawHandle) -> FileDescriptor {
        Self {
            handle: OwnedHandle::from_raw_handle(handle),
        }
    }
}

impl IntoRawSocket for FileDescriptor {
    fn into_raw_socket(self) -> RawSocket {
        // FIXME: this isn't a guaranteed conversion!
        debug_assert!(self.handle.is_socket_handle());
        self.handle.into_raw_handle() as RawSocket
    }
}

impl AsRawSocket for FileDescriptor {
    fn as_raw_socket(&self) -> RawSocket {
        // FIXME: this isn't a guaranteed conversion!
        debug_assert!(self.handle.is_socket_handle());
        self.handle.as_raw_handle() as RawSocket
    }
}

impl AsSocket for FileDescriptor {
    fn as_socket(&self) -> BorrowedSocket {
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

impl FromRawSocket for FileDescriptor {
    unsafe fn from_raw_socket(handle: RawSocket) -> FileDescriptor {
        Self {
            handle: OwnedHandle::from_raw_handle(handle as RawHandle),
        }
    }
}

impl io::Read for FileDescriptor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.handle.is_socket_handle() {
            // It's important to use the winsock functions to read/write
            // even though ReadFile and WriteFile technically work; only
            // the winsock functions respect non-blocking mode.
            let num_read = unsafe {
                recv(
                    self.as_socket_descriptor(),
                    buf.as_mut_ptr() as *mut _,
                    buf.len() as _,
                    0,
                )
            };
            if num_read < 0 {
                Err(IoError::last_os_error())
            } else {
                Ok(num_read as usize)
            }
        } else {
            let mut num_read = 0;
            let ok = unsafe {
                ReadFile(
                    self.handle.as_raw_handle() as *mut _,
                    buf.as_mut_ptr() as *mut _,
                    buf.len() as _,
                    &mut num_read,
                    ptr::null_mut(),
                )
            };
            if ok == 0 {
                let err = IoError::last_os_error();
                if err.kind() == std::io::ErrorKind::BrokenPipe {
                    Ok(0)
                } else {
                    Err(err)
                }
            } else {
                Ok(num_read as usize)
            }
        }
    }
}

impl io::Write for FileDescriptor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.handle.is_socket_handle() {
            let num_wrote = unsafe {
                send(
                    self.as_socket_descriptor(),
                    buf.as_ptr() as *const _,
                    buf.len() as _,
                    0,
                )
            };
            if num_wrote < 0 {
                Err(IoError::last_os_error())
            } else {
                Ok(num_wrote as usize)
            }
        } else {
            let mut num_wrote = 0;
            let ok = unsafe {
                WriteFile(
                    self.handle.as_raw_handle() as *mut _,
                    buf.as_ptr() as *const _,
                    buf.len() as u32,
                    &mut num_wrote,
                    ptr::null_mut(),
                )
            };
            if ok == 0 {
                Err(IoError::last_os_error())
            } else {
                Ok(num_wrote as usize)
            }
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Pipe {
    pub fn new() -> Result<Pipe> {
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: 0,
        };
        let mut read: HANDLE = INVALID_HANDLE_VALUE as _;
        let mut write: HANDLE = INVALID_HANDLE_VALUE as _;
        if unsafe { CreatePipe(&mut read, &mut write, &mut sa, 0) } == 0 {
            Err(Error::Pipe(IoError::last_os_error()))
        } else {
            Ok(Pipe {
                read: FileDescriptor {
                    handle: OwnedHandle {
                        handle: read as _,
                        handle_type: HandleType::Pipe,
                    },
                },
                write: FileDescriptor {
                    handle: OwnedHandle {
                        handle: write as _,
                        handle_type: HandleType::Pipe,
                    },
                },
            })
        }
    }
}

fn init_winsock() {
    static START: Once = Once::new();
    START.call_once(|| unsafe {
        let mut data: WSADATA = std::mem::zeroed();
        let ret = WSAStartup(
            0x202, // version 2.2
            &mut data,
        );
        assert_eq!(ret, 0, "failed to initialize winsock");
    });
}

fn socket(af: i32, sock_type: i32, proto: i32) -> Result<FileDescriptor> {
    let s = unsafe {
        WSASocketW(
            af,
            sock_type,
            proto,
            ptr::null_mut(),
            0,
            WSA_FLAG_NO_HANDLE_INHERIT,
        )
    };
    if s == INVALID_SOCKET {
        Err(Error::Socket(IoError::last_os_error()))
    } else {
        Ok(FileDescriptor {
            handle: OwnedHandle {
                handle: s as _,
                handle_type: HandleType::Socket,
            },
        })
    }
}

#[doc(hidden)]
pub fn socketpair_impl() -> Result<(FileDescriptor, FileDescriptor)> {
    init_winsock();

    let s = socket(AF_INET, SOCK_STREAM, 0)?;

    let mut in_addr: SOCKADDR_IN = unsafe { std::mem::zeroed() };
    in_addr.sin_family = AF_INET as _;
    unsafe {
        *in_addr.sin_addr.S_un.S_addr_mut() = htonl(INADDR_LOOPBACK);
    }

    unsafe {
        if bind(
            s.as_raw_handle() as _,
            std::mem::transmute(&in_addr),
            std::mem::size_of_val(&in_addr) as _,
        ) != 0
        {
            return Err(Error::Bind(IoError::last_os_error()));
        }
    }

    let mut addr_len = std::mem::size_of_val(&in_addr) as i32;

    unsafe {
        if getsockname(
            s.as_raw_handle() as _,
            std::mem::transmute(&mut in_addr),
            &mut addr_len,
        ) != 0
        {
            return Err(Error::Getsockname(IoError::last_os_error()));
        }
    }

    unsafe {
        if listen(s.as_raw_handle() as _, 1) != 0 {
            return Err(Error::Listen(IoError::last_os_error()));
        }
    }

    let client = socket(AF_INET, SOCK_STREAM, 0)?;

    unsafe {
        if connect(
            client.as_raw_handle() as _,
            std::mem::transmute(&in_addr),
            addr_len,
        ) != 0
        {
            return Err(Error::Connect(IoError::last_os_error()));
        }
    }

    let server = unsafe { accept(s.as_raw_handle() as _, ptr::null_mut(), ptr::null_mut()) };
    if server == INVALID_SOCKET {
        return Err(Error::Accept(IoError::last_os_error()));
    }
    let server = FileDescriptor {
        handle: OwnedHandle {
            handle: server as _,
            handle_type: HandleType::Socket,
        },
    };

    Ok((server, client))
}

#[doc(hidden)]
pub fn poll_impl(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize> {
    let poll_result = unsafe {
        WSAPoll(
            pfd.as_mut_ptr(),
            pfd.len() as _,
            duration
                .map(|wait| wait.as_millis() as libc::c_int)
                .unwrap_or(-1),
        )
    };
    if poll_result < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(poll_result as usize)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Read, Write};

    #[test]
    fn socketpair() {
        let (mut a, mut b) = super::socketpair_impl().unwrap();
        a.write_all(b"hello").unwrap();
        let mut buf = [0u8; 5];
        assert_eq!(b.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"hello");
    }
}
