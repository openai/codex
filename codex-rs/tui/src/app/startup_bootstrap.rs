//! Server-owned model selection for connected fresh starts.
//!
//! Embedded and explicit-profile starts retain legacy behavior. Unsupported `config/read` never
//! restores an implicit local model, while managed new-thread defaults may satisfy an empty catalog.

use super::*;
use crate::app_server_session::StartupLaunchChoices;

pub(crate) fn uses_server_owned_fresh_bootstrap(
    target: &AppServerTarget,
    selection: &SessionSelection,
    loader_overrides: &LoaderOverrides,
) -> bool {
    !matches!(target, AppServerTarget::Embedded)
        && matches!(
            selection,
            SessionSelection::StartFresh | SessionSelection::Exit
        )
        && loader_overrides.user_config_profile.is_none()
}

pub(super) async fn bootstrap_server_owned_start(
    app_server: &mut AppServerSession,
    config: &mut Config,
    launch_choices: &mut StartupLaunchChoices,
    server_defaults_read: bool,
    cli_kv_overrides: &[(String, TomlValue)],
    harness_overrides: &ConfigOverrides,
) -> Result<AppServerBootstrap> {
    if !server_defaults_read {
        config.model = launch_choices.model.clone();
        if launch_choices.effort.is_none() {
            config.model_reasoning_effort = None;
        }
    }
    let bootstrap = if launch_choices.model.is_none() && launch_choices.effort.is_none() {
        app_server.bootstrap_for_new_thread(config).await
    } else {
        app_server.bootstrap(config).await
    }?;
    let previous = (config.model.clone(), config.model_reasoning_effort.clone());
    apply_managed_new_thread_defaults(
        config,
        app_server.managed_new_thread_defaults(),
        cli_kv_overrides,
        harness_overrides,
    );
    if (config.model.clone(), config.model_reasoning_effort.clone()) != previous {
        launch_choices.model = config.model.clone();
        launch_choices.effort = config
            .model_reasoning_effort
            .as_ref()
            .map(|effort| serde_json::json!(effort));
    }
    Ok(bootstrap)
}
