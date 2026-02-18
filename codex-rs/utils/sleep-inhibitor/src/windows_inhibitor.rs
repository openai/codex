use crate::PlatformSleepInhibitor;
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use tracing::warn;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Power::POWER_REQUEST_TYPE;
use windows_sys::Win32::System::Power::PowerClearRequest;
use windows_sys::Win32::System::Power::PowerCreateRequest;
use windows_sys::Win32::System::Power::PowerRequestExecutionRequired;
use windows_sys::Win32::System::Power::PowerSetRequest;
use windows_sys::Win32::System::SystemServices::POWER_REQUEST_CONTEXT_VERSION;
use windows_sys::Win32::System::Threading::POWER_REQUEST_CONTEXT_SIMPLE_STRING;
use windows_sys::Win32::System::Threading::REASON_CONTEXT;
use windows_sys::Win32::System::Threading::REASON_CONTEXT_0;

const ASSERTION_REASON: &str = "Codex is running an active turn";

#[derive(Debug, Default)]
pub(crate) struct WindowsSleepInhibitor {
    request: Option<PowerRequest>,
}

impl WindowsSleepInhibitor {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl PlatformSleepInhibitor for WindowsSleepInhibitor {
    fn acquire(&mut self) {
        if self.request.is_some() {
            return;
        }

        match PowerRequest::new_execution_required(ASSERTION_REASON) {
            Ok(request) => {
                self.request = Some(request);
            }
            Err(error) => {
                warn!(
                    reason = %error,
                    "Failed to acquire Windows sleep-prevention request"
                );
            }
        }
    }

    fn release(&mut self) {
        self.request = None;
    }
}

#[derive(Debug)]
struct PowerRequest {
    handle: windows_sys::Win32::Foundation::HANDLE,
    request_type: POWER_REQUEST_TYPE,
}

impl PowerRequest {
    fn new_execution_required(reason: &str) -> Result<Self, String> {
        let mut wide_reason: Vec<u16> = OsStr::new(reason).encode_wide().chain(once(0)).collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: wide_reason.as_mut_ptr(),
            },
        };
        let handle = unsafe { PowerCreateRequest(&context) };
        if handle == 0 || handle == INVALID_HANDLE_VALUE {
            let error = std::io::Error::last_os_error();
            return Err(format!("PowerCreateRequest failed: {error}"));
        }

        let request_type = PowerRequestExecutionRequired;
        if unsafe { PowerSetRequest(handle, request_type) } == 0 {
            let error = std::io::Error::last_os_error();
            let _ = unsafe { CloseHandle(handle) };
            return Err(format!("PowerSetRequest failed: {error}"));
        }

        Ok(Self {
            handle,
            request_type,
        })
    }
}

impl Drop for PowerRequest {
    fn drop(&mut self) {
        let _ = unsafe { PowerClearRequest(self.handle, self.request_type) };
        let _ = unsafe { CloseHandle(self.handle) };
    }
}
