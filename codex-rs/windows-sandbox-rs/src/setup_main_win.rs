#![cfg(target_os = "windows")]

use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use codex_windows_sandbox::convert_string_sid_to_sid;
use codex_windows_sandbox::dpapi_protect;
use codex_windows_sandbox::ensure_allow_write_aces;
use codex_windows_sandbox::fetch_dacl_handle;
use codex_windows_sandbox::load_or_create_cap_sids;
use codex_windows_sandbox::log_note;
use codex_windows_sandbox::path_mask_allows;
use codex_windows_sandbox::sandbox_dir;
use codex_windows_sandbox::string_from_sid_bytes;
use codex_windows_sandbox::LOG_FILE_NAME;
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
use windows_sys::Win32::Storage::FileSystem::FILE_DELETE_CHILD;
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
    #[serde(default)]
    refresh_only: bool,
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
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
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

fn sid_bytes_to_psid(sid: &[u8]) -> Result<*mut c_void> {
    let sid_str = string_from_sid_bytes(sid).map_err(anyhow::Error::msg)?;
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

fn add_inheritable_allow_no_log(path: &Path, sid: &[u8], mask: u32) -> Result<()> {
    unsafe {
        let mut psid: *mut c_void = std::ptr::null_mut();
        let sid_str = string_from_sid_bytes(sid).map_err(anyhow::Error::msg)?;
        let sid_w = to_wide(OsStr::new(&sid_str));
        if ConvertStringSidToSidW(sid_w.as_ptr(), &mut psid) == 0 {
            return Err(anyhow::anyhow!(
                "ConvertStringSidToSidW failed: {}",
                GetLastError()
            ));
        }
        let (existing_dacl, sd) = fetch_dacl_handle(path)?;
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
            to_wide(path.as_os_str()).as_ptr() as *mut u16,
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

fn lock_sandbox_dir(
    dir: &Path,
    real_user: &str,
    sandbox_user_sids: &[Vec<u8>],
    log: &mut File,
) -> Result<()> {
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
    let sandbox_entries: Vec<(Vec<u8>, u32)> = sandbox_user_sids
        .iter()
        .map(|sid| {
            (
                sid.clone(),
                FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
            )
        })
        .collect();
    unsafe {
        let mut eas: Vec<EXPLICIT_ACCESS_W> = Vec::new();
        let mut sids: Vec<*mut c_void> = Vec::new();
        for (sid_bytes, mask) in entries
            .iter()
            .map(|(s, m)| (s, *m))
            .chain(sandbox_entries.iter().map(|(s, m)| (s, *m)))
        {
            let sid_str = string_from_sid_bytes(sid_bytes).map_err(anyhow::Error::msg)?;
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
    _read_roots: &[PathBuf],
    _write_roots: &[PathBuf],
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
        read_roots: Vec::new(),
        write_roots: Vec::new(),
    };
    let users_path = sandbox_dir.join("sandbox_users.json");
    let marker_path = sandbox_dir.join("setup_marker.json");
    std::fs::write(users_path, serde_json::to_vec_pretty(&users)?)?;
    std::fs::write(marker_path, serde_json::to_vec_pretty(&marker)?)?;
    Ok(())
}

pub fn main() -> Result<()> {
    let ret = real_main();
    if let Err(e) = &ret {
        // Best-effort: log unexpected top-level errors.
        if let Ok(codex_home) = std::env::var("CODEX_HOME") {
            let sbx_dir = sandbox_dir(Path::new(&codex_home));
            let _ = std::fs::create_dir_all(&sbx_dir);
            let log_path = sbx_dir.join(LOG_FILE_NAME);
            if let Ok(mut f) = File::options().create(true).append(true).open(&log_path) {
                let _ = writeln!(
                    f,
                    "[{}] top-level error: {}",
                    chrono::Utc::now().to_rfc3339(),
                    e
                );
            }
        }
    }
    ret
}

fn real_main() -> Result<()> {
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
    let sbx_dir = sandbox_dir(&payload.codex_home);
    std::fs::create_dir_all(&sbx_dir)?;
    let log_path = sbx_dir.join(LOG_FILE_NAME);
    let mut log = File::options()
        .create(true)
        .append(true)
        .open(&log_path)
        .context("open log")?;
    log_line(&mut log, "setup binary started")?;
    log_note("setup binary started", Some(sbx_dir.as_path()));
    let result = run_setup(&payload, &mut log, &sbx_dir);
    if let Err(err) = &result {
        let _ = log_line(&mut log, &format!("setup error: {err:?}"));
        log_note(&format!("setup error: {err:?}"), Some(sbx_dir.as_path()));
    }
    result
}

fn run_setup(payload: &Payload, log: &mut File, sbx_dir: &Path) -> Result<()> {
    let refresh_only = payload.refresh_only;
    let offline_pwd = if refresh_only {
        None
    } else {
        Some(random_password())
    };
    let online_pwd = if refresh_only {
        None
    } else {
        Some(random_password())
    };
    if refresh_only {
        log_line(log, "refresh-only mode: skipping user creation/firewall")?;
    } else {
        log_line(
            log,
            &format!(
                "ensuring sandbox users offline={} online={}",
                payload.offline_username, payload.online_username
            ),
        )?;
        ensure_local_user(
            &payload.offline_username,
            offline_pwd.as_ref().unwrap(),
            log,
        )?;
        ensure_local_user(&payload.online_username, online_pwd.as_ref().unwrap(), log)?;
    }
    let offline_sid = resolve_sid(&payload.offline_username)?;
    let online_sid = resolve_sid(&payload.online_username)?;
    let offline_psid = sid_bytes_to_psid(&offline_sid)?;
    let online_psid = sid_bytes_to_psid(&online_sid)?;
    let offline_sid_str = string_from_sid_bytes(&offline_sid).map_err(anyhow::Error::msg)?;
    log_line(
        log,
        &format!(
            "resolved SIDs offline={} online={}",
            offline_sid_str,
            string_from_sid_bytes(&online_sid).map_err(anyhow::Error::msg)?
        ),
    )?;
    let caps = load_or_create_cap_sids(&payload.codex_home);
    let cap_psid = unsafe {
        convert_string_sid_to_sid(&caps.workspace)
            .ok_or_else(|| anyhow::anyhow!("convert capability SID failed"))?
    };
    let mut refresh_errors: Vec<String> = Vec::new();
    let users_sid = resolve_sid("Users")?;
    let users_psid = sid_bytes_to_psid(&users_sid)?;
    let auth_sid = resolve_sid("Authenticated Users")?;
    let auth_psid = sid_bytes_to_psid(&auth_sid)?;
    let everyone_sid = resolve_sid("Everyone")?;
    let everyone_psid = sid_bytes_to_psid(&everyone_sid)?;
    let rx_psids = vec![users_psid, auth_psid, everyone_psid];
    log_line(log, &format!("resolved capability SID {}", caps.workspace))?;
    if !refresh_only {
        run_netsh_firewall(&offline_sid_str, log)?;
    }

    log_line(
        log,
        &format!(
            "refresh: processing {} read roots, {} write roots",
            payload.read_roots.len(),
            payload.write_roots.len()
        ),
    )?;
    for root in &payload.read_roots {
        if !root.exists() {
            log_line(
                log,
                &format!("read root {} missing; skipping", root.display()),
            )?;
            continue;
        }
        match path_mask_allows(
            root,
            &rx_psids,
            FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
            true,
        ) {
            Ok(has) => {
                if has {
                    log_line(
                        log,
                        &format!(
                            "Users/AU/Everyone already has RX on {}; skipping",
                            root.display()
                        ),
                    )?;
                    continue;
                }
            }
            Err(e) => {
                refresh_errors.push(format!(
                    "read mask check failed on {}: {}",
                    root.display(),
                    e
                ));
                log_line(
                    log,
                    &format!(
                        "read mask check failed on {}: {}; continuing",
                        root.display(),
                        e
                    ),
                )?;
            }
        }
        log_line(
            log,
            &format!("granting read ACE to {} for sandbox users", root.display()),
        )?;
        let mut successes = 0usize;
        let read_mask = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
        for (label, sid_bytes) in [("offline", &offline_sid), ("online", &online_sid)] {
            match try_add_inheritable_allow_with_timeout(
                root,
                sid_bytes,
                read_mask,
                log,
                Duration::from_millis(100),
            ) {
                Ok(_) => {
                    successes += 1;
                }
                Err(e) => {
                    log_line(
                        log,
                        &format!(
                            "grant read ACE timed out/failed on {} for {label}: {e}",
                            root.display()
                        ),
                    )?;
                    // Best-effort: continue to next SID/root.
                }
            }
        }
        if successes == 2 {
            log_line(log, &format!("granted read ACE to {}", root.display()))?;
        } else {
            log_line(
                log,
                &format!(
                    "read ACE incomplete on {} (success {}/2)",
                    root.display(),
                    successes
                ),
            )?;
        }
    }

    for root in &payload.write_roots {
        if !root.exists() {
            log_line(
                log,
                &format!("write root {} missing; skipping", root.display()),
            )?;
            continue;
        }
        let sids = vec![offline_psid, online_psid, cap_psid];
        let write_mask = FILE_GENERIC_READ
            | FILE_GENERIC_WRITE
            | FILE_GENERIC_EXECUTE
            | DELETE
            | FILE_DELETE_CHILD;
        let mut need_grant = false;
        for (label, psid) in [
            ("offline", offline_psid),
            ("online", online_psid),
            ("cap", cap_psid),
        ] {
            let has = match path_mask_allows(root, &[psid], write_mask, true) {
                Ok(h) => h,
                Err(e) => {
                    refresh_errors.push(format!(
                        "write mask check failed on {} for {label}: {}",
                        root.display(),
                        e
                    ));
                    log_line(
                        log,
                        &format!(
                            "write mask check failed on {} for {label}: {}; continuing",
                            root.display(),
                            e
                        ),
                    )?;
                    false
                }
            };
            log_line(
                log,
                &format!(
                    "write check {label} on {} => {}",
                    root.display(),
                    if has { "present" } else { "missing" }
                ),
            )?;
            if !has {
                need_grant = true;
            }
        }
        if need_grant {
            log_line(
                log,
                &format!(
                    "granting write ACE to {} for sandbox users and capability SID",
                    root.display()
                ),
            )?;
            match unsafe { ensure_allow_write_aces(root, &sids) } {
                Ok(res) => {
                    log_line(
                        log,
                        &format!(
                            "write ACE {} on {}",
                            if res { "added" } else { "already present" },
                            root.display()
                        ),
                    )?;
                }
                Err(e) => {
                    refresh_errors.push(format!("write ACE failed on {}: {}", root.display(), e));
                    log_line(
                        log,
                        &format!("write ACE grant failed on {}: {}", root.display(), e),
                    )?;
                }
            }
        } else {
            log_line(
                log,
                &format!(
                    "write ACE already present for all sandbox SIDs on {}",
                    root.display()
                ),
            )?;
        }
    }

    if refresh_only {
        log_line(
            log,
            &format!(
                "setup refresh: processed {} read roots, {} write roots; errors={:?}",
                payload.read_roots.len(),
                payload.write_roots.len(),
                refresh_errors
            ),
        )?;
    }
    if !refresh_only {
        lock_sandbox_dir(
            &sandbox_dir(&payload.codex_home),
            &payload.real_user,
            &[offline_sid.clone(), online_sid.clone()],
            log,
        )?;
        log_line(log, "sandbox dir ACL applied")?;
        write_secrets(
            &payload.codex_home,
            &payload.offline_username,
            offline_pwd.as_ref().unwrap(),
            &payload.online_username,
            online_pwd.as_ref().unwrap(),
            &payload.read_roots,
            &payload.write_roots,
        )?;
        log_line(
            log,
            "sandbox users and marker written (sandbox_users.json, setup_marker.json)",
        )?;
    }
    unsafe {
        if !offline_psid.is_null() {
            LocalFree(offline_psid as HLOCAL);
        }
        if !online_psid.is_null() {
            LocalFree(online_psid as HLOCAL);
        }
        if !cap_psid.is_null() {
            LocalFree(cap_psid as HLOCAL);
        }
        if !users_psid.is_null() {
            LocalFree(users_psid as HLOCAL);
        }
        if !auth_psid.is_null() {
            LocalFree(auth_psid as HLOCAL);
        }
        if !everyone_psid.is_null() {
            LocalFree(everyone_psid as HLOCAL);
        }
    }
    if refresh_only && !refresh_errors.is_empty() {
        log_line(
            log,
            &format!("setup refresh completed with errors: {:?}", refresh_errors),
        )?;
        anyhow::bail!("setup refresh had errors");
    }
    log_line(log, "setup binary completed")?;
    log_note("setup binary completed", Some(sbx_dir));
    Ok(())
}
