#![cfg(target_os = "windows")]

mod firewall;

use anyhow::Context;
use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use codex_windows_sandbox::canonicalize_path;
use codex_windows_sandbox::convert_string_sid_to_sid;
use codex_windows_sandbox::ensure_allow_mask_aces_with_inheritance;
use codex_windows_sandbox::ensure_allow_write_aces;
use codex_windows_sandbox::extract_setup_failure;
use codex_windows_sandbox::hide_newly_created_users;
use codex_windows_sandbox::is_command_cwd_root;
use codex_windows_sandbox::load_or_create_cap_sids;
use codex_windows_sandbox::log_note;
use codex_windows_sandbox::path_mask_allows;
use codex_windows_sandbox::protect_workspace_agents_dir;
use codex_windows_sandbox::protect_workspace_codex_dir;
use codex_windows_sandbox::sandbox_dir;
use codex_windows_sandbox::sandbox_secrets_dir;
use codex_windows_sandbox::string_from_sid_bytes;
use codex_windows_sandbox::to_wide;
use codex_windows_sandbox::workspace_cap_sid_for_cwd;
use codex_windows_sandbox::write_setup_error_report;
use codex_windows_sandbox::SetupErrorCode;
use codex_windows_sandbox::SetupErrorReport;
use codex_windows_sandbox::SetupFailure;
use codex_windows_sandbox::LOG_FILE_NAME;
use codex_windows_sandbox::SETUP_VERSION;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::ffi::c_void;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::Authorization::SetEntriesInAclW;
use windows_sys::Win32::Security::Authorization::SetNamedSecurityInfoW;
use windows_sys::Win32::Security::Authorization::EXPLICIT_ACCESS_W;
use windows_sys::Win32::Security::Authorization::GRANT_ACCESS;
use windows_sys::Win32::Security::Authorization::SE_FILE_OBJECT;
use windows_sys::Win32::Security::Authorization::TRUSTEE_IS_SID;
use windows_sys::Win32::Security::Authorization::TRUSTEE_W;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::CONTAINER_INHERIT_ACE;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::OBJECT_INHERIT_ACE;
use windows_sys::Win32::Storage::FileSystem::DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_DELETE_CHILD;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_EXECUTE;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;

const DENY_ACCESS: i32 = 3;

