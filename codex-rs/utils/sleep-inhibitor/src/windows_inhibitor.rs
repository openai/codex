use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use tracing::warn;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Power::POWER_REQUEST_TYPE;
use windows_sys::Win32::System::Power::PowerClearRequest;
use windows_sys::Win32::System::Power::PowerCreateRequest;
use windows_sys::Win32::System::Power::PowerRequestSystemRequired;
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

pub(crate) use WindowsSleepInhibitor as SleepInhibitor;

impl WindowsSleepInhibitor {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn acquire(&mut self) {
        if self.request.is_some() {
            return;
        }

        match PowerRequest::new_system_required(ASSERTION_REASON) {
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

    pub(crate) fn release(&mut self) {
        self.request = None;
    }
}

#[derive(Debug)]
struct PowerRequest {
    handle: windows_sys::Win32::Foundation::HANDLE,
    request_type: POWER_REQUEST_TYPE,
}

impl PowerRequest {
    fn new_system_required(reason: &str) -> Result<Self, String> {
        let mut wide_reason: Vec<u16> = OsStr::new(reason).encode_wide().chain(once(0)).collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: wide_reason.as_mut_ptr(),
            },
        };
        // SAFETY: `context` points to a valid `REASON_CONTEXT` for the duration
        // of the call and Windows copies the relevant data before returning.
        let handle = unsafe { PowerCreateRequest(&context) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            let error = std::io::Error::last_os_error();
            return Err(format!("PowerCreateRequest failed: {error}"));
        }

        // Match macOS `PreventUserIdleSystemSleep`: prevent idle system sleep
        // without forcing the display to stay on.
        let request_type = PowerRequestSystemRequired;
        // SAFETY: `handle` is a live power request handle and `request_type` is a
        // valid power request enum value.
        if unsafe { PowerSetRequest(handle, request_type) } == 0 {
            let error = std::io::Error::last_os_error();
            // SAFETY: `handle` was returned by `PowerCreateRequest` and has not
            // been closed yet on this error path.
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
        // SAFETY: `self.handle` is the handle owned by this `PowerRequest`, and
        // `self.request_type` is the request type that was set on acquire.
        if unsafe { PowerClearRequest(self.handle, self.request_type) } == 0 {
            let error = std::io::Error::last_os_error();
            warn!(
                reason = %error,
                "Failed to clear Windows sleep-prevention request"
            );
        }
        // SAFETY: `self.handle` is owned by this struct and closed exactly once
        // in `Drop`.
        if unsafe { CloseHandle(self.handle) } == 0 {
            let error = std::io::Error::last_os_error();
            warn!(
                reason = %error,
                "Failed to close Windows sleep-prevention request handle"
            );
        }
    }
}
