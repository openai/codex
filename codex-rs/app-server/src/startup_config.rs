//! Early policy discovery and configuration-server sign-in. Never initializes environments, thread storage, or plugins.

mod oauth;
#[cfg(test)]
mod oauth_tests;

use crate::config_manager::ConfigManager;
use crate::request_processors::account_processor::clear_account_configuration;
use crate::request_processors::account_processor::logout_configured_account;
use crate::request_processors::config_errors::cloud_config_bundle_load_error;
use crate::request_processors::config_errors::config_load_error;
use crate::request_processors::config_processor::map_requirements_to_api;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_core::config::find_codex_home;
use codex_login::AuthManager;
use codex_login::default_client::SetOriginatorError;
use codex_login::default_client::USER_AGENT_SUFFIX;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::default_client::set_default_originator;
use codex_login::is_workload_identity_selected;
use codex_login::read_codex_access_token_from_env;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use std::io;
use std::io::Write;
use std::sync::Arc;

#[derive(Debug, clap::Args)]
pub struct StartupConfigArgs {
    /// Sign out using the managed auth path before loading fresh startup requirements.
    #[arg(
        long,
        requires = "print_startup_config",
        conflicts_with = "startup_oauth"
    )]
    pub startup_sign_out: bool,

    /// Serve only configuration-server OAuth requests over standard input and output.
    #[arg(long, group = "startup_mode", conflicts_with_all = ["listen", "stdio", "remote_control", "managed_daemon"])]
    pub startup_oauth: bool,

    /// Validate configuration and print requirements as JSON without starting the app server.
    #[arg(long, group = "startup_mode", conflicts_with_all = ["listen", "stdio", "remote_control", "managed_daemon"])]
    pub print_startup_config: bool,

    /// ClientInfo JSON used for configuration/auth request identity.
    #[arg(long, requires = "startup_mode", value_parser = |value: &str| serde_json::from_str::<ClientInfo>(value))]
    pub startup_client_info: Option<ClientInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartupConfig {
    app_server_version: &'static str,
    config_requirements: ConfigRequirementsReadResponse,
}

/// Runs the selected early startup operation before the regular app server initializes.
pub async fn print_startup_config(
    arg0_paths: Arg0DispatchPaths,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    startup: StartupConfigArgs,
) -> io::Result<()> {
    let manager = startup_manager(
        arg0_paths,
        cli_config_overrides,
        loader_overrides,
        strict_config,
        startup.startup_client_info,
    );
    if startup.startup_oauth {
        let manager = manager.map_err(|error| io::Error::other(error.message))?;
        return oauth::serve(manager, tokio::io::stdin(), tokio::io::stdout()).await;
    }
    let result = match manager {
        Ok(manager) => read_startup_config(manager, startup.startup_sign_out).await,
        Err(error) => Err(error),
    };
    let output = match &result {
        Ok(config) => serde_json::to_vec(config)?,
        Err(error) => serde_json::to_vec(&serde_json::json!({ "error": error }))?,
    };
    let mut stdout = io::stdout().lock();
    stdout.write_all(&output)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    result
        .map(|_| ())
        .map_err(|_| io::Error::other("startup configuration unavailable"))
}

