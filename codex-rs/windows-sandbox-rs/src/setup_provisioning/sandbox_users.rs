use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use rand::RngCore;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use serde::Serialize;
use std::ffi::OsStr;
use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::GENERIC_WRITE;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::NetworkManagement::NetManagement::LOCALGROUP_MEMBERS_INFO_3;
use windows_sys::Win32::NetworkManagement::NetManagement::NERR_Success;
use windows_sys::Win32::NetworkManagement::NetManagement::NetLocalGroupAddMembers;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserAdd;
use windows_sys::Win32::NetworkManagement::NetManagement::NetUserSetInfo;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_DONT_EXPIRE_PASSWD;
use windows_sys::Win32::NetworkManagement::NetManagement::UF_SCRIPT;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_INFO_1;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_INFO_1003;
use windows_sys::Win32::NetworkManagement::NetManagement::USER_PRIV_USER;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::Authorization::SDDL_REVISION_1;
use windows_sys::Win32::Security::LookupAccountSidW;
use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Security::SID_NAME_USE;
use windows_sys::Win32::Storage::FileSystem::CREATE_NEW;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;

use crate::SANDBOX_USERS_GROUP;
use crate::SETUP_VERSION;
use crate::SetupErrorCode;
use crate::SetupFailure;
use crate::dpapi_protect;
use crate::ensure_sandbox_users_group;
use crate::resolve_sid;
use crate::sandbox_dir;
use crate::sandbox_secrets_dir;
use crate::string_from_sid_bytes;
use crate::to_wide;
use crate::write_file_atomically;

use super::SetupMode;

#[cfg(test)]
#[path = "sandbox_users_tests.rs"]
mod tests;

const SID_USERS: &str = "S-1-5-32-545";

pub fn resolve_sandbox_users_group_sid() -> Result<Vec<u8>> {
    resolve_sid(SANDBOX_USERS_GROUP)
}

pub(super) fn provision_sandbox_users(
    codex_home: &Path,
    offline_username: &str,
    online_username: &str,
    new_user_flags: u32,
    log: &mut dyn Write,
    mode: SetupMode,
) -> Result<()> {
    if let Err(err) = ensure_sandbox_users_group() {
        let message = format!("failed to create local group {SANDBOX_USERS_GROUP}: {err}");
        super::log_line(log, &message)?;
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperUsersGroupCreateFailed,
            message,
        )));
    }
    super::log_line(
        log,
        &format!("ensuring sandbox users offline={offline_username} online={online_username}"),
    )?;
    let offline_password = random_password();
    let online_password = random_password();
    ensure_sandbox_user(offline_username, &offline_password, new_user_flags, log)?;
    ensure_sandbox_user(online_username, &online_password, new_user_flags, log)?;
    write_secrets(
        codex_home,
        offline_username,
        &offline_password,
        online_username,
        &online_password,
        mode,
    )?;
    Ok(())
}

pub fn ensure_sandbox_user(
    username: &str,
    password: &str,
    new_user_flags: u32,
    log: &mut dyn Write,
) -> Result<()> {
    ensure_local_user(username, password, new_user_flags, log)?;
    ensure_local_group_member(SANDBOX_USERS_GROUP, username)?;
    Ok(())
}

pub fn ensure_local_user(
    name: &str,
    password: &str,
    new_user_flags: u32,
    log: &mut dyn Write,
) -> Result<()> {
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
            usri1_flags: UF_SCRIPT | UF_DONT_EXPIRE_PASSWD | new_user_flags,
            usri1_script_path: std::ptr::null_mut(),
        };
        let status = NetUserAdd(
            std::ptr::null(),
            1,
            &info as *const _ as *mut u8,
            std::ptr::null_mut(),
        );
        if status != NERR_Success {
            // Reset only the password, preserving account flags owned by enterprise policy.
            // A separate flag update could fail after the reset but before credentials are saved.
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
                super::log_line(log, &format!("NetUserSetInfo failed for {name} code {upd}"))?;
                return Err(anyhow::Error::new(SetupFailure::new(
                    SetupErrorCode::HelperUserCreateOrUpdateFailed,
                    format!("failed to create/update user {name}, code {status}/{upd}"),
                )));
            }
        }

        // Ensure the principal is a regular local user account.
        if let Ok(group_name) = lookup_account_name_for_sid(SID_USERS) {
            let group = to_wide(OsStr::new(&group_name));
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
        } else {
            super::log_line(
                log,
                "LookupAccountSidW failed for Users SID; skipping Users group membership",
            )?;
        }
    }
    Ok(())
}

