use crate::ArtifactRuntimePlatform;
use crate::runtime::default_cached_runtime_root;
use crate::runtime::load_cached_runtime;
use std::path::Path;
use std::path::PathBuf;
use which::which;

const CODEX_APP_PRODUCT_NAMES: [&str; 6] = [
    "Codex",
    "Codex (Dev)",
    "Codex (Agent)",
    "Codex (Nightly)",
    "Codex (Alpha)",
    "Codex (Beta)",
];

/// The JavaScript runtime used to execute the artifact tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsRuntimeKind {
    Node,
    Electron,
}

/// A discovered JavaScript executable and the way it should be invoked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsRuntime {
    executable_path: PathBuf,
    kind: JsRuntimeKind,
}

impl JsRuntime {
    pub(crate) fn node(executable_path: PathBuf) -> Self {
        Self {
            executable_path,
            kind: JsRuntimeKind::Node,
        }
    }

    pub(crate) fn electron(executable_path: PathBuf) -> Self {
        Self {
            executable_path,
            kind: JsRuntimeKind::Electron,
        }
    }

    /// Returns the executable to spawn for artifact commands.
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    /// Returns whether the command must set `ELECTRON_RUN_AS_NODE=1`.
    pub fn requires_electron_run_as_node(&self) -> bool {
        self.kind == JsRuntimeKind::Electron
    }
}

/// Returns `true` when artifact execution can find both runtime assets and a JS executable.
pub fn is_js_runtime_available(codex_home: &Path, runtime_version: &str) -> bool {
    load_cached_runtime(&default_cached_runtime_root(codex_home), runtime_version)
        .ok()
        .and_then(|runtime| runtime.resolve_js_runtime().ok())
        .or_else(resolve_machine_js_runtime)
        .is_some()
}

/// Returns `true` when this machine can use the managed artifact runtime flow.
///
/// This is a platform capability check, not a cache or binary availability check.
/// Callers that rely on `ArtifactRuntimeManager::ensure_installed()` should use this
/// to decide whether the feature can be exposed on the current machine.
pub fn can_manage_artifact_runtime() -> bool {
    ArtifactRuntimePlatform::detect_current().is_ok()
}

pub(crate) fn resolve_machine_js_runtime() -> Option<JsRuntime> {
    resolve_js_runtime_from_candidates(
        None,
        system_node_runtime(),
        system_electron_runtime(),
        codex_app_runtime_candidates(),
    )
}

pub(crate) fn resolve_js_runtime_from_candidates(
    preferred_node_path: Option<&Path>,
    node_runtime: Option<JsRuntime>,
    electron_runtime: Option<JsRuntime>,
    codex_app_candidates: Vec<PathBuf>,
) -> Option<JsRuntime> {
    preferred_node_path
        .and_then(node_runtime_from_path)
        .or(node_runtime)
        .or(electron_runtime)
        .or_else(|| {
            codex_app_candidates
                .into_iter()
                .find_map(|candidate| electron_runtime_from_path(&candidate))
        })
}

pub(crate) fn system_node_runtime() -> Option<JsRuntime> {
    which("node")
        .ok()
        .and_then(|path| node_runtime_from_path(&path))
}

pub(crate) fn system_electron_runtime() -> Option<JsRuntime> {
    which("electron")
        .ok()
        .and_then(|path| electron_runtime_from_path(&path))
}

pub(crate) fn node_runtime_from_path(path: &Path) -> Option<JsRuntime> {
    path.is_file().then(|| JsRuntime::node(path.to_path_buf()))
}

pub(crate) fn electron_runtime_from_path(path: &Path) -> Option<JsRuntime> {
    path.is_file()
        .then(|| JsRuntime::electron(path.to_path_buf()))
}

pub(crate) fn codex_app_runtime_candidates() -> Vec<PathBuf> {
    match std::env::consts::OS {
        "macos" => {
            let mut roots = vec![PathBuf::from("/Applications")];
            if let Some(home) = std::env::var_os("HOME") {
                roots.push(PathBuf::from(home).join("Applications"));
            }

            roots
                .into_iter()
                .flat_map(|root| {
                    CODEX_APP_PRODUCT_NAMES
                        .into_iter()
                        .map(move |product_name| {
                            root.join(format!("{product_name}.app"))
                                .join("Contents")
                                .join("MacOS")
                                .join(product_name)
                        })
                })
                .collect()
        }
        "windows" => {
            let mut roots = Vec::new();
            if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                roots.push(PathBuf::from(local_app_data).join("Programs"));
            }
            if let Some(program_files) = std::env::var_os("ProgramFiles") {
                roots.push(PathBuf::from(program_files));
            }
            if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
                roots.push(PathBuf::from(program_files_x86));
            }

            roots
                .into_iter()
                .flat_map(|root| {
                    CODEX_APP_PRODUCT_NAMES
                        .into_iter()
                        .map(move |product_name| {
                            root.join(product_name).join(format!("{product_name}.exe"))
                        })
                })
                .collect()
        }
        "linux" => [PathBuf::from("/opt"), PathBuf::from("/usr/lib")]
            .into_iter()
            .flat_map(|root| {
                CODEX_APP_PRODUCT_NAMES
                    .into_iter()
                    .map(move |product_name| root.join(product_name).join(product_name))
            })
            .collect(),
        _ => Vec::new(),
    }
}
