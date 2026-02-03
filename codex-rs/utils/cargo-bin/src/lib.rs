use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub use runfiles;

/// Bazel sets this when runfiles directories are disabled, which we do on all platforms for consistency.
const RUNFILES_MANIFEST_ONLY_ENV: &str = "RUNFILES_MANIFEST_ONLY";

#[derive(Debug, thiserror::Error)]
pub enum CargoBinError {
    #[error("failed to read current exe")]
    CurrentExe {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read current directory")]
    CurrentDir {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to resolve repo root")]
    RepoRoot {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to run `cargo metadata`")]
    CargoMetadata {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse `cargo metadata` output")]
    CargoMetadataJson {
        #[source]
        source: serde_json::Error,
    },
    #[error("`cargo metadata` did not include a bin target named {name:?}")]
    BinNotInMetadata { name: String },
    #[error("`cargo build` failed for {package:?} ({bin:?}) with status {status:?}: {stderr}")]
    CargoBuildFailed {
        package: String,
        bin: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("CARGO_BIN_EXE env var {key} resolved to {path:?}, but it does not exist")]
    ResolvedPathDoesNotExist { key: String, path: PathBuf },
    #[error("could not locate binary {name:?}; tried env vars {env_keys:?}; {fallback}")]
    NotFound {
        name: String,
        env_keys: Vec<String>,
        fallback: String,
    },
}

/// Returns an absolute path to a binary target built for the current test run.
///
/// In `cargo test`, `CARGO_BIN_EXE_*` env vars are absolute.
/// In `bazel test`, `CARGO_BIN_EXE_*` env vars are rlocationpaths, intended to be consumed by `rlocation`.
/// This helper allows callers to transparently support both.
pub fn cargo_bin(name: &str) -> Result<PathBuf, CargoBinError> {
    let env_keys = cargo_bin_env_keys(name);
    for key in &env_keys {
        if let Some(value) = std::env::var_os(key) {
            return resolve_bin_from_env(key, value);
        }
    }
    match cargo_bin_via_workspace_build(name) {
        Ok(path) => Ok(path),
        Err(err) => Err(CargoBinError::NotFound {
            name: name.to_owned(),
            env_keys,
            fallback: format!("cargo build fallback failed: {err}"),
        }),
    }
}

fn cargo_bin_env_keys(name: &str) -> Vec<String> {
    let mut keys = Vec::with_capacity(2);
    keys.push(format!("CARGO_BIN_EXE_{name}"));

    // Cargo replaces dashes in target names when exporting env vars.
    let underscore_name = name.replace('-', "_");
    if underscore_name != name {
        keys.push(format!("CARGO_BIN_EXE_{underscore_name}"));
    }

    keys
}

pub fn runfiles_available() -> bool {
    std::env::var_os(RUNFILES_MANIFEST_ONLY_ENV).is_some()
}

fn resolve_bin_from_env(key: &str, value: OsString) -> Result<PathBuf, CargoBinError> {
    let raw = PathBuf::from(&value);
    if runfiles_available() {
        let runfiles = runfiles::Runfiles::create().map_err(|err| CargoBinError::CurrentExe {
            source: std::io::Error::other(err),
        })?;
        if let Some(resolved) = runfiles::rlocation!(runfiles, &raw)
            && resolved.exists()
        {
            return Ok(resolved);
        }
    } else if raw.is_absolute() && raw.exists() {
        return Ok(raw);
    }

    Err(CargoBinError::ResolvedPathDoesNotExist {
        key: key.to_owned(),
        path: raw,
    })
}

fn cargo_bin_via_workspace_build(name: &str) -> Result<PathBuf, CargoBinError> {
    let workspace_root = repo_root().map_err(|source| CargoBinError::RepoRoot { source })?;
    let codex_rs_root = workspace_root.join("codex-rs");
    let metadata_output = Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(&codex_rs_root)
        .output()
        .map_err(|source| CargoBinError::CargoMetadata { source })?;
    if !metadata_output.status.success() {
        return Err(CargoBinError::CargoMetadata {
            source: io::Error::other(String::from_utf8_lossy(&metadata_output.stderr)),
        });
    }
    let meta: serde_json::Value = serde_json::from_slice(&metadata_output.stdout)
        .map_err(|source| CargoBinError::CargoMetadataJson { source })?;
    let target_dir = meta
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| CargoBinError::BinNotInMetadata {
            name: name.to_owned(),
        })?;
    let packages = meta
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| CargoBinError::BinNotInMetadata {
            name: name.to_owned(),
        })?;

    let mut package_name: Option<String> = None;
    for package in packages {
        let Some(targets) = package.get("targets").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for target in targets {
            let Some(target_name) = target.get("name").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(kinds) = target.get("kind").and_then(serde_json::Value::as_array) else {
                continue;
            };
            let is_bin = kinds.iter().any(|k| k.as_str() == Some("bin"));
            if is_bin && target_name == name {
                package_name = package
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(ToOwned::to_owned);
                break;
            }
        }
        if package_name.is_some() {
            break;
        }
    }

    let package = package_name.ok_or_else(|| CargoBinError::BinNotInMetadata {
        name: name.to_owned(),
    })?;

    let build_output = Command::new("cargo")
        .args(["build", "-p", &package, "--bin", name])
        .current_dir(&codex_rs_root)
        .output()
        .map_err(|source| CargoBinError::CargoMetadata { source })?;
    if !build_output.status.success() {
        return Err(CargoBinError::CargoBuildFailed {
            package,
            bin: name.to_owned(),
            status: build_output.status,
            stderr: String::from_utf8_lossy(&build_output.stderr).to_string(),
        });
    }

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let mut path = target_dir.join(profile).join(name);
    if cfg!(windows) {
        path.set_extension("exe");
    }
    if path.exists() {
        Ok(path)
    } else {
        Err(CargoBinError::ResolvedPathDoesNotExist {
            key: "cargo build".to_owned(),
            path,
        })
    }
}

/// Macro that derives the path to a test resource at runtime, the value of
/// which depends on whether Cargo or Bazel is being used to build and run a
/// test. Note the return value may be a relative or absolute path.
/// (Incidentally, this is a macro rather than a function because it reads
/// compile-time environment variables that need to be captured at the call
/// site.)
///
/// This is expected to be used exclusively in test code because Codex CLI is a
/// standalone binary with no packaged resources.
#[macro_export]
macro_rules! find_resource {
    ($resource:expr) => {{
        let resource = std::path::Path::new(&$resource);
        if $crate::runfiles_available() {
            // When this code is built and run with Bazel:
            // - we inject `BAZEL_PACKAGE` as a compile-time environment variable
            //   that points to native.package_name()
            // - at runtime, Bazel will set runfiles-related env vars
            $crate::resolve_bazel_runfile(option_env!("BAZEL_PACKAGE"), resource)
        } else {
            let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            Ok(manifest_dir.join(resource))
        }
    }};
}

pub fn resolve_bazel_runfile(
    bazel_package: Option<&str>,
    resource: &Path,
) -> std::io::Result<PathBuf> {
    let runfiles = runfiles::Runfiles::create()
        .map_err(|err| std::io::Error::other(format!("failed to create runfiles: {err}")))?;
    let runfile_path = match bazel_package {
        Some(bazel_package) => PathBuf::from("_main").join(bazel_package).join(resource),
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BAZEL_PACKAGE was not set at compile time",
            ));
        }
    };
    let runfile_path = normalize_runfile_path(&runfile_path);
    if let Some(resolved) = runfiles::rlocation!(runfiles, &runfile_path)
        && resolved.exists()
    {
        return Ok(resolved);
    }
    let runfile_path_display = runfile_path.display();
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("runfile does not exist at: {runfile_path_display}"),
    ))
}

pub fn resolve_cargo_runfile(resource: &Path) -> std::io::Result<PathBuf> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir.join(resource))
}

pub fn repo_root() -> io::Result<PathBuf> {
    let marker = if runfiles_available() {
        let runfiles = runfiles::Runfiles::create()
            .map_err(|err| io::Error::other(format!("failed to create runfiles: {err}")))?;
        let marker_path = option_env!("CODEX_REPO_ROOT_MARKER")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "CODEX_REPO_ROOT_MARKER was not set at compile time",
                )
            })?;
        runfiles::rlocation!(runfiles, &marker_path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "repo_root.marker not available in runfiles",
            )
        })?
    } else {
        resolve_cargo_runfile(Path::new("repo_root.marker"))?
    };
    let mut root = marker;
    for _ in 0..4 {
        root = root
            .parent()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "repo_root.marker did not have expected parent depth",
                )
            })?
            .to_path_buf();
    }
    Ok(root)
}

fn normalize_runfile_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if matches!(components.last(), Some(std::path::Component::Normal(_))) {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }

    components
        .into_iter()
        .fold(PathBuf::new(), |mut acc, component| {
            acc.push(component.as_os_str());
            acc
        })
}
