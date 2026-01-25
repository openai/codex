use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub use path_absolutize;

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
    #[error("CARGO_BIN_EXE env var {key} resolved to {path:?}, but it does not exist")]
    ResolvedPathDoesNotExist { key: String, path: PathBuf },
    #[error("could not locate binary {name:?}; tried env vars {env_keys:?}; {fallback}")]
    NotFound {
        name: String,
        env_keys: Vec<String>,
        fallback: String,
    },
    #[error("failed to run `cargo metadata`: {source}")]
    CargoMetadataSpawn {
        #[source]
        source: std::io::Error,
    },
    #[error("`cargo metadata` failed with status {status:?}: {stderr}")]
    CargoMetadataFailed {
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("failed to parse `cargo metadata` output: {source}")]
    CargoMetadataParse {
        #[source]
        source: serde_json::Error,
    },
    #[error("`cargo metadata` output missing field {field:?}")]
    CargoMetadataMissingField { field: &'static str },
    #[error("binary target {name:?} not found in workspace via `cargo metadata`")]
    BinaryNotInWorkspace { name: String },
    #[error("binary target {name:?} is ambiguous; candidates: {candidates:?}")]
    AmbiguousBinary {
        name: String,
        candidates: Vec<String>,
    },
    #[error("failed to run `cargo build` for {package:?} ({bin:?}): {source}")]
    CargoBuildSpawn {
        package: String,
        bin: String,
        #[source]
        source: std::io::Error,
    },
    #[error("`cargo build` failed for {package:?} ({bin:?}) with status {status:?}: {stderr}")]
    CargoBuildFailed {
        package: String,
        bin: String,
        status: std::process::ExitStatus,
        stderr: String,
    },
}

/// Returns an absolute path to a binary target built for the current test run.
///
/// In `cargo test`, `CARGO_BIN_EXE_*` env vars are absolute, but Buck2 may set
/// them to project-relative paths (e.g. `buck-out/...`). Those paths break if a
/// test later changes its working directory. This helper makes the path
/// absolute up-front so callers can safely `chdir` afterwards.
pub fn cargo_bin(name: &str) -> Result<PathBuf, CargoBinError> {
    let env_keys = cargo_bin_env_keys(name);
    for key in &env_keys {
        if let Some(value) = std::env::var_os(key) {
            return resolve_bin_from_env(key, value);
        }
    }

    match assert_cmd::Command::cargo_bin(name) {
        Ok(cmd) => {
            let abs = absolutize_from_buck_or_cwd(PathBuf::from(cmd.get_program()))?;
            if abs.exists() {
                Ok(abs)
            } else {
                Err(CargoBinError::ResolvedPathDoesNotExist {
                    key: "assert_cmd::Command::cargo_bin".to_owned(),
                    path: abs,
                })
            }
        }
        Err(err) => match cargo_bin_via_workspace_build(name) {
            Ok(bin) => Ok(bin),
            Err(build_err) => Err(CargoBinError::NotFound {
                name: name.to_owned(),
                env_keys,
                fallback: format!(
                    "assert_cmd fallback failed: {err}; cargo build fallback failed: {build_err}"
                ),
            }),
        },
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
        // When this code is built and run with Bazel:
        // - we inject `BAZEL_PACKAGE` as a compile-time environment variable
        //   that points to native.package_name()
        // - at runtime, Bazel will set `RUNFILES_DIR` to the runfiles directory
        //
        // Therefore, the compile-time value of `BAZEL_PACKAGE` will always be
        // included in the compiled binary (even if it is built with Cargo), but
        // we only check it at runtime if `RUNFILES_DIR` is set.
        let resource = std::path::Path::new(&$resource);
        match std::env::var("RUNFILES_DIR") {
            Ok(bazel_runtime_files) => match option_env!("BAZEL_PACKAGE") {
                Some(bazel_package) => {
                    use $crate::path_absolutize::Absolutize;

                    let manifest_dir = std::path::PathBuf::from(bazel_runtime_files)
                        .join("_main")
                        .join(bazel_package)
                        .join(resource);
                    // Note we also have to normalize (but not canonicalize!)
                    // the path for _Bazel_ because the original value ends with
                    // `codex-rs/exec-server/tests/common/../suite/bash`, but
                    // the `tests/common` folder will not exist at runtime under
                    // Bazel. As such, we have to normalize it before passing it
                    // to `dotslash fetch`.
                    manifest_dir.absolutize().map(|p| p.to_path_buf())
                }
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "BAZEL_PACKAGE not set in Bazel build",
                )),
            },
            Err(_) => {
                let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                Ok(manifest_dir.join(resource))
            }
        }
    }};
}

fn resolve_bin_from_env(key: &str, value: OsString) -> Result<PathBuf, CargoBinError> {
    let abs = absolutize_from_buck_or_cwd(PathBuf::from(value))?;

    if abs.exists() {
        Ok(abs)
    } else {
        Err(CargoBinError::ResolvedPathDoesNotExist {
            key: key.to_owned(),
            path: abs,
        })
    }
}

fn absolutize_from_buck_or_cwd(path: PathBuf) -> Result<PathBuf, CargoBinError> {
    if path.is_absolute() {
        return Ok(path);
    }

    if let Some(root) =
        buck_project_root().map_err(|source| CargoBinError::CurrentExe { source })?
    {
        return Ok(root.join(path));
    }

    Ok(std::env::current_dir()
        .map_err(|source| CargoBinError::CurrentDir { source })?
        .join(path))
}

#[derive(Debug, Clone)]
struct CargoMetadataTarget {
    name: String,
    kinds: Vec<String>,
}

