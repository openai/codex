use codex_app_server_protocol::AuthMode;
use codex_common::CliConfigOverrides;
use codex_core::CodexAuth;
use codex_core::auth::AuthCredentialsStoreMode;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::list_oauth_accounts;
use codex_core::auth::login_with_api_key;
use codex_core::auth::logout;
use codex_core::auth::remove_all_oauth_accounts;
use codex_core::auth::remove_oauth_account;
use codex_core::config::Config;
use codex_login::ServerOptions;
use codex_login::run_device_code_login;
use codex_login::run_login_server;
use codex_protocol::config_types::ForcedLoginMethod;
use std::io::IsTerminal;
use std::io::Read;
use std::path::PathBuf;

const CHATGPT_LOGIN_DISABLED_MESSAGE: &str =
    "ChatGPT login is disabled. Use API key login instead.";
const API_KEY_LOGIN_DISABLED_MESSAGE: &str =
    "API key login is disabled. Use ChatGPT login instead.";
const LOGIN_SUCCESS_MESSAGE: &str = "Successfully logged in";

fn print_login_server_start(actual_port: u16, auth_url: &str) {
    eprintln!(
        "Starting local login server on http://localhost:{actual_port}.\nIf your browser did not open, navigate to this URL to authenticate:\n\n{auth_url}"
    );
}

pub async fn login_with_chatgpt(
    codex_home: PathBuf,
    forced_chatgpt_workspace_id: Option<String>,
    cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let opts = ServerOptions::new(
        codex_home,
        CLIENT_ID.to_string(),
        forced_chatgpt_workspace_id,
        cli_auth_credentials_store_mode,
    );
    let server = run_login_server(opts)?;

    print_login_server_start(server.actual_port, &server.auth_url);

    server.block_until_done().await
}

