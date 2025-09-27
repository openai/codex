use codex_common::CliConfigOverrides;
use codex_common::create_config_summary_entries;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;

const EXIT_CODE_INVALID_CONFIG: i32 = 3;

pub(crate) fn validate_config(cli_overrides: CliConfigOverrides, should_print: bool) {
    let cli_overrides = match cli_overrides.parse_overrides() {
        Ok(overrides) => overrides,
        Err(err) => {
            eprintln!("Error parsing -c overrides: {err}");
            std::process::exit(EXIT_CODE_INVALID_CONFIG);
        }
    };

    match Config::load_with_cli_overrides(cli_overrides, ConfigOverrides::default()) {
        Ok(config) => {
            if should_print {
                println!("Current default config settings:");
                println!("--------------------------------");
                let entries = create_config_summary_entries(&config);

                for (key, value) in entries {
                    println!("{key}: {value}");
                }
                println!("--------------------------------");
                println!("* Note that various commands may override specific settings. *");
            }
        }
        Err(err) => {
            if should_print {
                eprintln!("Config validation error: {err}");
            }
            std::process::exit(EXIT_CODE_INVALID_CONFIG);
        }
    }
}
