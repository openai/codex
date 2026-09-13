use crate::acl::add_deny_read_ace;
use crate::acl::revoke_ace;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashSet;
use std::ffi::c_void;
use std::path::Path;
use std::path::PathBuf;

/// Build the exact ACL paths that should receive a deny-read ACE.
///
/// We keep both the lexical policy path and, when it already exists, the
/// canonical target. The lexical path covers the path users configured and lets
/// missing exact denies be materialized later; the canonical path also covers
/// an existing reparse-point target so a sandbox cannot read the same object
/// through the resolved location. Filesystem roots are rejected after this
/// canonicalization so aliases cannot install a root deny ACE.
pub fn plan_deny_read_acl_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut planned = Vec::new();
    let mut seen = HashSet::new();
    for path in paths {
        push_planned_path(&mut planned, &mut seen, path.to_path_buf());
        let canonical = match dunce::canonicalize(path) {
            Ok(canonical) => canonical,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("canonicalize deny-read path {}", path.display()));
            }
        };
        push_planned_path(&mut planned, &mut seen, canonical);
    }
    if let Some(root) = planned
        .iter()
        .find(|path| path.is_absolute() && path.parent().is_none())
    {
        anyhow::bail!(
            "refusing to apply a deny-read ACE to filesystem root {}",
            root.display()
        );
    }
    Ok(planned)
}

fn push_planned_path(planned: &mut Vec<PathBuf>, seen: &mut HashSet<String>, path: PathBuf) {
    if seen.insert(lexical_path_key(&path)) {
        planned.push(path);
    }
}

pub(crate) fn lexical_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

/// Applies deny-read ACEs to explicit paths. Missing paths are materialized as
/// directories before the ACE is applied so a sandboxed command cannot create a
/// previously absent denied path and then read from it in the same run.
/// If any path fails, deny ACEs applied by this call are revoked before the
/// error is returned so a one-shot sandbox run does not leave partial state.
///
/// # Safety
/// Caller must pass a valid SID pointer for the sandbox principal being denied.
pub unsafe fn apply_deny_read_acls(paths: &[PathBuf], psid: *mut c_void) -> Result<Vec<PathBuf>> {
    let planned = plan_deny_read_acl_paths(paths)?;
    let mut applied = Vec::new();
    let mut seen = HashSet::new();
    let mut added_in_this_call: Vec<PathBuf> = Vec::new();
    for path in planned {
        let result = (|| -> Result<bool> {
            if !path.exists() {
                std::fs::create_dir_all(&path)
                    .with_context(|| format!("create deny-read path {}", path.display()))?;
            }
            add_deny_read_ace(&path, psid)
                .with_context(|| format!("apply deny-read ACE to {}", path.display()))
        })();
        let added = match result {
            Ok(added) => added,
            Err(err) => {
                for added_path in &added_in_this_call {
                    let _ = revoke_ace(added_path, psid);
                }
                return Err(err);
            }
        };
        if added {
            added_in_this_call.push(path.clone());
        }
        push_planned_path(&mut applied, &mut seen, path);
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::plan_deny_read_acl_paths;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn plan_preserves_missing_paths() {
        let tmp = TempDir::new().expect("tempdir");
        let missing = tmp.path().join("future-secret.env");

        assert_eq!(
            plan_deny_read_acl_paths(std::slice::from_ref(&missing)).expect("plan deny-read ACL"),
            vec![missing]
        );
    }

    #[test]
    fn plan_includes_existing_canonical_targets() {
        let tmp = TempDir::new().expect("tempdir");
        let existing = tmp.path().join("secret.env");
        std::fs::write(&existing, "secret").expect("write secret");

        let planned: HashSet<PathBuf> = plan_deny_read_acl_paths(std::slice::from_ref(&existing))
            .expect("plan deny-read ACL")
            .into_iter()
            .collect();
        let expected: HashSet<PathBuf> = [
            existing.clone(),
            dunce::canonicalize(&existing).expect("canonical path"),
        ]
        .into_iter()
        .collect();

        assert_eq!(planned, expected);
    }

    #[test]
    fn plan_rejects_file_system_root() {
        let cwd = std::env::current_dir().expect("current directory");
        let root = cwd.ancestors().last().expect("filesystem root");

        let err = plan_deny_read_acl_paths(&[root.to_path_buf()])
            .expect_err("filesystem root must not receive a deny-read ACE");

        assert_eq!(
            err.to_string(),
            format!(
                "refusing to apply a deny-read ACE to filesystem root {}",
                root.display()
            )
        );
    }
}
