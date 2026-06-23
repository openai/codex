use super::*;
use crate::mcp_types::McpServerConfig;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
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

fn stdio_mcp(command: &str) -> McpServerConfig {
    stdio_server(command, &[])
}

fn stdio_mcp_with_args(command: &str, args: &[&str]) -> McpServerConfig {
    stdio_server(command, args)
}

fn http_mcp(url: &str) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
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

fn filter_mcp_servers_by_requirements(
    mcp_servers: &mut HashMap<String, McpServerConfig>,
    exact_requirements: Option<&Sourced<BTreeMap<String, McpServerRequirement>>>,
    matchers: Option<&Sourced<BTreeMap<String, McpServerMatcher>>>,
) {
    let policy = McpServerPolicy::new(exact_requirements.cloned(), matchers.cloned());
    let constrained = policy
        .constrain(std::mem::take(mcp_servers))
        .expect("MCP policy should produce a valid constraint");
    *mcp_servers = constrained.get().clone();
}

fn filter_plugin_mcp_servers_by_requirements(
    plugin_config_name: &str,
    mcp_servers: &mut HashMap<String, McpServerConfig>,
    plugin_requirements: Option<&Sourced<BTreeMap<String, PluginRequirementsToml>>>,
    matchers: Option<&Sourced<BTreeMap<String, McpServerMatcher>>>,
) {
    ConfigRequirements {
        plugins: plugin_requirements.cloned(),
        mcp_server_matchers: matchers.cloned(),
        ..Default::default()
    }
    .apply_to_plugin_mcp_servers(plugin_config_name, mcp_servers);
}

#[test]
fn filter_mcp_servers_by_allowlist_enforces_identity_rules() {
    const MISMATCHED_COMMAND_SERVER: &str = "mismatched-command-should-disable";
    const MISMATCHED_URL_SERVER: &str = "mismatched-url-should-disable";
    const MATCHED_COMMAND_SERVER: &str = "matched-command-should-allow";
    const MATCHED_URL_SERVER: &str = "matched-url-should-allow";
    const DIFFERENT_NAME_SERVER: &str = "different-name-should-disable";

    const GOOD_CMD: &str = "good-cmd";
    const GOOD_URL: &str = "https://example.com/good";

    let mut servers = HashMap::from([
        (MISMATCHED_COMMAND_SERVER.to_string(), stdio_mcp("docs-cmd")),
        (
            MISMATCHED_URL_SERVER.to_string(),
            http_mcp("https://example.com/mcp"),
        ),
        (MATCHED_COMMAND_SERVER.to_string(), stdio_mcp(GOOD_CMD)),
        (MATCHED_URL_SERVER.to_string(), http_mcp(GOOD_URL)),
        (DIFFERENT_NAME_SERVER.to_string(), stdio_mcp("same-cmd")),
    ]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(
        BTreeMap::from([
            (
                MISMATCHED_URL_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Url {
                        url: "https://example.com/other".to_string(),
                    },
                },
            ),
            (
                MISMATCHED_COMMAND_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "other-cmd".to_string(),
                    },
                },
            ),
            (
                MATCHED_URL_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Url {
                        url: GOOD_URL.to_string(),
                    },
                },
            ),
            (
                MATCHED_COMMAND_SERVER.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: GOOD_CMD.to_string(),
                    },
                },
            ),
        ]),
        source.clone(),
    );
    filter_mcp_servers_by_requirements(
        &mut servers,
        Some(&requirements),
        /*mcp_matchers*/ None,
    );

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            (MISMATCHED_URL_SERVER.to_string(), (false, reason.clone())),
            (
                MISMATCHED_COMMAND_SERVER.to_string(),
                (false, reason.clone()),
            ),
            (MATCHED_URL_SERVER.to_string(), (true, None)),
            (MATCHED_COMMAND_SERVER.to_string(), (true, None)),
            (DIFFERENT_NAME_SERVER.to_string(), (false, reason)),
        ])
    );
}

