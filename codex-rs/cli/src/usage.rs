use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_login::AuthMode;
use codex_login::CodexAuth;

use codex_chatgpt::usage::get_usage as get_chatgpt_usage;

#[derive(Debug, clap::Parser)]
pub struct UsageCommand {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,
}

pub async fn run_usage_command(cli_config_overrides: CliConfigOverrides) -> anyhow::Result<()> {
    let config = load_config_or_exit(cli_config_overrides);

    match CodexAuth::from_codex_home(&config.codex_home, config.preferred_auth_method) {
        Ok(Some(auth)) => match auth.mode {
            AuthMode::ApiKey => {
                let plan = auth
                    .get_plan_type()
                    .unwrap_or_else(|| "unknown".to_string());
                println!("Plan: {plan}");
                println!(
                    "Using an API key. Guardrail usage does not apply; billing is per-token.\nSee https://platform.openai.com/account/usage for detailed usage."
                );
                Ok(())
            }
            AuthMode::ChatGPT => {
                let plan = auth
                    .get_plan_type()
                    .unwrap_or_else(|| "unknown".to_string());
                match get_chatgpt_usage(&config).await {
                    Ok(summary) => {
                        println!("Plan: {plan}");
                        if let Some(when) = summary.next_reset_at.as_deref() {
                            println!("Next reset: {when}");
                        }
                        if let (Some(u), Some(l)) = (
                            summary.standard_used_minutes,
                            summary.standard_limit_minutes,
                        ) {
                            println!("Standard: {u} / {l} minutes used");
                        }
                        if let (Some(u), Some(l)) = (
                            summary.reasoning_used_minutes,
                            summary.reasoning_limit_minutes,
                        ) {
                            println!("Reasoning: {u} / {l} minutes used");
                        }

                        // If no buckets printed, fall back to a generic message.
                        if summary.standard_used_minutes.is_none()
                            && summary.reasoning_used_minutes.is_none()
                        {
                            println!("Usage data retrieved, but no bucket details available.");
                        }
                        Ok(())
                    }
                    Err(e) => {
                        println!(
                            "Plan: {plan}\nUnable to retrieve usage from ChatGPT backend.\nReason: {e}\nUsage information is currently unavailable."
                        );
                        Ok(())
                    }
                }
            }
        },
        Ok(None) => {
            println!("Not logged in. Usage information requires authentication.\nRun: codex login");
            Ok(())
        }
        Err(e) => {
            println!(
                "Unable to determine authentication status.\nReason: {e}\nUsage information is currently unavailable."
            );
            Ok(())
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