async fn read_startup_config(
    manager: ConfigManager,
    sign_out: bool,
) -> Result<StartupConfig, JSONRPCErrorError> {
    if sign_out && (is_workload_identity_selected() || read_codex_access_token_from_env().is_some())
    {
        return Err(crate::error_code::invalid_request(
            "Configured external authentication is owned by the app-server host and cannot be changed during startup recovery.",
        ));
    }
    let cwd = std::env::current_dir().ok();
    let loaded = manager
        .load_managed_startup_config(cwd.clone())
        .await
        .map_err(startup_error)?;
    let config = match loaded.config {
        Ok(config) => config,
        // Recovery retains local credential settings when cloud policy is unavailable.
        Err(_) if sign_out => loaded.bootstrap.clone(),
        Err(error) => {
            let loader = manager.current_managed_config_loader();
            let server = loader
                .config_server_resolution()
                .map(|resolution| &resolution.resolved)
                .or_else(|| loader.config_server());
            return Err(if server.is_some_and(|server| server.oauth.is_some()) {
                config_server_error(error)
            } else {
                startup_error(error)
            });
        }
    };
    let config = if sign_out {
        // Preserve local credential authority while using the effective HTTP route.
        let mut auth_config = loaded.bootstrap.auth_config();
        auth_config.auth_route_config = config.auth_route_config();
        auth_config.chatgpt_base_url = Some(config.chatgpt_base_url.clone());
        let auth = AuthManager::shared_from_auth_config(
            auth_config,
            /*enable_codex_api_key_env*/ false,
        )
        .await
        .map_err(|error| startup_error(io::Error::other(error)))?;
        set_default_client_residency_requirement(config.enforce_residency.value());
        let config = logout_configured_account(&manager, &auth, &config).await?;
        clear_account_configuration(&manager, &config).await?;
        manager
            .update_managed_config_loader(Arc::clone(&auth), &config)
            .await;
        manager
            .load_latest_config(cwd)
            .await
            .map_err(startup_error)?
    } else {
        config
    };
    config
        .config_layer_stack
        .ensure_config_server_applied()
        .map_err(startup_error)?;
    // Validate and emit the fetched snapshot, or the fresh signed-out snapshot.
    config
        .validate_windows_mxc_requirement()
        .map_err(startup_error)?;
    let auth_config = config.auth_config();
    auth_config.validate().map_err(startup_error)?;
    let allowed_login_methods = auth_config.managed_auth_policy.allowed_login_methods(
        auth_config.forced_login_method,
        auth_config.forced_chatgpt_workspace_id.as_deref(),
    );
    let requirements = config.config_layer_stack.requirements_toml();
    Ok(StartupConfig {
        app_server_version: env!("CARGO_PKG_VERSION"),
        config_requirements: ConfigRequirementsReadResponse {
            supports_independent_speed_modes: Some(true),
            requirements: map_requirements_to_api(
                (!requirements.is_empty()).then(|| requirements.clone()),
                allowed_login_methods,
            ),
        },
    })
}

fn startup_manager(
    arg0_paths: Arg0DispatchPaths,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    client_info: Option<ClientInfo>,
) -> Result<ConfigManager, JSONRPCErrorError> {
    if let Some(client) = client_info {
        match set_default_originator(client.name.clone()) {
            Ok(()) | Err(SetOriginatorError::AlreadyInitialized) => {}
            Err(SetOriginatorError::InvalidHeaderValue) => {
                return Err(crate::error_code::invalid_params("invalid client name"));
            }
        }
        if let Ok(mut suffix) = USER_AGENT_SUFFIX.lock() {
            *suffix = Some(format!("{}; {}", client.name, client.version));
        }
    }
    let loader_overrides = crate::loader_overrides_with_test_user_config_file(
        loader_overrides,
        crate::test_user_config_file_from_env(),
    )
    .map_err(startup_error)?;
    let overrides = cli_config_overrides
        .parse_overrides()
        .map_err(|error| startup_error(io::Error::new(io::ErrorKind::InvalidInput, error)))?;
    Ok(ConfigManager::new(
        find_codex_home().map_err(startup_error)?.to_path_buf(),
        overrides,
        loader_overrides,
        strict_config,
        Default::default(),
        arg0_paths,
        Arc::new(NoopThreadConfigLoader),
    ))
}

fn config_server_error(error: io::Error) -> JSONRPCErrorError {
    let Some(auth) = cloud_config_bundle_load_error(&error)
        .filter(|err| err.code() == CloudConfigBundleLoadErrorCode::Auth)
    else {
        return startup_error(error);
    };
    if !auth.requires_login() {
        return config_server_login_failed(auth.to_string());
    }

    let mut error = crate::error_code::invalid_request("Configuration server sign-in required");
    error.data = Some(serde_json::json!({
        "reason": "configServer",
        "errorCode": "Auth",
        "action": "configServerLogin",
    }));
    error
}

fn config_server_login_failed(message: impl Into<String>) -> JSONRPCErrorError {
    let mut error = crate::error_code::invalid_request(message);
    error.data = Some(serde_json::json!({
        "reason": "configServer",
        "errorCode": "LoginFailed",
    }));
    error
}

fn startup_error(error: io::Error) -> JSONRPCErrorError {
    let mut response = config_load_error(&error);
    if response.data.is_none() {
        let warning = crate::config_warning_from_error("Invalid configuration", &error);
        response.data = Some(serde_json::json!({
            "reason": "configLoad",
            "filePath": warning.path,
            "line": warning.range.as_ref().map(|range| range.start.line),
            "column": warning.range.as_ref().map(|range| range.start.column),
            "detail": error.to_string(),
        }));
    }
    response
}
