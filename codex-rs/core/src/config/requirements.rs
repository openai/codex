use codex_config::ConfigRequirements;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_config::config_toml::ConfigToml;
use codex_config::types::AuthCredentialsStoreMode;
use codex_config::types::FeedbackConfigToml;
use codex_config::types::OtelConfigToml;
use codex_config::types::ShellEnvironmentPolicyToml;
use codex_protocol::config_types::ForcedLoginMethod;

use super::otel;

pub(super) struct AppliedConfigRequirements {
    pub auth_credentials_store: AuthCredentialsStoreRequirement,
}

pub(super) enum AuthCredentialsStoreRequirement {
    Exact,
    UserConfigured,
}

impl AuthCredentialsStoreRequirement {
    pub fn resolve(self, configured: AuthCredentialsStoreMode) -> AuthCredentialsStoreMode {
        match self {
            Self::Exact => configured,
            Self::UserConfigured => super::resolve_cli_auth_credentials_store_mode(
                configured,
                env!("CARGO_PKG_VERSION"),
            ),
        }
    }
}

pub(super) struct ResolvedAuthRestrictions {
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub forced_chatgpt_workspace_id: Option<Vec<String>>,
}

pub(super) struct ConfiguredAuthRestrictions {
    forced_login_method: Option<ForcedLoginMethod>,
    forced_chatgpt_workspace_id: Option<Vec<String>>,
}

impl ConfiguredAuthRestrictions {
    pub fn from_config(config: &ConfigToml) -> Self {
        let forced_chatgpt_workspace_id = config
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
        Self {
            forced_login_method: config.forced_login_method,
            forced_chatgpt_workspace_id,
        }
    }
}

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
        validate_otel_requirement(&requirement.value)?;
        apply_otel_requirement(&mut config.otel, requirement, startup_warnings);
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

    Ok(AppliedConfigRequirements {
        auth_credentials_store: if requirements.cli_auth_credentials_store.is_some() {
            AuthCredentialsStoreRequirement::Exact
        } else {
            AuthCredentialsStoreRequirement::UserConfigured
        },
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

    macro_rules! replace_leaf {
        ($field:ident) => {
            if let Some(required) = $field.as_ref() {
                conflict |= configured
                    .$field
                    .as_ref()
                    .is_some_and(|configured| configured != required);
                configured.$field = Some(required.clone());
            }
        };
    }

    replace_leaf!(inherit);
    replace_leaf!(ignore_default_excludes);
    replace_leaf!(exclude);
    replace_leaf!(include_only);
    replace_leaf!(experimental_use_profile);

    if let Some(required) = required_set.as_ref() {
        let configured_set = configured.r#set.get_or_insert_default();
        conflict |= required.iter().any(|(key, value)| {
            configured_set
                .get(key)
                .is_some_and(|current| current != value)
        });
        configured_set.extend(required.clone());
    }

    push_leaf_override_warning(
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
    let conflict = if let Some(required) = *enabled {
        let conflict = configured
            .enabled
            .is_some_and(|configured| configured != required);
        configured.enabled = Some(required);
        conflict
    } else {
        false
    };
    push_leaf_override_warning("feedback", conflict, source, startup_warnings);
}

fn validate_otel_requirement(requirement: &OtelConfigToml) -> std::io::Result<()> {
    if let Some(span_attributes) = requirement.span_attributes.as_ref() {
        codex_otel::validate_span_attributes(span_attributes).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid required `otel.span_attributes`: {err}"),
            )
        })?;
    }
    if let Some(tracestate) = requirement.tracestate.as_ref() {
        codex_otel::validate_tracestate_entries(tracestate).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid required `otel.tracestate`: {err}"),
            )
        })?;
    }
    Ok(())
}

fn apply_otel_requirement(
    configured: &mut Option<OtelConfigToml>,
    requirement: &Sourced<OtelConfigToml>,
    startup_warnings: &mut Vec<String>,
) {
    let Sourced {
        value: required,
        source,
    } = requirement;
    let OtelConfigToml {
        log_user_prompt,
        environment,
        exporter,
        trace_exporter,
        metrics_exporter,
        span_attributes,
        tracestate,
    } = required;
    let configured = configured.get_or_insert_default();
    let mut conflict = false;

    macro_rules! replace_leaf {
        ($field:ident) => {
            if let Some(required) = $field.as_ref() {
                conflict |= configured
                    .$field
                    .as_ref()
                    .is_some_and(|configured| configured != required);
                configured.$field = Some(required.clone());
            }
        };
    }

    replace_leaf!(log_user_prompt);
    replace_leaf!(environment);
    replace_leaf!(exporter);
    replace_leaf!(trace_exporter);
    replace_leaf!(metrics_exporter);

    if let Some(required) = span_attributes.as_ref() {
        let configured = configured.span_attributes.get_or_insert_default();
        conflict |= required
            .iter()
            .any(|(key, value)| configured.get(key).is_some_and(|current| current != value));
        configured.extend(required.clone());
    }
    if let Some(required) = tracestate.as_ref() {
        let configured_tracestate = configured.tracestate.take().unwrap_or_default();
        conflict |= required.iter().any(|(member, required_fields)| {
            configured_tracestate
                .get(member)
                .is_some_and(|configured_fields| {
                    required_fields.iter().any(|(key, value)| {
                        configured_fields
                            .get(key)
                            .is_some_and(|current| current != value)
                    })
                })
        });
        configured.tracestate = Some(otel::merge_required_tracestate(
            configured_tracestate,
            required.clone(),
            source,
            startup_warnings,
        ));
    }

    if conflict {
        tracing::warn!(
            ?source,
            "configured OTEL leaves are overridden by exact requirements"
        );
        startup_warnings.push(format!(
            "Configured leaves under `otel` are overridden by requirements from {source}."
        ));
    }
}

fn push_leaf_override_warning(
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
        "configured value contains leaves overridden by exact requirements for {field_name}"
    );
    startup_warnings.push(format!(
        "Configured leaves under `{field_name}` are overridden by requirements from {source}."
    ));
}

pub(super) fn resolve_auth_restrictions(
    configured: ConfiguredAuthRestrictions,
    requirements: &ConfigRequirements,
) -> std::io::Result<ResolvedAuthRestrictions> {
    let forced_chatgpt_workspace_id = match requirements.allowed_chatgpt_workspaces.as_ref() {
        Some(requirement) => Some(match configured.forced_chatgpt_workspace_id {
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
        None => configured.forced_chatgpt_workspace_id,
    };

    let forced_login_method = match requirements.allowed_login_methods.as_ref() {
        Some(requirement) => {
            let chatgpt_allowed = requirement.value.get("chatgpt").copied().unwrap_or(false);
            let api_allowed = requirement.value.get("api").copied().unwrap_or(false);
            match configured.forced_login_method {
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
        None => configured.forced_login_method,
    };

    Ok(ResolvedAuthRestrictions {
        forced_login_method,
        forced_chatgpt_workspace_id,
    })
}
