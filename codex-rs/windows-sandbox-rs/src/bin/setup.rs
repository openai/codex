use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use codex_windows_sandbox::add_allow_ace;
use codex_windows_sandbox::dpapi_protect;
use codex_windows_sandbox::sandbox_dir;
use codex_windows_sandbox::string_from_sid_bytes;
use codex_windows_sandbox::SETUP_VERSION;
use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;
use serde::Deserialize;
use serde::Serialize;
use std::ffi::c_void;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use windows::core::Interface;
use windows::core::BSTR;
use windows::Win32::Foundation::VARIANT_TRUE;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::INetFwRule3;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwPolicy2;
use windows::Win32::NetworkManagement::WindowsFirewall::NetFwRule;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_ACTION_BLOCK;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_IP_PROTOCOL_ANY;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_PROFILE2_ALL;
use windows::Win32::NetworkManagement::WindowsFirewall::NET_FW_RULE_DIR_OUT;
use windows::Win32::System::Com::CoCreateInstance;
use windows::Win32::System::Com::CoInitializeEx;
use windows::Win32::System::Com::CoUninitialize;
use windows::Win32::System::Com::CLSCTX_INPROC_SERVER;
use windows::Win32::System::Com::COINIT_APARTMENTTHREADED;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::NetworkManagement::NetManagement::NERR_Success;
use windows_sys::Win32::NetworkManagement::NetManagement::NetLocalGroupAddMembers;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserAdd;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserSetInfo;
use windows_sys::Win32::NetworkManagement::NetManagement::LOCALGROUP_MEMBERS_INFO_3;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_DONT_EXPIRE_PASSWD;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_SCRIPT;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_INFO_1;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_INFO_1003;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_PRIV_USER;
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::Authorization::GetEffectiveRightsFromAclW;
use windows_sys::Win32::Security::Authorization::GetNamedSecurityInfoW;
use windows_sys::Win32::Security::Authorization::SetEntriesInAclW;
use windows_sys::Win32::Security::Authorization::SetNamedSecurityInfoW;
use windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
use windows_sys::Win32::Security::Authorization::GRANT_ACCESS;
use windows_sys::Win32::Security::Authorization::SE_FILE_OBJECT;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID;
use windows_sys::Win32::Security::Authorization::TRUSTEE_W;
use windows_sys::Win32::Security::LookupAccountNameW;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::CONTAINER_INHERIT_ACE;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::OBJECT_INHERIT_ACE;
use windows_sys::Win32::Security::SID_NAME_USE;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;

#[derive(Debug, Deserialize)]
struct Payload {
    version: u32,
    offline_username: String,
    online_username: String,
    codex_home: PathBuf,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    real_user: String,
}

#[derive(Serialize)]
struct SandboxUserRecord {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct SandboxUsersFile {
    version: u32,
    offline: SandboxUserRecord,
    online: SandboxUserRecord,
}

#[derive(Serialize)]
struct SetupMarker {
    version: u32,
    offline_username: String,
    online_username: String,
    created_at: String,
}

fn log_line(log: &mut File, msg: &str) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    writeln!(log, "[{ts}] {msg}")?;
    Ok(())
}

fn to_wide(s: &OsStr) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    v.push(0);
    v
}

fn random_password() -> String {
    const CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_=+";
    let mut rng = SmallRng::from_entropy();
    let mut buf = [0u8; 24];
    rng.fill_bytes(&mut buf);
    buf.iter()
        .map(|b| {
            let idx = (*b as usize) % CHARS.len();
            CHARS[idx] as char
        })
        .collect()
}

fn sid_to_string(sid: &[u8]) -> Result<String> {
    string_from_sid_bytes(sid).map_err(anyhow::Error::msg)
}

fn sid_bytes_to_psid(sid: &[u8]) -> Result<*mut c_void> {
    let sid_str = sid_to_string(sid)?;
    let sid_w = to_wide(OsStr::new(&sid_str));
    let mut psid: *mut c_void = std::ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(sid_w.as_ptr(), &mut psid) } == 0 {
        return Err(anyhow::anyhow!(
            "ConvertStringSidToSidW failed: {}",
            unsafe { GetLastError() }
        ));
    }
    Ok(psid)
}

