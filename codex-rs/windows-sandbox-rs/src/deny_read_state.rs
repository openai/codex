use crate::acl::revoke_ace;
use crate::deny_read_acl::apply_deny_read_acls;
use crate::deny_read_acl::lexical_path_key;
use crate::logging::log_note;
use crate::setup::sandbox_dir;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const DENY_READ_ACL_STATE_FILE: &str = "deny_read_acl_state.json";

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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
        Ok(bytes) if bytes.is_empty() => {
            recover_invalid_state(path, "deny-read ACL state file was empty");
            Ok(PersistentDenyReadAclState::default())
        }
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(state) => Ok(state),
            Err(err) => {
                recover_invalid_state(
                    path,
                    &format!("parse deny-read ACL state {} failed: {err}", path.display()),
                );
                Ok(PersistentDenyReadAclState::default())
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(PersistentDenyReadAclState::default())
        }
        Err(err) => {
            Err(err).with_context(|| format!("read deny-read ACL state {}", path.display()))
        }
    }
}

fn store_state(path: &Path, state: &PersistentDenyReadAclState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state).context("serialize deny-read ACL state")?;
    std::fs::write(path, bytes)
        .with_context(|| format!("write deny-read ACL state {}", path.display()))
}

fn recover_invalid_state(path: &Path, reason: &str) {
    log_note(reason, path.parent());

    let backup_path = invalid_state_backup_path(path);
    match std::fs::rename(path, &backup_path) {
        Ok(()) => log_note(
            &format!(
                "moved invalid deny-read ACL state {} aside to {}",
                path.display(),
                backup_path.display()
            ),
            path.parent(),
        ),
        Err(err) => log_note(
            &format!(
                "failed to move invalid deny-read ACL state {} aside: {err}",
                path.display()
            ),
            path.parent(),
        ),
    }
}

fn invalid_state_backup_path(path: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(DENY_READ_ACL_STATE_FILE);
    path.with_file_name(format!("{file_name}.corrupt-{timestamp}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_state_recovers_from_empty_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join(DENY_READ_ACL_STATE_FILE);
        std::fs::write(&path, []).expect("write empty state");

        let state = load_state(&path).expect("recover empty state");

        assert_eq!(state, PersistentDenyReadAclState::default());
        assert!(!path.exists(), "expected invalid state file to be moved aside");
        let backups = std::fs::read_dir(tempdir.path())
            .expect("read backup dir")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect backups");
        assert_eq!(backups.len(), 1);
        assert!(
            backups[0]
                .file_name()
                .to_string_lossy()
                .starts_with("deny_read_acl_state.json.corrupt-")
        );
    }

    #[test]
    fn load_state_recovers_from_invalid_json() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join(DENY_READ_ACL_STATE_FILE);
        std::fs::write(&path, b"\0\0\0\0").expect("write corrupt state");

        let state = load_state(&path).expect("recover corrupt state");

        assert_eq!(state, PersistentDenyReadAclState::default());
        assert!(!path.exists(), "expected invalid state file to be moved aside");
        let backups = std::fs::read_dir(tempdir.path())
            .expect("read backup dir")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect backups");
        assert_eq!(backups.len(), 1);
        assert!(
            backups[0]
                .file_name()
                .to_string_lossy()
                .starts_with("deny_read_acl_state.json.corrupt-")
        );
    }
}
