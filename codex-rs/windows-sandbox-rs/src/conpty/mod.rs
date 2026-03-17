//! ConPTY helpers for spawning sandboxed processes with a PTY on Windows.
//!
//! This module encapsulates ConPTY creation and process spawn with the required
//! `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` plumbing. It is shared by both the legacy
//! restricted‑token path and the elevated runner path when unified_exec runs with
//! `tty=true`. The helpers are not tied to the IPC layer and can be reused by other
//! Windows sandbox flows that need a PTY.

mod proc_thread_attr;

use self::proc_thread_attr::ProcThreadAttributeList;
use crate::desktop::LaunchDesktop;
use crate::winutil::format_last_error;
use crate::winutil::quote_windows_arg;
use crate::winutil::to_wide;
use anyhow::Result;
use codex_utils_pty::RawConPty;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::ClosePseudoConsole;
use windows_sys::Win32::System::Threading::CreateProcessAsUserW;
use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows_sys::Win32::System::Threading::EXTENDED_STARTUPINFO_PRESENT;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
use windows_sys::Win32::System::Threading::STARTUPINFOEXW;

use crate::process::make_env_block;

/// Owns a ConPTY handle and its backing pipe handles.
pub struct ConptyInstance {
    pub hpc: HANDLE,
    pub input_write: HANDLE,
    pub output_read: HANDLE,
    _desktop: LaunchDesktop,
}

impl Drop for ConptyInstance {
    fn drop(&mut self) {
        unsafe {
            if self.input_write != 0 && self.input_write != INVALID_HANDLE_VALUE {
                CloseHandle(self.input_write);
            }
            if self.output_read != 0 && self.output_read != INVALID_HANDLE_VALUE {
                CloseHandle(self.output_read);
            }
            if self.hpc != 0 && self.hpc != INVALID_HANDLE_VALUE {
                ClosePseudoConsole(self.hpc);
            }
        }
    }
}

impl ConptyInstance {
    /// Consume the instance and return raw handles without closing them.
    pub fn into_raw(self) -> (HANDLE, HANDLE, HANDLE) {
        let me = std::mem::ManuallyDrop::new(self);
        (me.hpc, me.input_write, me.output_read)
    }
}

/// Create a ConPTY with backing pipes.
///
/// This is public so callers that need lower-level PTY setup can build on the same
/// primitive, although the common entry point is `spawn_conpty_process_as_user`.
pub fn create_conpty(cols: i16, rows: i16) -> Result<ConptyInstance> {
    let raw = RawConPty::new(cols, rows)?;
    let (hpc, input_write, output_read) = raw.into_raw_handles();

    Ok(ConptyInstance {
        hpc: hpc as HANDLE,
        input_write: input_write as HANDLE,
        output_read: output_read as HANDLE,
        _desktop: LaunchDesktop::prepare(false, None)?,
    })
}

/// Spawn a process under `h_token` with ConPTY attached.
///
/// This is the main shared ConPTY entry point and is used by both the legacy/direct path
/// and the elevated runner path whenever a PTY-backed sandboxed process is needed.
pub fn spawn_conpty_process_as_user(
    h_token: HANDLE,
    argv: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
    use_private_desktop: bool,
    logs_base_dir: Option<&Path>,
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
    let desktop = LaunchDesktop::prepare(use_private_desktop, logs_base_dir)?;
    si.StartupInfo.lpDesktop = desktop.startup_info_desktop();

    let conpty = create_conpty(80, 24)?;
    let mut attrs = ProcThreadAttributeList::new(1)?;
    attrs.set_pseudoconsole(conpty.hpc)?;
    si.lpAttributeList = attrs.as_mut_ptr();

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        CreateProcessAsUserW(
            h_token,
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
        return Err(anyhow::anyhow!(
            "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={}",
            err,
            format_last_error(err),
            cwd.display(),
            cmdline_str,
            env_block.len()
        ));
    }
    let mut conpty = conpty;
    conpty._desktop = desktop;
    Ok((pi, conpty))
}