pub fn ensure_local_group_member(group_name: &str, member_name: &str) -> Result<()> {
    // If the member is already in the group, NetLocalGroupAddMembers may
    // return an error code. We don't care.
    let group_w = to_wide(OsStr::new(group_name));
    let member_w = to_wide(OsStr::new(member_name));
    unsafe {
        let member = LOCALGROUP_MEMBERS_INFO_3 {
            lgrmi3_domainandname: member_w.as_ptr() as *mut u16,
        };
        let _ = NetLocalGroupAddMembers(
            std::ptr::null(),
            group_w.as_ptr(),
            3,
            &member as *const _ as *mut u8,
            1,
        );
    }
    Ok(())
}

fn lookup_account_name_for_sid(sid_str: &str) -> Result<String> {
    let sid_w = to_wide(OsStr::new(sid_str));
    let mut psid: *mut c_void = std::ptr::null_mut();
    if unsafe { ConvertStringSidToSidW(sid_w.as_ptr(), &mut psid) } == 0 {
        return Err(anyhow::anyhow!(
            "ConvertStringSidToSidW failed for {sid_str}: {}",
            unsafe { GetLastError() }
        ));
    }
    let mut name_len: u32 = 0;
    let mut domain_len: u32 = 0;
    let mut use_type: SID_NAME_USE = 0;
    let ok = unsafe {
        LookupAccountSidW(
            std::ptr::null(),
            psid,
            std::ptr::null_mut(),
            &mut name_len,
            std::ptr::null_mut(),
            &mut domain_len,
            &mut use_type,
        )
    };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        if err != ERROR_INSUFFICIENT_BUFFER {
            unsafe {
                LocalFree(psid as _);
            }
            return Err(anyhow::anyhow!(
                "LookupAccountSidW preflight failed for {sid_str}: {err}"
            ));
        }
    }
    let mut name_buf: Vec<u16> = vec![0u16; name_len as usize];
    let mut domain_buf: Vec<u16> = vec![0u16; domain_len as usize];
    let ok = unsafe {
        LookupAccountSidW(
            std::ptr::null(),
            psid,
            name_buf.as_mut_ptr(),
            &mut name_len,
            domain_buf.as_mut_ptr(),
            &mut domain_len,
            &mut use_type,
        )
    };
    unsafe {
        LocalFree(psid as _);
    }
    if ok == 0 {
        return Err(anyhow::anyhow!(
            "LookupAccountSidW failed for {sid_str}: {}",
            unsafe { GetLastError() }
        ));
    }
    let name = String::from_utf16_lossy(&name_buf);
    Ok(name.trim_end_matches('\0').to_string())
}

pub fn sid_bytes_to_psid(sid: &[u8]) -> Result<*mut c_void> {
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
    proxy_ports: Vec<u16>,
    allow_local_binding: bool,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
}

fn write_secrets(
    codex_home: &Path,
    offline_user: &str,
    offline_pwd: &str,
    online_user: &str,
    online_pwd: &str,
    mode: SetupMode,
) -> Result<()> {
    let secrets_dir = sandbox_secrets_dir(codex_home);
    std::fs::create_dir_all(&secrets_dir).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperUsersFileWriteFailed,
            format!(
                "failed to create secrets dir {}: {err}",
                secrets_dir.display()
            ),
        ))
    })?;
    let offline_blob = dpapi_protect(offline_pwd.as_bytes()).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperDpapiProtectFailed,
            format!("dpapi protect failed for offline user: {err}"),
        ))
    })?;
    let online_blob = dpapi_protect(online_pwd.as_bytes()).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperDpapiProtectFailed,
            format!("dpapi protect failed for online user: {err}"),
        ))
    })?;
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
    let users_path = secrets_dir.join("sandbox_users.json");
    let users_json = serde_json::to_vec_pretty(&users).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperUsersFileWriteFailed,
            format!("serialize sandbox users failed: {err}"),
        ))
    })?;
    let write_result = match mode {
        SetupMode::ProvisionOnly => write_file_atomically(&users_path, &users_json),
        SetupMode::Full | SetupMode::InteractiveProvision | SetupMode::ReadAclsOnly => {
            std::fs::write(&users_path, users_json).map_err(anyhow::Error::from)
        }
    };
    write_result.map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperUsersFileWriteFailed,
            format!(
                "write sandbox users file {} failed: {err}",
                users_path.display()
            ),
        ))
    })?;
    Ok(())
}