pub async fn run_login_with_chatgpt(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    if matches!(config.forced_login_method, Some(ForcedLoginMethod::Api)) {
        eprintln!("{CHATGPT_LOGIN_DISABLED_MESSAGE}");
        std::process::exit(1);
    }

    let forced_chatgpt_workspace_id = config.forced_chatgpt_workspace_id.clone();

    match login_with_chatgpt(
        config.codex_home,
        forced_chatgpt_workspace_id,
        config.cli_auth_credentials_store_mode,
    )
    .await
    {
        Ok(_) => {
            eprintln!("{LOGIN_SUCCESS_MESSAGE}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_with_api_key(
    cli_config_overrides: CliConfigOverrides,
    api_key: String,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    if matches!(config.forced_login_method, Some(ForcedLoginMethod::Chatgpt)) {
        eprintln!("{API_KEY_LOGIN_DISABLED_MESSAGE}");
        std::process::exit(1);
    }

    match login_with_api_key(
        &config.codex_home,
        &api_key,
        config.cli_auth_credentials_store_mode,
    ) {
        Ok(_) => {
            eprintln!("{LOGIN_SUCCESS_MESSAGE}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub fn read_api_key_from_stdin() -> String {
    let mut stdin = std::io::stdin();

    if stdin.is_terminal() {
        eprintln!(
            "--with-api-key expects the API key on stdin. Try piping it, e.g. `printenv OPENAI_API_KEY | codex login --with-api-key`."
        );
        std::process::exit(1);
    }

    eprintln!("Reading API key from stdin...");

    let mut buffer = String::new();
    if let Err(err) = stdin.read_to_string(&mut buffer) {
        eprintln!("Failed to read API key from stdin: {err}");
        std::process::exit(1);
    }

    let api_key = buffer.trim().to_string();
    if api_key.is_empty() {
        eprintln!("No API key provided via stdin.");
        std::process::exit(1);
    }

    api_key
}

/// Login using the OAuth device code flow.
pub async fn run_login_with_device_code(
    cli_config_overrides: CliConfigOverrides,
    issuer_base_url: Option<String>,
    client_id: Option<String>,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    if matches!(config.forced_login_method, Some(ForcedLoginMethod::Api)) {
        eprintln!("{CHATGPT_LOGIN_DISABLED_MESSAGE}");
        std::process::exit(1);
    }
    let forced_chatgpt_workspace_id = config.forced_chatgpt_workspace_id.clone();
    let mut opts = ServerOptions::new(
        config.codex_home,
        client_id.unwrap_or(CLIENT_ID.to_string()),
        forced_chatgpt_workspace_id,
        config.cli_auth_credentials_store_mode,
    );
    if let Some(iss) = issuer_base_url {
        opts.issuer = iss;
    }
    match run_device_code_login(opts).await {
        Ok(()) => {
            eprintln!("{LOGIN_SUCCESS_MESSAGE}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in with device code: {e}");
            std::process::exit(1);
        }
    }
}

/// Prefers device-code login (with `open_browser = false`) when headless environment is detected, but keeps
/// `codex login` working in environments where device-code may be disabled/feature-gated.
/// If `run_device_code_login` returns `ErrorKind::NotFound` ("device-code unsupported"), this
/// falls back to starting the local browser login server.
pub async fn run_login_with_device_code_fallback_to_browser(
    cli_config_overrides: CliConfigOverrides,
    issuer_base_url: Option<String>,
    client_id: Option<String>,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    if matches!(config.forced_login_method, Some(ForcedLoginMethod::Api)) {
        eprintln!("{CHATGPT_LOGIN_DISABLED_MESSAGE}");
        std::process::exit(1);
    }

    let forced_chatgpt_workspace_id = config.forced_chatgpt_workspace_id.clone();
    let mut opts = ServerOptions::new(
        config.codex_home,
        client_id.unwrap_or(CLIENT_ID.to_string()),
        forced_chatgpt_workspace_id,
        config.cli_auth_credentials_store_mode,
    );
    if let Some(iss) = issuer_base_url {
        opts.issuer = iss;
    }
    opts.open_browser = false;

    match run_device_code_login(opts.clone()).await {
        Ok(()) => {
            eprintln!("{LOGIN_SUCCESS_MESSAGE}");
            std::process::exit(0);
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("Device code login is not enabled; falling back to browser login.");
                match run_login_server(opts) {
                    Ok(server) => {
                        print_login_server_start(server.actual_port, &server.auth_url);
                        match server.block_until_done().await {
                            Ok(()) => {
                                eprintln!("{LOGIN_SUCCESS_MESSAGE}");
                                std::process::exit(0);
                            }
                            Err(e) => {
                                eprintln!("Error logging in: {e}");
                                std::process::exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error logging in: {e}");
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Error logging in with device code: {e}");
                std::process::exit(1);
            }
        }
    }
}

pub async fn run_login_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    match CodexAuth::from_auth_storage(&config.codex_home, config.cli_auth_credentials_store_mode) {
        Ok(Some(auth)) => match auth.mode {
            AuthMode::ApiKey => match auth.get_token() {
                Ok(api_key) => {
                    eprintln!("Logged in using an API key - {}", safe_format_key(&api_key));
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Unexpected error retrieving API key: {e}");
                    std::process::exit(1);
                }
            },
            AuthMode::ChatGPT => {
                eprintln!("Logged in using ChatGPT");
                std::process::exit(0);
            }
        },
        Ok(None) => {
            eprintln!("Not logged in");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error checking login status: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_accounts(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    let accounts =
        match list_oauth_accounts(&config.codex_home, config.cli_auth_credentials_store_mode) {
            Ok(accounts) => accounts,
            Err(e) => {
                eprintln!("Error listing ChatGPT accounts: {e}");
                std::process::exit(1);
            }
        };

    if accounts.is_empty() {
        eprintln!("No ChatGPT accounts stored.");
        std::process::exit(0);
    }

    eprintln!("ChatGPT accounts:");
    for account in accounts {
        let active = if account.active { "*" } else { " " };
        let label = account
            .label
            .as_deref()
            .or(account.email.as_deref())
            .unwrap_or("unknown");
        let email = account.email.as_deref().unwrap_or("-");
        let account_id = account.account_id.as_deref().unwrap_or("-");
        let last_refresh = account
            .last_refresh
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let cooldown = account
            .health
            .cooldown_until
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string());
        eprintln!(
            "{active} {label}  id={id}  email={email}  account={account_id}  last_refresh={last_refresh}  cooldown={cooldown}",
            id = account.record_id
        );
    }
    std::process::exit(0);
}

pub async fn run_logout(
    cli_config_overrides: CliConfigOverrides,
    account: Option<String>,
    all_accounts: bool,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;

    if let Some(selector) = account {
        let accounts =
            match list_oauth_accounts(&config.codex_home, config.cli_auth_credentials_store_mode) {
                Ok(accounts) => accounts,
                Err(e) => {
                    eprintln!("Error listing ChatGPT accounts: {e}");
                    std::process::exit(1);
                }
            };
        let matches: Vec<_> = accounts
            .iter()
            .filter(|acc| {
                acc.record_id == selector
                    || acc.email.as_deref() == Some(selector.as_str())
                    || acc.label.as_deref() == Some(selector.as_str())
            })
            .collect();

        if matches.is_empty() {
            eprintln!("No ChatGPT account matches '{selector}'.");
            std::process::exit(1);
        }

        if matches.len() > 1 {
            eprintln!("Multiple ChatGPT accounts match '{selector}'. Use the record id instead.");
            std::process::exit(1);
        }

        let record_id = &matches[0].record_id;
        match remove_oauth_account(
            &config.codex_home,
            config.cli_auth_credentials_store_mode,
            record_id,
        ) {
            Ok(true) => {
                eprintln!("Logged out ChatGPT account {record_id}");
                std::process::exit(0);
            }
            Ok(false) => {
                eprintln!("ChatGPT account {record_id} was not found.");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Error logging out ChatGPT account {record_id}: {e}");
                std::process::exit(1);
            }
        }
    }

    if all_accounts {
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "Refusing to log out all ChatGPT accounts without confirmation (stdin is not a TTY)."
            );
            std::process::exit(1);
        }

        let confirmed = match confirm("This will log out all ChatGPT accounts. Continue? [y/N]: ") {
            Ok(value) => value,
            Err(e) => {
                eprintln!("Error reading confirmation: {e}");
                std::process::exit(1);
            }
        };
        if !confirmed {
            eprintln!("Canceled.");
            std::process::exit(1);
        }

        let accounts =
            match list_oauth_accounts(&config.codex_home, config.cli_auth_credentials_store_mode) {
                Ok(accounts) => accounts,
                Err(e) => {
                    eprintln!("Error listing ChatGPT accounts: {e}");
                    std::process::exit(1);
                }
            };
        let count = accounts.len();
        match remove_all_oauth_accounts(&config.codex_home, config.cli_auth_credentials_store_mode)
        {
            Ok(true) => {
                eprintln!("Logged out {count} ChatGPT account(s).");
                std::process::exit(0);
            }
            Ok(false) => {
                eprintln!("No ChatGPT accounts stored.");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Error logging out ChatGPT accounts: {e}");
                std::process::exit(1);
            }
        }
    }

    match logout(&config.codex_home, config.cli_auth_credentials_store_mode) {
        Ok(true) => {
            eprintln!("Successfully logged out");
            std::process::exit(0);
        }
        Ok(false) => {
            eprintln!("Not logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging out: {e}");
            std::process::exit(1);
        }
    }
}

fn confirm(prompt: &str) -> std::io::Result<bool> {
    eprintln!("{prompt}");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let answer = input.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

async fn load_config_or_exit(cli_config_overrides: CliConfigOverrides) -> Config {
    let cli_overrides = match cli_config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    match Config::load_with_cli_overrides(cli_overrides).await {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            std::process::exit(1);
        }
    }
}

fn safe_format_key(key: &str) -> String {
    if key.len() <= 13 {
        return "***".to_string();
    }
    let prefix = &key[..8];
    let suffix = &key[key.len() - 5..];
    format!("{prefix}***{suffix}")
}

#[cfg(test)]
mod tests {
    use super::safe_format_key;

    #[test]
    fn formats_long_key() {
        let key = "sk-proj-1234567890ABCDE";
        assert_eq!(safe_format_key(key), "sk-proj-***ABCDE");
    }

    #[test]
    fn short_key_returns_stars() {
        let key = "sk-proj-12345";
        assert_eq!(safe_format_key(key), "***");
    }
}