#[derive(Debug, Clone)]
struct CargoMetadataPackage {
    name: String,
    targets: Vec<CargoMetadataTarget>,
}

#[derive(Debug, Clone)]
struct CargoMetadata {
    target_directory: PathBuf,
    packages: Vec<CargoMetadataPackage>,
}

fn cargo_metadata(workspace_root: &PathBuf) -> Result<CargoMetadata, CargoBinError> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .current_dir(workspace_root)
        .output()
        .map_err(|source| CargoBinError::CargoMetadataSpawn { source })?;

    if !output.status.success() {
        return Err(CargoBinError::CargoMetadataFailed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|source| CargoBinError::CargoMetadataParse { source })?;

    let target_directory = meta
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or(CargoBinError::CargoMetadataMissingField {
            field: "target_directory",
        })?;

    let packages = meta
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or(CargoBinError::CargoMetadataMissingField { field: "packages" })?;

    let mut parsed_packages = Vec::with_capacity(packages.len());
    for package in packages {
        let Some(package_name) = package.get("name").and_then(serde_json::Value::as_str) else {
            return Err(CargoBinError::CargoMetadataMissingField {
                field: "packages[].name",
            });
        };
        let Some(targets) = package.get("targets").and_then(serde_json::Value::as_array) else {
            return Err(CargoBinError::CargoMetadataMissingField {
                field: "packages[].targets",
            });
        };

        let mut parsed_targets = Vec::with_capacity(targets.len());
        for target in targets {
            let Some(target_name) = target.get("name").and_then(serde_json::Value::as_str) else {
                return Err(CargoBinError::CargoMetadataMissingField {
                    field: "targets[].name",
                });
            };
            let Some(kinds) = target.get("kind").and_then(serde_json::Value::as_array) else {
                return Err(CargoBinError::CargoMetadataMissingField {
                    field: "targets[].kind",
                });
            };
            let mut parsed_kinds = Vec::with_capacity(kinds.len());
            for kind in kinds {
                let Some(kind) = kind.as_str() else {
                    return Err(CargoBinError::CargoMetadataMissingField {
                        field: "targets[].kind[]",
                    });
                };
                parsed_kinds.push(kind.to_string());
            }
            parsed_targets.push(CargoMetadataTarget {
                name: target_name.to_string(),
                kinds: parsed_kinds,
            });
        }

        parsed_packages.push(CargoMetadataPackage {
            name: package_name.to_string(),
            targets: parsed_targets,
        });
    }

    Ok(CargoMetadata {
        target_directory: PathBuf::from(target_directory),
        packages: parsed_packages,
    })
}

fn cargo_workspace_root_from_exe() -> Result<Option<PathBuf>, CargoBinError> {
    let exe = std::env::current_exe().map_err(|source| CargoBinError::CurrentExe { source })?;
    for ancestor in exe.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "target") {
            return Ok(ancestor.parent().map(PathBuf::from));
        }
    }
    Ok(None)
}

fn cargo_workspace_root() -> Result<PathBuf, CargoBinError> {
    if let Some(root) = cargo_workspace_root_from_exe()? {
        return Ok(root);
    }

    Ok(std::env::current_dir().map_err(|source| CargoBinError::CurrentDir { source })?)
}

fn cargo_bin_via_workspace_build(name: &str) -> Result<PathBuf, CargoBinError> {
    let workspace_root = cargo_workspace_root()?;
    let meta = cargo_metadata(&workspace_root)?;

    let mut candidates: Vec<String> = Vec::new();
    for package in &meta.packages {
        let is_bin = package
            .targets
            .iter()
            .any(|target| target.name == name && target.kinds.iter().any(|kind| kind == "bin"));
        if is_bin {
            candidates.push(package.name.clone());
        }
    }

    let package = match candidates.as_slice() {
        [] => {
            return Err(CargoBinError::BinaryNotInWorkspace {
                name: name.to_string(),
            });
        }
        [only] => only.as_str(),
        _ => {
            candidates.sort();
            return Err(CargoBinError::AmbiguousBinary {
                name: name.to_string(),
                candidates,
            });
        }
    };

    let exe_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let candidate_path = meta.target_directory.join("debug").join(&exe_name);

    let output = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg(package)
        .arg("--bin")
        .arg(name)
        .current_dir(&workspace_root)
        .output()
        .map_err(|source| CargoBinError::CargoBuildSpawn {
            package: package.to_string(),
            bin: name.to_string(),
            source,
        })?;

    if !output.status.success() {
        return Err(CargoBinError::CargoBuildFailed {
            package: package.to_string(),
            bin: name.to_string(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    if candidate_path.exists() {
        Ok(candidate_path)
    } else {
        Err(CargoBinError::BinaryNotInWorkspace {
            name: name.to_string(),
        })
    }
}

/// Best-effort attempt to find the Buck project root for the currently running
/// process.
///
/// Prefer this over `env!("CARGO_MANIFEST_DIR")` when running under Buck2: our
/// Buck generator sets `CARGO_MANIFEST_DIR="."` for compilation, which makes
/// `env!("CARGO_MANIFEST_DIR")` unusable for locating workspace files.
pub fn buck_project_root() -> Result<Option<PathBuf>, std::io::Error> {
    if let Some(root) = std::env::var_os("BUCK_PROJECT_ROOT") {
        let root = PathBuf::from(root);
        if root.is_absolute() {
            return Ok(Some(root));
        }
    }

    // Fall back to deriving the project root from the location of the test
    // runner executable:
    //   <project>/buck-out/v2/gen/.../__tests__/test-binary
    let exe = std::env::current_exe()?;
    for ancestor in exe.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "buck-out") {
            return Ok(ancestor.parent().map(PathBuf::from));
        }
    }

    Ok(None)
}
