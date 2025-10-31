use crate::policy::SandboxMode;
use crate::policy::SandboxPolicy;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

pub fn compute_allow_paths(
    policy: &SandboxPolicy,
    _policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Vec<PathBuf> {
    let mut allow: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if matches!(policy.0, SandboxMode::WorkspaceWrite) {
        let abs = command_cwd.to_path_buf();
        if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
            allow.push(abs);
        }
    }
    if !matches!(policy.0, SandboxMode::ReadOnly) {
        for key in ["TEMP", "TMP"] {
            if let Some(v) = env_map.get(key) {
                let abs = PathBuf::from(v);
                if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
                    allow.push(abs);
                }
            } else if let Ok(v) = std::env::var(key) {
                let abs = PathBuf::from(v);
                if seen.insert(abs.to_string_lossy().to_string()) && abs.exists() {
                    allow.push(abs);
                }
            }
        }
    }
    allow
}
