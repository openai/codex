use crate::ResolvedPluginMetricsOperation;
use codex_analytics::PluginMeasurementRow;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::Environment;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WriteFileOptions;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::sync::Arc;
use tempfile::NamedTempFile;
use uuid::Uuid;

pub const PLUGIN_METRICS_OUTPUT_ENV_VAR: &str = "CODEX_PLUGIN_METRICS_OUTPUT";
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const MAX_OUTPUT_ROWS: usize = 100;

#[derive(Debug, PartialEq)]
pub struct PluginMeasurementBatch {
    pub plugin_id: String,
    pub execution_id: String,
    pub operation: String,
    pub rows: Vec<PluginMeasurementRow>,
}

pub struct PluginMetricsSidecar {
    output: PluginMetricsOutput,
    absolute_output_dir: AbsolutePathBuf,
    output_env_value: String,
    resolved: ResolvedPluginMetricsOperation,
    execution_id: String,
}

enum PluginMetricsOutput {
    Local {
        file: NamedTempFile,
        _directory: tempfile::TempDir,
    },
    Remote {
        file_stream: tokio::sync::Mutex<FileSystemReadStream>,
        _directory: RemotePluginMetricsDirectory,
    },
}

struct RemotePluginMetricsDirectory {
    filesystem: Arc<dyn ExecutorFileSystem>,
    path: PathUri,
}

impl Drop for RemotePluginMetricsDirectory {
    fn drop(&mut self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let filesystem = Arc::clone(&self.filesystem);
        let path = self.path.clone();
        runtime.spawn(async move {
            let _ = filesystem
                .remove(
                    &path,
                    RemoveOptions {
                        recursive: true,
                        force: true,
                        follow_symlinks: true,
                    },
                    /*sandbox*/ None,
                )
                .await;
        });
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputEnvelope {
    version: u32,
    measurements: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputMeasurement {
    name: String,
    value: f64,
    #[serde(default)]
    dimensions: BTreeMap<String, String>,
}

impl PluginMetricsSidecar {
    pub fn create(resolved: ResolvedPluginMetricsOperation) -> Option<Self> {
        let sidecar_dir = tempfile::Builder::new()
            .prefix("codex-plugin-metrics-")
            .tempdir()
            .ok()?;
        let output_file = tempfile::Builder::new()
            .prefix("measurements-")
            .suffix(".json")
            .tempfile_in(sidecar_dir.path())
            .ok()?;
        let absolute_output_dir = AbsolutePathBuf::from_absolute_path(sidecar_dir.path()).ok()?;
        let absolute_output_path = AbsolutePathBuf::from_absolute_path(output_file.path()).ok()?;
        let output_env_value = absolute_output_path.as_path().to_str()?.to_string();
        Some(Self {
            output: PluginMetricsOutput::Local {
                file: output_file,
                _directory: sidecar_dir,
            },
            absolute_output_dir,
            output_env_value,
            resolved,
            execution_id: Uuid::new_v4().to_string(),
        })
    }

    pub async fn create_remote(
        environment: &Environment,
        resolved: ResolvedPluginMetricsOperation,
    ) -> Option<Self> {
        let temp_dir = environment.info().await.ok()?.temp_dir?;
        // Permission overlays still use host-native AbsolutePathBuf roots, so a
        // foreign executor path cannot be granted its exact sidecar directory.
        if !cfg!(unix) || temp_dir.infer_path_convention() != Some(PathConvention::Posix) {
            tracing::debug!(
                executor_temp_dir = %temp_dir,
                "plugin metrics require POSIX executor paths on a POSIX frontend"
            );
            return None;
        }
        let execution_id = Uuid::new_v4().to_string();
        let directory_path = temp_dir
            .join(&format!("codex-plugin-metrics-{execution_id}"))
            .ok()?;
        let absolute_output_dir = directory_path.to_abs_path().ok()?;
        let filesystem = environment.get_filesystem();
        filesystem
            .create_directory(
                &directory_path,
                CreateDirectoryOptions {
                    recursive: false,
                    follow_symlinks: true,
                },
                /*sandbox*/ None,
            )
            .await
            .ok()?;
        let directory = RemotePluginMetricsDirectory {
            filesystem,
            path: directory_path,
        };
        let output_path = directory.path.join("measurements.json").ok()?;
        directory
            .filesystem
            .write_file(
                &output_path,
                Vec::new(),
                WriteFileOptions::default(),
                /*sandbox*/ None,
            )
            .await
            .ok()?;
        let file_stream = directory
            .filesystem
            .read_file_stream(&output_path, /*sandbox*/ None)
            .await
            .ok()?;
        Some(Self {
            output: PluginMetricsOutput::Remote {
                file_stream: tokio::sync::Mutex::new(file_stream),
                _directory: directory,
            },
            absolute_output_dir,
            output_env_value: output_path.inferred_native_path_string(),
            resolved,
            execution_id,
        })
    }

    pub fn install_output_env(&self, env: &mut HashMap<String, String>) {
        env.insert(
            PLUGIN_METRICS_OUTPUT_ENV_VAR.to_string(),
            self.output_env_value.clone(),
        );
    }

    #[cfg(test)]
    fn absolute_output_path(&self) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(&self.output_env_value).expect("absolute output path")
    }

    pub fn additional_permissions(&self) -> AdditionalPermissionProfile {
        AdditionalPermissionProfile {
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                /*read*/ None,
                /*write*/ Some(vec![self.absolute_output_dir.clone()]),
            )),
            ..Default::default()
        }
    }

