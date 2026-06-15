use crate::desktop::LaunchDesktop;
use crate::logging;
use crate::proc_thread_attr::ProcThreadAttributeList;
use crate::winutil::argv_to_command_line;
use crate::winutil::format_last_error;
use crate::winutil::to_wide;
use anyhow::Result;
use anyhow::anyhow;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::SetHandleInformation;
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::Console::GetStdHandle;
use windows_sys::Win32::System::Console::STD_ERROR_HANDLE;
use windows_sys::Win32::System::Console::STD_INPUT_HANDLE;
use windows_sys::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows_sys::Win32::System::Threading::CreateProcessAsUserW;
use windows_sys::Win32::System::Threading::EXTENDED_STARTUPINFO_PRESENT;
use windows_sys::Win32::System::Threading::PROCESS_INFORMATION;
use windows_sys::Win32::System::Threading::STARTF_USESTDHANDLES;
use windows_sys::Win32::System::Threading::STARTUPINFOEXW;
use windows_sys::Win32::System::Threading::STARTUPINFOW;

pub struct CreatedProcess {
    pub process_info: PROCESS_INFORMATION,
    pub startup_info: STARTUPINFOW,
    _desktop: LaunchDesktop,
}

const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_PATH_NOT_FOUND: i32 = 3;

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
        let mut s = to_wide(format!("{k}={v}"));
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

fn command_has_explicit_path(command: &str) -> bool {
    command.contains('\\') || command.contains('/') || command.contains(':')
}

fn path_exts(env_map: &HashMap<String, String>) -> Vec<String> {
    let raw = env_map
        .get("PATHEXT")
        .cloned()
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
    raw.split(';')
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| ext.to_ascii_lowercase())
        .collect()
}

fn path_candidates(command: &str, env_map: &HashMap<String, String>) -> Vec<String> {
    let command_path = Path::new(command);
    let has_known_ext = command_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{}", ext).to_ascii_lowercase())
        .is_some_and(|ext| path_exts(env_map).iter().any(|candidate| candidate == &ext));
    if has_known_ext {
        return vec![command.to_string()];
    }

    let mut candidates = vec![command.to_string()];
    for ext in path_exts(env_map) {
        candidates.push(format!("{command}{ext}"));
    }
    candidates
}

fn resolved_path_on_env(command: &str, env_map: &HashMap<String, String>) -> Option<PathBuf> {
    if command.is_empty() || command_has_explicit_path(command) {
        return None;
    }
    let path = env_map.get("PATH")?;
    let candidates = path_candidates(command, env_map);
    path.split(';')
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
        .map(Path::new)
        .find_map(|dir| {
            candidates
                .iter()
                .map(|candidate| dir.join(candidate))
                .find(|candidate| candidate.is_file())
        })
}

