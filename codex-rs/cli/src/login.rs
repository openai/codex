use codex_common::CliConfigOverrides;
use codex_core::CodexAuth;
use codex_core::auth::CLIENT_ID;
use codex_core::auth::OPENAI_API_KEY_ENV_VAR;
use codex_core::auth::login_with_api_key;
use codex_core::auth::logout;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_login::ServerOptions;
use codex_login::run_login_server;
use std::io::{self, Write};
use std::net::TcpStream;
use codex_protocol::mcp_protocol::AuthMode;
use std::env;
use std::path::PathBuf;

pub async fn login_with_chatgpt(codex_home: PathBuf, originator: String) -> std::io::Result<()> {
    let opts = ServerOptions::new(codex_home, CLIENT_ID.to_string(), originator);
    let server = run_login_server(opts)?;

    eprintln!(
        "\nStarting local login server on http://localhost:{}.\n\
If your browser did not open, use ANY browser to visit:\n\n{}\n",
        server.actual_port, server.auth_url
    );

    eprintln!("If the browser cannot reach localhost, paste the final redirected URL here.");
    eprintln!("(Or paste `code=<...>&state=<...>`). Press Enter to skip and just wait.\n");

    eprint!("paste> ");
    io::stderr().flush().ok();
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    let paste = line.trim();

    if !paste.is_empty() {
        let (code, state) = if paste.contains("code=") {
            let q = if paste.starts_with("http") {
                url::Url::parse(paste)
                    .ok()
                    .and_then(|u| u.query().map(|s| s.to_string()))
                    .unwrap_or_else(|| paste.to_string())
            } else {
                paste.to_string()
            };
            let mut code = None::<String>;
            let mut state = None::<String>;
            for pair in q.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    match k {
                        "code" => code = Some(v.to_string()),
                        "state" => state = Some(v.to_string()),
                        _ => {}
                    }
                }
            }
            (code.unwrap_or_default(), state.unwrap_or_default())
        } else {
            (paste.to_string(), String::new())
        };

        if !code.is_empty() {
            let path = format!(
                "/auth/callback?code={}&state={}",
                urlencoding::encode(&code),
                urlencoding::encode(&state)
            );
            let req = format!(
                "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                path
            );
            let mut stream =
                TcpStream::connect(("127.0.0.1", server.actual_port))?;
            use std::io::Write as _;
            stream.write_all(req.as_bytes())?;
        }
    }

    server.block_until_done().await
}

pub async fn run_login_with_chatgpt(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match login_with_chatgpt(
        config.codex_home,
        config.responses_originator_header.clone(),
    )
    .await
    {
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

pub async fn run_login_with_api_key(
    cli_config_overrides: CliConfigOverrides,
    api_key: String,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

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

    match CodexAuth::from_codex_home(
        &config.codex_home,
        config.preferred_auth_method,
        &config.responses_originator_header,
    ) {
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
