//! Best-effort, numeric service-stop evidence. Core exports it only when SCM says stopped.
//! Missing/older records stay unknown; no event-log text or user data is collected.

use std::mem::size_of_val;
use std::ptr;
use windows_sys::Win32::System::Registry as registry;
use windows_sys::Win32::System::Services as services;
use windows_sys::Win32::System::Services::SERVICE_STATUS_PROCESS;

/// Last lifecycle outcome reported by the service, not an inferred crash diagnosis.
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum ServiceStopReason {
    Starting = 1,
    StopRequested = 2,
    Shutdown = 3,
    OwnerRemoved = 4,
    StartupFailed = 5,
    BrokerFailed = 6,
    RegistrationInterrupted = 7,
}

impl ServiceStopReason {
    /// Called by the service, which owns the key. Failure must not affect its lifecycle.
    pub fn record(self, hresult: u32) {
        let Ok(service_name) = crate::windows_sandbox_service_name() else {
            return;
        };
        let key = crate::to_wide(format!(r"SYSTEM\CurrentControlSet\Services\{service_name}"));
        let value = ((self as u64) << 32) | u64::from(hresult);
        let mut handle = ptr::null_mut();
        unsafe {
            // Do not recreate a service key that Windows is uninstalling.
            if registry::RegOpenKeyExW(
                registry::HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                0,
                registry::KEY_SET_VALUE,
                &mut handle,
            ) != 0
            {
                return;
            }
            registry::RegSetValueExW(
                handle,
                windows_sys::w!("CodexLastStop"),
                0,
                registry::REG_QWORD,
                ptr::from_ref(&value).cast(),
                size_of_val(&value) as u32,
            );
            registry::RegCloseKey(handle);
        }
    }
}

pub(crate) fn record_stopped(status: &SERVICE_STATUS_PROCESS, service: services::SC_HANDLE) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let Ok(service_name) = crate::windows_sandbox_service_name() else {
        return;
    };
    let key = crate::to_wide(format!(r"SYSTEM\CurrentControlSet\Services\{service_name}"));
    let read_stop = || {
        let mut value = 0_u64;
        let mut size = size_of_val(&value) as u32;
        unsafe {
            registry::RegGetValueW(
                registry::HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                windows_sys::w!("CodexLastStop"),
                registry::RRF_RT_REG_QWORD | registry::RRF_ZEROONFAILURE,
                ptr::null_mut(),
                ptr::from_mut(&mut value).cast(),
                &mut size,
            );
        }
        value
    };
    let value = read_stop();
    // A concurrent restart can replace the registry record after the first query.
    // Only pair it with SCM evidence if the stopped state and exit codes still match.
    let mut current: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
    let mut bytes_needed = 0;
    if unsafe {
        services::QueryServiceStatusEx(
            service,
            services::SC_STATUS_PROCESS_INFO,
            ptr::from_mut(&mut current).cast(),
            size_of_val(&current) as u32,
            &mut bytes_needed,
        )
    } == 0
        || current.dwCurrentState != services::SERVICE_STOPPED
        || current.dwWin32ExitCode != status.dwWin32ExitCode
        || current.dwServiceSpecificExitCode != status.dwServiceSpecificExitCode
        || read_stop() != value
    {
        return;
    }
    // This counts observations, not unique outages. SCM's exit code also covers
    // crashes/kills that cannot write a final record. The registry is advisory.
    let _ = metrics.counter(
        "codex.windows_sandbox.service_stopped",
        /*inc*/ 1,
        &[
            (
                "last_reported_reason",
                match (value >> 32) as u32 {
                    1 => "no_stop_recorded",
                    2 => "stop_requested",
                    3 => "shutdown",
                    4 => "owner_removed",
                    5 => "startup_failed",
                    6 => "broker_failed",
                    7 => "registration_interrupted",
                    _ => "unknown",
                },
            ),
            ("last_reported_hresult", &format!("0x{:08x}", value as u32)),
            ("win32_exit_code", &status.dwWin32ExitCode.to_string()),
            (
                "service_exit_code",
                &status.dwServiceSpecificExitCode.to_string(),
            ),
        ],
    );
}
