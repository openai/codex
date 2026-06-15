use codex_config::ConfigRequirements;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_config::config_toml::ConfigToml;
use codex_config::types::AuthCredentialsStoreMode;
use codex_config::types::FeedbackConfigToml;
use codex_config::types::ShellEnvironmentPolicyToml;
use codex_protocol::config_types::ForcedLoginMethod;

use super::otel;

/// Runtime values that cannot be applied by mutating [`ConfigToml`] alone.
///
/// Other managed settings are written back into `ConfigToml`. These values need
/// additional credential-store compatibility handling or intersection with the
/// configured authentication restrictions before constructing the final config.
pub(super) struct AppliedConfigRequirements {
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub forced_chatgpt_workspace_id: Option<Vec<String>>,
}

/// Applies managed requirements to regular config before final config construction.
///
/// Managed values replace or merge with their configured counterparts, and
/// conflicts produce source-aware startup warnings. Effective authentication
/// values that need additional resolution are returned separately. Invalid or
/// conflicting requirements return an error.
pub(super) fn apply_to_config(
    config: &mut ConfigToml,
    requirements: &ConfigRequirements,
    startup_warnings: &mut Vec<String>,
) -> std::io::Result<AppliedConfigRequirements> {
    macro_rules! apply_exact {
        ($field:ident) => {
            apply_exact_requirement(
                stringify!($field),
                &mut config.$field,
                requirements.$field.as_ref(),
                startup_warnings,
            );
        };
    }

    apply_exact!(cli_auth_credentials_store);
    apply_exact!(chatgpt_base_url);
    apply_exact!(sqlite_home);
    apply_exact!(log_dir);
    apply_exact!(model_catalog_json);
    apply_exact!(check_for_update_on_startup);
    apply_exact!(allow_login_shell);
    apply_shell_environment_policy_requirement(
        &mut config.shell_environment_policy,
        requirements.shell_environment_policy.as_ref(),
        startup_warnings,
    );
    if let Some(requirement) = requirements.otel.as_ref() {
        otel::apply_requirement(&mut config.otel, requirement, startup_warnings)?;
    }
    apply_feedback_requirement(
        &mut config.feedback,
        requirements.feedback.as_ref(),
        startup_warnings,
    );
    if let Some(requirement) = requirements.windows_sandbox_private_desktop.as_ref() {
        apply_exact_requirement(
            "windows.sandbox_private_desktop",
            &mut config
                .windows
                .get_or_insert_default()
                .sandbox_private_desktop,
            Some(requirement),
            startup_warnings,
        );
    }

    let (forced_login_method, forced_chatgpt_workspace_id) =
        resolve_auth_restrictions(config, requirements)?;
    let configured_auth_credentials_store = config.cli_auth_credentials_store.unwrap_or_default();
    let cli_auth_credentials_store_mode = if requirements.cli_auth_credentials_store.is_some() {
        configured_auth_credentials_store
    } else {
        super::resolve_cli_auth_credentials_store_mode(
            configured_auth_credentials_store,
            env!("CARGO_PKG_VERSION"),
        )
    };

    Ok(AppliedConfigRequirements {
        cli_auth_credentials_store_mode,
        forced_login_method,
        forced_chatgpt_workspace_id,
    })
}

fn apply_exact_requirement<T>(
    field_name: &'static str,
    configured_value: &mut Option<T>,
    requirement: Option<&Sourced<T>>,
    startup_warnings: &mut Vec<String>,
) where
    T: Clone + PartialEq + std::fmt::Debug,
{
    let Some(Sourced { value, source }) = requirement else {
        return;
    };
    if configured_value
        .as_ref()
        .is_some_and(|configured| configured != value)
    {
        tracing::warn!(
            ?source,
            ?value,
            "configured value is overridden by an exact requirement for {field_name}"
        );
        startup_warnings.push(format!(
            "Configured value for `{field_name}` is overridden by the required value {value:?} from {source}."
        ));
    }
    *configured_value = Some(value.clone());
}

pub(super) fn replace_required_leaf<T: Clone + PartialEq>(
    configured: &mut Option<T>,
    required: &Option<T>,
) -> bool {
    let Some(required) = required else {
        return false;
    };
    let conflict = configured
        .as_ref()
        .is_some_and(|configured| configured != required);
    *configured = Some(required.clone());
    conflict
}

