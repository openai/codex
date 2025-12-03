use anyhow::Result;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::{require_logon_sandbox_creds, SandboxPolicy};
use std::collections::HashMap;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::Threading::CreateProcessWithLogonW;
use windows_sys::Win32::System::Threading::LOGON_WITH_PROFILE;
use windows_sys::Win32::System::Threading::{PROCESS_INFORMATION, STARTUPINFOW};

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let codex_home = dirs_next::home_dir().unwrap_or(cwd.clone()).join(".codex");
    let policy = SandboxPolicy::ReadOnly;
    let _policy_json = serde_json::to_string(&policy)?;
    let env_map: HashMap<String, String> = HashMap::new();

    // Fetch sandbox creds (will prompt setup if missing).
    let creds = require_logon_sandbox_creds(&policy, &cwd, &cwd, &env_map, &codex_home)?;

    // Optional target override:
    // - "stub" to launch runner-stub.exe
    // - any other argument list is treated as the full command line to run.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let target = args.first().cloned().unwrap_or_else(|| "cmd".to_string());
    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    let user_w = to_wide(&creds.username);
    let domain_w = to_wide(".");
    let password_w = to_wide(&creds.password);
    let cmdline = if target == "stub" {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("runner-stub.exe")))
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "runner-stub.exe".to_string())
    } else if !args.is_empty() {
        args.join(" ")
    } else {
        "cmd /c whoami".to_string()
    };
    let cmd_w = to_wide(&cmdline);
    let cwd_w = to_wide(&cwd);
    let ok = unsafe {
        CreateProcessWithLogonW(
            user_w.as_ptr(),
            domain_w.as_ptr(),
            password_w.as_ptr(),
            LOGON_WITH_PROFILE,
            std::ptr::null(),
            cmd_w.as_ptr() as *mut _,
            0,
            std::ptr::null(),
            cwd_w.as_ptr(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        println!("CreateProcessWithLogonW failed: {}", err);
        return Ok(());
    }
    println!(
        "CreateProcessWithLogonW succeeded pid={} (target={})",
        pi.dwProcessId, target
    );
    Ok(())
}
