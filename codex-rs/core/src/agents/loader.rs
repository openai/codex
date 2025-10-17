use super::types::AgentDefinition;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// エージェント定義ローダー
pub struct AgentLoader {
    /// エージェント定義ディレクトリ
    agents_dir: PathBuf,
    /// キャッシュされたエージェント定義
    cache: HashMap<String, AgentDefinition>,
}

impl AgentLoader {
    /// 新しいローダーを作成
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let agents_dir = base_dir.as_ref().join(".codex/agents");
        Self {
            agents_dir,
            cache: HashMap::new(),
        }
    }

    /// すべてのエージェント定義を読み込む
    pub fn load_all(&mut self) -> Result<Vec<AgentDefinition>> {
        if !self.agents_dir.exists() {
            warn!("Agents directory not found: {}", self.agents_dir.display());
            return Ok(vec![]);
        }

        let mut agents = Vec::new();

        for entry in std::fs::read_dir(&self.agents_dir).with_context(|| {
            format!(
                "Failed to read agents directory: {}",
                self.agents_dir.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                match self.load_agent(&path) {
                    Ok(agent) => {
                        info!("Loaded agent: {}", agent.name);
                        agents.push(agent);
                    }
                    Err(e) => {
                        warn!("Failed to load agent from {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(agents)
    }

    /// 特定のエージェント定義を読み込む
    pub fn load_by_name(&mut self, name: &str) -> Result<AgentDefinition> {
        // キャッシュをチェック
        if let Some(agent) = self.cache.get(name) {
            debug!("Loading agent '{}' from cache", name);
            return Ok(agent.clone());
        }

        // ファイルから読み込み
        let yaml_path = self.agents_dir.join(format!("{name}.yaml"));
        let yml_path = self.agents_dir.join(format!("{name}.yml"));

        let path = if yaml_path.exists() {
            yaml_path
        } else if yml_path.exists() {
            yml_path
        } else {
            anyhow::bail!("Agent definition not found: {}", name);
        };

        let agent = self.load_agent(&path)?;

        // キャッシュに保存
        self.cache.insert(name.to_string(), agent.clone());

        Ok(agent)
    }

    /// ファイルからエージェント定義を読み込む
    fn load_agent(&self, path: &Path) -> Result<AgentDefinition> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read agent file: {}", path.display()))?;

        let agent: AgentDefinition = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse agent YAML: {}", path.display()))?;

        debug!("Loaded agent definition: {:?}", agent);

        Ok(agent)
    }

    /// キャッシュをクリア
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// 利用可能なエージェント名のリストを取得
    pub fn list_available_agents(&self) -> Result<Vec<String>> {
        if !self.agents_dir.exists() {
            return Ok(vec![]);
        }

        let mut names = Vec::new();

        for entry in std::fs::read_dir(&self.agents_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "yaml" || ext == "yml" {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        names.push(name.to_string());
                    }
                }
            }
        }

        names.sort();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_agent() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".codex/agents");
        fs::create_dir_all(&agents_dir).unwrap();

        let agent_yaml = r#"
name: "Test Agent"
goal: "Test goal"
tools:
  mcp:
    - search
  fs:
    read: true
    write: false
  net:
    allow: []
  shell: []
policies:
  context:
    max_tokens: 16000
    retention: "job"
  secrets:
    redact: false
success_criteria:
  - "基準1"
artifacts:
  - "artifacts/output.md"
"#;

        fs::write(agents_dir.join("test-agent.yaml"), agent_yaml).unwrap();

        let mut loader = AgentLoader::new(temp_dir.path());
        let agent = loader.load_by_name("test-agent").unwrap();

        assert_eq!(agent.name, "Test Agent");
        assert_eq!(agent.goal, "Test goal");
    }

    #[test]
    fn test_list_available_agents() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join(".codex/agents");
        fs::create_dir_all(&agents_dir).unwrap();

        fs::write(agents_dir.join("agent1.yaml"), "name: Agent1\ngoal: Goal1\ntools: {}\npolicies: {context: {}}\nsuccess_criteria: []\nartifacts: []").unwrap();
        fs::write(agents_dir.join("agent2.yaml"), "name: Agent2\ngoal: Goal2\ntools: {}\npolicies: {context: {}}\nsuccess_criteria: []\nartifacts: []").unwrap();

        let loader = AgentLoader::new(temp_dir.path());
        let agents = loader.list_available_agents().unwrap();

        assert_eq!(agents, vec!["agent1", "agent2"]);
    }
}
