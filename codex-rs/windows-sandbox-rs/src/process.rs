use crate::logging;
use crate::winutil::format_last_error;
use crate::winutil::quote_windows_arg;
use crate::winutil::to_wide;
use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::SetHandleInformation;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Console::GetStdHandle;
use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows_sys::Win32::System::Threading::CreateProcessAsUserW;
use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
use windows_sys::Win32::System::Threading::STARTUPINFOW;

pub fn make_env_block(env: &HashMap<String, String>) -> Vec<u16> {
    let mut items: Vec<(String, String)> =
        env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    items.sort_by(|a, b| {
        a.0.to_uppercase()
            .cmp(&b.0.to_uppercase())
            .then(a.0.cmp(&b.0))
    });
    let mut w: Vec<u16> = Vec::new();
    for (k, v) in items {
        let mut s = to_wide(format!("{}={}", k, v));
        s.pop();
        w.extend_from_slice(&s);
        w.push(0);
    }
    w.push(0);
    w
}

unsafe fn ensure_inheritable_stdio(si: &mut STARTUPINFOW) -> Result<()> {
    for kind in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        let h = GetStdHandle(kind);
        if h == 0 || h == INVALID_HANDLE_VALUE {
            return Err(anyhow!("GetStdHandle failed: {}", GetLastError()));
        }
        if SetHandleInformation(h, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
            return Err(anyhow!("SetHandleInformation failed: {}", GetLastError()));
        }
    }
    si.dwFlags |= STARTF_USESTDHANDLES;
    si.hStdInput = GetStdHandle(STD_INPUT_HANDLE);
    si.hStdOutput = GetStdHandle(STD_OUTPUT_HANDLE);
    si.hStdError = GetStdHandle(STD_ERROR_HANDLE);
    Ok(())
}

/// # Safety
/// Caller must provide a valid primary token handle (`h_token`) with appropriate access,
/// and the `argv`, `cwd`, and `env_map` must remain valid for the duration of the call.
pub unsafe fn create_process_as_user(
    h_token: HANDLE,
    argv: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
    logs_base_dir: Option<&Path>,
    stdio: Option<(HANDLE, HANDLE, HANDLE)>,
) -> Result<(PROCESS_INFORMATION, STARTUPINFOW)> {
    let cmdline_str = argv
        .iter()
        .map(|a| quote_windows_arg(a))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cmdline: Vec<u16> = to_wide(&cmdline_str);
    let env_block = make_env_block(env_map);
    let mut si: STARTUPINFOW = std::mem::zeroed();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    // Some processes (e.g., PowerShell) can fail with STATUS_DLL_INIT_FAILED
    // if lpDesktop is not set when launching with a restricted token.
    // Point explicitly at the interactive desktop.
    let desktop = to_wide("Winsta0\\Default");
    si.lpDesktop = desktop.as_ptr() as *mut u16;
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    // Ensure handles are inheritable when custom stdio is supplied.
    let inherit_handles = match stdio {
        Some((stdin_h, stdout_h, stderr_h)) => {
            si.dwFlags |= STARTF_USESTDHANDLES;
            si.hStdInput = stdin_h;
            si.hStdOutput = stdout_h;
            si.hStdError = stderr_h;
            for h in [stdin_h, stdout_h, stderr_h] {
                if SetHandleInformation(h, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
                    return Err(anyhow!(
                        "SetHandleInformation failed for stdio handle: {}",
                        GetLastError()
                    ));
                }
            }
            true
        }
        None => {
            ensure_inheritable_stdio(&mut si)?;
            true
        }
    };

    let ok = CreateProcessAsUserW(
        h_token,
        std::ptr::null(),
        cmdline.as_mut_ptr(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        inherit_handles as i32,
        CREATE_UNICODE_ENVIRONMENT,
        env_block.as_ptr() as *mut c_void,
        to_wide(cwd).as_ptr(),
        &si,
        &mut pi,
    );
    if ok == 0 {
        let err = GetLastError() as i32;
        let msg = format!(
            "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={} | si_flags={}",
            err,
            format_last_error(err),
            cwd.display(),
            cmdline_str,
            env_block.len(),
            si.dwFlags,
        );
        logging::debug_log(&msg, logs_base_dir);
        return Err(anyhow!("CreateProcessAsUserW failed: {}", err));
    }
    Ok((pi, si))
}
