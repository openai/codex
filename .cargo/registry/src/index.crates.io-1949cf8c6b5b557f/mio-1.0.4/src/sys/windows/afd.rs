use std::ffi::c_void;
use std::fmt;
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;

use windows_sys::Wdk::Storage::FileSystem::NtCancelIoFileEx;
use windows_sys::Wdk::System::IO::NtDeviceIoControlFile;
use windows_sys::Win32::Foundation::{
    RtlNtStatusToDosError, HANDLE, NTSTATUS, STATUS_NOT_FOUND, STATUS_PENDING, STATUS_SUCCESS,
};
use windows_sys::Win32::System::IO::{IO_STATUS_BLOCK, IO_STATUS_BLOCK_0};

const IOCTL_AFD_POLL: u32 = 0x00012024;

/// Winsock2 AFD driver instance.
///
/// All operations are unsafe due to IO_STATUS_BLOCK parameter are being used by Afd driver during STATUS_PENDING before I/O Completion Port returns its result.
#[derive(Debug)]
pub struct Afd {
    fd: File,
}

#[repr(C)]
#[derive(Debug)]
pub struct AfdPollHandleInfo {
    pub handle: HANDLE,
    pub events: u32,
    pub status: NTSTATUS,
}

unsafe impl Send for AfdPollHandleInfo {}

#[repr(C)]
pub struct AfdPollInfo {
    pub timeout: i64,
    // Can have only value 1.
    pub number_of_handles: u32,
    pub exclusive: u32,
    pub handles: [AfdPollHandleInfo; 1],
}

impl fmt::Debug for AfdPollInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AfdPollInfo").finish()
    }
}

impl Afd {
    /// Poll `Afd` instance with `AfdPollInfo`.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe due to memory of `IO_STATUS_BLOCK` still being used by `Afd` instance while `Ok(false)` (`STATUS_PENDING`).
    /// `iosb` needs to be untouched after the call while operation is in effective at ALL TIME except for `cancel` method.
    /// So be careful not to `poll` twice while polling.
    /// User should deallocate there overlapped value when error to prevent memory leak.
    pub unsafe fn poll(
        &self,
        info: &mut AfdPollInfo,
        iosb: *mut IO_STATUS_BLOCK,
        overlapped: *mut c_void,
    ) -> io::Result<bool> {
        let info_ptr = info as *mut _ as *mut c_void;
        (*iosb).Anonymous.Status = STATUS_PENDING;
        let status = NtDeviceIoControlFile(
            self.fd.as_raw_handle() as HANDLE,
            std::ptr::null_mut(),
            None,
            overlapped,
            iosb,
            IOCTL_AFD_POLL,
            info_ptr,
            size_of::<AfdPollInfo>() as u32,
            info_ptr,
            size_of::<AfdPollInfo>() as u32,
        );
        match status {
            STATUS_SUCCESS => Ok(true),
            STATUS_PENDING => Ok(false),
            _ => Err(io::Error::from_raw_os_error(
                RtlNtStatusToDosError(status) as i32
            )),
        }
    }

    /// Cancel previous polled request of `Afd`.
    ///
    /// iosb needs to be used by `poll` first for valid `cancel`.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe due to memory of `IO_STATUS_BLOCK` still being used by `Afd` instance while `Ok(false)` (`STATUS_PENDING`).
    /// Use it only with request is still being polled so that you have valid `IO_STATUS_BLOCK` to use.
    /// User should NOT deallocate there overlapped value after the `cancel` to prevent double free.
    pub unsafe fn cancel(&self, iosb: *mut IO_STATUS_BLOCK) -> io::Result<()> {
        if (*iosb).Anonymous.Status != STATUS_PENDING {
            return Ok(());
        }

        let mut cancel_iosb = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        let status = NtCancelIoFileEx(self.fd.as_raw_handle() as HANDLE, iosb, &mut cancel_iosb);
        if status == STATUS_SUCCESS || status == STATUS_NOT_FOUND {
            return Ok(());
        }
        Err(io::Error::from_raw_os_error(
            RtlNtStatusToDosError(status) as i32
        ))
    }
}

