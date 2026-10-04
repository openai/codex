use crate::acl::revoke_ace;
use crate::deny_read_acl::apply_deny_read_acls;
use crate::deny_read_acl::lexical_path_key;
use crate::setup::sandbox_dir;
use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::c_void;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::Foundation as win;
use windows_sys::Win32::Storage::FileSystem as winfs;

const DENY_READ_ACL_STATE_FILE: &str = "deny_read_acl_state.json";

#[derive(Default, Deserialize, Serialize)]
struct PersistentDenyReadAclState {
    principals: BTreeMap<String, Vec<PathBuf>>,
}

/// Reconciles the persistent deny-read ACEs owned by one sandbox principal.
///
/// Workspace-write and elevated sandbox sessions intentionally leave ACLs in
/// place after a command exits, because descendants may outlive the launcher.
/// That makes the ACL set stateful across runs. Persist the paths applied for
/// each SID, apply the new desired set first, and only then revoke stale paths
/// from the same SID so profile changes do not leave old deny-read ACEs behind.
///
/// Malformed bookkeeping is rebuilt after applying current denies. Unknown
/// historical restrictions cannot safely be revoked and remain in place.
///
/// # Safety
/// Caller must pass a valid SID pointer matching `principal_sid`.
pub unsafe fn sync_persistent_deny_read_acls(
    codex_home: &Path,
    principal_sid: &str,
    desired_paths: &[PathBuf],
    psid: *mut c_void,
) -> Result<Vec<PathBuf>> {
    let state_path = sandbox_dir(codex_home).join(DENY_READ_ACL_STATE_FILE);
    let mut state = load_state(&state_path)?;
    let previous_paths = state
        .principals
        .get(principal_sid)
        .cloned()
        .unwrap_or_default();

    let applied_paths = unsafe { apply_deny_read_acls(desired_paths, psid) }?;
    let desired_keys = applied_paths
        .iter()
        .map(|path| lexical_path_key(path))
        .collect::<HashSet<_>>();

    for path in previous_paths {
        if !desired_keys.contains(&lexical_path_key(&path)) {
            let _ = revoke_ace(&path, psid);
        }
    }

    if applied_paths.is_empty() {
        state.principals.remove(principal_sid);
    } else {
        state
            .principals
            .insert(principal_sid.to_string(), applied_paths.clone());
    }
    store_state(&state_path, &state)?;

    Ok(applied_paths)
}

fn load_state(path: &Path) -> Result<PersistentDenyReadAclState> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let stable = Instant::now() >= deadline;
        let bytes = (|| -> Result<Vec<u8>> {
            // Ordinary reads allow legacy writers; malformed recovery excludes live writers.
            let share_mode = if stable {
                winfs::FILE_SHARE_READ | winfs::FILE_SHARE_DELETE
            } else {
                winfs::FILE_SHARE_READ | winfs::FILE_SHARE_WRITE | winfs::FILE_SHARE_DELETE
            };
            let mut file = OpenOptions::new()
                .read(true)
                .share_mode(share_mode)
                .custom_flags(winfs::FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path)?;
            // Validate the same leaf before either accepting state or recovering it.
            validate_state_file(&file)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        })();
        match bytes {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(state) => return Ok(state),
                Err(_) if !stable => std::thread::sleep(Duration::from_millis(25)),
                Err(_) => return Ok(PersistentDenyReadAclState::default()),
            },
            Err(err)
                if err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(PersistentDenyReadAclState::default());
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("read deny-read ACL state {}", path.display()));
            }
        }
    }
}

fn store_state(path: &Path, state: &PersistentDenyReadAclState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state).context("serialize deny-read ACL state")?;
    (|| -> Result<()> {
        // Recovery makes malformed files writable. Never follow the state leaf
        // or truncate it before checking the actual object. Resolve configured
        // parent paths normally, including junctions and volume GUIDs.
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut file = loop {
            let opened = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(winfs::FILE_SHARE_READ | winfs::FILE_SHARE_WRITE)
                .custom_flags(winfs::FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path);
            match opened {
                Ok(file) => break file,
                Err(error)
                    if error.raw_os_error() == Some(win::ERROR_SHARING_VIOLATION as i32)
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(error) => return Err(error.into()),
            }
        };
        validate_state_file(&file)?;
        file.write_all(&bytes)?;
        file.set_len(bytes.len() as u64)?;
        Ok(())
    })()
    .with_context(|| format!("write deny-read ACL state {}", path.display()))
}

fn validate_state_file(file: &File) -> Result<()> {
    let mut info = unsafe { std::mem::zeroed::<winfs::BY_HANDLE_FILE_INFORMATION>() };
    if unsafe { winfs::GetFileInformationByHandle(file.as_raw_handle() as win::HANDLE, &mut info) }
        == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    ensure!(
        info.dwFileAttributes & winfs::FILE_ATTRIBUTE_REPARSE_POINT == 0,
        "state file is a reparse point"
    );
    ensure!(info.nNumberOfLinks == 1, "state file has multiple links");
    Ok(())
}

#[cfg(test)]
#[path = "deny_read_state_tests.rs"]
mod tests;
