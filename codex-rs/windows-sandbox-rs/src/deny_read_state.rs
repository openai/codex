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
use tempfile::NamedTempFile;

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
                quarantine_corrupt_state_file(path)?;
                Ok(PersistentDenyReadAclState::default())
            }
        },
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(PersistentDenyReadAclState::default()),
        Err(err) => {
            Err(err).with_context(|| format!("read deny-read ACL state {}", path.display()))
        }
    }
}

fn quarantine_corrupt_state_file(path: &Path) -> Result<()> {
    let quarantine_path = corrupt_state_backup_path(path);
    match std::fs::remove_file(&quarantine_path) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "remove previous corrupt deny-read ACL backup {}",
                    quarantine_path.display()
                )
            });
        }
    }
    std::fs::rename(path, &quarantine_path).with_context(|| {
        format!(
            "quarantine corrupt deny-read ACL state {} to {}",
            path.display(),
            quarantine_path.display()
        )
    })
}

fn corrupt_state_backup_path(path: &Path) -> PathBuf {
    let backup_name = match path.file_name() {
        Some(name) => {
            let mut name = name.to_os_string();
            name.push(".corrupt");
            name
        }
        None => "deny_read_acl_state.json.corrupt".into(),
    };
    path.with_file_name(backup_name)
}

fn store_state(path: &Path, state: &PersistentDenyReadAclState) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(state).context("serialize deny-read ACL state")?;
    let parent = path.parent().with_context(|| {
        format!(
            "locate parent dir for deny-read ACL state {}",
            path.display()
        )
    })?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create deny-read ACL state dir {}", parent.display()))?;

    let temp_file = NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "create temp deny-read ACL state file in {}",
            parent.display()
        )
    })?;
    std::fs::write(temp_file.path(), bytes).with_context(|| {
        format!(
            "write temp deny-read ACL state file {}",
            temp_file.path().display()
        )
    })?;
    temp_file
        .persist(path)
        .map_err(|err| err.error)
        .with_context(|| format!("persist deny-read ACL state {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::corrupt_state_backup_path;
    use super::load_state;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn load_state_recovers_from_corrupt_json() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("deny_read_acl_state.json");
        fs::write(&path, b"{not json").expect("write corrupt state");

        let state = load_state(&path).expect("load state");

        assert!(state.principals.is_empty());
        assert!(!path.exists());
        assert_eq!(
            fs::read(corrupt_state_backup_path(&path)).expect("read backup"),
            b"{not json"
        );
    }

    #[test]
    fn load_state_recovers_from_nul_filled_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("deny_read_acl_state.json");
        fs::write(&path, vec![0_u8; 32]).expect("write corrupt state");

        let state = load_state(&path).expect("load state");

        assert!(state.principals.is_empty());
        assert!(!path.exists());
        assert_eq!(
            fs::read(corrupt_state_backup_path(&path)).expect("read backup"),
            vec![0_u8; 32]
        );
    }

    #[test]
    fn load_state_reads_valid_json() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("deny_read_acl_state.json");
        fs::write(
            &path,
            r#"{"principals":{"S-1-1-0":["C:\\Users\\alice\\.ssh"]}}"#,
        )
        .expect("write state");

        let state = load_state(&path).expect("load state");

        let saved_paths = state
            .principals
            .get("S-1-1-0")
            .expect("principal should exist");
        assert_eq!(saved_paths.len(), 1);
    }
}
