//! Read the existing Codex login for Git; the host retains ownership of refresh.

use anyhow::Result;
use anyhow::ensure;
use chrono::Utc;
use clap::Args;
use clap::ValueEnum;
use codex_app_server::in_process::EmbeddedNetworkPolicy;
use codex_cloud_config::managed_config_bundle_loader_for_auth;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_login::AuthManager;
use codex_login::is_workload_identity_selected;
use codex_login::token_data::parse_jwt_expiration;
use codex_protocol::auth::AuthMode;
use codex_utils_cli::CliConfigOverrides;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use url::Url;

#[derive(Debug, Args)]
pub struct GitCredentialArgs {
    #[command(flatten)]
    pub config_overrides: CliConfigOverrides,
    #[arg(value_enum)]
    operation: Operation,
}

#[derive(Clone, Debug, ValueEnum)]
enum Operation {
    Get,
    Store,
    Erase,
    Capability,
}

pub async fn run_git_credential(args: GitCredentialArgs) -> Result<()> {
    if matches!(args.operation, Operation::Capability) {
        println!("version 0\ncapability authtype");
        return Ok(());
    }
    let input = read_request()?;
    if !matches!(args.operation, Operation::Get) {
        // Git approval/rejection never stores another copy or changes the Codex login.
        return Ok(());
    }
    let fields = || input.lines().filter_map(|line| line.split_once('='));
    let field = |name| fields().find_map(|(key, value)| (key == name).then_some(value));
    if !fields().any(|pair| pair == ("capability[]", "authtype"))
        || field("protocol") != Some("https")
    {
        return Ok(());
    }
    let (Some(host), Some(path)) = (field("host"), field("path")) else {
        return Ok(());
    };
    if !path.starts_with("backend-api/codex/sites/git/")
        || path
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '\\' | '?' | '#'))
        || path.split('/').any(|part| matches!(part, "." | ".."))
    {
        return Ok(());
    }
    let Ok(destination) = Url::parse(&format!("https://{host}/{path}")) else {
        return Ok(());
    };
    if !matches!(
        destination.host_str(),
        Some("chatgpt.com" | "chatgpt-staging.com")
    ) || destination.port_or_known_default() != Some(443)
        || !destination
            .path()
            .starts_with("/backend-api/codex/sites/git/")
        || !destination.username().is_empty()
        || destination.password().is_some()
    {
        return Ok(());
    }

    let Some(token) = git_token_for_origin(args.config_overrides, &destination).await? else {
        println!("quit=true\n");
        return Ok(());
    };
    writeln!(
        std::io::stdout().lock(),
        "capability[]=authtype\nauthtype=Bearer\ncredential={token}\nephemeral=1\n"
    )?;
    Ok(())
}

async fn git_token_for_origin(
    overrides: CliConfigOverrides,
    destination: &Url,
) -> Result<Option<String>> {
    // Repository configuration must not select the credential source or its API origin.
    let builder = ConfigBuilder::default()
        .cli_overrides(overrides.parse_overrides().map_err(config_unavailable)?)
        .without_project_context();
    let config = builder.clone().build().await.map_err(config_unavailable)?;
    let matches_origin_and_store = |config: &Config| {
        config.cli_auth_credentials_store_mode == AuthCredentialsStoreMode::File
            && Url::parse(&config.chatgpt_base_url)
                .is_ok_and(|base| base.origin() == destination.origin())
    };
    // Git reads a file-backed user login. Other identities can require a network exchange.
    ensure!(
        matches_origin_and_store(&config) && !is_workload_identity_selected(),
        "Codex Git authentication requires a file-backed ChatGPT login for this origin"
    );
    let manager = AuthManager::shared_from_config_with_auth_modes(
        &config,
        /*enable_codex_api_key_env*/ false,
        vec![AuthMode::Chatgpt],
    )
    .await
    .map_err(|_| anyhow::anyhow!("Codex authentication is unavailable"))?;
    // Git runs inside the execution sandbox, which cannot persist a rotated login.
    let Some(auth) = manager
        .auth_cached()
        .filter(|auth| !auth.is_fedramp_account())
    else {
        return Ok(None);
    };
    let token = auth
        .get_token()
        .map_err(|_| anyhow::anyhow!("Codex authentication is unavailable"))?;
    let expiry = parse_jwt_expiration(&token).ok().flatten();
    let unexpired = || expiry.is_some_and(|expiry| expiry > Utc::now());
    if !unexpired() || !token.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Ok(None);
    }
    let requirements = config.config_layer_stack.requirements_toml();
    // Config-server OAuth uses the OS keyring, which Git must never open.
    ensure!(
        requirements
            .config_server
            .as_ref()
            .is_none_or(|server| server.oauth.is_none()),
        "Git cannot verify Codex workspace policy using a protected login"
    );
    let login_changes = manager.auth_change_receiver();
    let network = EmbeddedNetworkPolicy::load(&Default::default()).await;
    let bundle = managed_config_bundle_loader_for_auth(
        network.bind_bootstrap_auth(config.auth_config()),
        &auth,
        requirements,
    )
    .await
    .map_err(config_unavailable)?;
    let effective = builder
        .cloud_config_bundle(bundle)
        .build()
        .await
        .map_err(config_unavailable)?;
    effective
        .config_layer_stack
        .ensure_config_server_applied()
        .map_err(config_unavailable)?;
    ensure!(
        matches_origin_and_store(&effective) && effective.auth_config().allows_auth(&auth),
        "Codex workspace policy does not permit this Git login"
    );
    // The host can sign out or rotate the saved login while Git waits for policy.
    manager
        .reload_with_cached_auth()
        .await
        .map_err(config_unavailable)?;
    ensure!(
        !login_changes.has_changed()? && unexpired(),
        "Codex login changed or expired while checking Git access"
    );
    Ok(Some(token))
}

fn config_unavailable<E>(_: E) -> anyhow::Error {
    anyhow::anyhow!("Codex authentication configuration is unavailable")
}

fn read_request() -> Result<String> {
    let mut stdin = std::io::stdin().lock().take(16 * 1024 + 1);
    let mut input = String::new();
    loop {
        let start = input.len();
        stdin.read_line(&mut input)?;
        ensure!(stdin.limit() > 0, "Git credential request is too large");
        if matches!(&input[start..], "" | "\n" | "\r\n") {
            return Ok(input);
        }
    }
}
