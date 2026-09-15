//! Owns Windows SCM registration, service state, and sandbox provisioning event logs.

use std::ffi::c_void;
use std::io;
use std::ptr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use windows_sys::Win32::Foundation::ERROR_CALL_NOT_IMPLEMENTED;
use windows_sys::Win32::Foundation::ERROR_SERVICE_SPECIFIC_ERROR;
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::System::EventLog::DeregisterEventSource;
use windows_sys::Win32::System::EventLog::EVENTLOG_ERROR_TYPE;
use windows_sys::Win32::System::EventLog::EVENTLOG_INFORMATION_TYPE;
use windows_sys::Win32::System::EventLog::RegisterEventSourceW;
use windows_sys::Win32::System::EventLog::ReportEventW;
use windows_sys::Win32::System::Services::RegisterServiceCtrlHandlerExW;
use windows_sys::Win32::System::Services::SERVICE_ACCEPT_SESSIONCHANGE;
use windows_sys::Win32::System::Services::SERVICE_ACCEPT_SHUTDOWN;
use windows_sys::Win32::System::Services::SERVICE_ACCEPT_STOP;
use windows_sys::Win32::System::Services::SERVICE_CONTROL_INTERROGATE;
use windows_sys::Win32::System::Services::SERVICE_CONTROL_SESSIONCHANGE;
use windows_sys::Win32::System::Services::SERVICE_CONTROL_SHUTDOWN;
use windows_sys::Win32::System::Services::SERVICE_CONTROL_STOP;
use windows_sys::Win32::System::Services::SERVICE_RUNNING;
use windows_sys::Win32::System::Services::SERVICE_START_PENDING;
use windows_sys::Win32::System::Services::SERVICE_STATUS;
use windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE;
use windows_sys::Win32::System::Services::SERVICE_STOP_PENDING;
use windows_sys::Win32::System::Services::SERVICE_STOPPED;
use windows_sys::Win32::System::Services::SERVICE_TABLE_ENTRYW;
use windows_sys::Win32::System::Services::SERVICE_WIN32_OWN_PROCESS;
use windows_sys::Win32::System::Services::SetServiceStatus;
use windows_sys::Win32::System::Services::StartServiceCtrlDispatcherW;

mod runtime_lifecycle;

pub(crate) use runtime_lifecycle::retry_cleanup;

const EVENT_SERVICE_STARTED: u32 = 1000;
const EVENT_SERVICE_STOP_REQUESTED: u32 = 1001;
const EVENT_SERVICE_STOPPED: u32 = 1002;
pub(crate) const EVENT_SERVICE_FAILED: u32 = 1003;
pub(crate) const EVENT_PROVISIONING_SUCCEEDED: u32 = 2000;
pub(crate) const EVENT_PROVISIONING_FAILED: u32 = 2001;
pub(crate) const EVENT_REQUEST_REJECTED: u32 = 2002;
pub(crate) const EVENT_CLEANUP_STARTED: u32 = 3002;
pub(crate) const EVENT_CLEANUP_FINISHED: u32 = 3003;
pub(crate) const EVENT_CLEANUP_DETAIL: u32 = 3004;
const MAX_EVENT_MESSAGE_UNITS: usize = 1024;

static SERVICE_STATE: OnceLock<ServiceState> = OnceLock::new();

struct ServiceState {
    service_name: String,
    pipe_name: String,
    shutdown: Arc<AtomicBool>,
    uninstalling: Arc<AtomicBool>,
    status_handle: OnceLock<SERVICE_STATUS_HANDLE>,
    current_status: AtomicU32,
    stop_requested: AtomicBool,
}

pub(crate) fn run() -> Result<()> {
    let service_name = codex_windows_sandbox::windows_sandbox_service_name()?;
    let state = ServiceState {
        service_name: service_name.clone(),
        pipe_name: codex_windows_sandbox::windows_sandbox_service_pipe_name()?,
        shutdown: Arc::new(AtomicBool::new(false)),
        uninstalling: Arc::new(AtomicBool::new(false)),
        status_handle: OnceLock::new(),
        current_status: AtomicU32::new(SERVICE_START_PENDING),
        stop_requested: AtomicBool::new(false),
    };
    SERVICE_STATE
        .set(state)
        .map_err(|_| anyhow::anyhow!("the service dispatcher was already initialized"))?;

    let mut service_name = service_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let service_table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: service_name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: ptr::null_mut(),
            lpServiceProc: None,
        },
    ];

    // The table and its service-name buffer remain alive until the dispatcher returns.
    let dispatched = unsafe { StartServiceCtrlDispatcherW(service_table.as_ptr()) };
    if dispatched == 0 {
        return Err(io::Error::last_os_error()).context("start the Windows service dispatcher");
    }

    Ok(())
}

#[cfg(debug_assertions)]
pub(crate) fn run_foreground() -> Result<()> {
    crate::package_identity::enable_foreground_mode();
    crate::ipc::run(
        Arc::new(AtomicBool::new(false)),
        || {
            eprintln!(
                "{} listening on {}",
                codex_windows_sandbox::windows_sandbox_service_name()?,
                codex_windows_sandbox::windows_sandbox_service_pipe_name()?
            );
            Ok(())
        },
        runtime_lifecycle::foreground_owner,
        || Ok(()),
    )
}

unsafe extern "system" fn service_main(_argument_count: u32, _arguments: *mut *mut u16) {
    let Some(state) = SERVICE_STATE.get() else {
        return;
    };

    if let Err(error) = service_main_inner(state) {
        log_error(
            EVENT_SERVICE_FAILED,
            &format!("The Codex sandbox service encountered a fatal error: {error:#}"),
        );
        eprintln!("{} failed: {error:#}", state.service_name);
        if state.status_handle.get().is_some() {
            let _ = state.report_status(SERVICE_STOPPED, ERROR_SERVICE_SPECIFIC_ERROR);
        }
    }
}

