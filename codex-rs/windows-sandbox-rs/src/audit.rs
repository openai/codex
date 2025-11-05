use crate::acl::dacl_has_write_allow_for_sid;
use crate::token::world_sid;
use crate::winutil::to_wide;
use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Foundation::HLOCAL;
use windows_sys::Win32::Security::Authorization::GetNamedSecurityInfoW;
use windows_sys::Win32::Security::Authorization::GetSecurityInfo;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Storage::FileSystem::CreateFileW;
use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE;
use windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

fn unique_push(set: &mut HashSet<PathBuf>, out: &mut Vec<PathBuf>, p: PathBuf) {
    if let Ok(abs) = p.canonicalize() {
        if set.insert(abs.clone()) {
            out.push(abs);
        }
    }
}

fn gather_candidates(cwd: &Path, env: &std::collections::HashMap<String, String>) -> Vec<PathBuf> {
    let mut set: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    // 1) CWD first (so immediate children get scanned early)
    unique_push(&mut set, &mut out, cwd.to_path_buf());
    // 2) TEMP/TMP next (often small, quick to scan)
    for k in ["TEMP", "TMP"] {
        if let Some(v) = env.get(k).cloned().or_else(|| std::env::var(k).ok()) {
            unique_push(&mut set, &mut out, PathBuf::from(v));
        }
    }
    // 3) User roots
    if let Some(up) = std::env::var_os("USERPROFILE") {
        unique_push(&mut set, &mut out, PathBuf::from(up));
    }
    if let Some(pubp) = std::env::var_os("PUBLIC") {
        unique_push(&mut set, &mut out, PathBuf::from(pubp));
    }
    // 4) PATH entries (best-effort)
    if let Some(path) = env
        .get("PATH")
        .cloned()
        .or_else(|| std::env::var("PATH").ok())
    {
        for part in path.split(std::path::MAIN_SEPARATOR) {
            if !part.is_empty() {
                unique_push(&mut set, &mut out, PathBuf::from(part));
            }
        }
    }
    // 5) Core system roots last
    for p in [
        PathBuf::from("C:/"),
        PathBuf::from("C:/Windows"),
        PathBuf::from("C:/ProgramData"),
    ] {
        unique_push(&mut set, &mut out, p);
    }
    out
}

unsafe fn path_has_world_write_allow(path: &Path) -> Result<bool> {
    // Prefer handle-based query (often faster than name-based), fallback to name-based on error
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();

    let mut try_named = false;
    let wpath = to_wide(path);
    let h = CreateFileW(
        wpath.as_ptr(),
        0x00020000, // READ_CONTROL
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        std::ptr::null_mut(),
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS,
        0,
    );
    if h == INVALID_HANDLE_VALUE {
        try_named = true;
    } else {
        let code = GetSecurityInfo(
            h,
            1, // SE_FILE_OBJECT
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut p_dacl,
            std::ptr::null_mut(),
            &mut p_sd,
        );
        CloseHandle(h);
        if code != ERROR_SUCCESS {
            try_named = true;
            if !p_sd.is_null() {
                LocalFree(p_sd as HLOCAL);
                p_sd = std::ptr::null_mut();
                p_dacl = std::ptr::null_mut();
            }
        }
    }

    if try_named {
        let code = GetNamedSecurityInfoW(
            wpath.as_ptr(),
            1,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut p_dacl,
            std::ptr::null_mut(),
            &mut p_sd,
        );
        if code != ERROR_SUCCESS {
            if !p_sd.is_null() {
                LocalFree(p_sd as HLOCAL);
            }
            return Ok(false);
        }
    }

    let mut world = world_sid()?;
    let psid_world = world.as_mut_ptr() as *mut c_void;
    let has = dacl_has_write_allow_for_sid(p_dacl, psid_world);
    if !p_sd.is_null() {
        LocalFree(p_sd as HLOCAL);
    }
    Ok(has)
}

pub fn audit_everyone_writable(
    cwd: &Path,
    env: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let start = Instant::now();
    let mut flagged: Vec<PathBuf> = Vec::new();
    let mut checked = 0usize;
    // Fast path: check CWD immediate children first so workspace issues are caught early.
    if let Ok(read) = std::fs::read_dir(cwd) {
        for ent in read.flatten().take(250) {
            if start.elapsed() > Duration::from_secs(5) || checked > 5000 {
                break;
            }
            let ft = match ent.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_symlink() || !ft.is_dir() {
                continue;
            }
            let p = ent.path();
            checked += 1;
            let has = unsafe { path_has_world_write_allow(&p)? };
            if has {
                flagged.push(p);
            }
        }
    }
    // Continue with broader candidate sweep
    let candidates = gather_candidates(cwd, env);
    for root in candidates {
        if start.elapsed() > Duration::from_secs(5) || checked > 5000 {
            break;
        }
        checked += 1;
        let has_root = unsafe { path_has_world_write_allow(&root)? };
        if has_root {
            flagged.push(root.clone());
        }
        // one level down best-effort
        if let Ok(read) = std::fs::read_dir(&root) {
            for ent in read.flatten().take(250) {
                let p = ent.path();
                if start.elapsed() > Duration::from_secs(5) || checked > 5000 {
                    break;
                }
                // Skip reparse points (symlinks/junctions) to avoid auditing link ACLs
                let ft = match ent.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                if ft.is_symlink() {
                    continue;
                }
                if ft.is_dir() {
                    checked += 1;
                    let has_child = unsafe { path_has_world_write_allow(&p)? };
                    if has_child {
                        flagged.push(p);
                    }
                }
            }
        }
    }
    let elapsed_ms = start.elapsed().as_millis();
    if !flagged.is_empty() {
        let mut list = String::new();
        for p in &flagged {
            list.push_str(&format!("\n - {}", p.display()));
        }
        crate::logging::log_note(
            &format!(
                "AUDIT: world-writable scan FAILED; checked={checked}; duration_ms={elapsed_ms}; flagged:{}",
                list
            ),
            Some(cwd),
        );
        let mut list_err = String::new();
        for p in flagged {
            list_err.push_str(&format!("\n - {}", p.display()));
        }
        return Err(anyhow!(
            "Refusing to run: found directories writable by Everyone: {}",
            list_err
        ));
    }
    // Log success once if nothing flagged
    crate::logging::log_note(
        &format!(
            "AUDIT: world-writable scan OK; checked={checked}; duration_ms={elapsed_ms}"
        ),
        Some(cwd),
    );
    Ok(())
}
