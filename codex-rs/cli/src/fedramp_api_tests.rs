use super::CodexFlavor;
use super::FEDRAMP_API_DEFAULTS_TOML;
use super::blessed_config_overrides;
use super::reject_unsafe_cli_args;
use codex_utils_cli::CliConfigOverrides;
use pretty_assertions::assert_eq;

#[test]
fn blessed_defaults_are_valid_toml() {
    let parsed: toml::Value =
        toml::from_str(FEDRAMP_API_DEFAULTS_TOML).expect("blessed defaults should parse");
    let table = parsed
        .as_table()
        .expect("blessed defaults should be a TOML table");
    assert_eq!(
        table.get("openai_base_url"),
        Some(&toml::Value::String(
            "https://gov.api.openai.com/v1".to_string()
        ))
    );
    assert_eq!(
        table.get("forced_login_method"),
        Some(&toml::Value::String("api".to_string()))
    );
    assert_eq!(
        table.get("model_provider"),
        Some(&toml::Value::String("openai".to_string()))
    );
    assert_eq!(
        table.get("mcp_servers"),
        Some(&toml::Value::Table(toml::Table::new()))
    );
    assert_eq!(
        table.get("model_providers"),
        Some(&toml::Value::Table(toml::Table::new()))
    );
    assert_eq!(
        table.get("plugins"),
        Some(&toml::Value::Table(toml::Table::new()))
    );

    let features = table
        .get("features")
        .and_then(toml::Value::as_table)
        .expect("features should be a TOML table");
    for key in [
        "apps",
        "enable_mcp_apps",
        "plugins",
        "remote_plugin",
        "tool_suggest",
        "memories",
        "memory_tool",
        "multi_agent",
        "multi_agent_mode",
        "multi_agent_v2",
        "remote_control",
        "plugin_hooks",
        "plugin_sharing",
        "skill_mcp_dependency_install",
        "mentions_v2",
    ] {
        assert_eq!(
            features.get(key),
            Some(&toml::Value::Boolean(false)),
            "{key} should be disabled"
        );
    }

    let memories = table
        .get("memories")
        .and_then(toml::Value::as_table)
        .expect("memories should be a TOML table");
    for key in ["generate_memories", "use_memories", "dedicated_tools"] {
        assert_eq!(
            memories.get(key),
            Some(&toml::Value::Boolean(false)),
            "{key} should be disabled"
        );
    }

    let analytics = table
        .get("analytics")
        .and_then(toml::Value::as_table)
        .expect("analytics should be a TOML table");
    assert_eq!(analytics.get("enabled"), Some(&toml::Value::Boolean(false)));

    let otel = table
        .get("otel")
        .and_then(toml::Value::as_table)
        .expect("otel should be a TOML table");
    for key in ["exporter", "trace_exporter", "metrics_exporter"] {
        assert_eq!(
            otel.get(key),
            Some(&toml::Value::String("none".to_string())),
            "{key} should be disabled"
        );
    }
    assert_eq!(
        otel.get("log_user_prompt"),
        Some(&toml::Value::Boolean(false))
    );
}

#[test]
fn blessed_defaults_override_user_values() {
    let mut config = toml::Value::Table(toml::Table::from_iter([
        (
            "openai_base_url".to_string(),
            toml::Value::String("https://api.openai.com/v1".to_string()),
        ),
        (
            "model_provider".to_string(),
            toml::Value::String("ollama".to_string()),
        ),
        (
            "mcp_servers".to_string(),
            toml::Value::Table(toml::Table::from_iter([(
                "attacker".to_string(),
                toml::Value::Table(toml::Table::from_iter([(
                    "command".to_string(),
                    toml::Value::String("evil".to_string()),
                )])),
            )])),
        ),
        (
            "skills".to_string(),
            toml::Value::Table(toml::Table::from_iter([(
                "config".to_string(),
                toml::Value::Array(vec![toml::Value::Table(toml::Table::from_iter([
                    (
                        "name".to_string(),
                        toml::Value::String("attacker".to_string()),
                    ),
                    ("enabled".to_string(), toml::Value::Boolean(true)),
                ]))]),
            )])),
        ),
    ]));
    let overrides = CliConfigOverrides {
        raw_overrides: blessed_config_overrides().expect("blessed defaults should flatten"),
    };
    overrides
        .apply_on_value(&mut config)
        .expect("blessed defaults should apply");

    assert_eq!(
        config.get("openai_base_url"),
        Some(&toml::Value::String(
            "https://gov.api.openai.com/v1".to_string()
        ))
    );
    assert_eq!(
        config.get("model_provider"),
        Some(&toml::Value::String("openai".to_string()))
    );
    assert_eq!(
        config.get("mcp_servers"),
        Some(&toml::Value::Table(toml::Table::new()))
    );
    assert_eq!(
        config
            .get("skills")
            .and_then(toml::Value::as_table)
            .and_then(|skills| skills.get("config")),
        Some(&toml::Value::Array(Vec::new()))
    );
}

#[test]
fn rejects_unsafe_config_and_provider_flags() {
    for args in [
        vec!["-c", "openai_base_url=\"https://api.openai.com/v1\""],
        vec!["--config=openai_base_url=\"https://api.openai.com/v1\""],
        vec!["--enable=plugins"],
        vec!["--profile", "custom"],
        vec!["--oss"],
        vec!["--local-provider=ollama"],
        vec!["--remote=wss://example.com"],
    ] {
        let args = args.into_iter().map(Into::into);
        assert!(reject_unsafe_cli_args(args).is_err());
    }
}

#[test]
fn allows_prompt_after_argument_separator() {
    let args = vec!["--", "--enable", "plugins"]
        .into_iter()
        .map(Into::into);
    assert!(reject_unsafe_cli_args(args).is_ok());
}

#[test]
fn compiled_binary_name_selects_flavor() {
    assert_eq!(
        CodexFlavor::from_compiled_binary_name(Some("codex-fedramp-api")),
        CodexFlavor::FedRAMPApi
    );
    assert_eq!(
        CodexFlavor::from_compiled_binary_name(Some("codex")),
        CodexFlavor::Normal
    );
}
