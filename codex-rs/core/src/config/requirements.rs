use codex_config::ConfigRequirements;
use codex_config::RequirementSource;
use codex_config::Sourced;
use codex_config::config_toml::ConfigToml;
use codex_config::types::FeedbackConfigToml;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;

/// Applies managed requirements to regular config before final config construction.
///
/// Managed values replace their configured counterparts, and conflicts produce
/// source-aware startup warnings.
pub(super) fn apply_to_config(
    config: &mut ConfigToml,
    requirements: &ConfigRequirements,
    startup_warnings: &mut Vec<String>,
) {
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

    apply_exact!(sqlite_home);
    apply_exact!(log_dir);
    apply_exact!(model_catalog_json);
    apply_exact!(check_for_update_on_startup);
    apply_exact!(allow_login_shell);
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

fn replace_required_leaf<T: Clone + PartialEq>(
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

pub(super) fn push_sqlite_home_env_override_warning(
    configured_sqlite_home: Option<&AbsolutePathBuf>,
    sqlite_home_env: Option<&Path>,
    requirement: Option<&Sourced<AbsolutePathBuf>>,
    startup_warnings: &mut Vec<String>,
) {
    if configured_sqlite_home.is_some() {
        return;
    }
    let Some(sqlite_home_env) = sqlite_home_env else {
        return;
    };
    let Some(Sourced { value, source }) = requirement else {
        return;
    };
    if sqlite_home_env == value.as_path() {
        return;
    }

    tracing::warn!(
        ?source,
        ?value,
        "`CODEX_SQLITE_HOME` is overridden by an exact requirement for sqlite_home"
    );
    startup_warnings.push(format!(
        "Environment value for `$CODEX_SQLITE_HOME` is overridden by the required `sqlite_home` value {value:?} from {source}."
    ));
}

/// Emits one source-aware warning when a structured requirement replaces one
/// or more configured values.
fn push_structured_requirement_override_warning(
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