fn create_process_path_hint(
    err: i32,
    argv: &[String],
    env_map: &HashMap<String, String>,
) -> Option<String> {
    if !matches!(err, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
        return None;
    }
    let command = argv.first()?;
    let resolved = resolved_path_on_env(command, env_map)?;
    Some(format!(
        "command `{command}` resolves on PATH to `{}`, but the Windows sandbox still reported {} ({}); this usually means the sandboxed user cannot traverse or execute that path",
        resolved.display(),
        err,
        format_last_error(err),
    ))
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
    use_private_desktop: bool,
) -> Result<CreatedProcess> {
    let cmdline_str = argv_to_command_line(argv);
    let mut cmdline: Vec<u16> = to_wide(&cmdline_str);
    let env_block = make_env_block(env_map);
    let desktop = LaunchDesktop::prepare(use_private_desktop, logs_base_dir)?;
    let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
    let cwd_wide = to_wide(cwd);
    let env_block_len = env_block.len();
    match stdio {
        Some((stdin_h, stdout_h, stderr_h)) => {
            let mut si: STARTUPINFOEXW = std::mem::zeroed();
            si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            // Some processes (e.g., PowerShell) can fail with STATUS_DLL_INIT_FAILED
            // if lpDesktop is not set when launching with a restricted token.
            // Point explicitly at the interactive desktop or a private desktop.
            si.StartupInfo.lpDesktop = desktop.startup_info_desktop();
            si.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
            si.StartupInfo.hStdInput = stdin_h;
            si.StartupInfo.hStdOutput = stdout_h;
            si.StartupInfo.hStdError = stderr_h;
            let mut inherited_handles = vec![stdin_h, stdout_h];
            if !inherited_handles.contains(&stderr_h) {
                inherited_handles.push(stderr_h);
            }
            for &handle in &inherited_handles {
                if SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
                    return Err(anyhow!(
                        "SetHandleInformation failed for stdio handle: {}",
                        GetLastError()
                    ));
                }
            }
            let mut attrs = ProcThreadAttributeList::new(/*attr_count*/ 1)?;
            attrs.set_handle_list(inherited_handles)?;
            si.lpAttributeList = attrs.as_mut_ptr();

            let creation_flags = CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT;
            let ok = CreateProcessAsUserW(
                h_token,
                std::ptr::null(),
                cmdline.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                creation_flags,
                env_block.as_ptr() as *mut c_void,
                cwd_wide.as_ptr(),
                &si.StartupInfo,
                &mut pi,
            );
            if ok == 0 {
                let err = GetLastError() as i32;
                let path_hint = create_process_path_hint(err, argv, env_map);
                let msg = format!(
                    "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={} | si_flags={} | creation_flags={}",
                    err,
                    format_last_error(err),
                    cwd.display(),
                    cmdline_str,
                    env_block_len,
                    si.StartupInfo.dwFlags,
                    creation_flags,
                );
                logging::debug_log(&msg, logs_base_dir);
                return Err(match path_hint {
                    Some(path_hint) => anyhow!("CreateProcessAsUserW failed: {err}; {path_hint}"),
                    None => anyhow!("CreateProcessAsUserW failed: {err}"),
                });
            }
            Ok(CreatedProcess {
                process_info: pi,
                startup_info: si.StartupInfo,
                _desktop: desktop,
            })
        }
        None => {
            let mut si: STARTUPINFOW = std::mem::zeroed();
            si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            si.lpDesktop = desktop.startup_info_desktop();
            ensure_inheritable_stdio(&mut si)?;

            let creation_flags = CREATE_UNICODE_ENVIRONMENT;
            let ok = CreateProcessAsUserW(
                h_token,
                std::ptr::null(),
                cmdline.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                creation_flags,
                env_block.as_ptr() as *mut c_void,
                cwd_wide.as_ptr(),
                &si,
                &mut pi,
            );
            if ok == 0 {
                let err = GetLastError() as i32;
                let path_hint = create_process_path_hint(err, argv, env_map);
                let msg = format!(
                    "CreateProcessAsUserW failed: {} ({}) | cwd={} | cmd={} | env_u16_len={} | si_flags={} | creation_flags={}",
                    err,
                    format_last_error(err),
                    cwd.display(),
                    cmdline_str,
                    env_block_len,
                    si.dwFlags,
                    creation_flags,
                );
                logging::debug_log(&msg, logs_base_dir);
                return Err(match path_hint {
                    Some(path_hint) => anyhow!("CreateProcessAsUserW failed: {err}; {path_hint}"),
                    None => anyhow!("CreateProcessAsUserW failed: {err}"),
                });
            }
            Ok(CreatedProcess {
                process_info: pi,
                startup_info: si,
                _desktop: desktop,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::create_process_path_hint;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn path_hint_mentions_resolved_executable_for_bare_command() {
        let temp = tempdir().expect("tempdir");
        let exe = temp.path().join("go.exe");
        std::fs::write(&exe, b"stub").expect("write stub");
        let env = HashMap::from([
            ("PATH".to_string(), temp.path().display().to_string()),
            ("PATHEXT".to_string(), ".EXE;.CMD".to_string()),
        ]);

        let hint = create_process_path_hint(2, &["go".to_string()], &env).expect("path hint");

        assert!(hint.contains("go"));
        assert!(hint.contains(&exe.display().to_string()));
    }

    #[test]
    fn path_hint_skips_explicit_paths() {
        let env = HashMap::from([("PATH".to_string(), "C:\\tools".to_string())]);
        assert!(create_process_path_hint(2, &["C:\\tools\\go.exe".to_string()], &env).is_none());
    }
}

/// Controls whether the child's stdin handle is kept open for writing.
#[allow(dead_code)]
pub enum StdinMode {
    Closed,
    Open,
}

/// Controls how stderr is wired for a pipe-spawned process.
#[allow(dead_code)]
pub enum StderrMode {
    MergeStdout,
    Separate,
}

/// Handles returned by `spawn_process_with_pipes`.
#[allow(dead_code)]
pub struct PipeSpawnHandles {
    pub process: PROCESS_INFORMATION,
    pub stdin_write: Option<HANDLE>,
    pub stdout_read: HANDLE,
    pub stderr_read: Option<HANDLE>,
    pub(crate) desktop: LaunchDesktop,
}

/// Spawns a process with anonymous pipes and returns the relevant handles.
#[allow(clippy::too_many_arguments)]
pub fn spawn_process_with_pipes(
    h_token: HANDLE,
    argv: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
    stdin_mode: StdinMode,
    stderr_mode: StderrMode,
    use_private_desktop: bool,
    logs_base_dir: Option<&Path>,
) -> Result<PipeSpawnHandles> {
    let mut in_r: HANDLE = 0;
    let mut in_w: HANDLE = 0;
    let mut out_r: HANDLE = 0;
    let mut out_w: HANDLE = 0;
    let mut err_r: HANDLE = 0;
    let mut err_w: HANDLE = 0;
    unsafe {
        if CreatePipe(&mut in_r, &mut in_w, ptr::null_mut(), 0) == 0 {
            return Err(anyhow!("CreatePipe stdin failed: {}", GetLastError()));
        }
        if CreatePipe(&mut out_r, &mut out_w, ptr::null_mut(), 0) == 0 {
            CloseHandle(in_r);
            CloseHandle(in_w);
            return Err(anyhow!("CreatePipe stdout failed: {}", GetLastError()));
        }
        if matches!(stderr_mode, StderrMode::Separate)
            && CreatePipe(&mut err_r, &mut err_w, ptr::null_mut(), 0) == 0
        {
            CloseHandle(in_r);
            CloseHandle(in_w);
            CloseHandle(out_r);
            CloseHandle(out_w);
            return Err(anyhow!("CreatePipe stderr failed: {}", GetLastError()));
        }
    }

    let stderr_handle = match stderr_mode {
        StderrMode::MergeStdout => out_w,
        StderrMode::Separate => err_w,
    };

    let stdio = Some((in_r, out_w, stderr_handle));
    let spawn_result = unsafe {
        create_process_as_user(
            h_token,
            argv,
            cwd,
            env_map,
            logs_base_dir,
            stdio,
            use_private_desktop,
        )
    };
    let created = match spawn_result {
        Ok(v) => v,
        Err(err) => {
            unsafe {
                CloseHandle(in_r);
                CloseHandle(in_w);
                CloseHandle(out_r);
                CloseHandle(out_w);
                if matches!(stderr_mode, StderrMode::Separate) {
                    CloseHandle(err_r);
                    CloseHandle(err_w);
                }
            }
            return Err(err);
        }
    };
    let CreatedProcess {
        process_info: pi,
        _desktop: desktop,
        ..
    } = created;

    unsafe {
        CloseHandle(in_r);
        CloseHandle(out_w);
        if matches!(stderr_mode, StderrMode::Separate) {
            CloseHandle(err_w);
        }
        if matches!(stdin_mode, StdinMode::Closed) {
            CloseHandle(in_w);
        }
    }

    Ok(PipeSpawnHandles {
        process: pi,
        stdin_write: match stdin_mode {
            StdinMode::Open => Some(in_w),
            StdinMode::Closed => None,
        },
        stdout_read: out_r,
        stderr_read: match stderr_mode {
            StderrMode::Separate => Some(err_r),
            StderrMode::MergeStdout => None,
        },
        desktop,
    })
}

/// Reads a HANDLE until EOF and invokes `on_chunk` for each read.
pub fn read_handle_loop<F>(handle: HANDLE, mut on_chunk: F) -> std::thread::JoinHandle<()>
where
    F: FnMut(&[u8]) + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let mut read_bytes: u32 = 0;
            let ok = unsafe {
                ReadFile(
                    handle,
                    buf.as_mut_ptr(),
                    buf.len() as u32,
                    &mut read_bytes,
                    ptr::null_mut(),
                )
            };
            if ok == 0 || read_bytes == 0 {
                break;
            }
            on_chunk(&buf[..read_bytes as usize]);
        }
        unsafe {
            CloseHandle(handle);
        }
    })
}
