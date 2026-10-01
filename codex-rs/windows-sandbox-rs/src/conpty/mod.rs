//! ConPTY helpers for spawning sandboxed processes with a PTY on Windows.
//!
//! This module encapsulates ConPTY creation and process spawn with the required
//! `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` plumbing. It is shared by both the legacy
//! restricted‑token path and the elevated runner path when unified_exec runs with
//! `tty=true`. The helpers are not tied to the IPC layer and can be reused by other
//! Windows sandbox flows that need a PTY.

use crate::desktop::LaunchDesktop;
use crate::proc_thread_attr::ProcThreadAttributeList;
use crate::winutil::format_last_error;
use crate::winutil::quote_windows_arg;
use crate::winutil::to_wide;
use anyhow::Context;
use anyhow::Result;
use codex_utils_pty::JobObject;
use codex_utils_pty::PsuedoCon;
use codex_utils_pty::RawConPty;
use std::collections::HashMap;
use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::IntoRawHandle;
use std::os::windows::io::OwnedHandle;
use std::path::Path;
use std::sync::Arc;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::HPCON;
use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows_sys::Win32::System::Threading::CreateProcessAsUserW;
use windows_sys::Win32::System::Threading::EXTENDED_STARTUPINFO_PRESENT;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
use windows_sys::Win32::System::Threading::STARTUPINFOEXW;

use crate::process::make_env_block;

/// Owns a ConPTY handle and its backing pipe handles.
pub struct ConptyInstance {
    pseudoconsole: Option<PsuedoCon>,
    input_write: Option<OwnedHandle>,
    output_read: Option<OwnedHandle>,
    job: Option<Arc<JobObject>>,
    _desktop: Option<LaunchDesktop>,
}

impl Drop for ConptyInstance {
    fn drop(&mut self) {
        drop(self.input_write.take());
        drop(self.output_read.take());
        drop(self.pseudoconsole.take());
    }
}

impl ConptyInstance {
    pub fn raw_handle(&self) -> Option<HPCON> {
        self.pseudoconsole
            .as_ref()
            .map(|pseudoconsole| pseudoconsole.raw_handle() as HPCON)
    }

    pub fn take_input_write(&mut self) -> HANDLE {
        self.input_write
            .take()
            .map_or(std::ptr::null_mut(), IntoRawHandle::into_raw_handle)
    }

    pub fn take_output_read(&mut self) -> HANDLE {
        self.output_read
            .take()
            .map_or(std::ptr::null_mut(), IntoRawHandle::into_raw_handle)
    }

    /// Returns the Job Object containing the spawned process, if this instance owns one.
    pub fn job(&self) -> Option<Arc<JobObject>> {
        self.job.as_ref().map(Arc::clone)
    }
}

/// Create a ConPTY with backing pipes.
///
/// This is public so callers that need lower-level PTY setup can build on the same
/// primitive, although the common entry point is `spawn_conpty_process_as_user`.
#[allow(dead_code)]
pub fn create_conpty(cols: i16, rows: i16) -> Result<ConptyInstance> {
    let raw = RawConPty::new(cols, rows)?;
    let (pseudoconsole, input_write, output_read) = raw.into_handles();
    // SAFETY: into_raw_handle transfers each pipe to its new owner.
    let input_write = unsafe { OwnedHandle::from_raw_handle(input_write.into_raw_handle()) };
    let output_read = unsafe { OwnedHandle::from_raw_handle(output_read.into_raw_handle()) };

    Ok(ConptyInstance {
        pseudoconsole: Some(pseudoconsole),
        input_write: Some(input_write),
        output_read: Some(output_read),
        job: None,
        _desktop: None,
    })
}

/// Spawn a process under `h_token` with ConPTY attached.
///
/// This is the main shared ConPTY entry point and is used by both the legacy/direct path
/// and the elevated runner path whenever a PTY-backed sandboxed process is needed.
pub fn spawn_conpty_process_as_user(
    h_token: BorrowedHandle<'_>,
    argv: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
    desktop: LaunchDesktop,
) -> Result<(PROCESS_INFORMATION, ConptyInstance)> {
    let cmdline_str = argv
        .iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cmdline: Vec<u16> = to_wide(&cmdline_str);
    let env_block = make_env_block(env_map);
    let mut si: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
    si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
    si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
    si.StartupInfo.lpDesktop = desktop.startup_info_desktop();
    let job = Arc::new(JobObject::create().context("create process job")?);

    let raw = RawConPty::new(/*cols*/ 80, /*rows*/ 24)?;
    let (pseudoconsole, input_write, output_read) = raw.into_handles();
    let hpc = pseudoconsole.raw_handle() as HPCON;
    // SAFETY: into_raw_handle transfers each pipe to its new owner.
    let input_write = unsafe { OwnedHandle::from_raw_handle(input_write.into_raw_handle()) };
    let output_read = unsafe { OwnedHandle::from_raw_handle(output_read.into_raw_handle()) };
    let conpty = ConptyInstance {
        pseudoconsole: Some(pseudoconsole),
        input_write: Some(input_write),
        output_read: Some(output_read),
        job: Some(Arc::clone(&job)),
        _desktop: Some(desktop),
    };
    let preserve_app_context = crate::app_package::current_process_has_package_identity()?;
    let mut attrs = ProcThreadAttributeList::new(2 + u32::from(preserve_app_context))?;
    attrs.set_pseudoconsole(hpc)?;
    attrs.set_job(job.as_raw_handle() as HANDLE)?;
    if preserve_app_context {
        attrs.preserve_desktop_app_context()?;
    }
    si.lpAttributeList = attrs.as_mut_ptr();

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        CreateProcessAsUserW(
            h_token.as_raw_handle(),
            std::ptr::null(),
            cmdline.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            env_block.as_ptr() as *mut c_void,
            to_wide(cwd).as_ptr(),
            &si.StartupInfo,
            &mut pi,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() } as i32;
        let message = format!(
            "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={}",
            err,
            format_last_error(err),
            cwd.display(),
            cmdline_str,
            env_block.len()
        );
        return Err(std::io::Error::from_raw_os_error(err)).context(message);
    }
    Ok((pi, conpty))
}
