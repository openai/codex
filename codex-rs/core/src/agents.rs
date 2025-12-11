use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

#[derive(Debug, Clone)]
pub(crate) struct AgentDefinition {
    pub(crate) name: String,
    pub(crate) prompt: Option<String>,
    pub(crate) sub_agents: Vec<String>,
    pub(crate) read_only: bool,
    pub(crate) model: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentsConfig {
    agents: HashMap<String, AgentDefinition>,
}

#[derive(Debug, Deserialize)]
struct RawAgentDefinition {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    sub_agents: Vec<String>,
    #[serde(default)]
    read_only: bool,
    #[serde(default)]
    model: Option<String>,
}

impl AgentsConfig {
    pub(crate) const FILE_NAME: &'static str = "agents.toml";

    pub(crate) async fn try_load(codex_home: &Path) -> Option<Self> {
        let path = codex_home.join(Self::FILE_NAME);
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
            Err(err) => {
                warn!("failed to read {}: {err}", path.display());
                return None;
            }
        };

        match Self::from_toml_str(&content) {
            Ok(config) => Some(config),
            Err(err) => {
                warn!("failed to parse {}: {err}", path.display());
                None
            }
        }
    }

    fn from_toml_str(contents: &str) -> Result<Self, String> {
        let raw: HashMap<String, RawAgentDefinition> =
            toml::from_str(contents).map_err(|err| format!("invalid toml: {err}"))?;

        let mut agents = HashMap::new();
        for (name, agent) in raw {
            if let Some(model) = agent.model.as_deref()
                && model.trim().is_empty()
            {
                return Err(format!("agent {name}: model must be non-empty when set"));
            }

            let prompt = agent.prompt.and_then(|prompt| {
                if prompt.trim().is_empty() {
                    None
                } else {
                    Some(prompt)
                }
            });
            agents.insert(
                name.clone(),
                AgentDefinition {
                    name,
                    prompt,
                    sub_agents: agent.sub_agents,
                    read_only: agent.read_only,
                    model: agent.model,
                },
            );
        }

        if !agents.contains_key("main") {
            return Err("missing required agent: main".to_string());
        }

        for agent in agents.values() {
            for sub in &agent.sub_agents {
                if !agents.contains_key(sub) {
                    return Err(format!(
                        "agent {}: unknown sub_agent {sub}",
                        agent.name.as_str()
                    ));
                }
            }
        }

        Ok(Self { agents })
    }

    pub(crate) fn agent(&self, name: &str) -> Option<&AgentDefinition> {
        self.agents.get(name)
    }

    pub(crate) fn main(&self) -> &AgentDefinition {
        self.agents
            .get("main")
            .expect("agents config validated main agent")
    }
}