#[test]
fn filter_mcp_servers_by_allowlist_allows_all_when_unset() {
    let mut servers = HashMap::from([
        ("server-a".to_string(), stdio_mcp("cmd-a")),
        ("server-b".to_string(), http_mcp("https://example.com/b")),
    ]);

    filter_mcp_servers_by_requirements(
        &mut servers,
        /*mcp_requirements*/ None,
        /*mcp_matchers*/ None,
    );

    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            ("server-a".to_string(), (true, None)),
            ("server-b".to_string(), (true, None)),
        ])
    );
}

#[test]
fn filter_mcp_servers_by_matchers_enforces_command_and_positional_args() {
    let mut servers = HashMap::from([
        (
            "internal_mcp_proxy".to_string(),
            stdio_mcp_with_args(
                "company-cli",
                &[
                    "mcp",
                    "proxy",
                    "--server",
                    "https://pricing.mcp.internal.example.com",
                ],
            ),
        ),
        (
            "unlisted".to_string(),
            stdio_mcp_with_args(
                "company-cli",
                &[
                    "mcp",
                    "proxy",
                    "--server",
                    "https://pricing.mcp.internal.example.com",
                ],
            ),
        ),
        (
            "wrong-order".to_string(),
            stdio_mcp_with_args(
                "company-cli",
                &[
                    "proxy",
                    "mcp",
                    "--server",
                    "https://pricing.mcp.internal.example.com",
                ],
            ),
        ),
        (
            "trailing-arg".to_string(),
            stdio_mcp_with_args(
                "company-cli",
                &[
                    "mcp",
                    "proxy",
                    "--server",
                    "https://pricing.mcp.internal.example.com",
                    "--verbose",
                ],
            ),
        ),
        (
            "wrong-host".to_string(),
            stdio_mcp_with_args(
                "company-cli",
                &["mcp", "proxy", "--server", "https://mcp.example.com"],
            ),
        ),
    ]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let matcher = McpServerMatcher::Command(McpServerCommandMatcher {
        command: "company-cli".to_string(),
        args: vec![
            McpServerValueMatcher::Exact {
                value: "mcp".to_string(),
            },
            McpServerValueMatcher::Exact {
                value: "proxy".to_string(),
            },
            McpServerValueMatcher::Exact {
                value: "--server".to_string(),
            },
            McpServerValueMatcher::Regex {
                expression:
                    r"^https://[A-Za-z0-9-]+\.mcp\.internal\.example\.com(?::443)?(?:/.*)?$"
                        .to_string(),
            },
        ],
    });
    let matchers = Sourced::new(
        BTreeMap::from([
            ("internal_mcp_proxy".to_string(), matcher.clone()),
            ("wrong-order".to_string(), matcher.clone()),
            ("trailing-arg".to_string(), matcher.clone()),
            ("wrong-host".to_string(), matcher),
        ]),
        source.clone(),
    );

    filter_mcp_servers_by_requirements(
        &mut servers,
        /*mcp_requirements*/ None,
        Some(&matchers),
    );

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            ("internal_mcp_proxy".to_string(), (true, None)),
            ("unlisted".to_string(), (false, reason.clone())),
            ("wrong-order".to_string(), (false, reason.clone())),
            ("trailing-arg".to_string(), (false, reason.clone())),
            ("wrong-host".to_string(), (false, reason)),
        ])
    );
}

