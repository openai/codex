use crate::protocol::SandboxPolicy;
use crate::windows_sandbox::run_setup_refresh_with_extra_read_roots;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub fn grant_read_root_non_elevated(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
    read_root: &Path,
) -> Result<PathBuf> {
    if !read_root.is_absolute() {
        anyhow::bail!("path must be absolute: {}", read_root.display());
    }
    if !read_root.exists() {
        anyhow::bail!("path does not exist: {}", read_root.display());
    }
    if !read_root.is_dir() {
        anyhow::bail!("path must be a directory: {}", read_root.display());
    }

    let canonical_root = dunce::canonicalize(read_root)?;
    run_setup_refresh_with_extra_read_roots(
        policy,
        policy_cwd,
        command_cwd,
        env_map,
        codex_home,
        vec![canonical_root.clone()],
    )?;
    Ok(canonical_root)
}

#[cfg(test)]
mod tests {
    use super::grant_read_root_non_elevated;
    use crate::protocol::SandboxPolicy;
    use std::collections::HashMap;
    use std::path::Path;
    use tempfile::TempDir;

    fn policy() -> SandboxPolicy {
        SandboxPolicy::new_workspace_write_policy()
    }

    #[test]
    fn rejects_relative_path() {
        let tmp = TempDir::new().expect("tempdir");
        let err = grant_read_root_non_elevated(
            &policy(),
            tmp.path(),
            tmp.path(),
            &HashMap::new(),
            tmp.path(),
            Path::new("relative"),
        )
        .expect_err("relative path should fail");
        assert!(err.to_string().contains("path must be absolute"));
    }

    #[test]
    fn rejects_missing_path() {
        let tmp = TempDir::new().expect("tempdir");
        let missing = tmp.path().join("does-not-exist");
        let err = grant_read_root_non_elevated(
            &policy(),
            tmp.path(),
            tmp.path(),
            &HashMap::new(),
            tmp.path(),
            missing.as_path(),
        )
        .expect_err("missing path should fail");
        assert!(err.to_string().contains("path does not exist"));
    }

    #[test]
    fn rejects_file_path() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("file.txt");
        std::fs::write(&file_path, "hello").expect("write file");
        let err = grant_read_root_non_elevated(
            &policy(),
            tmp.path(),
            tmp.path(),
            &HashMap::new(),
            tmp.path(),
            file_path.as_path(),
        )
        .expect_err("file path should fail");
        assert!(err.to_string().contains("path must be a directory"));
    }
}
