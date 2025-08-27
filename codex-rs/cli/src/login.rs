use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_login::AuthMode;
use codex_login::CLIENT_ID;
use codex_login::CodexAuth;
use codex_login::LoginError;
use codex_login::OPENAI_API_KEY_ENV_VAR;
use codex_login::ServerOptions;
use codex_login::login_with_api_key;
use codex_login::login_with_native_browser;
use codex_login::logout;
use codex_login::run_login_server;
use std::env;
use std::path::PathBuf;

pub async fn login_with_chatgpt(codex_home: PathBuf) -> std::io::Result<()> {
    let opts = ServerOptions::new(codex_home, CLIENT_ID.to_string());
    let server = run_login_server(opts)?;

    eprintln!(
        "Starting local login server on http://localhost:{}.\nIf your browser did not open, navigate to this URL to authenticate:\n\n{}",
        server.actual_port, server.auth_url,
    );

    server.block_until_done().await
}

pub async fn run_login_with_chatgpt(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    if !maybe_confirm_relogin(&config).await {
        eprintln!("Keeping existing login; aborted starting a new login.");
        std::process::exit(0);
    }

    match login_with_chatgpt(config.codex_home).await {
        Ok(_) => {
            eprintln!("Successfully logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_with_browser(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    if !maybe_confirm_relogin(&config).await {
        eprintln!("Keeping existing login; aborted starting a new login.");
        std::process::exit(0);
    }

    match login_with_native_browser(&config.codex_home).await {
        Ok(_) => {
            // Load details to show a friendly summary
            match CodexAuth::from_codex_home(&config.codex_home, config.preferred_auth_method) {
                Ok(Some(auth)) => {
                    let mut summary =
                        String::from("✅ Successfully logged in using native browser");
                    if let Ok(tokens) = auth.get_token_data().await {
                        if let Some(email) = tokens.id_token.email.as_deref() {
                            summary.push_str(&format!(" – {}", email));
                        }
                        if let Some(plan) = tokens.id_token.get_chatgpt_plan_type() {
                            summary.push_str(&format!(" (plan: {})", plan));
                        }
                    }
                    eprintln!("{}", summary);
                }
                _ => {
                    eprintln!("✅ Successfully logged in using native browser");
                }
            }
            std::process::exit(0);
        }
        Err(e) => match e {
            LoginError::Aborted => {
                eprintln!("Login aborted (native browser window closed)");
                std::process::exit(2);
            }
            LoginError::UnsupportedOs => {
                eprintln!("Native browser login is only supported on macOS at this time");
                std::process::exit(1);
            }
            other => {
                eprintln!("Error logging in with native browser: {other}");
                std::process::exit(1);
            }
        },
    }
}

pub async fn run_login_with_api_key(
    cli_config_overrides: CliConfigOverrides,
    api_key: String,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    if !maybe_confirm_relogin(&config).await {
        eprintln!("Keeping existing login; aborted starting a new login.");
        std::process::exit(0);
    }

    match login_with_api_key(&config.codex_home, &api_key) {
        Ok(_) => {
            eprintln!("Successfully logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match CodexAuth::from_codex_home(&config.codex_home, config.preferred_auth_method) {
        Ok(Some(auth)) => match auth.mode {
            AuthMode::ApiKey => match auth.get_token().await {
                Ok(api_key) => {
                    eprintln!("Logged in using an API key - {}", safe_format_key(&api_key));

                    if let Ok(env_api_key) = env::var(OPENAI_API_KEY_ENV_VAR)
                        && env_api_key == api_key
                    {
                        eprintln!(
                            "   API loaded from OPENAI_API_KEY environment variable or .env file"
                        );
                    }
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

pub async fn run_logout(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match logout(&config.codex_home) {
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

fn load_config_or_exit(cli_config_overrides: CliConfigOverrides) -> Config {
    let cli_overrides = match cli_config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let config_overrides = ConfigOverrides::default();
    match Config::load_with_cli_overrides(cli_overrides, config_overrides) {
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

async fn maybe_confirm_relogin(config: &Config) -> bool {
    // Only treat persisted credentials (auth.json) as an "existing login" for confirmation.
    // If users only have OPENAI_API_KEY in their environment, do not block with a confirm.
    let auth_file = codex_login::get_auth_file(&config.codex_home);
    if !auth_file.exists() {
        return true;
    }

    // If auth.json exists, load and present details for confirmation.
    let Some(existing) =
        CodexAuth::from_codex_home(&config.codex_home, config.preferred_auth_method)
            .ok()
            .flatten()
    else {
        return true;
    };

    let mode_label = match existing.mode {
        AuthMode::ApiKey => "API key",
        AuthMode::ChatGPT => "ChatGPT",
    };
    // Compose a prompt for the TUI confirm modal (or fallback to stdin when not a TTY).
    let mut prompt = format!("You are already logged in using: {mode_label}\n");
    if matches!(existing.mode, AuthMode::ChatGPT) {
        if let Ok(tokens) = existing.get_token_data().await {
            if let Some(email) = tokens.id_token.email.as_deref() {
                prompt.push_str(&format!("  • Account: {}\n", email));
            }
            if let Some(plan) = tokens.id_token.get_chatgpt_plan_type() {
                prompt.push_str(&format!("  • Plan: {}\n", plan));
            }
        }
    } else if existing.mode == AuthMode::ApiKey
        && let Ok(token) = existing.get_token().await
    {
        prompt.push_str(&format!("  • API key: {}\n", safe_format_key(&token)));
    }
    eprintln!("{}", prompt);
    eprint!("Proceed to start a new login? [y/N]: ");
    use std::io;
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
    let answer = buf.trim().to_ascii_lowercase();
    matches!(answer.as_str(), "y" | "yes")
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