#[test]
fn filter_mcp_servers_by_requirements_enforces_all_same_name_rules() {
    const BOTH_MATCH: &str = "both-match";
    const EXACT_ONLY_MATCHES: &str = "exact-only-matches";
    const MATCHER_ONLY_MATCHES: &str = "matcher-only-matches";

    let mut servers = HashMap::from([
        (
            BOTH_MATCH.to_string(),
            stdio_mcp_with_args("company-cli", &["approved"]),
        ),
        (
            EXACT_ONLY_MATCHES.to_string(),
            stdio_mcp_with_args("company-cli", &["rejected"]),
        ),
        (
            MATCHER_ONLY_MATCHES.to_string(),
            stdio_mcp_with_args("company-cli", &["approved"]),
        ),
    ]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(
        BTreeMap::from([
            (
                BOTH_MATCH.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "company-cli".to_string(),
                    },
                },
            ),
            (
                EXACT_ONLY_MATCHES.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "company-cli".to_string(),
                    },
                },
            ),
            (
                MATCHER_ONLY_MATCHES.to_string(),
                McpServerRequirement {
                    identity: McpServerIdentity::Command {
                        command: "different-command".to_string(),
                    },
                },
            ),
        ]),
        source.clone(),
    );
    let matcher = McpServerMatcher::Command(McpServerCommandMatcher {
        command: "company-cli".to_string(),
        args: vec![McpServerValueMatcher::Exact {
            value: "approved".to_string(),
        }],
    });
    let matchers = Sourced::new(
        BTreeMap::from([
            (BOTH_MATCH.to_string(), matcher.clone()),
            (EXACT_ONLY_MATCHES.to_string(), matcher.clone()),
            (MATCHER_ONLY_MATCHES.to_string(), matcher),
        ]),
        source.clone(),
    );

    filter_mcp_servers_by_requirements(&mut servers, Some(&requirements), Some(&matchers));

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            (BOTH_MATCH.to_string(), (true, None)),
            (EXACT_ONLY_MATCHES.to_string(), (false, reason.clone())),
            (MATCHER_ONLY_MATCHES.to_string(), (false, reason)),
        ])
    );
}

#[test]
fn filter_mcp_servers_by_allowlist_blocks_all_when_empty() {
    let mut servers = HashMap::from([
        ("server-a".to_string(), stdio_mcp("cmd-a")),
        ("server-b".to_string(), http_mcp("https://example.com/b")),
    ]);

    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(BTreeMap::new(), source.clone());
    filter_mcp_servers_by_requirements(
        &mut servers,
        Some(&requirements),
        /*mcp_matchers*/ None,
    );

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            ("server-a".to_string(), (false, reason.clone())),
            ("server-b".to_string(), (false, reason)),
        ])
    );
}

#[test]
fn filter_plugin_mcp_servers_by_allowlist_enforces_plugin_and_matcher_rules() {
    const MATCHED_SERVER: &str = "matched-should-allow";
    const MISMATCHED_SERVER: &str = "mismatched-should-disable";
    const MATCHER_MISMATCHED_SERVER: &str = "matcher-mismatched-should-disable";
    const UNLISTED_SERVER: &str = "unlisted-should-disable";
    const GOOD_CMD: &str = "good-cmd";

    let mut servers = HashMap::from([
        (MATCHED_SERVER.to_string(), stdio_mcp(GOOD_CMD)),
        (MISMATCHED_SERVER.to_string(), stdio_mcp("bad-cmd")),
        (
            MATCHER_MISMATCHED_SERVER.to_string(),
            stdio_mcp_with_args(GOOD_CMD, &["unexpected"]),
        ),
        (
            UNLISTED_SERVER.to_string(),
            http_mcp("https://example.com/mcp"),
        ),
    ]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(
        BTreeMap::from([(
            "sample@test".to_string(),
            PluginRequirementsToml {
                mcp_servers: Some(BTreeMap::from([
                    (
                        MATCHED_SERVER.to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Command {
                                command: GOOD_CMD.to_string(),
                            },
                        },
                    ),
                    (
                        MISMATCHED_SERVER.to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Command {
                                command: GOOD_CMD.to_string(),
                            },
                        },
                    ),
                    (
                        MATCHER_MISMATCHED_SERVER.to_string(),
                        McpServerRequirement {
                            identity: McpServerIdentity::Command {
                                command: GOOD_CMD.to_string(),
                            },
                        },
                    ),
                ])),
            },
        )]),
        source.clone(),
    );
    let matchers = Sourced::new(
        BTreeMap::from([
            (
                MATCHED_SERVER.to_string(),
                McpServerMatcher::Command(McpServerCommandMatcher {
                    command: GOOD_CMD.to_string(),
                    args: Vec::new(),
                }),
            ),
            (
                MISMATCHED_SERVER.to_string(),
                McpServerMatcher::Command(McpServerCommandMatcher {
                    command: "bad-cmd".to_string(),
                    args: Vec::new(),
                }),
            ),
            (
                MATCHER_MISMATCHED_SERVER.to_string(),
                McpServerMatcher::Command(McpServerCommandMatcher {
                    command: GOOD_CMD.to_string(),
                    args: Vec::new(),
                }),
            ),
        ]),
        source.clone(),
    );

    filter_plugin_mcp_servers_by_requirements(
        "sample@test",
        &mut servers,
        Some(&requirements),
        Some(&matchers),
    );

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            (MATCHED_SERVER.to_string(), (true, None)),
            (MISMATCHED_SERVER.to_string(), (false, reason.clone())),
            (
                MATCHER_MISMATCHED_SERVER.to_string(),
                (false, reason.clone()),
            ),
            (UNLISTED_SERVER.to_string(), (false, reason)),
        ])
    );
}