/// Service provisioning retains the marker's exclusive handle; legacy setup reopens it at commit.
pub(super) enum PreparedSetupMarker {
    Retained(File),
    Reopen,
}

// Create the final marker with its protected ACL. The empty file intentionally fails readiness
// checks until setup succeeds. Only service provisioning pins it for the entire operation.
pub(super) fn prepare_setup_marker(
    codex_home: &Path,
    real_user: &str,
    mode: SetupMode,
) -> Result<PreparedSetupMarker> {
    let marker_path = sandbox_dir(codex_home).join("setup_marker.json");
    match std::fs::remove_file(&marker_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperSetupMarkerWriteFailed,
                format!(
                    "remove setup marker file {} failed: {err}",
                    marker_path.display()
                ),
            )));
        }
    }

    let real_user_sid = resolve_sid(real_user)
        .and_then(|sid| string_from_sid_bytes(&sid).map_err(anyhow::Error::msg))
        .map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperSetupMarkerWriteFailed,
                format!("resolve real user SID for setup marker failed: {err}"),
            ))
        })?;
    let sddl = to_wide(format!(
        "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{real_user_sid})"
    ));
    let mut security_descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut security_descriptor,
            std::ptr::null_mut(),
        )
    };
    if converted == 0 {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSetupMarkerWriteFailed,
            format!(
                "create setup marker security descriptor failed: {}",
                unsafe { GetLastError() }
            ),
        )));
    }

    let security_attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security_descriptor,
        bInheritHandle: 0,
    };
    let marker_path_wide = to_wide(marker_path.as_os_str());
    let marker_handle = unsafe {
        CreateFileW(
            marker_path_wide.as_ptr(),
            GENERIC_WRITE,
            /*dwsharemode*/ 0,
            &security_attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            /*htemplatefile*/ 0,
        )
    };
    let create_error = unsafe { GetLastError() };
    unsafe {
        LocalFree(security_descriptor as _);
    }
    if marker_handle == INVALID_HANDLE_VALUE {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSetupMarkerWriteFailed,
            format!(
                "create protected setup marker file {} failed: {}",
                marker_path.display(),
                create_error
            ),
        )));
    }
    let file = unsafe { File::from_raw_handle(marker_handle as *mut c_void) };
    match mode {
        SetupMode::ProvisionOnly => Ok(PreparedSetupMarker::Retained(file)),
        SetupMode::Full | SetupMode::InteractiveProvision | SetupMode::ReadAclsOnly => {
            drop(file);
            Ok(PreparedSetupMarker::Reopen)
        }
    }
}

pub(super) fn commit_setup_marker(
    file: PreparedSetupMarker,
    codex_home: &Path,
    offline_user: &str,
    online_user: &str,
    proxy_ports: &[u16],
    allow_local_binding: bool,
) -> Result<()> {
    let marker = SetupMarker {
        version: SETUP_VERSION,
        offline_username: offline_user.to_string(),
        online_username: online_user.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        proxy_ports: proxy_ports.to_vec(),
        allow_local_binding,
        read_roots: Vec::new(),
        write_roots: Vec::new(),
    };
    let marker_path = sandbox_dir(codex_home).join("setup_marker.json");
    let marker_json = serde_json::to_vec_pretty(&marker).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSetupMarkerWriteFailed,
            format!("serialize setup marker failed: {err}"),
        ))
    })?;
    let write_result = match file {
        PreparedSetupMarker::Retained(mut file) => file.write_all(&marker_json),
        PreparedSetupMarker::Reopen => std::fs::write(&marker_path, marker_json),
    };
    write_result.map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSetupMarkerWriteFailed,
            format!(
                "write setup marker file {} failed: {err}",
                marker_path.display()
            ),
        ))
    })?;
    Ok(())
}
