use crate::acl::revoke_ace;
use crate::deny_read_acl::apply_deny_read_acls;
use crate::deny_read_acl::lexical_path_key;
use crate::setup::sandbox_dir;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::c_void;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

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
            revoke_ace(&path, psid);
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
    match std::fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(state) => Ok(state),
            Err(_) => {
                recover_invalid_state_file(path)?;
                Ok(PersistentDenyReadAclState::default())
            }
        },
        Err(err) if err.kind() == ErrorKind::NotFound => {
            Ok(PersistentDenyReadAclState::default())
        }
        Err(err) => {
            Err(err).with_context(|| format!("read deny-read ACL state {}", path.display()))
        }
    }
}

fn recover_invalid_state_file(path: &Path) -> Result<()> {
    let quarantine_path = path.with_extension("json.corrupt");
    match std::fs::rename(path, &quarantine_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            std::fs::remove_file(&quarantine_path).with_context(|| {
                format!("remove stale deny-read ACL quarantine {}", quarantine_path.display())
            })?;
            std::fs::rename(path, &quarantine_path).with_context(|| {
                format!(
                    "quarantine invalid deny-read ACL state {} -> {}",
                    path.display(),
                    quarantine_path.display()
                )
            })
        }
        Err(err) => Err(err).with_context(|| {
            format!(
                "quarantine invalid deny-read ACL state {} -> {}",
                path.display(),
                quarantine_path.display()
            )
        }),
    }
}

fn store_state(path: &Path, state: &PersistentDenyReadAclState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state).context("serialize deny-read ACL state")?;
    std::fs::write(path, bytes)
        .with_context(|| format!("write deny-read ACL state {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::PersistentDenyReadAclState;
    use super::load_state;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn load_state_recovers_from_invalid_json() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("deny_read_acl_state.json");
        fs::write(&path, b"\0\0\0").expect("write corrupt state");

        let state = load_state(&path).expect("recover state");

        assert!(state.principals.is_empty());
        assert!(!path.exists());
        assert!(path.with_extension("json.corrupt").exists());
    }

    #[test]
    fn load_state_replaces_stale_quarantine_file() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("deny_read_acl_state.json");
        let quarantine_path = path.with_extension("json.corrupt");
        fs::write(&path, b"{").expect("write corrupt state");
        fs::write(&quarantine_path, b"stale").expect("write stale quarantine");

        let state = load_state(&path).expect("recover state");

        assert_eq!(state.principals, PersistentDenyReadAclState::default().principals);
        assert_eq!(fs::read(&quarantine_path).expect("read quarantine"), b"{");
    }
}
