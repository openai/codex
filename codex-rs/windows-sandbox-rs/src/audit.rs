use crate::acl::add_deny_write_ace;
use crate::acl::path_mask_allows;
use crate::cap::cap_sid_file;
use crate::cap::load_or_create_cap_sids;
use crate::logging::{debug_log, log_note};
use crate::path_normalization::canonical_path_key;
use crate::policy::SandboxPolicy;
use crate::token::convert_string_sid_to_sid;
use crate::token::world_sid;
use anyhow::anyhow;
use anyhow::Result;
use std::collections::HashSet;
use std::ffi::c_void;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Storage::FileSystem::FILE_APPEND_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_DATA;
use windows_sys::Win32::Storage::FileSystem::FILE_WRITE_EA;

// Preflight scan limits
const MAX_ITEMS_PER_DIR: i32 = 1000;
const AUDIT_TIME_LIMIT_SECS: i64 = 2;
const MAX_CHECKED_LIMIT: i32 = 50000;
// Case-insensitive suffixes (normalized to forward slashes) to skip during one-level child scan
const SKIP_DIR_SUFFIXES: &[&str] = &[
    "/windows/installer",
    "/windows/registration",
    "/programdata",
];

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
        for part in std::env::split_paths(OsStr::new(&path)) {
            if !part.as_os_str().is_empty() {
                unique_push(&mut set, &mut out, part);
            }
        }
    }
    // 5) Core system roots last
    for p in [PathBuf::from("C:/"), PathBuf::from("C:/Windows")] {
        unique_push(&mut set, &mut out, p);
    }
    out
}

unsafe fn path_has_world_write_allow(path: &Path) -> Result<bool> {
    let mut world = world_sid()?;
    let psid_world = world.as_mut_ptr() as *mut c_void;
    let write_mask = FILE_WRITE_DATA | FILE_APPEND_DATA | FILE_WRITE_EA | FILE_WRITE_ATTRIBUTES;
    path_mask_allows(path, &[psid_world], write_mask, false)
}