cfg_io_source! {
    use std::mem::zeroed;
    use std::os::windows::io::{FromRawHandle, RawHandle};
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::{NtCreateFile, FILE_OPEN};
    use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, UNICODE_STRING};
    use windows_sys::Win32::Storage::FileSystem::{
        SetFileCompletionNotificationModes, FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
    };
    use windows_sys::Win32::System::WindowsProgramming::FILE_SKIP_SET_EVENT_ON_HANDLE;

    use super::iocp::CompletionPort;

    const AFD_HELPER_ATTRIBUTES: OBJECT_ATTRIBUTES = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: null_mut(),
        ObjectName: &AFD_OBJ_NAME as *const _ as *mut _,
        Attributes: 0,
        SecurityDescriptor: null_mut(),
        SecurityQualityOfService: null_mut(),
    };

    const AFD_OBJ_NAME: UNICODE_STRING = UNICODE_STRING {
        Length: (AFD_HELPER_NAME.len() * size_of::<u16>()) as u16,
        MaximumLength: (AFD_HELPER_NAME.len() * size_of::<u16>()) as u16,
        Buffer: AFD_HELPER_NAME.as_ptr() as *mut _,
    };

    const AFD_HELPER_NAME: &[u16] = &[
        '\\' as _,
        'D' as _,
        'e' as _,
        'v' as _,
        'i' as _,
        'c' as _,
        'e' as _,
        '\\' as _,
        'A' as _,
        'f' as _,
        'd' as _,
        '\\' as _,
        'M' as _,
        'i' as _,
        'o' as _
    ];

    static NEXT_TOKEN: AtomicUsize = AtomicUsize::new(0);

    impl AfdPollInfo {
        pub fn zeroed() -> AfdPollInfo {
            unsafe { zeroed() }
        }
    }

    impl Afd {
        /// Create new Afd instance.
        pub(crate) fn new(cp: &CompletionPort) -> io::Result<Afd> {
            let mut afd_helper_handle: HANDLE = INVALID_HANDLE_VALUE;
            let mut iosb = IO_STATUS_BLOCK {
                Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
                Information: 0,
            };

            unsafe {
                let status = NtCreateFile(
                    &mut afd_helper_handle as *mut _,
                    SYNCHRONIZE,
                    &AFD_HELPER_ATTRIBUTES as *const _ as *mut _,
                    &mut iosb,
                    null_mut(),
                    0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    FILE_OPEN,
                    0,
                    null_mut(),
                    0,
                );
                if status != STATUS_SUCCESS {
                    let raw_err = io::Error::from_raw_os_error(
                        RtlNtStatusToDosError(status) as i32
                    );
                    let msg = format!("Failed to open \\Device\\Afd\\Mio: {}", raw_err);
                    return Err(io::Error::new(raw_err.kind(), msg));
                }
                let fd = File::from_raw_handle(afd_helper_handle as RawHandle);
                // Increment by 2 to reserve space for other types of handles.
                // Non-AFD types (currently only NamedPipe), use odd numbered
                // tokens. This allows the selector to differentiate between them
                // and dispatch events accordingly.
                let token = NEXT_TOKEN.fetch_add(2, Ordering::Relaxed) + 2;
                let afd = Afd { fd };
                cp.add_handle(token, &afd.fd)?;
                match SetFileCompletionNotificationModes(
                    afd_helper_handle,
                    FILE_SKIP_SET_EVENT_ON_HANDLE as u8 // This is just 2, so fits in u8
                ) {
                    0 => Err(io::Error::last_os_error()),
                    _ => Ok(afd),
                }
            }
        }
    }
}

pub const POLL_RECEIVE: u32 = 0b0_0000_0001;
pub const POLL_RECEIVE_EXPEDITED: u32 = 0b0_0000_0010;
pub const POLL_SEND: u32 = 0b0_0000_0100;
pub const POLL_DISCONNECT: u32 = 0b0_0000_1000;
pub const POLL_ABORT: u32 = 0b0_0001_0000;
pub const POLL_LOCAL_CLOSE: u32 = 0b0_0010_0000;
// Not used as it indicated in each event where a connection is connected, not
// just the first time a connection is established.
// Also see https://github.com/piscisaureus/wepoll/commit/8b7b340610f88af3d83f40fb728e7b850b090ece.
pub const POLL_CONNECT: u32 = 0b0_0100_0000;
pub const POLL_ACCEPT: u32 = 0b0_1000_0000;
pub const POLL_CONNECT_FAIL: u32 = 0b1_0000_0000;

pub const KNOWN_EVENTS: u32 = POLL_RECEIVE
    | POLL_RECEIVE_EXPEDITED
    | POLL_SEND
    | POLL_DISCONNECT
    | POLL_ABORT
    | POLL_LOCAL_CLOSE
    | POLL_ACCEPT
    | POLL_CONNECT_FAIL;
