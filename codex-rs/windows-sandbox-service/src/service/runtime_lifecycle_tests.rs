//! Verifies teardown retries distinguish SCM stop from system shutdown.

use super::retry_cleanup;
use crate::service::SERVICE_STATE;
use crate::service::ServiceState;
use crate::service::service_control_handler;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use std::io;
use std::ptr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::System::Services::SERVICE_CONTROL_SHUTDOWN;
use windows_sys::Win32::System::Services::SERVICE_CONTROL_STOP;
use windows_sys::Win32::System::Services::SERVICE_STOPPED;

#[test]
fn cleanup_retries_after_scm_stop_but_not_system_shutdown() -> Result<()> {
    // One test owns the process-global state. The pre-set shutdown flag keeps
    // the real control handler from calling SCM or spawning a pipe wake thread.
    assert!(
        SERVICE_STATE
            .set(ServiceState {
                service_name: "CodexSandboxServiceTests".into(),
                pipe_name: r"\\.\pipe\CodexSandboxServiceTests".into(),
                shutdown: Arc::new(AtomicBool::new(true)),
                uninstalling: Arc::new(AtomicBool::new(false)),
                status_handle: OnceLock::new(),
                current_status: AtomicU32::new(SERVICE_STOPPED),
                stop_requested: AtomicBool::new(false),
            })
            .is_ok()
    );
    let state = SERVICE_STATE.get().context("service state is missing")?;
    for (control, failures, expected_attempts, expected_success) in [
        (SERVICE_CONTROL_SHUTDOWN, 1, 1, false),
        (SERVICE_CONTROL_STOP, 1, 2, true),
        (SERVICE_CONTROL_STOP, usize::MAX, 5, false),
    ] {
        assert_eq!(
            unsafe {
                service_control_handler(
                    control,
                    /*_event_type*/ 0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            },
            NO_ERROR
        );
        let mut attempts = 0;
        let result = retry_cleanup(|| {
            attempts += 1;
            if attempts <= failures {
                return Err(io::Error::from_raw_os_error(ERROR_SHARING_VIOLATION as i32).into());
            }
            Ok(())
        });
        assert_eq!(result.is_ok(), expected_success);
        assert_eq!(attempts, expected_attempts);
        assert!(state.shutdown.load(Ordering::Acquire));
        if let Err(error) = result {
            assert_eq!(
                error
                    .downcast_ref::<io::Error>()
                    .context("cleanup did not preserve the I/O error")?
                    .raw_os_error(),
                Some(ERROR_SHARING_VIOLATION as i32)
            );
        }
    }
    Ok(())
}
