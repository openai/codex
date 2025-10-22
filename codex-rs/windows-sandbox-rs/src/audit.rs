use crate::acl::dacl_effective_allows_write;
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
    // Core roots
    for p in [
        PathBuf::from("C:/"),
        PathBuf::from("C:/Windows"),
        PathBuf::from("C:/ProgramData"),
    ] {
        unique_push(&mut set, &mut out, p);
    }
    // User roots
    if let Some(up) = std::env::var_os("USERPROFILE") {
        unique_push(&mut set, &mut out, PathBuf::from(up));
    }
    if let Some(pubp) = std::env::var_os("PUBLIC") {
        unique_push(&mut set, &mut out, PathBuf::from(pubp));
    }
    // CWD
    unique_push(&mut set, &mut out, cwd.to_path_buf());
    // TEMP/TMP
    for k in ["TEMP", "TMP"] {
        if let Some(v) = env.get(k).cloned().or_else(|| std::env::var(k).ok()) {
            unique_push(&mut set, &mut out, PathBuf::from(v));
        }
    }
    // PATH entries
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
    out
}

unsafe fn path_has_world_write_allow(path: &Path) -> Result<bool> {
    let mut p_sd: *mut c_void = std::ptr::null_mut();
    let mut p_dacl: *mut ACL = std::ptr::null_mut();
    let code = GetNamedSecurityInfoW(
        to_wide(path).as_ptr(),
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
    let mut world = world_sid()?;
    let psid_world = world.as_mut_ptr() as *mut c_void;
    let has = dacl_effective_allows_write(p_dacl, psid_world);
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
    let candidates = gather_candidates(cwd, env);
    for root in candidates {
        if start.elapsed() > Duration::from_secs(5) || checked > 5000 {
            break;
        }
        checked += 1;
        if unsafe { path_has_world_write_allow(&root)? } {
            flagged.push(root.clone());
        }
        // one level down best-effort
        if let Ok(read) = std::fs::read_dir(&root) {
            for ent in read.flatten().take(50) {
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
                    if unsafe { path_has_world_write_allow(&p)? } {
                        flagged.push(p);
                    }
                }
            }
        }
    }
    if !flagged.is_empty() {
        let mut list = String::new();
        for p in flagged {
            list.push_str(&format!("\n - {}", p.display()));
        }
        return Err(anyhow!(
            "Refusing to run: found directories writable by Everyone: {}",
            list
        ));
    }
    Ok(())
}