fn ensure_local_user(name: &str, password: &str, log: &mut File) -> Result<()> {
    let name_w = to_wide(OsStr::new(name));
    let pwd_w = to_wide(OsStr::new(password));
    unsafe {
        let info = USER_INFO_1 {
            usri1_name: name_w.as_ptr() as *mut u16,
            usri1_password: pwd_w.as_ptr() as *mut u16,
            usri1_password_age: 0,
            usri1_priv: USER_PRIV_USER,
            usri1_home_dir: std::ptr::null_mut(),
            usri1_comment: std::ptr::null_mut(),
            usri1_flags: UF_SCRIPT | UF_DONT_EXPIRE_PASSWD,
            usri1_script_path: std::ptr::null_mut(),
        };
        let status = NetUserAdd(
            std::ptr::null(),
            1,
            &info as *const _ as *mut u8,
            std::ptr::null_mut(),
        );
        if status != NERR_Success {
            // Try update password via level 1003.
            let pw_info = USER_INFO_1003 {
                usri1003_password: pwd_w.as_ptr() as *mut u16,
            };
            let upd = NetUserSetInfo(
                std::ptr::null(),
                name_w.as_ptr(),
                1003,
                &pw_info as *const _ as *mut u8,
                std::ptr::null_mut(),
            );
            if upd != NERR_Success {
                log_line(log, &format!("NetUserSetInfo failed for {name} code {upd}"))?;
                return Err(anyhow::anyhow!(
                    "failed to create/update user {name}, code {status}/{upd}"
                ));
            }
        }
        let group = to_wide(OsStr::new("Users"));
        let member = LOCALGROUP_MEMBERS_INFO_3 {
            lgrmi3_domainandname: name_w.as_ptr() as *mut u16,
        };
        let _ = NetLocalGroupAddMembers(
            std::ptr::null(),
            group.as_ptr(),
            3,
            &member as *const _ as *mut u8,
            1,
        );
    }
    Ok(())
}

fn resolve_sid(name: &str) -> Result<Vec<u8>> {
    let name_w = to_wide(OsStr::new(name));
    let mut sid_buffer = vec![0u8; 68];
    let mut sid_len: u32 = sid_buffer.len() as u32;
    let mut domain: Vec<u16> = Vec::new();
    let mut domain_len: u32 = 0;
    let mut use_type: SID_NAME_USE = 0;
    loop {
        let ok = unsafe {
            LookupAccountNameW(
                std::ptr::null(),
                name_w.as_ptr(),
                sid_buffer.as_mut_ptr() as *mut c_void,
                &mut sid_len,
                domain.as_mut_ptr(),
                &mut domain_len,
                &mut use_type,
            )
        };
        if ok != 0 {
            sid_buffer.truncate(sid_len as usize);
            return Ok(sid_buffer);
        }
        let err = unsafe { GetLastError() };
        if err == ERROR_INSUFFICIENT_BUFFER {
            sid_buffer.resize(sid_len as usize, 0);
            domain.resize(domain_len as usize, 0);
            continue;
        }
        return Err(anyhow::anyhow!(
            "LookupAccountNameW failed for {name}: {}",
            err
        ));
    }
}