#[test]
fn filter_plugin_mcp_servers_by_allowlist_blocks_unlisted_plugin() {
    let mut servers = HashMap::from([("server-a".to_string(), stdio_mcp("cmd-a"))]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let requirements = Sourced::new(
        BTreeMap::from([(
            "other@test".to_string(),
            PluginRequirementsToml {
                mcp_servers: Some(BTreeMap::from([(
                    "server-a".to_string(),
                    McpServerRequirement {
                        identity: McpServerIdentity::Command {
                            command: "cmd-a".to_string(),
                        },
                    },
                )])),
            },
        )]),
        source.clone(),
    );

    filter_plugin_mcp_servers_by_requirements(
        "sample@test",
        &mut servers,
        Some(&requirements),
        /*mcp_matchers*/ None,
    );

    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([(
            "server-a".to_string(),
            (
                false,
                Some(McpServerDisabledReason::Requirements { source })
            )
        )])
    );
}

#[test]
fn filter_plugin_mcp_servers_by_matchers_enforces_name_and_invocation() {
    const MATCHED_SERVER: &str = "matched";
    const MISMATCHED_SERVER: &str = "mismatched";
    const UNLISTED_SERVER: &str = "unlisted";

    let mut servers = HashMap::from([
        (
            MATCHED_SERVER.to_string(),
            stdio_mcp_with_args("company-cli", &["approved"]),
        ),
        (
            MISMATCHED_SERVER.to_string(),
            stdio_mcp_with_args("company-cli", &["rejected"]),
        ),
        (
            UNLISTED_SERVER.to_string(),
            stdio_mcp_with_args("company-cli", &["approved"]),
        ),
    ]);
    let source = RequirementSource::LegacyManagedConfigTomlFromMdm;
    let matcher = McpServerMatcher::Command(McpServerCommandMatcher {
        command: "company-cli".to_string(),
        args: vec![McpServerValueMatcher::Exact {
            value: "approved".to_string(),
        }],
    });
    let matchers = Sourced::new(
        BTreeMap::from([
            (MATCHED_SERVER.to_string(), matcher.clone()),
            (MISMATCHED_SERVER.to_string(), matcher),
        ]),
        source.clone(),
    );

    filter_plugin_mcp_servers_by_requirements(
        "sample@test",
        &mut servers,
        /*plugin_requirements*/ None,
        Some(&matchers),
    );

    let reason = Some(McpServerDisabledReason::Requirements { source });
    assert_eq!(
        servers
            .iter()
            .map(|(name, server)| (
                name.clone(),
                (server.enabled, server.disabled_reason.clone())
            ))
            .collect::<HashMap<String, (bool, Option<McpServerDisabledReason>)>>(),
        HashMap::from([
            (MATCHED_SERVER.to_string(), (true, None)),
            (MISMATCHED_SERVER.to_string(), (false, reason.clone())),
            (UNLISTED_SERVER.to_string(), (false, reason)),
        ])
    );
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
