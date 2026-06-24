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
    let requirement = McpServerRequirement::Command(McpServerCommandMatcher {
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

    assert!(requirement.matches(&stdio_server(
        "company-cli",
        &["mcp", "https://pricing.example.com"]
    )));
    assert!(!requirement.matches(&stdio_server(
        "company-cli",
        &["https://pricing.example.com", "mcp"]
    )));
    assert!(!requirement.matches(&stdio_server(
        "company-cli",
        &["mcp", "https://pricing.example.com", "--verbose"]
    )));
    assert!(!requirement.matches(&stdio_server(
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
fn regex_matcher_validation_rejects_expression_that_cannot_be_wrapped() {
    let matcher = McpServerValueMatcher::Regex {
        expression: "(?x)mcp # trailing comment".to_string(),
    };

    let err = matcher
        .validate()
        .expect_err("expression should not be valid for full-value matching");
    assert!(
        err.contains("cannot be used for full-value matching"),
        "{err}"
    );
}

#[test]
fn legacy_command_identity_keeps_ignoring_arguments() {
    let requirement: McpServerRequirement = toml::from_str(
        r#"
[identity]
command = "company-cli"
"#,
    )
    .expect("legacy command identity");

    assert!(requirement.matches(&stdio_server(
        "company-cli",
        &["any", "arguments", "remain", "allowed"]
    )));
    assert!(!requirement.matches(&stdio_server("different-cli", &[])));
}

#[test]
fn requirement_deserializes_command_and_url_matcher_shapes() {
    let command: McpServerRequirement = toml::from_str(
        r#"
command = "company-cli"
args = [
    { match = "exact", value = "mcp" },
    { match = "regex", expression = '^https://[a-z]+\.example\.com$' },
]
"#,
    )
    .expect("command matcher");
    let url: McpServerRequirement = toml::from_str(
        r#"
url = { match = "prefix", value = "https://mcp.example.com/" }
"#,
    )
    .expect("URL matcher");

    assert_eq!(
        command,
        McpServerRequirement::Command(McpServerCommandMatcher {
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
        McpServerRequirement::Url(McpServerUrlMatcher {
            url: McpServerValueMatcher::Prefix {
                value: "https://mcp.example.com/".to_string(),
            },
        })
    );
}

#[test]
fn requirement_rejects_identity_combined_with_matcher_keys() {
    for contents in [
        r#"
command = "company-cli"
[identity]
command = "company-cli"
"#,
        r#"
args = []
[identity]
command = "company-cli"
"#,
        r#"
url = { match = "prefix", value = "https://mcp.example.com/" }
[identity]
url = "https://mcp.example.com"
"#,
    ] {
        let err = toml::from_str::<McpServerRequirement>(contents)
            .expect_err("identity and matcher keys should be mutually exclusive");
        assert!(
            err.to_string().contains(
                "`identity` cannot be combined with matcher keys `command`, `args`, or `url`"
            ),
            "{err}"
        );
    }
}

#[test]
fn identity_requirement_keeps_ignoring_unrelated_sibling_fields() {
    let requirement: McpServerRequirement = toml::from_str(
        r#"
unrelated = "ignored"
[identity]
command = "company-cli"
"#,
    )
    .expect("legacy identity with unrelated sibling field");

    assert_eq!(
        requirement,
        McpServerRequirement::Identity {
            identity: McpServerIdentity::Command {
                command: "company-cli".to_string(),
            },
        }
    );
}
