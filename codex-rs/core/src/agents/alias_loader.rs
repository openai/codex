use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAliases {
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub shortcuts: HashMap<String, String>,
}

impl Default for AgentAliases {
    fn default() -> Self {
        Self {
            aliases: Self::default_aliases(),
            shortcuts: Self::default_shortcuts(),
        }
    }
}

impl AgentAliases {
    /// Load aliases from .codex/aliases.yaml
    pub fn load() -> Result<Self> {
        let aliases_path = Self::get_aliases_path()?;

        if !aliases_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&aliases_path)
            .context(format!("Failed to read {}", aliases_path.display()))?;

        let aliases: AgentAliases =
            serde_yaml::from_str(&content).context("Failed to parse aliases.yaml")?;

        Ok(aliases)
    }

    /// Resolve an alias or shortcut to the actual agent name
    pub fn resolve(&self, input: &str) -> String {
        // Strip @ prefix if present
        let input = input.strip_prefix('@').unwrap_or(input);

        // Check aliases first (e.g., @cr -> code-reviewer)
        if let Some(agent) = self.aliases.get(&format!("@{input}")) {
            return agent.clone();
        }

        // Then check shortcuts (e.g., review -> code-reviewer)
        if let Some(agent) = self.shortcuts.get(input) {
            return agent.clone();
        }

        // Return original if no match
        input.to_string()
    }

    /// Parse @mention from text (e.g., "@code-reviewer please review this")
    pub fn extract_mention(text: &str) -> Option<(&str, &str)> {
        if let Some(stripped) = text.strip_prefix('@') {
            if let Some(space_idx) = stripped.find(char::is_whitespace) {
                let agent = &stripped[..space_idx];
                let rest = stripped[space_idx..].trim_start();
                return Some((agent, rest));
            } else {
                // No space, entire text is the agent name
                return Some((stripped, ""));
            }
        }
        None
    }

    /// Check if text starts with @mention
    pub fn has_mention(text: &str) -> bool {
        text.trim_start().starts_with('@')
    }

    fn get_aliases_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".codex").join("aliases.yaml"))
    }

    fn default_aliases() -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("@cr".to_string(), "code-reviewer".to_string());
        map.insert("@ts".to_string(), "ts-reviewer".to_string());
        map.insert("@py".to_string(), "python-reviewer".to_string());
        map.insert("@unity".to_string(), "unity-reviewer".to_string());
        map.insert("@sec".to_string(), "sec-audit".to_string());
        map.insert("@tg".to_string(), "test-gen".to_string());
        map.insert("@res".to_string(), "researcher".to_string());
        map.insert("@mcp".to_string(), "codex-mcp-researcher".to_string());
        map
    }

    fn default_shortcuts() -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("review".to_string(), "code-reviewer".to_string());
        map.insert("audit".to_string(), "sec-audit".to_string());
        map.insert("test".to_string(), "test-gen".to_string());
        map.insert("ask".to_string(), "researcher".to_string());
        map.insert("research".to_string(), "researcher".to_string());
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_alias() {
        let aliases = AgentAliases::default();
        assert_eq!(aliases.resolve("@cr"), "code-reviewer");
        assert_eq!(aliases.resolve("cr"), "code-reviewer");
        assert_eq!(aliases.resolve("@sec"), "sec-audit");
    }

    #[test]
    fn test_resolve_shortcut() {
        let aliases = AgentAliases::default();
        assert_eq!(aliases.resolve("review"), "code-reviewer");
        assert_eq!(aliases.resolve("audit"), "sec-audit");
        assert_eq!(aliases.resolve("ask"), "researcher");
    }

    #[test]
    fn test_extract_mention() {
        let (agent, rest) =
            AgentAliases::extract_mention("@code-reviewer please review this").unwrap();
        assert_eq!(agent, "code-reviewer");
        assert_eq!(rest, "please review this");

        let (agent, rest) = AgentAliases::extract_mention("@cr fix bugs").unwrap();
        assert_eq!(agent, "cr");
        assert_eq!(rest, "fix bugs");
    }

    #[test]
    fn test_has_mention() {
        assert!(AgentAliases::has_mention("@code-reviewer test"));
        assert!(AgentAliases::has_mention("  @cr hello"));
        assert!(!AgentAliases::has_mention("no mention here"));
    }
}