mod read_acl_mutex;
mod sandbox_users;
use read_acl_mutex::acquire_read_acl_mutex;
use read_acl_mutex::read_acl_mutex_exists;
use sandbox_users::provision_sandbox_users;
use sandbox_users::resolve_sandbox_users_group_sid;
use sandbox_users::resolve_sid;
use sandbox_users::sid_bytes_to_psid;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Payload {
    version: u32,
    offline_username: String,
    online_username: String,
    codex_home: PathBuf,
    command_cwd: PathBuf,
    read_roots: Vec<PathBuf>,
    write_roots: Vec<PathBuf>,
    real_user: String,
    #[serde(default)]
    mode: SetupMode,
    #[serde(default)]
    refresh_only: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
enum SetupMode {
    #[default]
    Full,
    ReadAclsOnly,
}

fn log_line(log: &mut File, msg: &str) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    writeln!(log, "[{ts}] {msg}").map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperLogFailed,
            format!("failed to write setup log line: {err}"),
        ))
    })?;
    Ok(())
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
    sandbox_group_psid: *mut c_void,
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
        let sandbox_has = read_mask_allows_or_log(
            root,
            &[subjects.sandbox_group_psid],
            Some("sandbox_group"),
            access_mask,
            access_label,
            refresh_errors,
            log,
        )?;
        if sandbox_has {
            continue;
        }
        log_line(
            log,
            &format!(
                "granting {access_label} ACE to {} for sandbox users",
                root.display()
            ),
        )?;
        let result = unsafe {
            ensure_allow_mask_aces_with_inheritance(
                root,
                &[subjects.sandbox_group_psid],
                access_mask,
                inheritance,
            )
        };
        if let Err(err) = result {
            refresh_errors.push(format!(
                "grant {access_label} ACE failed on {} for sandbox_group: {err}",
                root.display()
            ));
            log_line(
                log,
                &format!(
                    "grant {access_label} ACE failed on {} for sandbox_group: {err}",
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

fn lock_sandbox_dir(
    dir: &Path,
    real_user: &str,
    sandbox_group_sid: &[u8],
    sandbox_group_access_mode: i32,
    _log: &mut File,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let system_sid = resolve_sid("SYSTEM")?;
    let admins_sid = resolve_sid("Administrators")?;
    let real_sid = resolve_sid(real_user)?;
    let entries = [
        (
            sandbox_group_sid.to_vec(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
            sandbox_group_access_mode,
        ),
        (
            system_sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
            GRANT_ACCESS,
        ),
        (
            admins_sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE,
            GRANT_ACCESS,
        ),
        (
            real_sid,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE,
            GRANT_ACCESS,
        ),
    ];
    unsafe {
        let mut eas: Vec<EXPLICIT_ACCESS_W> = Vec::new();
        let mut sids: Vec<*mut c_void> = Vec::new();
        for (sid_bytes, mask, access_mode) in entries.iter().map(|(s, m, a)| (s, *m, *a)) {
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
                grfAccessMode: access_mode,
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
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperRequestArgsFailed,
            "expected payload argument",
        )));
    }
    let payload_b64 = args.remove(1);
    let payload_json = BASE64.decode(payload_b64).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperRequestArgsFailed,
            format!("failed to decode payload b64: {err}"),
        ))
    })?;
    let payload: Payload = serde_json::from_slice(&payload_json).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperRequestArgsFailed,
            format!("failed to parse payload json: {err}"),
        ))
    })?;
    if payload.version != SETUP_VERSION {
        return Err(anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperRequestArgsFailed,
            format!(
                "setup version mismatch: expected {SETUP_VERSION}, got {}",
                payload.version
            ),
        )));
    }
    let sbx_dir = sandbox_dir(&payload.codex_home);
    std::fs::create_dir_all(&sbx_dir).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSandboxDirCreateFailed,
            format!("failed to create sandbox dir {}: {err}", sbx_dir.display()),
        ))
    })?;
    let log_path = sbx_dir.join(LOG_FILE_NAME);
    let mut log = File::options()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperLogFailed,
                format!("open log {} failed: {err}", log_path.display()),
            ))
        })?;
    let result = run_setup(&payload, &mut log, &sbx_dir);
    if let Err(err) = &result {
        let _ = log_line(&mut log, &format!("setup error: {err:?}"));
        log_note(&format!("setup error: {err:?}"), Some(sbx_dir.as_path()));
        let failure = extract_setup_failure(err)
            .map(|f| SetupFailure::new(f.code, f.message.clone()))
            .unwrap_or_else(|| {
                SetupFailure::new(SetupErrorCode::HelperUnknownError, err.to_string())
            });
        let report = SetupErrorReport {
            code: failure.code,
            message: failure.message.clone(),
        };
        if let Err(write_err) = write_setup_error_report(&payload.codex_home, &report) {
            let _ = log_line(
                &mut log,
                &format!("setup error report write failed: {write_err}"),
            );
            log_note(
                &format!("setup error report write failed: {write_err}"),
                Some(sbx_dir.as_path()),
            );
        }
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
    let sandbox_group_sid = resolve_sandbox_users_group_sid()?;
    let sandbox_group_psid = sid_bytes_to_psid(&sandbox_group_sid)?;
    let mut refresh_errors: Vec<String> = Vec::new();
    let users_sid = resolve_sid("Users")?;
    let users_psid = sid_bytes_to_psid(&users_sid)?;
    let auth_sid = resolve_sid("Authenticated Users")?;
    let auth_psid = sid_bytes_to_psid(&auth_sid)?;
    let everyone_sid = resolve_sid("Everyone")?;
    let everyone_psid = sid_bytes_to_psid(&everyone_sid)?;
    let rx_psids = vec![users_psid, auth_psid, everyone_psid];
    let subjects = ReadAclSubjects {
        sandbox_group_psid,
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
        if !sandbox_group_psid.is_null() {
            LocalFree(sandbox_group_psid as HLOCAL);
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
    if refresh_only {
    } else {
        let provision_result = provision_sandbox_users(
            &payload.codex_home,
            &payload.offline_username,
            &payload.online_username,
            log,
        );
        if let Err(err) = provision_result {
            if extract_setup_failure(&err).is_some() {
                return Err(err);
            }
            return Err(anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperUserProvisionFailed,
                format!("provision sandbox users failed: {err}"),
            )));
        }
        let users = vec![
            payload.offline_username.clone(),
            payload.online_username.clone(),
        ];
        hide_newly_created_users(&users, sbx_dir);
    }
    let offline_sid = resolve_sid(&payload.offline_username).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSidResolveFailed,
            format!(
                "resolve SID for offline user {} failed: {err}",
                payload.offline_username
            ),
        ))
    })?;
    let offline_sid_str = string_from_sid_bytes(&offline_sid).map_err(anyhow::Error::msg)?;

    let sandbox_group_sid = resolve_sandbox_users_group_sid().map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSidResolveFailed,
            format!("resolve sandbox users group SID failed: {err}"),
        ))
    })?;
    let sandbox_group_psid = sid_bytes_to_psid(&sandbox_group_sid).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperSidResolveFailed,
            format!("convert sandbox users group SID to PSID failed: {err}"),
        ))
    })?;

    let caps = load_or_create_cap_sids(&payload.codex_home).map_err(|err| {
        anyhow::Error::new(SetupFailure::new(
            SetupErrorCode::HelperCapabilitySidFailed,
            format!("load or create capability SIDs failed: {err}"),
        ))
    })?;
    let cap_psid = unsafe {
        convert_string_sid_to_sid(&caps.workspace).ok_or_else(|| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperCapabilitySidFailed,
                format!("convert capability SID {} failed", caps.workspace),
            ))
        })?
    };
    let workspace_sid_str = workspace_cap_sid_for_cwd(&payload.codex_home, &payload.command_cwd)?;
    let workspace_psid = unsafe {
        convert_string_sid_to_sid(&workspace_sid_str)
            .ok_or_else(|| anyhow::anyhow!("convert workspace capability SID failed"))?
    };
    let mut refresh_errors: Vec<String> = Vec::new();
    if !refresh_only {
        let firewall_result = firewall::ensure_offline_outbound_block(&offline_sid_str, log);
        if let Err(err) = firewall_result {
            if extract_setup_failure(&err).is_some() {
                return Err(err);
            }
            return Err(anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperFirewallRuleCreateOrAddFailed,
                format!("ensure offline outbound block failed: {err}"),
            )));
        }
    }

    if payload.read_roots.is_empty() {
        log_line(log, "no read roots to grant; skipping read ACL helper")?;
    } else {
        match read_acl_mutex_exists() {
            Ok(true) => {
                log_line(log, "read ACL helper already running; skipping spawn")?;
            }
            Ok(false) => {
                spawn_read_acl_helper(payload, log).map_err(|err| {
                    anyhow::Error::new(SetupFailure::new(
                        SetupErrorCode::HelperReadAclHelperSpawnFailed,
                        format!("spawn read ACL helper failed: {err}"),
                    ))
                })?;
            }
            Err(err) => {
                log_line(
                    log,
                    &format!("read ACL mutex check failed: {err}; spawning anyway"),
                )?;
                spawn_read_acl_helper(payload, log).map_err(|spawn_err| {
                    anyhow::Error::new(SetupFailure::new(
                        SetupErrorCode::HelperReadAclHelperSpawnFailed,
                        format!(
                            "spawn read ACL helper failed after mutex error {err}: {spawn_err}"
                        ),
                    ))
                })?;
            }
        }
    }

    let cap_sid_str = caps.workspace.clone();
    let sandbox_group_sid_str =
        string_from_sid_bytes(&sandbox_group_sid).map_err(anyhow::Error::msg)?;
    let write_mask =
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE | FILE_DELETE_CHILD;
    let mut grant_tasks: Vec<PathBuf> = Vec::new();

    let mut seen_write_roots: HashSet<PathBuf> = HashSet::new();
    let canonical_command_cwd = canonicalize_path(&payload.command_cwd);

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
        let is_command_cwd = is_command_cwd_root(root, &canonical_command_cwd);
        let cap_label = if is_command_cwd {
            "workspace_cap"
        } else {
            "cap"
        };
        let cap_psid_for_root = if is_command_cwd {
            workspace_psid
        } else {
            cap_psid
        };
        for (label, psid) in [
            ("sandbox_group", sandbox_group_psid),
            (cap_label, cap_psid_for_root),
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
                    "granting write ACE to {} for sandbox group and capability SID",
                    root.display()
                ),
            )?;
            grant_tasks.push(root.clone());
        }
    }

    let (tx, rx) = mpsc::channel::<(PathBuf, Result<bool>)>();
    std::thread::scope(|scope| {
        for root in grant_tasks {
            let is_command_cwd = is_command_cwd_root(&root, &canonical_command_cwd);
            let sid_strings = if is_command_cwd {
                vec![sandbox_group_sid_str.clone(), workspace_sid_str.clone()]
            } else {
                vec![sandbox_group_sid_str.clone(), cap_sid_str.clone()]
            };
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
            &sandbox_group_sid,
            GRANT_ACCESS,
            log,
        )
        .map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperSandboxLockFailed,
                format!(
                    "lock sandbox dir {} failed: {err}",
                    sandbox_dir(&payload.codex_home).display()
                ),
            ))
        })?;
        lock_sandbox_dir(
            &sandbox_secrets_dir(&payload.codex_home),
            &payload.real_user,
            &sandbox_group_sid,
            DENY_ACCESS,
            log,
        )
        .map_err(|err| {
            anyhow::Error::new(SetupFailure::new(
                SetupErrorCode::HelperSandboxLockFailed,
                format!(
                    "lock sandbox secrets dir {} failed: {err}",
                    sandbox_secrets_dir(&payload.codex_home).display()
                ),
            ))
        })?;
        let legacy_users = sandbox_dir(&payload.codex_home).join("sandbox_users.json");
        if legacy_users.exists() {
            let _ = std::fs::remove_file(&legacy_users);
        }
    }

    // Protect the current workspace's `.codex` and `.agents` directories from tampering
    // (write/delete) by using a workspace-specific capability SID. If a directory doesn't exist
    // yet, skip it (it will be picked up on the next refresh).
    match unsafe { protect_workspace_codex_dir(&payload.command_cwd, workspace_psid) } {
        Ok(true) => {
            let cwd_codex = payload.command_cwd.join(".codex");
            log_line(
                log,
                &format!(
                    "applied deny ACE to protect workspace .codex {}",
                    cwd_codex.display()
                ),
            )?;
        }
        Ok(false) => {}
        Err(err) => {
            let cwd_codex = payload.command_cwd.join(".codex");
            refresh_errors.push(format!("deny ACE failed on {}: {err}", cwd_codex.display()));
            log_line(
                log,
                &format!("deny ACE failed on {}: {err}", cwd_codex.display()),
            )?;
        }
    }
    match unsafe { protect_workspace_agents_dir(&payload.command_cwd, workspace_psid) } {
        Ok(true) => {
            let cwd_agents = payload.command_cwd.join(".agents");
            log_line(
                log,
                &format!(
                    "applied deny ACE to protect workspace .agents {}",
                    cwd_agents.display()
                ),
            )?;
        }
        Ok(false) => {}
        Err(err) => {
            let cwd_agents = payload.command_cwd.join(".agents");
            refresh_errors.push(format!(
                "deny ACE failed on {}: {err}",
                cwd_agents.display()
            ));
            log_line(
                log,
                &format!("deny ACE failed on {}: {err}", cwd_agents.display()),
            )?;
        }
    }
    unsafe {
        if !sandbox_group_psid.is_null() {
            LocalFree(sandbox_group_psid as HLOCAL);
        }
        if !cap_psid.is_null() {
            LocalFree(cap_psid as HLOCAL);
        }
        if !workspace_psid.is_null() {
            LocalFree(workspace_psid as HLOCAL);
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