fn trustee_has_rx(path: &Path, trustee: &str) -> Result<bool> {
    let sid = resolve_sid(trustee)?;
    unsafe {
        let sid_str = sid_to_string(&sid)?;
        let sid_w = to_wide(OsStr::new(&sid_str));
        let mut psid: *mut c_void = std::ptr::null_mut();
        if ConvertStringSidToSidW(sid_w.as_ptr(), &mut psid) == 0 {
            return Err(anyhow::anyhow!(
                "ConvertStringSidToSidW failed: {}",
                GetLastError()
            ));
        }
        let path_w = to_wide(path.as_os_str());
        let mut existing_dacl: *mut ACL = std::ptr::null_mut();
        let mut sd: *mut c_void = std::ptr::null_mut();
        let get_res = GetNamedSecurityInfoW(
            path_w.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut existing_dacl,
            std::ptr::null_mut(),
            &mut sd,
        );
        if get_res != 0 {
            return Err(anyhow::anyhow!(
                "GetNamedSecurityInfoW failed for {}: {}",
                path.display(),
                get_res
            ));
        }
        let trustee = TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_SID,
            ptstrName: psid as *mut u16,
        };
        let mut mask: u32 = 0;
        let eff = GetEffectiveRightsFromAclW(existing_dacl, &trustee, &mut mask);
        if eff != 0 {
            return Err(anyhow::anyhow!(
                "GetEffectiveRightsFromAclW failed for {}: {}",
                path.display(),
                eff
            ));
        }
        if !sd.is_null() {
            LocalFree(sd as HLOCAL);
        }
        if !psid.is_null() {
            LocalFree(psid as HLOCAL);
        }
        let needed = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
        Ok((mask & needed) == needed)
    }
}

fn collect_system_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(sr) = std::env::var("SystemRoot") {
        roots.push(PathBuf::from(sr));
    } else {
        roots.push(PathBuf::from(r"C:\Windows"));
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        roots.push(PathBuf::from(pf));
    } else {
        roots.push(PathBuf::from(r"C:\Program Files"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        roots.push(PathBuf::from(pf86));
    } else {
        roots.push(PathBuf::from(r"C:\Program Files (x86)"));
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        roots.push(PathBuf::from(pd));
    } else {
        roots.push(PathBuf::from(r"C:\ProgramData"));
    }
    roots
}

fn add_inheritable_allow_no_log(path: &Path, sid: &[u8], mask: u32) -> Result<()> {
    unsafe {
        let mut psid: *mut c_void = std::ptr::null_mut();
        let sid_str = sid_to_string(sid)?;
        let sid_w = to_wide(OsStr::new(&sid_str));
        if ConvertStringSidToSidW(sid_w.as_ptr(), &mut psid) == 0 {
            return Err(anyhow::anyhow!(
                "ConvertStringSidToSidW failed: {}",
                GetLastError()
            ));
        }
        let path_w = to_wide(path.as_os_str());

        let mut existing_dacl: *mut ACL = std::ptr::null_mut();
        let mut sd: *mut c_void = std::ptr::null_mut();
        let get_res = GetNamedSecurityInfoW(
            path_w.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut existing_dacl,
            std::ptr::null_mut(),
            &mut sd,
        );
        if get_res != 0 {
            return Err(anyhow::anyhow!(
                "GetNamedSecurityInfoW failed for {}: {}",
                path.display(),
                get_res
            ));
        }
        let trustee = TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: 0,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_SID,
            ptstrName: psid as *mut u16,
        };
        let ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: mask,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
            Trustee: trustee,
        };
        let mut new_dacl: *mut ACL = std::ptr::null_mut();
        let set = SetEntriesInAclW(1, &ea, existing_dacl, &mut new_dacl);
        if set != 0 {
            return Err(anyhow::anyhow!("SetEntriesInAclW failed: {}", set));
        }
        let res = SetNamedSecurityInfoW(
            path_w.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_dacl,
            std::ptr::null_mut(),
        );
        if res != 0 {
            return Err(anyhow::anyhow!(
                "SetNamedSecurityInfoW failed for {}: {}",
                path.display(),
                res
            ));
        }
        if !new_dacl.is_null() {
            LocalFree(new_dacl as HLOCAL);
        }
        if !sd.is_null() {
            LocalFree(sd as HLOCAL);
        }
        if !psid.is_null() {
            LocalFree(psid as HLOCAL);
        }
    }
    Ok(())
}

fn try_add_inheritable_allow_with_timeout(
    path: &Path,
    sid: &[u8],
    mask: u32,
    _log: &mut File,
    timeout: Duration,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Result<()>>();
    let path_buf = path.to_path_buf();
    let sid_vec = sid.to_vec();
    std::thread::spawn(move || {
        let res = add_inheritable_allow_no_log(&path_buf, &sid_vec, mask);
        let _ = tx.send(res);
    });
    match rx.recv_timeout(timeout) {
        Ok(res) => res,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(anyhow::anyhow!(
            "ACL grant timed out on {} after {:?}",
            path.display(),
            timeout
        )),
        Err(e) => Err(anyhow::anyhow!(
            "ACL grant channel error on {}: {e}",
            path.display()
        )),
    }
}

