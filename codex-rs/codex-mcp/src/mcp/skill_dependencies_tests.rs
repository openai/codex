use super::*;
use codex_protocol::protocol::SkillDependencies;
use codex_protocol::protocol::SkillMetadata;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::test_support::PathBufExt as _;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

fn skill_with_tools(tools: Vec<SkillToolDependency>) -> SkillMetadata {
    SkillMetadata {
        name: "skill".to_string(),
        description: "skill".to_string(),
        short_description: None,
        interface: None,
        dependencies: Some(SkillDependencies { tools }),
        path: PathBuf::from("/tmp/skill").abs(),
        scope: SkillScope::User,
        enabled: true,
    }
}

#[test]
fn collect_missing_respects_canonical_installed_key() {
    let url = "https://example.com/mcp".to_string();
    let skills = vec![skill_with_tools(vec![SkillToolDependency {
        r#type: "mcp".to_string(),
        value: "github".to_string(),
        description: None,
        transport: Some("streamable_http".to_string()),
        command: None,
        url: Some(url.clone()),
    }])];
    let installed = HashMap::from([(
        "alias".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    )]);

    assert_eq!(
        collect_missing_mcp_dependencies(&skills, &installed),
        HashMap::new()
    );
}

#[test]
fn collect_missing_dedupes_by_canonical_key_but_preserves_original_name() {
    let url = "https://example.com/one".to_string();
    let skills = vec![skill_with_tools(vec![
        SkillToolDependency {
            r#type: "mcp".to_string(),
            value: "alias-one".to_string(),
            description: None,
            transport: Some("streamable_http".to_string()),
            command: None,
            url: Some(url.clone()),
        },
        SkillToolDependency {
            r#type: "mcp".to_string(),
            value: "alias-two".to_string(),
            description: None,
            transport: Some("streamable_http".to_string()),
            command: None,
            url: Some(url.clone()),
        },
    ])];

    let expected = HashMap::from([(
        "alias-one".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var: None,
                http_headers: None,
                env_http_headers: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    )]);

    assert_eq!(
        collect_missing_mcp_dependencies(&skills, &HashMap::new()),
        expected
    );
}