    pub async fn finish(mut self, exit_code: i32) -> Option<PluginMeasurementBatch> {
        if exit_code != 0 {
            return None;
        }
        let mut contents = Vec::new();
        match &mut self.output {
            PluginMetricsOutput::Local { file, .. } => {
                let output_file = file.as_file_mut();
                output_file.seek(SeekFrom::Start(0)).ok()?;
                output_file
                    .take(MAX_OUTPUT_BYTES + 1)
                    .read_to_end(&mut contents)
                    .ok()?;
            }
            PluginMetricsOutput::Remote { file_stream, .. } => {
                let file_stream = file_stream.get_mut();
                while let Some(chunk) = file_stream.next().await {
                    let chunk = chunk.ok()?;
                    if contents.len().saturating_add(chunk.len()) > MAX_OUTPUT_BYTES as usize {
                        return None;
                    }
                    contents.extend_from_slice(&chunk);
                }
            }
        }
        let rows = parse_output(&contents, &self.resolved)?;
        (!rows.is_empty()).then(|| PluginMeasurementBatch {
            plugin_id: self.resolved.plugin_id.as_key(),
            execution_id: self.execution_id,
            operation: self.resolved.operation.operation_name,
            rows,
        })
    }
}

pub fn strip_output_env(env: &mut HashMap<String, String>) {
    if cfg!(windows) {
        env.retain(|key, _| !key.eq_ignore_ascii_case(PLUGIN_METRICS_OUTPUT_ENV_VAR));
    } else {
        env.remove(PLUGIN_METRICS_OUTPUT_ENV_VAR);
    }
}

fn parse_output(
    contents: &[u8],
    resolved: &ResolvedPluginMetricsOperation,
) -> Option<Vec<PluginMeasurementRow>> {
    if contents.len() as u64 > MAX_OUTPUT_BYTES {
        return None;
    }
    let output: OutputEnvelope = serde_json::from_slice(contents).ok()?;
    if output.version != 1 || output.measurements.len() > MAX_OUTPUT_ROWS {
        return None;
    }

    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for value in output.measurements {
        let Ok(measurement) = serde_json::from_value::<OutputMeasurement>(value) else {
            continue;
        };
        let Some(definition) = resolved.operation.measurements.get(&measurement.name) else {
            continue;
        };
        if !measurement.value.is_finite()
            || measurement.dimensions.len() != definition.enum_dimensions.len()
            || !definition.enum_dimensions.iter().all(|(name, allowed)| {
                measurement
                    .dimensions
                    .get(name)
                    .is_some_and(|value| allowed.contains(value))
            })
            || !seen.insert((measurement.name.clone(), measurement.dimensions.clone()))
        {
            continue;
        }
        rows.push(PluginMeasurementRow {
            measurement_name: measurement.name,
            number_value: measurement.value,
            dimensions: measurement.dimensions,
        });
    }
    Some(rows)
}

#[cfg(test)]
#[path = "plugin_metrics_sidecar_tests.rs"]
mod tests;
