//! Parser for agent definitions (YAML and Markdown with YAML frontmatter).

use super::AgentDefinition;
use crate::subagent::SubagentErr;

/// Parse an agent definition from YAML string.
pub fn parse_agent_definition(content: &str) -> Result<AgentDefinition, SubagentErr> {
    // Check if content has YAML frontmatter (markdown format)
    if content.trim_start().starts_with("---") {
        parse_markdown_definition(content)
    } else {
        parse_yaml_definition(content)
    }
}

/// Parse pure YAML agent definition.
fn parse_yaml_definition(content: &str) -> Result<AgentDefinition, SubagentErr> {
    serde_yaml::from_str(content)
        .map_err(|e| SubagentErr::ParseError(format!("YAML parse error: {e}")))
}

/// Parse Markdown with YAML frontmatter.
fn parse_markdown_definition(content: &str) -> Result<AgentDefinition, SubagentErr> {
    let content = content.trim_start();

    // Find the frontmatter boundaries
    if !content.starts_with("---") {
        return Err(SubagentErr::ParseError(
            "Markdown agent definition must start with YAML frontmatter (---)".to_string(),
        ));
    }

    // Find the closing ---
    let after_first = &content[3..];
    let end_idx = after_first.find("\n---").ok_or_else(|| {
        SubagentErr::ParseError("Missing closing --- for YAML frontmatter".to_string())
    })?;

    let yaml_content = &after_first[..end_idx];

    // Parse the YAML frontmatter
    let mut definition: AgentDefinition = serde_yaml::from_str(yaml_content)
        .map_err(|e| SubagentErr::ParseError(format!("YAML frontmatter parse error: {e}")))?;

    // The markdown body after frontmatter can be used as system prompt if not set
    let body_start = 3 + end_idx + 4; // Skip first --- + yaml + \n---
    if body_start < content.len() {
        let body = content[body_start..].trim();
        if !body.is_empty() && definition.prompt_config.system_prompt.is_none() {
            definition.prompt_config.system_prompt = Some(body.to_string());
        }
    }

    Ok(definition)
}

/// Substitute template variables in a string.
/// Supports ${variable} syntax.
#[allow(dead_code)] // Pre-built infrastructure for variable templating
pub fn substitute_variables(
    template: &str,
    variables: &std::collections::HashMap<String, String>,
) -> String {
    let mut result = template.to_string();
    for (key, value) in variables {
        let pattern = format!("${{{key}}}");
        result = result.replace(&pattern, value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
agentType: test-agent
displayName: Test Agent
whenToUse: For testing
tools:
  - read_file
  - glob
source: user
"#;
        let def = parse_agent_definition(yaml).unwrap();
        assert_eq!(def.agent_type, "test-agent");
        assert_eq!(def.display_name, Some("Test Agent".to_string()));
    }

    #[test]
    fn test_parse_markdown() {
        let md = r#"---
agentType: md-agent
displayName: Markdown Agent
---

You are a specialized agent for testing.
"#;
        let def = parse_agent_definition(md).unwrap();
        assert_eq!(def.agent_type, "md-agent");
        assert!(
            def.prompt_config
                .system_prompt
                .unwrap()
                .contains("specialized agent")
        );
    }

    #[test]
    fn test_substitute_variables() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "World".to_string());
        vars.insert("cwd".to_string(), "/home/user".to_string());

        let result = substitute_variables("Hello ${name}! Working in ${cwd}", &vars);
        assert_eq!(result, "Hello World! Working in /home/user");
    }
}
