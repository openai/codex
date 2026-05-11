#[cfg(debug_assertions)]
use std::fs::File;
#[cfg(debug_assertions)]
use std::io::BufRead;
use std::path::PathBuf;

#[cfg(debug_assertions)]
const BAZEL_BWRAP_ENV_VAR: &str = "CARGO_BIN_EXE_bwrap";

#[cfg(debug_assertions)]
pub(crate) fn candidate() -> Option<PathBuf> {
    if option_env!("BAZEL_PACKAGE").is_none() || !runfiles_env_present() {
        return None;
    }

    let raw = PathBuf::from(std::env::var_os(BAZEL_BWRAP_ENV_VAR)?);
    if raw.is_absolute() {
        return Some(raw);
    }
    resolve_runfile(raw.to_str()?)
}

#[cfg(not(debug_assertions))]
pub(crate) fn candidate() -> Option<PathBuf> {
    None
}

#[cfg(debug_assertions)]
fn runfiles_env_present() -> bool {
    std::env::var_os("RUNFILES_DIR").is_some()
        || std::env::var_os("TEST_SRCDIR").is_some()
        || std::env::var_os("RUNFILES_MANIFEST_FILE").is_some()
}

#[cfg(debug_assertions)]
fn resolve_runfile(logical_path: &str) -> Option<PathBuf> {
    let mut logical_paths = vec![logical_path.to_string()];
    if let Ok(workspace) = std::env::var("TEST_WORKSPACE")
        && !workspace.is_empty()
    {
        logical_paths.push(format!("{workspace}/{logical_path}"));
    }

    for root_env in ["RUNFILES_DIR", "TEST_SRCDIR"] {
        let Some(root) = std::env::var_os(root_env) else {
            continue;
        };
        let root = PathBuf::from(root);
        for logical_path in &logical_paths {
            let candidate = root.join(logical_path);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    let manifest = PathBuf::from(std::env::var_os("RUNFILES_MANIFEST_FILE")?);
    let file = File::open(manifest).ok()?;
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        if logical_paths.iter().any(|logical_path| logical_path == key) {
            return Some(PathBuf::from(value));
        }
    }
    None
}
