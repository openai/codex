use crate::sandbox::StdioPolicy;
use crate::temp_user::EphemeralUser;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::os::windows::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::ExitStatus;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows::Win32::Foundation::PSID;
use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows::Win32::Security::DuplicateTokenEx;
use windows::Win32::Security::SID_AND_ATTRIBUTES;
use windows::Win32::Security::SecurityImpersonation;
use windows::Win32::Security::SetTokenInformation;
use windows::Win32::Security::TOKEN_ACCESS_MASK;
use windows::Win32::Security::TOKEN_ADJUST_DEFAULT;
use windows::Win32::Security::TOKEN_ADJUST_SESSIONID;
use windows::Win32::Security::TOKEN_ASSIGN_PRIMARY;
use windows::Win32::Security::TOKEN_DUPLICATE;
use windows::Win32::Security::TOKEN_MANDATORY_LABEL;
use windows::Win32::Security::TOKEN_QUERY;
use windows::Win32::Security::TokenIntegrityLevel;
use windows::Win32::Security::TokenPrimary;
use windows::Win32::System::Console::GetStdHandle;
use windows::Win32::System::Console::STD_ERROR_HANDLE;
use windows::Win32::System::Console::STD_INPUT_HANDLE;
use windows::Win32::System::Console::STD_OUTPUT_HANDLE;
use windows::Win32::System::Environment::CreateEnvironmentBlock;
use windows::Win32::System::Environment::DestroyEnvironmentBlock;
use windows::Win32::System::JobObjects::AssignProcessToJobObject;
use windows::Win32::System::JobObjects::CreateJobObjectW;
use windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
use windows::Win32::System::JobObjects::JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
use windows::Win32::System::JobObjects::SetInformationJobObject;
use windows::Win32::System::SystemServices::SE_GROUP_INTEGRITY;
use windows::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
use windows::Win32::System::Threading::CreateProcessAsUserW;
use windows::Win32::System::Threading::GetExitCodeProcess;
use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;
use windows::Win32::System::Threading::PROCESS_INFORMATION;
use windows::Win32::System::Threading::STARTUPINFOW;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::PCWSTR;
use windows::core::PWSTR;

pub fn spawn_as_user_low_il(
    user: &EphemeralUser,
    command: Vec<String>,
    cwd: PathBuf,
    _stdio: StdioPolicy,
    env_map: HashMap<String, String>,
) -> Result<ExitStatus> {
    unsafe {
        // 1) Logon token
        let token = logon_interactive(&user.username, &user.password).context("LogonUserW")?;
        let primary = duplicate_primary(token).context("DuplicateTokenEx")?;
        set_integrity_low(primary).context("SetTokenInformation(TokenIntegrityLevel)")?;

        // 2) STARTUPINFO with inherited stdio
        let stdin =
            inherit(GetStdHandle(STD_INPUT_HANDLE).context("GetStdHandle(STD_INPUT_HANDLE)")?)?;
        let stdout =
            inherit(GetStdHandle(STD_OUTPUT_HANDLE).context("GetStdHandle(STD_OUTPUT_HANDLE)")?)?;
        let stderr =
            inherit(GetStdHandle(STD_ERROR_HANDLE).context("GetStdHandle(STD_ERROR_HANDLE)")?)?;
        let mut si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        si.dwFlags |= windows::Win32::System::Threading::STARTF_USESTDHANDLES;
        si.hStdInput = stdin;
        si.hStdOutput = stdout;
        si.hStdError = stderr;

        // 3) Environment
        let mut env_block = build_env_block(&env_map, &cwd)?;

        // 4) Command line
        let mut cmdline = encode_cmdline(&command);
        let mut wcwd = wide(cwd.as_os_str());

        // 5) Spawn
        let mut pi = PROCESS_INFORMATION::default();
        CreateProcessAsUserW(
            primary,
            PCWSTR::null(),
            PWSTR(cmdline.as_mut_ptr()),
            None,
            None,
            true,
            PROCESS_CREATION_FLAGS(CREATE_UNICODE_ENVIRONMENT.0),
            Some(env_block.as_mut_ptr().cast()),
            if wcwd.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(wcwd.as_mut_ptr())
            },
            &si,
            &mut pi,
        )
        .context("CreateProcessAsUserW")?;
        // handles in si are inherited; we close the local copies
        let _ = CloseHandle(si.hStdInput);
        let _ = CloseHandle(si.hStdOutput);
        let _ = CloseHandle(si.hStdError);
        // env_block is a Vec; nothing to destroy beyond dropping it

        // 6) Job object guarding the tree
        let job = CreateJobObjectW(None, PCWSTR::null()).context("CreateJobObjectW")?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            windows::Win32::System::JobObjects::JobObjectExtendedLimitInformation,
            &limits as *const _ as _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .context("SetInformationJobObject")?;
        AssignProcessToJobObject(job, pi.hProcess).context("AssignProcessToJobObject")?;

        // 7) Wait and return status
        WaitForSingleObject(pi.hProcess, u32::MAX);
        let mut code = 0u32;
        GetExitCodeProcess(pi.hProcess, &mut code).context("GetExitCodeProcess")?;
        let _ = CloseHandle(pi.hThread);
        let _ = CloseHandle(pi.hProcess);
        let _ = CloseHandle(job);
        Ok(ExitStatus::from_raw(code))
    }
}

