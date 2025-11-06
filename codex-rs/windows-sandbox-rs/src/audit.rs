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
use windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_APPEND_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_EA;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_ATTRIBUTES;
const GENERIC_ALL_MASK: u32 = 0x1000_0000;
const GENERIC_WRITE_MASK: u32 = 0x4000_0000;
use windows_sys::Win32::Security::ACL;
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;
use windows_sys::Win32::Security::ACL_SIZE_INFORMATION;
use windows_sys::Win32::Security::AclSizeInformation;
use windows_sys::Win32::Security::GetAclInformation;
use windows_sys::Win32::Security::GetAce;
use windows_sys::Win32::Security::ACCESS_ALLOWED_ACE;
use windows_sys::Win32::Security::ACE_HEADER;
use windows_sys::Win32::Security::EqualSid;

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
    // Very fast mask-based check for world-writable grants (includes GENERIC_*).
    if !dacl_quick_world_write_mask_allows(p_dacl, psid_world) {
        if !p_sd.is_null() { LocalFree(p_sd as HLOCAL); }
        return Ok(false);
    }
    // Quick detector flagged a write grant for Everyone: treat as writable.
    let has = true;
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
// Fast mask-based check: does the DACL contain any ACCESS_ALLOWED ACE for
// Everyone that includes generic or specific write bits? Skips inherit-only
// ACEs (do not apply to the current object).
unsafe fn dacl_quick_world_write_mask_allows(p_dacl: *mut ACL, psid_world: *mut c_void) -> bool {
    if p_dacl.is_null() {
        return false;
    }
    const INHERIT_ONLY_ACE: u8 = 0x08;
    let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
    let ok = GetAclInformation(
        p_dacl as *const ACL,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
        AclSizeInformation,
    );
    if ok == 0 {
        return false;
    }
    for i in 0..(info.AceCount as usize) {
        let mut p_ace: *mut c_void = std::ptr::null_mut();
        if GetAce(p_dacl as *const ACL, i as u32, &mut p_ace) == 0 {
            continue;
        }
        let hdr = &*(p_ace as *const ACE_HEADER);
        if hdr.AceType != 0 { // ACCESS_ALLOWED_ACE_TYPE
            continue;
        }
        if (hdr.AceFlags & INHERIT_ONLY_ACE) != 0 {
            continue;
        }
        let base = p_ace as usize;
        let sid_ptr = (base
            + std::mem::size_of::<ACE_HEADER>()
            + std::mem::size_of::<u32>()) as *mut c_void; // skip header + mask
        if EqualSid(sid_ptr, psid_world) != 0 {
            let ace = &*(p_ace as *const ACCESS_ALLOWED_ACE);
            let mask = ace.Mask;
            let writey = FILE_GENERIC_WRITE
                | FILE_WRITE_DATA
                | FILE_APPEND_DATA
                | FILE_WRITE_EA
                | FILE_WRITE_ATTRIBUTES
                | GENERIC_WRITE_MASK
                | GENERIC_ALL_MASK;
            if (mask & writey) != 0 {
                return true;
            }
        }
    }
    false
}
