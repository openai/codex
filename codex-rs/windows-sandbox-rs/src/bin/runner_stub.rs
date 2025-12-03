use anyhow::Result;
use codex_windows_sandbox::{
    convert_string_sid_to_sid, create_readonly_token_with_cap_from,
    get_current_token_for_restriction, load_or_create_cap_sids, to_wide,
};
use std::collections::HashMap;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
use windows_sys::Win32::System::Threading::{
    CreateProcessAsUserW, WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, INFINITE,
    PROCESS_INFORMATION, STARTUPINFOW,
};

fn main() -> Result<()> {
    // Log current environment for diagnostics to a file under the sandbox user's profile.
    let env_dump = std::env::vars()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    // Attempt multiple destinations; log errors to stderr.
    if let Some(mut p) = dirs_next::home_dir() {
        p.push(".codex");
        if let Err(e) = std::fs::create_dir_all(&p) {
            eprintln!("failed to create {:?}: {e}", p);
        }
        p.push("runner_stub_env.txt");
        if let Err(e) = std::fs::write(&p, &env_dump) {
            eprintln!("failed to write {:?}: {e}", p);
        }
    } else {
        eprintln!("home_dir not available");
    }
    let public_path = std::path::Path::new(r"C:\Users\Public\runner_stub_env.txt");
    if let Err(e) = std::fs::write(public_path, &env_dump) {
        eprintln!("failed to write {:?}: {e}", public_path);
    }
    let cwd_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("runner_stub_env.txt");
    if let Err(e) = std::fs::write(&cwd_path, &env_dump) {
        eprintln!("failed to write {:?}: {e}", cwd_path);
    }

    // Create restricted token with readonly capability.
    let codex_home = dirs_next::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".codex");
    let caps = load_or_create_cap_sids(&codex_home);
    let psid_cap = unsafe { convert_string_sid_to_sid(&caps.readonly).unwrap() };

    let base = unsafe { get_current_token_for_restriction()? };
    let (restricted, _psid_used) = unsafe { create_readonly_token_with_cap_from(base, psid_cap)? };
    unsafe {
        CloseHandle(base);
    }

    // Launch a trivial command with the restricted token.
    let cmd = "cmd";
    let args = "/c echo restricted-stub";
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let mut cmdline = to_wide(format!("{cmd} {args}"));
    let cwd = std::env::current_dir()?;
    let mut env_block: Vec<u16> = Vec::new();
    let env_map: HashMap<String, String> = std::env::vars().collect();
    for (k, v) in env_map {
        let mut w = to_wide(format!("{k}={v}"));
        w.pop();
        env_block.extend_from_slice(&w);
        env_block.push(0);
    }
    env_block.push(0);
    let ok = unsafe {
        CreateProcessAsUserW(
            restricted,
            std::ptr::null(),
            cmdline.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            CREATE_UNICODE_ENVIRONMENT,
            env_block.as_ptr() as *const _,
            to_wide(&cwd).as_ptr(),
            &mut si,
            &mut pi,
        )
    };
    if ok == 0 {
        eprintln!("CreateProcessAsUserW failed: {}", unsafe { GetLastError() });
    } else {
        unsafe {
            WaitForSingleObject(pi.hProcess, INFINITE);
            if pi.hThread != 0 {
                CloseHandle(pi.hThread);
            }
            if pi.hProcess != 0 {
                CloseHandle(pi.hProcess);
            }
        }
    }
    unsafe {
        CloseHandle(restricted);
    }
    Ok(())
}