unsafe fn logon_interactive(user: &str, pass: &str) -> Result<HANDLE> {
    use windows::Win32::Security::LOGON32_LOGON_INTERACTIVE;
    use windows::Win32::Security::LOGON32_PROVIDER_DEFAULT;
    use windows::Win32::Security::LogonUserW;
    let user_w = widestring::U16CString::from_str(user)?;
    let domain_w = widestring::U16CString::from_str(".")?;
    let pass_w = widestring::U16CString::from_str(pass)?;
    let mut h = HANDLE::default();
    unsafe {
        LogonUserW(
            PCWSTR(user_w.as_ptr()),
            PCWSTR(domain_w.as_ptr()),
            PCWSTR(pass_w.as_ptr()),
            LOGON32_LOGON_INTERACTIVE,
            LOGON32_PROVIDER_DEFAULT,
            &mut h,
        )
        .context("LogonUserW")?;
    }
    Ok(h)
}

unsafe fn duplicate_primary(h: HANDLE) -> Result<HANDLE> {
    let mut dup = HANDLE::default();
    unsafe {
        DuplicateTokenEx(
            h,
            TOKEN_ACCESS_MASK(
                TOKEN_DUPLICATE.0
                    | TOKEN_QUERY.0
                    | TOKEN_ASSIGN_PRIMARY.0
                    | TOKEN_ADJUST_DEFAULT.0
                    | TOKEN_ADJUST_SESSIONID.0,
            ),
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut dup,
        )
        .context("DuplicateTokenEx")?;
    }
    Ok(dup)
}

// S-1-16-4096 == Low IL
unsafe fn set_integrity_low(token: HANDLE) -> Result<()> {
    let mut sid = PSID::default();
    let sid_text = widestring::U16CString::from_str("S-1-16-4096")?;
    unsafe {
        ConvertStringSidToSidW(PCWSTR(sid_text.as_ptr()), &mut sid)
            .context("ConvertStringSidToSidW")?;
    }
    let tml = TOKEN_MANDATORY_LABEL {
        Label: SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: SE_GROUP_INTEGRITY as u32,
        },
    };
    let size = std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32
        + unsafe { windows::Win32::Security::GetLengthSid(sid) as u32 };
    unsafe {
        SetTokenInformation(token, TokenIntegrityLevel, &tml as *const _ as _, size)
            .context("SetTokenInformation")?;
    }
    Ok(())
}

fn wide(p: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.encode_wide().chain(std::iter::once(0)).collect()
}
fn encode_cmdline(args: &[String]) -> Vec<u16> {
    fn needs_quotes(s: &str) -> bool {
        s.is_empty()
            || s.chars()
                .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'))
    }
    fn quote(s: &str) -> String {
        if !needs_quotes(s) {
            return s.to_owned();
        }
        let mut out = String::from("\"");
        let mut bs = 0;
        for ch in s.chars() {
            match ch {
                '\\' => {
                    bs += 1;
                }
                '"' => {
                    out.push_str(&"\\".repeat(bs * 2 + 1));
                    out.push('"');
                    bs = 0;
                }
                _ => {
                    if bs > 0 {
                        out.push_str(&"\\".repeat(bs * 2));
                        bs = 0;
                    }
                    out.push(ch);
                }
            }
        }
        if bs > 0 {
            out.push_str(&"\\".repeat(bs * 2));
        }
        out.push('"');
        out
    }
    let s = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            if i == 0 {
                a.clone()
            } else {
                format!(" {}", quote(a))
            }
        })
        .collect::<String>();
    wide(std::ffi::OsStr::new(&s))
}
fn build_env_block(env: &HashMap<String, String>, cwd: &PathBuf) -> Result<Vec<u16>> {
    // Start with a basic block from the user profile and append overrides
    let mut block_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    unsafe {
        CreateEnvironmentBlock(&mut block_ptr, HANDLE::default(), false)
            .context("CreateEnvironmentBlock")?;
    }
    // Serialize our own
    let mut pairs: Vec<(String, String)> =
        env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    // Ensure HOME/USERPROFILE point inside a Low-IL dir (cwd by default)
    pairs.push(("USERPROFILE".into(), cwd.display().to_string()));
    pairs.push(("HOME".into(), cwd.display().to_string()));
    pairs.sort_by(|a, b| {
        a.0.to_ascii_uppercase()
            .cmp(&b.0.to_ascii_uppercase())
            .then(a.0.cmp(&b.0))
    });
    let mut out = Vec::<u16>::new();
    for (k, v) in pairs {
        let s = format!("{k}={v}");
        out.extend(s.encode_utf16());
        out.push(0);
    }
    out.push(0);
    unsafe {
        if !block_ptr.is_null() {
            DestroyEnvironmentBlock(block_ptr).context("DestroyEnvironmentBlock")?;
        }
    }
    Ok(out)
}

unsafe fn inherit(h: HANDLE) -> Result<HANDLE> {
    if h == INVALID_HANDLE_VALUE || h.is_invalid() {
        anyhow::bail!("invalid handle");
    }
    unsafe {
        windows::Win32::Foundation::SetHandleInformation(
            h,
            windows::Win32::Foundation::HANDLE_FLAG_INHERIT.0,
            windows::Win32::Foundation::HANDLE_FLAG_INHERIT,
        )
        .context("SetHandleInformation")?;
    }
    Ok(h)
}