fn apply_shell_environment_policy_requirement(
    configured: &mut ShellEnvironmentPolicyToml,
    requirement: Option<&Sourced<ShellEnvironmentPolicyToml>>,
    startup_warnings: &mut Vec<String>,
) {
    let Some(Sourced { value, source }) = requirement else {
        return;
    };
    let ShellEnvironmentPolicyToml {
        inherit,
        ignore_default_excludes,
        exclude,
        r#set: required_set,
        include_only,
        experimental_use_profile,
    } = value;
    let mut conflict = false;

    conflict |= replace_required_leaf(&mut configured.inherit, inherit);
    conflict |= replace_required_leaf(
        &mut configured.ignore_default_excludes,
        ignore_default_excludes,
    );
    conflict |= replace_required_leaf(&mut configured.exclude, exclude);
    conflict |= replace_required_leaf(&mut configured.include_only, include_only);
    conflict |= replace_required_leaf(
        &mut configured.experimental_use_profile,
        experimental_use_profile,
    );

    if let Some(required) = required_set.as_ref() {
        let configured_set = configured.r#set.get_or_insert_default();
        conflict |= required.iter().any(|(key, value)| {
            configured_set
                .get(key)
                .is_some_and(|current| current != value)
        });
        configured_set.extend(required.clone());
    }

    push_structured_requirement_override_warning(
        "shell_environment_policy",
        conflict,
        source,
        startup_warnings,
    );
}

fn apply_feedback_requirement(
    configured: &mut Option<FeedbackConfigToml>,
    requirement: Option<&Sourced<FeedbackConfigToml>>,
    startup_warnings: &mut Vec<String>,
) {
    let Some(Sourced { value, source }) = requirement else {
        return;
    };
    let FeedbackConfigToml { enabled } = value;
    let configured = configured.get_or_insert_default();
    let conflict = replace_required_leaf(&mut configured.enabled, enabled);
    push_structured_requirement_override_warning("feedback", conflict, source, startup_warnings);
}

/// Emits one source-aware warning when a structured requirement replaces one
/// or more configured values.
pub(super) fn push_structured_requirement_override_warning(
    field_name: &str,
    conflict: bool,
    source: &RequirementSource,
    startup_warnings: &mut Vec<String>,
) {
    if !conflict {
        return;
    }
    tracing::warn!(
        ?source,
        "configured values are overridden by requirements for {field_name}"
    );
    startup_warnings.push(format!(
        "Configured values under `{field_name}` are overridden by requirements from {source}."
    ));
}

fn resolve_auth_restrictions(
    config: &ConfigToml,
    requirements: &ConfigRequirements,
) -> std::io::Result<(Option<ForcedLoginMethod>, Option<Vec<String>>)> {
    let configured_forced_chatgpt_workspace_id = config
        .forced_chatgpt_workspace_id
        .clone()
        .map(codex_config::config_toml::ForcedChatgptWorkspaceIds::into_vec)
        .map(|values| {
            values
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty());
    let forced_chatgpt_workspace_id = match requirements.allowed_chatgpt_workspaces.as_ref() {
        Some(requirement) => Some(match configured_forced_chatgpt_workspace_id {
            Some(configured) => configured
                .into_iter()
                .filter(|workspace| requirement.value.get(workspace).copied().unwrap_or(false))
                .collect(),
            None => requirement
                .value
                .iter()
                .filter_map(|(workspace, allowed)| allowed.then_some(workspace.clone()))
                .collect(),
        }),
        None => configured_forced_chatgpt_workspace_id,
    };

    let forced_login_method = match requirements.allowed_login_methods.as_ref() {
        Some(requirement) => {
            let chatgpt_allowed = requirement.value.get("chatgpt").copied().unwrap_or(false);
            let api_allowed = requirement.value.get("api").copied().unwrap_or(false);
            match config.forced_login_method {
                Some(ForcedLoginMethod::Chatgpt) if chatgpt_allowed => {
                    Some(ForcedLoginMethod::Chatgpt)
                }
                Some(ForcedLoginMethod::Api) if api_allowed => Some(ForcedLoginMethod::Api),
                Some(configured) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "configured `forced_login_method = \"{configured}\"` conflicts with `allowed_login_methods` from {}",
                            requirement.source
                        ),
                    ));
                }
                None => match (chatgpt_allowed, api_allowed) {
                    (true, true) => None,
                    (true, false) => Some(ForcedLoginMethod::Chatgpt),
                    (false, true) => Some(ForcedLoginMethod::Api),
                    (false, false) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!(
                                "`allowed_login_methods` from {} does not allow any login method",
                                requirement.source
                            ),
                        ));
                    }
                },
            }
        }
        None => config.forced_login_method,
    };

    Ok((forced_login_method, forced_chatgpt_workspace_id))
}
