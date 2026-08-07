use codex_utils_path_uri::PathUri;
use serde::Deserialize;
use serde::Serialize;

pub const ENVIRONMENT_CONFIG_READ_METHOD: &str = "environmentConfig/read";

/// Selects executor-local config and requirements fields by literal TOML key path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfigReadParams {
    pub cwd: PathUri,
    pub config_paths: Vec<Vec<String>>,
    pub requirements_paths: Vec<Vec<String>>,
}

/// Executor-local config and requirements layers selected for one environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfigReadResponse {
    /// Executor user home used to expand `~` in path-bearing values.
    pub user_home_dir: Option<PathUri>,
    /// Executor Codex home used as the base directory for cloud-provided layers.
    pub codex_home_dir: PathUri,
    /// Executor hostname used to select matching remote sandbox requirements.
    pub hostname: Option<String>,
    pub config: EnvironmentConfigLayerStack,
    pub requirements: EnvironmentConfigLayerStack,
}

/// One ordered set of selected TOML layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfigLayerStack {
    /// Layers ordered from lowest to highest precedence.
    pub layers: Vec<EnvironmentConfigLayer>,
    /// Position at which cloud-provided layers should be inserted.
    pub cloud_insertion_index: usize,
}

/// One selected executor-local TOML layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfigLayer {
    /// Opaque provenance for diagnostics. Callers must not derive behavior from it.
    pub source: String,
    /// Directory used to interpret relative paths in this layer.
    pub base_dir: PathUri,
    /// Selected raw TOML. Path-bearing values have not been normalized.
    pub toml: String,
}
