#![cfg(target_os = "windows")]

use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use codex_windows_sandbox::convert_string_sid_to_sid;
use codex_windows_sandbox::dpapi_protect;
use codex_windows_sandbox::ensure_allow_mask_aces_with_inheritance;
use codex_windows_sandbox::ensure_allow_write_aces;
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
use std::collections::HashSet;
use std::ffi::c_void;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
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

mod read_acl_mutex;
use read_acl_mutex::acquire_read_acl_mutex;
use read_acl_mutex::read_acl_mutex_exists;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Payload {
    version: u32,
    offline_username: String,
    online_username: String,
    codex_home: PathBuf,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    real_user: String,
    #[serde(default)]
    mode: SetupMode,
    #[serde(default)]
    refresh_only: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum SetupMode {
    Full,
    ReadAclsOnly,
}

impl Default for SetupMode {
    fn default() -> Self {
        Self::Full
    }
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

fn spawn_read_acl_helper(payload: &Payload, _log: &mut File) -> Result<()> {
    let mut read_payload = payload.clone();
    read_payload.mode = SetupMode::ReadAclsOnly;
    read_payload.refresh_only = true;
    let payload_json = serde_json::to_vec(&read_payload)?;
    let payload_b64 = BASE64.encode(payload_json);
    let exe = std::env::current_exe().context("locate setup helper")?;
    Command::new(&exe)
        .arg(payload_b64)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .context("spawn read ACL helper")?;
    Ok(())
}

struct ReadAclSubjects<'a> {
    offline_psid: *mut c_void,
    online_psid: *mut c_void,
    rx_psids: &'a [*mut c_void],
}

fn apply_read_acls(
    read_roots: &[PathBuf],
    subjects: &ReadAclSubjects<'_>,
    log: &mut File,
    refresh_errors: &mut Vec<String>,
    access_mask: u32,
    access_label: &str,
    inheritance: u32,
) -> Result<()> {
    for root in read_roots {
        if !root.exists() {
            log_line(
                log,
                &format!("{access_label} root {} missing; skipping", root.display()),
            )?;
            continue;
        }
        let builtin_has = read_mask_allows_or_log(
            root,
            subjects.rx_psids,
            None,
            access_mask,
            access_label,
            refresh_errors,
            log,
        )?;
        if builtin_has {
            continue;
        }
        let offline_has = read_mask_allows_or_log(
            root,
            &[subjects.offline_psid],
            Some("offline"),
            access_mask,
            access_label,
            refresh_errors,
            log,
        )?;
        let online_has = read_mask_allows_or_log(
            root,
            &[subjects.online_psid],
            Some("online"),
            access_mask,
            access_label,
            refresh_errors,
            log,
        )?;
        if offline_has && online_has {
            continue;
        }
        log_line(
            log,
            &format!(
                "granting {access_label} ACE to {} for sandbox users",
                root.display()
            ),
        )?;
        let mut successes = usize::from(offline_has) + usize::from(online_has);
        let mut missing_psids: Vec<*mut c_void> = Vec::new();
        let mut missing_labels: Vec<&str> = Vec::new();
        if !offline_has {
            missing_psids.push(subjects.offline_psid);
            missing_labels.push("offline");
        }
        if !online_has {
            missing_psids.push(subjects.online_psid);
            missing_labels.push("online");
        }
        if !missing_psids.is_empty() {
            let result = unsafe {
                ensure_allow_mask_aces_with_inheritance(
                    root,
                    &missing_psids,
                    access_mask,
                    inheritance,
                )
            };
            match result {
                Ok(_) => {
                    successes = 2;
                }
                Err(err) => {
                    let label_list = missing_labels.join(", ");
                    for label in &missing_labels {
                        refresh_errors.push(format!(
                            "grant {access_label} ACE failed on {} for {label}: {err}",
                            root.display()
                        ));
                    }
                    log_line(
                        log,
                        &format!(
                            "grant {access_label} ACE failed on {} for {}: {err}",
                            root.display(),
                            label_list
                        ),
                    )?;
                }
            }
        }
        if successes == 2 {
        } else {
            log_line(
                log,
                &format!(
                    "{access_label} ACE incomplete on {} (success {successes}/2)",
                    root.display()
                ),
            )?;
        }
    }
    Ok(())
}

fn read_mask_allows_or_log(
    root: &Path,
    psids: &[*mut c_void],
    label: Option<&str>,
    read_mask: u32,
    access_label: &str,
    refresh_errors: &mut Vec<String>,
    log: &mut File,
) -> Result<bool> {
    match path_mask_allows(root, psids, read_mask, true) {
        Ok(has) => Ok(has),
        Err(e) => {
            let label_suffix = label
                .map(|value| format!(" for {value}"))
                .unwrap_or_default();
            refresh_errors.push(format!(
                "{access_label} mask check failed on {}{}: {}",
                root.display(),
                label_suffix,
                e
            ));
            log_line(
                log,
                &format!(
                    "{access_label} mask check failed on {}{}: {}; continuing",
                    root.display(),
                    label_suffix,
                    e
                ),
            )?;
            Ok(false)
        }
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
    _log: &mut File,
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
    let result = run_setup(&payload, &mut log, &sbx_dir);
    if let Err(err) = &result {
        let _ = log_line(&mut log, &format!("setup error: {err:?}"));
        log_note(&format!("setup error: {err:?}"), Some(sbx_dir.as_path()));
    }
    result
}

fn run_setup(payload: &Payload, log: &mut File, sbx_dir: &Path) -> Result<()> {
    match payload.mode {
        SetupMode::ReadAclsOnly => run_read_acl_only(payload, log),
        SetupMode::Full => run_setup_full(payload, log, sbx_dir),
    }
}

fn run_read_acl_only(payload: &Payload, log: &mut File) -> Result<()> {
    let _read_acl_guard = match acquire_read_acl_mutex()? {
        Some(guard) => guard,
        None => {
            log_line(log, "read ACL helper already running; skipping")?;
            return Ok(());
        }
    };
    log_line(log, "read-acl-only mode: applying read ACLs")?;
    let offline_sid = resolve_sid(&payload.offline_username)?;
    let online_sid = resolve_sid(&payload.online_username)?;
    let offline_psid = sid_bytes_to_psid(&offline_sid)?;
    let online_psid = sid_bytes_to_psid(&online_sid)?;
    let mut refresh_errors: Vec<String> = Vec::new();
    let users_sid = resolve_sid("Users")?;
    let users_psid = sid_bytes_to_psid(&users_sid)?;
    let auth_sid = resolve_sid("Authenticated Users")?;
    let auth_psid = sid_bytes_to_psid(&auth_sid)?;
    let everyone_sid = resolve_sid("Everyone")?;
    let everyone_psid = sid_bytes_to_psid(&everyone_sid)?;
    let rx_psids = vec![users_psid, auth_psid, everyone_psid];
    let subjects = ReadAclSubjects {
        offline_psid,
        online_psid,
        rx_psids: &rx_psids,
    };
    apply_read_acls(
        &payload.read_roots,
        &subjects,
        log,
        &mut refresh_errors,
        FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
        "read",
        OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE,
    )?;
    unsafe {
        if !offline_psid.is_null() {
            LocalFree(offline_psid as HLOCAL);
        }
        if !online_psid.is_null() {
            LocalFree(online_psid as HLOCAL);
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
    if !refresh_errors.is_empty() {
        log_line(
            log,
            &format!("read ACL run completed with errors: {:?}", refresh_errors),
        )?;
        if payload.refresh_only {
            anyhow::bail!("read ACL run had errors");
        }
    }
    log_line(log, "read ACL run completed")?;
    Ok(())
}

fn run_setup_full(payload: &Payload, log: &mut File, sbx_dir: &Path) -> Result<()> {
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

    let caps = load_or_create_cap_sids(&payload.codex_home)?;
    let cap_psid = unsafe {
        convert_string_sid_to_sid(&caps.workspace)
            .ok_or_else(|| anyhow::anyhow!("convert capability SID failed"))?
    };
    let mut refresh_errors: Vec<String> = Vec::new();
    if !refresh_only {
        run_netsh_firewall(&offline_sid_str, log)?;
    }

    if payload.read_roots.is_empty() {
        log_line(log, "no read roots to grant; skipping read ACL helper")?;
    } else {
        match read_acl_mutex_exists() {
            Ok(true) => {
                log_line(log, "read ACL helper already running; skipping spawn")?;
            }
            Ok(false) => {
                spawn_read_acl_helper(payload, log)?;
            }
            Err(err) => {
                log_line(
                    log,
                    &format!("read ACL mutex check failed: {err}; spawning anyway"),
                )?;
                spawn_read_acl_helper(payload, log)?;
            }
        }
    }

    let cap_sid_str = caps.workspace.clone();
    let online_sid_str = string_from_sid_bytes(&online_sid).map_err(anyhow::Error::msg)?;
    let sid_strings = vec![offline_sid_str.clone(), online_sid_str, cap_sid_str];
    let write_mask =
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE | FILE_DELETE_CHILD;
    let mut grant_tasks: Vec<PathBuf> = Vec::new();

    let mut seen_write_roots: HashSet<PathBuf> = HashSet::new();

    for root in &payload.write_roots {
        if !seen_write_roots.insert(root.clone()) {
            continue;
        }
        if !root.exists() {
            log_line(
                log,
                &format!("write root {} missing; skipping", root.display()),
            )?;
            continue;
        }
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
            grant_tasks.push(root.clone());
        }
    }

    let (tx, rx) = mpsc::channel::<(PathBuf, Result<bool>)>();
    std::thread::scope(|scope| {
        for root in grant_tasks {
            let sid_strings = sid_strings.clone();
            let tx = tx.clone();
            scope.spawn(move || {
                // Convert SID strings to psids locally in this thread.
                let mut psids: Vec<*mut c_void> = Vec::new();
                for sid_str in &sid_strings {
                    if let Some(psid) = unsafe { convert_string_sid_to_sid(sid_str) } {
                        psids.push(psid);
                    } else {
                        let _ = tx.send((root.clone(), Err(anyhow::anyhow!("convert SID failed"))));
                        return;
                    }
                }

                let res = unsafe { ensure_allow_write_aces(&root, &psids) };

                for psid in psids {
                    unsafe {
                        LocalFree(psid as HLOCAL);
                    }
                }
                let _ = tx.send((root, res));
            });
        }
        drop(tx);
        for (root, res) in rx {
            match res {
                Ok(_) => {}
                Err(e) => {
                    refresh_errors.push(format!("write ACE failed on {}: {}", root.display(), e));
                    if log_line(
                        log,
                        &format!("write ACE grant failed on {}: {}", root.display(), e),
                    )
                    .is_err()
                    {
                        // ignore log errors inside scoped thread
                    }
                }
            }
        }
    });

    if refresh_only {
        log_line(
            log,
            &format!(
                "setup refresh: processed {} write roots (read roots delegated); errors={:?}",
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
        write_secrets(
            &payload.codex_home,
            &payload.offline_username,
            offline_pwd.as_ref().unwrap(),
            &payload.online_username,
            online_pwd.as_ref().unwrap(),
            &payload.read_roots,
            &payload.write_roots,
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
    }
    if refresh_only && !refresh_errors.is_empty() {
        log_line(
            log,
            &format!("setup refresh completed with errors: {:?}", refresh_errors),
        )?;
        anyhow::bail!("setup refresh had errors");
    }
    log_note("setup binary completed", Some(sbx_dir));
    Ok(())
}