pub fn audit_everyone_writable(
    cwd: &Path,
    env: &std::collections::HashMap<String, String>,
    logs_base_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let start = Instant::now();
    let mut flagged: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut checked = 0usize;
    let check_world_writable = |path: &Path| -> bool {
        match unsafe { path_has_world_write_allow(path) } {
            Ok(has) => has,
            Err(err) => {
                debug_log(
                    &format!(
                        "AUDIT: treating unreadable ACL as not world-writable: {} ({err})",
                        path.display()
                    ),
                    logs_base_dir,
                );
                false
            }
        }
    };
    // Fast path: check CWD immediate children first so workspace issues are caught early.
    if let Ok(read) = std::fs::read_dir(cwd) {
        for ent in read.flatten().take(MAX_ITEMS_PER_DIR as usize) {
            if start.elapsed() > Duration::from_secs(AUDIT_TIME_LIMIT_SECS as u64)
                || checked > MAX_CHECKED_LIMIT as usize
            {
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
            let has = check_world_writable(&p);
            if has {
                let key = canonical_path_key(&p);
                if seen.insert(key) {
                    flagged.push(p);
                }
            }
        }
    }
    // Continue with broader candidate sweep
    let candidates = gather_candidates(cwd, env);
    for root in candidates {
        if start.elapsed() > Duration::from_secs(AUDIT_TIME_LIMIT_SECS as u64)
            || checked > MAX_CHECKED_LIMIT as usize
        {
            break;
        }
        checked += 1;
        let has_root = check_world_writable(&root);
        if has_root {
            let key = canonical_path_key(&root);
            if seen.insert(key) {
                flagged.push(root.clone());
            }
        }
        // one level down best-effort
        if let Ok(read) = std::fs::read_dir(&root) {
            for ent in read.flatten().take(MAX_ITEMS_PER_DIR as usize) {
                let p = ent.path();
                if start.elapsed() > Duration::from_secs(AUDIT_TIME_LIMIT_SECS as u64)
                    || checked > MAX_CHECKED_LIMIT as usize
                {
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
                // Skip noisy/irrelevant Windows system subdirectories
                let pl = p.to_string_lossy().to_ascii_lowercase();
                let norm = pl.replace('\\', "/");
                if SKIP_DIR_SUFFIXES.iter().any(|s| norm.ends_with(s)) {
                    continue;
                }
                if ft.is_dir() {
                    checked += 1;
                    let has_child = check_world_writable(&p);
                    if has_child {
                        let key = canonical_path_key(&p);
                        if seen.insert(key) {
                            flagged.push(p);
                        }
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
                "AUDIT: world-writable scan FAILED; cwd={cwd:?}; checked={checked}; duration_ms={elapsed_ms}; flagged:{}",
                list
            ),
            logs_base_dir,
        );

        return Ok(flagged);
    }
    // Log success once if nothing flagged
    crate::logging::log_note(
        &format!("AUDIT: world-writable scan OK; checked={checked}; duration_ms={elapsed_ms}"),
        logs_base_dir,
    );
    Ok(Vec::new())
}

pub fn apply_world_writable_scan_and_denies(
    codex_home: &Path,
    cwd: &Path,
    env_map: &std::collections::HashMap<String, String>,
    sandbox_policy: &SandboxPolicy,
    logs_base_dir: Option<&Path>,
) -> Result<()> {
    let flagged = audit_everyone_writable(cwd, env_map, logs_base_dir)?;
    if flagged.is_empty() {
        return Ok(());
    }
    if let Err(err) = apply_capability_denies_for_world_writable(
        codex_home,
        &flagged,
        sandbox_policy,
        cwd,
        logs_base_dir,
    ) {
        log_note(
            &format!("AUDIT: failed to apply capability deny ACEs: {}", err),
            logs_base_dir,
        );
    }
    Ok(())
}

pub fn apply_capability_denies_for_world_writable(
    codex_home: &Path,
    flagged: &[PathBuf],
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
    logs_base_dir: Option<&Path>,
) -> Result<()> {
    if flagged.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(codex_home)?;
    let cap_path = cap_sid_file(codex_home);
    let caps = load_or_create_cap_sids(codex_home)?;
    std::fs::write(&cap_path, serde_json::to_string(&caps)?)?;
    let (active_sid, workspace_roots): (*mut c_void, Vec<PathBuf>) = match sandbox_policy {
        SandboxPolicy::WorkspaceWrite { writable_roots, .. } => {
            let sid = unsafe { convert_string_sid_to_sid(&caps.workspace) }
                .ok_or_else(|| anyhow!("ConvertStringSidToSidW failed for workspace capability"))?;
            let mut roots: Vec<PathBuf> =
                vec![dunce::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf())];
            for root in writable_roots {
                let candidate = root.as_path();
                roots.push(dunce::canonicalize(candidate).unwrap_or_else(|_| root.to_path_buf()));
            }
            (sid, roots)
        }
        SandboxPolicy::ReadOnly { .. } => (
            unsafe { convert_string_sid_to_sid(&caps.readonly) }.ok_or_else(|| {
                anyhow!("ConvertStringSidToSidW failed for readonly capability")
            })?,
            Vec::new(),
        ),
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
            return Ok(());
        }
    };
    for path in flagged {
        if workspace_roots.iter().any(|root| path.starts_with(root)) {
            continue;
        }
        let res = unsafe { add_deny_write_ace(path, active_sid) };
        match res {
            Ok(true) => log_note(
                &format!("AUDIT: applied capability deny ACE to {}", path.display()),
                logs_base_dir,
            ),
            Ok(false) => {}
            Err(err) => log_note(
                &format!(
                    "AUDIT: failed to apply capability deny ACE to {}: {}",
                    path.display(),
                    err
                ),
                logs_base_dir,
            ),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::gather_candidates;
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn gathers_path_entries_by_list_separator() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir_a = tmp.path().join("Tools");
        let dir_b = tmp.path().join("Bin");
        let dir_space = tmp.path().join("Program Files");
        fs::create_dir_all(&dir_a).expect("dir a");
        fs::create_dir_all(&dir_b).expect("dir b");
        fs::create_dir_all(&dir_space).expect("dir space");

        let mut env_map = HashMap::new();
        env_map.insert(
            "PATH".to_string(),
            format!(
                "{};{};{}",
                dir_a.display(),
                dir_b.display(),
                dir_space.display()
            ),
        );

        let candidates = gather_candidates(tmp.path(), &env_map);
        let canon_a = dir_a.canonicalize().expect("canon a");
        let canon_b = dir_b.canonicalize().expect("canon b");
        let canon_space = dir_space.canonicalize().expect("canon space");

        assert!(candidates.contains(&canon_a));
        assert!(candidates.contains(&canon_b));
        assert!(candidates.contains(&canon_space));
    }
}