fn service_main_inner(state: &ServiceState) -> Result<()> {
    let service_name = state
        .service_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    // The service controller only reads the name during registration.
    let status_handle = unsafe {
        RegisterServiceCtrlHandlerExW(
            service_name.as_ptr(),
            Some(service_control_handler),
            ptr::null(),
        )
    };
    if status_handle == 0 {
        return Err(io::Error::last_os_error()).context("register the service control handler");
    }
    state
        .status_handle
        .set(status_handle)
        .map_err(|_| anyhow::anyhow!("the service status handle was already registered"))?;

    state.report_status(SERVICE_START_PENDING, NO_ERROR)?;
    let package_lifecycle =
        crate::package_lifecycle::PackageLifecycle::new(Arc::clone(&state.uninstalling))?;
    runtime_lifecycle::run(state, &package_lifecycle)?;
    log_information(
        EVENT_SERVICE_STOPPED,
        "The Codex sandbox service has stopped.",
    );
    state.report_status(SERVICE_STOPPED, NO_ERROR)
}

unsafe extern "system" fn service_control_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut c_void,
    _context: *mut c_void,
) -> u32 {
    let Some(state) = SERVICE_STATE.get() else {
        return ERROR_CALL_NOT_IMPLEMENTED;
    };

    match control {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            if control == SERVICE_CONTROL_STOP {
                state.stop_requested.store(true, Ordering::Release);
            }
            if !state.shutdown.swap(true, Ordering::AcqRel) {
                if let Err(error) = state.report_status(SERVICE_STOP_PENDING, NO_ERROR) {
                    eprintln!("unable to report service shutdown: {error:#}");
                }
                log_information(
                    EVENT_SERVICE_STOP_REQUESTED,
                    "The Codex sandbox service was asked to stop.",
                );
                wake_listener();
            }
            NO_ERROR
        }
        SERVICE_CONTROL_INTERROGATE => {
            let current_status = state.current_status.load(Ordering::Acquire);
            if let Err(error) = state.report_status(current_status, NO_ERROR) {
                log_error(
                    EVENT_SERVICE_FAILED,
                    "The Codex sandbox service could not report its current status.",
                );
                eprintln!("unable to report the current service status: {error:#}");
            }
            NO_ERROR
        }
        SERVICE_CONTROL_SESSIONCHANGE => {
            wake_listener();
            NO_ERROR
        }
        _ => ERROR_CALL_NOT_IMPLEMENTED,
    }
}

pub(crate) fn wake_listener() {
    if let Some(state) = SERVICE_STATE.get() {
        std::thread::spawn(move || {
            crate::ipc::wake(&state.pipe_name, || {
                state.current_status.load(Ordering::Acquire) == SERVICE_STOPPED
            });
        });
    }
}

pub(crate) fn log_information(event_id: u32, message: &str) {
    log_event(EVENTLOG_INFORMATION_TYPE, event_id, message);
}

pub(crate) fn log_error(event_id: u32, message: &str) {
    log_event(EVENTLOG_ERROR_TYPE, event_id, message);
}

fn log_event(event_type: u16, event_id: u32, message: &str) {
    let source = SERVICE_STATE
        .get()
        .map_or("CodexSandboxService", |state| state.service_name.as_str())
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let event_log = unsafe { RegisterEventSourceW(ptr::null(), source.as_ptr()) };
    if event_log == 0 {
        eprintln!("unable to open the Windows event log: {message}");
        return;
    }

    let mut message_units = Vec::with_capacity(MAX_EVENT_MESSAGE_UNITS + 1);
    for character in message.chars() {
        let character = if character.is_control() {
            ' '
        } else {
            character
        };
        let mut buffer = [0_u16; 2];
        let encoded = character.encode_utf16(&mut buffer);
        if message_units.len() + encoded.len() > MAX_EVENT_MESSAGE_UNITS {
            break;
        }
        message_units.extend_from_slice(encoded);
    }
    message_units.push(0);
    let strings = [message_units.as_ptr()];
    let reported = unsafe {
        ReportEventW(
            event_log,
            event_type,
            0,
            event_id,
            ptr::null_mut(),
            1,
            0,
            strings.as_ptr(),
            ptr::null(),
        )
    };
    if reported == 0 {
        eprintln!("unable to write a Windows event-log entry (event {event_id})");
    }
    unsafe { DeregisterEventSource(event_log) };
}

impl ServiceState {
    fn report_status(&self, current_status: u32, win32_exit_code: u32) -> Result<()> {
        let status_handle = *self
            .status_handle
            .get()
            .context("the service status handle was not registered")?;
        let is_pending = matches!(current_status, SERVICE_START_PENDING | SERVICE_STOP_PENDING);
        let status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: current_status,
            dwControlsAccepted: if current_status == SERVICE_RUNNING {
                SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN | SERVICE_ACCEPT_SESSIONCHANGE
            } else {
                0
            },
            dwWin32ExitCode: win32_exit_code,
            dwServiceSpecificExitCode: u32::from(win32_exit_code == ERROR_SERVICE_SPECIFIC_ERROR),
            dwCheckPoint: u32::from(is_pending),
            dwWaitHint: if is_pending { 10_000 } else { 0 },
        };

        self.current_status.store(current_status, Ordering::Release);
        // The SCM synchronously copies the status structure during this call.
        let updated = unsafe { SetServiceStatus(status_handle, &status) };
        if updated == 0 {
            return Err(io::Error::last_os_error()).context("update the Windows service status");
        }

        Ok(())
    }
}