fn run_netsh_firewall(sid: &str, log: &mut File) -> Result<()> {
    let local_user_spec = format!("O:LSD:(A;;CC;;;{sid})");
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        return Err(anyhow::anyhow!("CoInitializeEx failed: {hr:?}"));
    }
    let result = unsafe {
        (|| -> Result<()> {
            let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| anyhow::anyhow!("CoCreateInstance NetFwPolicy2: {e:?}"))?;
            let rules = policy
                .Rules()
                .map_err(|e| anyhow::anyhow!("INetFwPolicy2::Rules: {e:?}"))?;
            let name = BSTR::from("Codex Sandbox Offline - Block Outbound");
            let rule: INetFwRule3 = match rules.Item(&name) {
                Ok(existing) => existing.cast().map_err(|e| {
                    anyhow::anyhow!("cast existing firewall rule to INetFwRule3: {e:?}")
                })?,
                Err(_) => {
                    let new_rule: INetFwRule3 =
                        CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER)
                            .map_err(|e| anyhow::anyhow!("CoCreateInstance NetFwRule: {e:?}"))?;
                    new_rule
                        .SetName(&name)
                        .map_err(|e| anyhow::anyhow!("SetName: {e:?}"))?;
                    new_rule
                        .SetDirection(NET_FW_RULE_DIR_OUT)
                        .map_err(|e| anyhow::anyhow!("SetDirection: {e:?}"))?;
                    new_rule
                        .SetAction(NET_FW_ACTION_BLOCK)
                        .map_err(|e| anyhow::anyhow!("SetAction: {e:?}"))?;
                    new_rule
                        .SetEnabled(VARIANT_TRUE)
                        .map_err(|e| anyhow::anyhow!("SetEnabled: {e:?}"))?;
                    new_rule
                        .SetProfiles(NET_FW_PROFILE2_ALL.0)
                        .map_err(|e| anyhow::anyhow!("SetProfiles: {e:?}"))?;
                    new_rule
                        .SetProtocol(NET_FW_IP_PROTOCOL_ANY.0)
                        .map_err(|e| anyhow::anyhow!("SetProtocol: {e:?}"))?;
                    rules
                        .Add(&new_rule)
                        .map_err(|e| anyhow::anyhow!("Rules::Add: {e:?}"))?;
                    new_rule
                }
            };
            rule.SetLocalUserAuthorizedList(&BSTR::from(local_user_spec.as_str()))
                .map_err(|e| anyhow::anyhow!("SetLocalUserAuthorizedList: {e:?}"))?;
            rule.SetEnabled(VARIANT_TRUE)
                .map_err(|e| anyhow::anyhow!("SetEnabled: {e:?}"))?;
            rule.SetProfiles(NET_FW_PROFILE2_ALL.0)
                .map_err(|e| anyhow::anyhow!("SetProfiles: {e:?}"))?;
            rule.SetAction(NET_FW_ACTION_BLOCK)
                .map_err(|e| anyhow::anyhow!("SetAction: {e:?}"))?;
            rule.SetDirection(NET_FW_RULE_DIR_OUT)
                .map_err(|e| anyhow::anyhow!("SetDirection: {e:?}"))?;
            rule.SetProtocol(NET_FW_IP_PROTOCOL_ANY.0)
                .map_err(|e| anyhow::anyhow!("SetProtocol: {e:?}"))?;
            log_line(
                log,
                &format!(
                "firewall rule configured via COM with LocalUserAuthorizedList={local_user_spec}"
            ),
            )?;
            Ok(())
        })()
    };
    unsafe {
        CoUninitialize();
    }
    result
}

