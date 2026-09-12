//! App-server-backed config read and update helpers for the TUI.
//!
//! This module centralizes the small typed update helpers the TUI uses
//! when a config mutation must be owned by the app server rather than written
//! to the local `config.toml` directly.

use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigWriteResponse;
use codex_app_server_protocol::EnvironmentInfoParams;
use codex_app_server_protocol::EnvironmentInfoResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SkillsConfigWriteParams;
use codex_app_server_protocol::SkillsConfigWriteResponse;
use codex_config::default_project_root_markers;
use codex_config::loader::find_project_root;
use codex_config::loader::normalized_project_trust_keys;
use codex_config::loader::project_trust_key;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::LOCAL_FS;
use codex_features::FEATURES;
use codex_git_utils::resolve_root_git_project_for_trust;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::config_types::TrustLevel;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathConvention;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use serde_json::Value as JsonValue;
use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectTrustHost {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteProjectTrust {
    pub trust_level: Option<TrustLevel>,
    pub cwd: PathBuf,
    pub trust_target: PathBuf,
}

pub(crate) fn replace_config_value(key_path: impl Into<String>, value: JsonValue) -> ConfigEdit {
    ConfigEdit {
        key_path: key_path.into(),
        value,
        merge_strategy: MergeStrategy::Replace,
    }
}

pub(crate) fn clear_config_value(key_path: impl Into<String>) -> ConfigEdit {
    replace_config_value(key_path, JsonValue::Null)
}

pub(crate) fn app_scoped_key_path(app_id: &str, key_path: &str) -> String {
    let app_id = serde_json::Value::String(app_id.to_string()).to_string();
    format!("apps.{app_id}.{key_path}")
}

pub(crate) fn format_config_error(err: &impl Display) -> String {
    format!("{err:#}")
}

fn trusted_project_edit(project_path: &Path) -> ConfigEdit {
    let project_key = project_trust_key(project_path)
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    replace_config_value(
        format!("projects.\"{project_key}\".trust_level"),
        serde_json::json!(TrustLevel::Trusted.to_string()),
    )
}

pub(crate) fn build_model_selection_edits(
    model: &str,
    effort: Option<impl ToString>,
) -> Vec<ConfigEdit> {
    let effort_edit = effort.map_or_else(
        || clear_config_value("model_reasoning_effort"),
        |effort| {
            replace_config_value(
                "model_reasoning_effort",
                serde_json::json!(effort.to_string()),
            )
        },
    );
    vec![
        replace_config_value("model", serde_json::json!(model)),
        effort_edit,
    ]
}

pub(crate) fn build_service_tier_selection_edits(service_tier: Option<&str>) -> Vec<ConfigEdit> {
    let service_tier_edit = service_tier.map_or_else(
        || clear_config_value("service_tier"),
        |service_tier| {
            let config_value = if service_tier == SERVICE_TIER_DEFAULT_REQUEST_VALUE {
                SERVICE_TIER_DEFAULT_REQUEST_VALUE
            } else {
                match codex_protocol::config_types::ServiceTier::from_request_value(service_tier) {
                    Some(codex_protocol::config_types::ServiceTier::Fast) => "fast",
                    Some(codex_protocol::config_types::ServiceTier::Flex) => "flex",
                    None => service_tier,
                }
            };
            replace_config_value("service_tier", serde_json::json!(config_value))
        },
    );
    vec![service_tier_edit]
}

pub(crate) fn build_feature_enabled_edit(feature_key: &str, enabled: bool) -> ConfigEdit {
    let key_path = format!("features.{feature_key}");
    let is_default_false_feature = FEATURES
        .iter()
        .find(|spec| spec.key == feature_key)
        .is_some_and(|spec| !spec.default_enabled);
    if enabled || !is_default_false_feature {
        replace_config_value(key_path, serde_json::json!(enabled))
    } else {
        clear_config_value(key_path)
    }
}

pub(crate) fn build_memory_settings_edits(
    use_memories: bool,
    generate_memories: bool,
) -> Vec<ConfigEdit> {
    vec![
        replace_config_value("memories.use_memories", serde_json::json!(use_memories)),
        replace_config_value(
            "memories.generate_memories",
            serde_json::json!(generate_memories),
        ),
    ]
}

pub(crate) fn build_oss_provider_edit(provider: &str) -> ConfigEdit {
    replace_config_value("oss_provider", serde_json::json!(provider))
}

pub(crate) async fn write_config_batch(
    request_handle: AppServerRequestHandle,
    edits: Vec<ConfigEdit>,
) -> Result<ConfigWriteResponse> {
    let request_id = RequestId::String(format!("tui-config-write-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::ConfigBatchWrite {
            request_id,
            params: ConfigBatchWriteParams {
                edits,
                file_path: None,
                expected_version: None,
                reload_user_config: true,
            },
        })
        .await
        .wrap_err("config/batchWrite failed in TUI")
}

pub(crate) async fn write_trusted_project(
    request_handle: AppServerRequestHandle,
    project_path: &Path,
) -> Result<ConfigWriteResponse> {
    write_config_batch(request_handle, vec![trusted_project_edit(project_path)]).await
}

pub(crate) async fn read_effective_config(
    request_handle: AppServerRequestHandle,
    cwd: String,
) -> Result<ConfigReadResponse> {
    let request_id = RequestId::String(format!("tui-config-read-{}", Uuid::new_v4()));
    request_handle
        .request_typed(ClientRequest::ConfigRead {
            request_id,
            params: ConfigReadParams {
                include_layers: false,
                cwd: Some(cwd),
            },
        })
        .await
        .wrap_err("config/read failed in TUI")
}

pub(crate) async fn read_remote_project_trust(
    request_handle: AppServerRequestHandle,
    cwd: &Path,
    host: ProjectTrustHost,
) -> Result<Option<RemoteProjectTrust>> {
    let cwd_string = cwd.to_string_lossy().into_owned();
    let cwd = match LegacyAppPathString::from_string(cwd_string.clone()).to_inferred_path_uri() {
        Some(cwd) => cwd,
        None => {
            let request_id = RequestId::String(format!("tui-environment-info-{}", Uuid::new_v4()));
            let environment: EnvironmentInfoResponse = request_handle
                .request_typed(ClientRequest::EnvironmentInfo {
                    request_id,
                    params: EnvironmentInfoParams {
                        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                    },
                })
                .await
                .wrap_err("environment/info failed while resolving remote project trust")?;
            environment
                .cwd
                .ok_or_else(|| color_eyre::eyre::eyre!("remote app server did not provide a cwd"))?
                .join(&cwd_string)
                .wrap_err("failed to resolve the remote project directory")?
        }
    };
    let cwd_uri = cwd;
    let cwd = cwd_uri.inferred_native_path_string();
    let request_id = RequestId::String(format!("tui-project-trust-read-{}", Uuid::new_v4()));
    let response: JsonValue = request_handle
        .request_typed(ClientRequest::ConfigRead {
            request_id,
            params: ConfigReadParams {
                include_layers: true,
                cwd: Some(cwd.clone()),
            },
        })
        .await
        .wrap_err("config/read failed while checking remote project trust")?;
    let project_layers = response
        .get("layers")
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter(|layer| layer["name"]["type"] == "project")
        .collect::<Vec<_>>();
    let disabled_project = project_layers.iter().rev().find(|layer| {
        layer
            .get("disabledReason")
            .and_then(JsonValue::as_str)
            .is_some()
    });
    let disabled_reason = disabled_project
        .and_then(|layer| layer.get("disabledReason"))
        .and_then(JsonValue::as_str);
    let projects = response["config"]["projects"].as_object();
    let windows_path = cwd_uri.infer_path_convention() == Some(PathConvention::Windows);
    let cwd_key = if windows_path {
        cwd.to_ascii_lowercase()
    } else {
        cwd.clone()
    };
    let project_entry = |key: &str| {
        let projects = projects?;
        projects
            .get_key_value(key)
            .filter(|(_, project)| project["trust_level"].as_str().is_some())
            .or_else(|| {
                projects
                    .iter()
                    .filter(|(path, project)| {
                        windows_path
                            && path.eq_ignore_ascii_case(key)
                            && project["trust_level"].as_str().is_some()
                    })
                    .min_by_key(|(path, _)| *path)
            })
    };
    let local_roots = if host == ProjectTrustHost::Local && project_layers.is_empty() {
        let cwd = AbsolutePathBuf::from_absolute_path(&cwd)?;
        let markers = serde_json::from_value::<Option<Vec<String>>>(
            response["config"]["project_root_markers"].clone(),
        )?
        .unwrap_or_else(default_project_root_markers);
        let project_root = find_project_root(LOCAL_FS.as_ref(), &cwd, &markers).await?;
        let git_root = resolve_root_git_project_for_trust(LOCAL_FS.as_ref(), &cwd).await;
        Some((
            normalized_project_trust_keys(project_root.as_path()),
            git_root.map(|root| normalized_project_trust_keys(root.as_path())),
        ))
    } else {
        None
    };
    let cwd_keys = if host == ProjectTrustHost::Local {
        normalized_project_trust_keys(Path::new(&cwd))
    } else {
        vec![cwd_key]
    };
    let saved_trust_target = |keys: &[String]| {
        keys.iter()
            .find_map(|key| project_entry(key).map(|(path, _)| path.as_str()))
    };
    let cwd_trust_target = saved_trust_target(&cwd_keys);
    let trust_target = cwd_trust_target
        .or_else(|| {
            disabled_reason
                .and_then(|reason| reason.split_once(", add "))
                .and_then(|(_, reason)| reason.rsplit_once(" as a trusted project in "))
                .map(|(trust_target, _)| trust_target)
                .or_else(|| {
                    disabled_project
                        .and_then(|layer| layer["name"]["dotCodexFolder"].as_str())
                        .and_then(|path| {
                            path.strip_suffix("/.codex")
                                .or_else(|| path.strip_suffix("\\.codex"))
                        })
                })
        })
        .or_else(|| {
            let (project_root, git_root) = local_roots.as_ref()?;
            saved_trust_target(project_root)
                .or_else(|| git_root.as_deref().and_then(saved_trust_target))
                .or_else(|| {
                    git_root
                        .as_ref()
                        .and_then(|keys| keys.first())
                        .map(String::as_str)
                })
                .or_else(|| project_root.first().map(String::as_str))
        })
        .unwrap_or(&cwd);
    let trust_level = project_entry(trust_target)
        .map(|(_, project)| project)
        .and_then(|project| project.get("trust_level"))
        .and_then(JsonValue::as_str)
        .and_then(|level| match level {
            "trusted" => Some(TrustLevel::Trusted),
            "untrusted" => Some(TrustLevel::Untrusted),
            _ => None,
        });
    let explicitly_untrusted = cwd_trust_target.is_none()
        && disabled_reason.is_some_and(|reason| {
            projects.into_iter().flatten().any(|(path, project)| {
                project.get("trust_level").and_then(JsonValue::as_str) == Some("untrusted")
                    && reason
                        .strip_prefix(path)
                        .is_some_and(|suffix| suffix.starts_with(" is marked as untrusted"))
            })
        });
    if !explicitly_untrusted
        && (trust_level == Some(TrustLevel::Trusted)
            || (trust_level.is_none()
                && disabled_project.is_none()
                && project_layers
                    .iter()
                    .any(|layer| layer.get("disabledReason").is_none())))
    {
        return Ok(None);
    }

    if host == ProjectTrustHost::Remote
        && trust_level.is_none()
        && !explicitly_untrusted
        && project_layers.is_empty()
        && projects.into_iter().flatten().any(|(path, project)| {
            project.get("trust_level").and_then(JsonValue::as_str) == Some("untrusted")
                && LegacyAppPathString::from_string(path.clone())
                    .to_inferred_path_uri()
                    .is_some_and(|project_uri| cwd_uri.starts_with(&project_uri))
        })
    {
        return Err(color_eyre::eyre::eyre!(
            "remote project directory is inside an explicitly untrusted project; pass the repository root explicitly with --cd"
        ));
    }

    Ok(Some(RemoteProjectTrust {
        trust_level: if explicitly_untrusted {
            Some(TrustLevel::Untrusted)
        } else {
            trust_level
        },
        cwd: PathBuf::from(&cwd),
        trust_target: PathBuf::from(trust_target),
    }))
}

pub(crate) async fn write_skill_enabled(
    request_handle: AppServerRequestHandle,
    path: AbsolutePathBuf,
    enabled: bool,
) -> Result<()> {
    let request_id = RequestId::String(format!("tui-skill-config-write-{}", Uuid::new_v4()));
    let _: SkillsConfigWriteResponse = request_handle
        .request_typed(ClientRequest::SkillsConfigWrite {
            request_id,
            params: SkillsConfigWriteParams {
                path: Some(path),
                name: None,
                enabled,
            },
        })
        .await
        .wrap_err("skills/config/write failed in TUI")?;
    Ok(())
}

#[cfg(test)]
#[path = "config_update_tests.rs"]
mod tests;

/// Read effective server settings, retaining compatibility with servers without config/read.
pub(crate) async fn read_effective_config_if_supported(
    request_handle: AppServerRequestHandle,
    cwd: &Path,
) -> Result<Option<codex_app_server_protocol::Config>> {
    match read_effective_config(request_handle, cwd.display().to_string()).await {
        Ok(response) => Ok(Some(response.config)),
        Err(err)
            if matches!(
                err.downcast_ref::<TypedRequestError>(),
                Some(TypedRequestError::Server { source, .. })
                    if source.code == -32601
                        || source.code == -32600
                            && source.message.contains("config/read")
                            && (source.message.contains("unknown variant")
                                || source.message.contains("unknown method"))
            ) =>
        {
            // Callers retain their legacy behavior when the server lacks config/read.
            Ok(None)
        }
        Err(err) => Err(err),
    }
}
