use std::io::ErrorKind;
use std::path::Path;

use codex_common::CliConfigOverrides;
use codex_core::CodexAuth;

use crate::login::load_config_or_exit;
use crate::login::login_with_chatgpt;

pub async fn run_setup_token(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides).await;
    let auth_path = codex_core::auth::get_auth_file(&config.codex_home);

    match has_login(&config.codex_home) {
        Ok(true) => {
            if let Err(err) = print_auth_json(&auth_path) {
                eprintln!(
                    "Failed to read existing auth.json at {}: {err}",
                    auth_path.display()
                );
                std::process::exit(1);
            }
            std::process::exit(0);
        }
        Ok(false) => {}
        Err(err) => {
            eprintln!(
                "Failed to inspect auth.json at {}: {err}",
                auth_path.display()
            );
            std::process::exit(1);
        }
    }

    if let Err(err) = login_with_chatgpt(config.codex_home.clone()).await {
        eprintln!("Error during login: {err}");
        std::process::exit(1);
    }

    if let Err(err) = print_auth_json(&auth_path) {
        eprintln!(
            "Failed to read auth.json after login at {}: {err}",
            auth_path.display()
        );
        std::process::exit(1);
    }

    std::process::exit(0);
}

fn has_login(codex_home: &Path) -> std::io::Result<bool> {
    match CodexAuth::from_codex_home(codex_home) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

fn print_auth_json(auth_path: &Path) -> std::io::Result<()> {
    let contents = std::fs::read_to_string(auth_path)?;
    print!("{contents}");
    Ok(())
}