fn lock_sandbox_dir(dir: &Path, real_user: &str, log: &mut File) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let system_sid = resolve_sid("SYSTEM")?;
    let admins_sid = resolve_sid("Administrators")?;
    let real_sid = resolve_sid(real_user)?;
    let entries = [
        (
            system_sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
        ),
        (
            admins_sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
        ),
        (
            real_sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE,
        ),
    ];
    unsafe {
        let mut eas: Vec<EXPLICIT_ACCESS_W> = Vec::new();
        let mut sids: Vec<*mut c_void> = Vec::new();
        for (sid_bytes, mask) in entries {
            let sid_str = sid_to_string(&sid_bytes)?;
            let sid_w = to_wide(OsStr::new(&sid_str));
            let mut psid: *mut c_void = std::ptr::null_mut();
            if ConvertStringSidToSidW(sid_w.as_ptr(), &mut psid) == 0 {
                return Err(anyhow::anyhow!(
                    "ConvertStringSidToSidW failed: {}",
                    GetLastError()
                ));
            }
            sids.push(psid);
            eas.push(EXPLICIT_ACCESS_W {
                grfAccessPermissions: mask,
                grfAccessMode: GRANT_ACCESS,
                grfInheritance: OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: std::ptr::null_mut(),
                    MultipleTrusteeOperation: 0,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_SID,
                    ptstrName: psid as *mut u16,
                },
            });
        }
        let mut new_dacl: *mut ACL = std::ptr::null_mut();
        let set = SetEntriesInAclW(
            eas.len() as u32,
            eas.as_ptr(),
            std::ptr::null_mut(),
            &mut new_dacl,
        );
        if set != 0 {
            return Err(anyhow::anyhow!(
                "SetEntriesInAclW sandbox dir failed: {}",
                set
            ));
        }
        let path_w = to_wide(dir.as_os_str());
        let res = SetNamedSecurityInfoW(
            path_w.as_ptr() as *mut u16,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_dacl,
            std::ptr::null_mut(),
        );
        if res != 0 {
            return Err(anyhow::anyhow!(
                "SetNamedSecurityInfoW sandbox dir failed: {}",
                res
            ));
        }
        if !new_dacl.is_null() {
            LocalFree(new_dacl as HLOCAL);
        }
        for sid in sids {
            if !sid.is_null() {
                LocalFree(sid as HLOCAL);
            }
        }
    }
    log_line(
        log,
        &format!("sandbox dir ACL applied at {}", dir.display()),
    )?;
    Ok(())
}

