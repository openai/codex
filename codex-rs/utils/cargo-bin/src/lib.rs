use std::ffi::OsString;
use std::path::PathBuf;

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
        Err(err) => Err(CargoBinError::NotFound {
            name: name.to_owned(),
            env_keys,
            fallback: format!("assert_cmd fallback failed: {err}"),
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
