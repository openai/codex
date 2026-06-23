use super::*;
use crate::mcp_types::McpServerConfig;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

fn stdio_server(command: &str, args: &[&str]) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command: command.to_string(),
            args: args.iter().map(ToString::to_string).collect(),
            env: None,
            env_vars: Vec::new(),
            cwd: None,
        },
        environment_id: crate::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

#[test]
fn command_matcher_matches_exact_positional_arguments() {
    let matcher = McpServerMatcher::Command(McpServerCommandMatcher {
        command: "company-cli".to_string(),
        args: vec![
            McpServerValueMatcher::Exact {
                value: "mcp".to_string(),
            },
            McpServerValueMatcher::Regex {
                expression: r"https://[a-z]+\.example\.com".to_string(),
            },
        ],
    });

    assert!(matcher.matches(&stdio_server(
        "company-cli",
        &["mcp", "https://pricing.example.com"]
    )));
    assert!(!matcher.matches(&stdio_server(
        "company-cli",
        &["https://pricing.example.com", "mcp"]
    )));
    assert!(!matcher.matches(&stdio_server(
        "company-cli",
        &["mcp", "https://pricing.example.com", "--verbose"]
    )));
    assert!(!matcher.matches(&stdio_server(
        "/usr/local/bin/company-cli",
        &["mcp", "https://pricing.example.com"]
    )));
}

#[test]
fn regex_matcher_requires_a_full_value_match() {
    let matcher = McpServerValueMatcher::Regex {
        expression: "mcp".to_string(),
    };

    assert!(matcher.matches("mcp"));
    assert!(!matcher.matches("mcp-proxy"));
    assert!(!matcher.matches("prefix-mcp"));
}

#[test]
fn regex_matcher_allows_a_later_alternative_to_match_the_full_value() {
    let matcher = McpServerValueMatcher::Regex {
        expression: r"https://api\.example\.com|https://api\.example\.com/mcp".to_string(),
    };

    assert!(matcher.matches("https://api.example.com/mcp"));
}

#[test]
fn matcher_deserializes_command_and_url_shapes() {
    let command: McpServerMatcher = toml::from_str(
        r#"
command = "company-cli"
args = [
    { match = "exact", value = "mcp" },
    { match = "regex", expression = '^https://[a-z]+\.example\.com$' },
]
"#,
    )
    .expect("command matcher");
    let url: McpServerMatcher = toml::from_str(
        r#"
url = { match = "prefix", value = "https://mcp.example.com/" }
"#,
    )
    .expect("URL matcher");

    assert_eq!(
        command,
        McpServerMatcher::Command(McpServerCommandMatcher {
            command: "company-cli".to_string(),
            args: vec![
                McpServerValueMatcher::Exact {
                    value: "mcp".to_string(),
                },
                McpServerValueMatcher::Regex {
                    expression: r"^https://[a-z]+\.example\.com$".to_string(),
                },
            ],
        })
    );
    assert_eq!(
        url,
        McpServerMatcher::Url(McpServerUrlMatcher {
            url: McpServerValueMatcher::Prefix {
                value: "https://mcp.example.com/".to_string(),
            },
        })
    );
}