fn write_secrets(
    codex_home: &Path,
    offline_user: &str,
    offline_pwd: &str,
    online_user: &str,
    online_pwd: &str,
) -> Result<()> {
    let sandbox_dir = sandbox_dir(codex_home);
    std::fs::create_dir_all(&sandbox_dir)?;
    let offline_blob = dpapi_protect(offline_pwd.as_bytes())?;
    let online_blob = dpapi_protect(online_pwd.as_bytes())?;
    let users = SandboxUsersFile {
        version: SETUP_VERSION,
        offline: SandboxUserRecord {
            username: offline_user.to_string(),
            password: BASE64.encode(offline_blob),
        },
        online: SandboxUserRecord {
            username: online_user.to_string(),
            password: BASE64.encode(online_blob),
        },
    };
    let marker = SetupMarker {
        version: SETUP_VERSION,
        offline_username: offline_user.to_string(),
        online_username: online_user.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let users_path = sandbox_dir.join("sandbox_users.json");
    let marker_path = sandbox_dir.join("setup_marker.json");
    std::fs::write(users_path, serde_json::to_vec_pretty(&users)?)?;
    std::fs::write(marker_path, serde_json::to_vec_pretty(&marker)?)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        anyhow::bail!("expected payload argument");
    }
    let payload_b64 = args.remove(1);
    let payload_json = BASE64
        .decode(payload_b64)
        .context("failed to decode payload b64")?;
    let payload: Payload =
        serde_json::from_slice(&payload_json).context("failed to parse payload json")?;
    if payload.version != SETUP_VERSION {
        anyhow::bail!("setup version mismatch");
    }
    let log_path = payload.codex_home.join("codex_sbx_setup.log");
    std::fs::create_dir_all(&payload.codex_home)?;
    let mut log = File::options()
        .create(true)
        .append(true)
        .open(&log_path)
        .context("open log")?;
    log_line(&mut log, "setup binary started")?;
    let offline_pwd = random_password();
    let online_pwd = random_password();
    log_line(
        &mut log,
        &format!(
            "ensuring sandbox users offline={} online={}",
            payload.offline_username, payload.online_username
        ),
    )?;
    ensure_local_user(&payload.offline_username, &offline_pwd, &mut log)?;
    ensure_local_user(&payload.online_username, &online_pwd, &mut log)?;
    let offline_sid = resolve_sid(&payload.offline_username)?;
    let online_sid = resolve_sid(&payload.online_username)?;
    let offline_psid = sid_bytes_to_psid(&offline_sid)?;
    let online_psid = sid_bytes_to_psid(&online_sid)?;
    let system_roots = collect_system_roots();
    let offline_sid_str = sid_to_string(&offline_sid)?;
    log_line(
        &mut log,
        &format!(
            "resolved SIDs offline={} online={}",
            offline_sid_str,
            sid_to_string(&online_sid)?
        ),
    )?;
    run_netsh_firewall(&offline_sid_str, &mut log)?;

    for root in &payload.read_roots {
        if !root.exists() {
            continue;
        }
        let mut skipped = false;
        for trustee in ["Users", "Authenticated Users", "Everyone"] {
            if trustee_has_rx(root, trustee).unwrap_or(false) {
                log_line(
                    &mut log,
                    &format!("{trustee} already has RX on {}; skipping", root.display()),
                )?;
                skipped = true;
                break;
            }
        }
        if skipped {
            continue;
        }
        if system_roots.contains(root) {
            log_line(
                &mut log,
                &format!(
                    "system root {} missing RX for Users/AU/Everyone; skipping to avoid hang",
                    root.display()
                ),
            )?;
            continue;
        }
        log_line(
            &mut log,
            &format!("granting read ACE to {} for sandbox users", root.display()),
        )?;
        let read_mask = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
        for (label, sid_bytes) in [("offline", &offline_sid), ("online", &online_sid)] {
            match try_add_inheritable_allow_with_timeout(
                root,
                sid_bytes,
                read_mask,
                &mut log,
                Duration::from_millis(25),
            ) {
                Ok(_) => {}
                Err(e) => {
                    log_line(
                        &mut log,
                        &format!(
                            "grant read ACE timed out/failed on {} for {label}: {e}",
                            root.display()
                        ),
                    )?;
                    // Best-effort: skip to next root.
                    continue;
                }
            }
        }
        log_line(&mut log, &format!("granted read ACE to {}", root.display()))?;
    }

    for root in &payload.write_roots {
        if !root.exists() {
            continue;
        }
        log_line(
            &mut log,
            &format!("granting write ACE to {} for sandbox users", root.display()),
        )?;
        unsafe {
            add_allow_ace(root, offline_psid)
                .with_context(|| format!("failed to grant write ACE on {}", root.display()))?;
            add_allow_ace(root, online_psid)
                .with_context(|| format!("failed to grant write ACE on {}", root.display()))?;
        }
        log_line(
            &mut log,
            &format!("granted write ACE to {}", root.display()),
        )?;
    }

    lock_sandbox_dir(
        &sandbox_dir(&payload.codex_home),
        &payload.real_user,
        &mut log,
    )?;
    log_line(&mut log, "sandbox dir ACL applied")?;
    write_secrets(
        &payload.codex_home,
        &payload.offline_username,
        &offline_pwd,
        &payload.online_username,
        &online_pwd,
    )?;
    log_line(
        &mut log,
        "sandbox users and marker written (sandbox_users.json, setup_marker.json)",
    )?;
    unsafe {
        if !offline_psid.is_null() {
            LocalFree(offline_psid as HLOCAL);
        }
        if !online_psid.is_null() {
            LocalFree(online_psid as HLOCAL);
        }
    }
    log_line(&mut log, "setup binary completed")?;
    Ok(())
}
