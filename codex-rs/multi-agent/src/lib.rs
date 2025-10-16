//! Multi-agent configuration loader and registry utilities.
//!
//! This crate keeps the multi-agent orchestration logic decoupled from the
//! rest of the codebase. It exposes a focused API around three main concepts:
//! an [`AgentId`] slug, an [`AgentRegistry`] that maps ids to directories under
//! `~/.codex/agents/`, and an [`AgentContext`] that bundles the effective
//! configuration for a selected agent.

use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::config::find_codex_home;
use codex_core::config_loader;
use codex_core::delegate_tool::DelegateToolAdapter;
use serde::Deserialize;
use serde::Serialize;
use toml::Value as TomlValue;

/// Identifier for a sub-agent directory under `~/.codex/agents`.
///
/// The slug must be lowercase ASCII and may contain letters, numbers,
/// underscores, and hyphens. This keeps directory names portable while staying
/// human-friendly (e.g., `rust_test_writer`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    /// Parse `raw` into an [`AgentId`] while enforcing slug constraints.
    pub fn parse(raw: &str) -> Result<Self> {
        if raw.is_empty() {
            bail!("Agent id cannot be empty");
        }

        if !raw
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
        {
            bail!("Invalid agent id `{raw}`; use lowercase letters, numbers, `-`, or `_`");
        }

        Ok(Self(raw.to_string()))
    }

    /// Access the slug as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Root-level registry responsible for resolving agent directories.
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    global_codex_home: PathBuf,
    agents_root: PathBuf,
}

impl AgentRegistry {
    /// Construct a registry for a given global Codex home directory.
    pub fn new(global_codex_home: PathBuf) -> Self {
        let agents_root = global_codex_home.join("agents");
        Self {
            global_codex_home,
            agents_root,
        }
    }

    /// Resolve and create (if needed) the directory for `agent_id`.
    pub fn ensure_agent_dir(&self, agent_id: &AgentId) -> Result<PathBuf> {
        let dir = self.agents_root.join(agent_id.as_str());
        fs::create_dir_all(&dir).with_context(|| {
            format!(
                "Failed to create agent directory at {}",
                dir.to_string_lossy()
            )
        })?;
        Ok(dir)
    }

    /// Enumerate all agent ids by inspecting the filesystem.
    pub fn list_agent_ids(&self) -> Result<Vec<AgentId>> {
        let iter = match fs::read_dir(&self.agents_root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "Failed to list agent directory {}",
                        self.agents_root.to_string_lossy()
                    )
                });
            }
        };

        let mut ids = Vec::new();
        for entry in iter {
            let entry = entry?;
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                && let Some(name) = entry.file_name().to_str()
                && let Ok(id) = AgentId::parse(name)
            {
                ids.push(id);
            }
        }

        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        Ok(ids)
    }

    /// Access the canonical Codex home all agents inherit from.
    pub fn global_codex_home(&self) -> &Path {
        &self.global_codex_home
    }

    /// Access the root directory that holds agent subdirectories.
    pub fn agents_root(&self) -> &Path {
        &self.agents_root
    }
}

/// Aggregated context for an agent (or the primary agent when `agent_id` is
/// `None`).
#[derive(Debug, Clone)]
pub struct AgentContext {
    agent_id: Option<AgentId>,
    codex_home: PathBuf,
    global_codex_home: PathBuf,
    config_toml: ConfigToml,
    config: Config,
    allowed_agents: Vec<AgentId>,
}

impl AgentContext {
    fn new(
        agent_id: Option<AgentId>,
        codex_home: PathBuf,
        global_codex_home: PathBuf,
        config_toml: ConfigToml,
        config: Config,
        allowed_agents: Vec<AgentId>,
    ) -> Self {
        Self {
            agent_id,
            codex_home,
            global_codex_home,
            config_toml,
            config,
            allowed_agents,
        }
    }

    /// Returns the resolved agent id (if any).
    pub fn agent_id(&self) -> Option<&AgentId> {
        self.agent_id.as_ref()
    }

    /// Returns the effective Codex home used for configuration, logs, and sessions.
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    /// Returns the shared global Codex home (`~/.codex`).
    pub fn global_codex_home(&self) -> &Path {
        &self.global_codex_home
    }

    /// Returns the merged `ConfigToml` that produced this context.
    pub fn config_toml(&self) -> &ConfigToml {
        &self.config_toml
    }

    /// Provides the resolved [`Config`] for this context.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns the configured sub-agent ids this context is allowed to invoke.
    pub fn allowed_agents(&self) -> &[AgentId] {
        &self.allowed_agents
    }

    /// Consume the context and return the inner [`Config`].
    pub fn into_config(self) -> Config {
        self.config
    }
}

/// Loader responsible for merging global, agent, and CLI overrides into a
/// single [`Config`] instance.
#[derive(Debug, Clone)]
pub struct AgentConfigLoader {
    registry: AgentRegistry,
}

impl AgentConfigLoader {
    /// Construct a loader rooted at the provided `global_codex_home`.
    pub fn new(global_codex_home: PathBuf) -> Self {
        Self {
            registry: AgentRegistry::new(global_codex_home),
        }
    }

    /// Construct a loader by discovering the global Codex home from the environment.
    pub fn from_env() -> Result<Self> {
        let global_codex_home = find_codex_home()
            .context("Failed to resolve Codex home while constructing AgentConfigLoader")?;
        Ok(Self::new(global_codex_home))
    }

    /// Access the underlying registry.
    pub fn registry(&self) -> &AgentRegistry {
        &self.registry
    }

    /// Load configuration for the provided `agent_slug`. When `agent_slug` is
    /// `None`, the primary (legacy) Codex context is returned.
    pub async fn load_by_slug(
        &self,
        agent_slug: Option<&str>,
        cli_overrides: &CliConfigOverrides,
        config_overrides: ConfigOverrides,
    ) -> Result<AgentContext> {
        let agent_id = match agent_slug {
            Some(slug) => Some(AgentId::parse(slug)?),
            None => None,
        };
        self.load(agent_id.as_ref(), cli_overrides, config_overrides)
            .await
    }

    /// Load configuration for `agent_id`, returning an [`AgentContext`].
    pub async fn load(
        &self,
        agent_id: Option<&AgentId>,
        cli_overrides: &CliConfigOverrides,
        config_overrides: ConfigOverrides,
    ) -> Result<AgentContext> {
        let mut merged_value =
            config_loader::load_config_as_toml(self.registry.global_codex_home())
                .await
                .with_context(|| {
                    format!(
                        "Failed to load global config from {}",
                        self.registry.global_codex_home().to_string_lossy()
                    )
                })?;

        let (agent_id_owned, agent_codex_home) = match agent_id {
            Some(id) => {
                let agent_dir = self.registry.ensure_agent_dir(id)?;
                let agent_value = config_loader::load_config_as_toml(agent_dir.as_path())
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to load agent config for `{}` at {}",
                            id,
                            agent_dir.to_string_lossy()
                        )
                    })?;
                merge_toml_values(&mut merged_value, &agent_value);
                (Some(id.clone()), agent_dir)
            }
            None => (None, self.registry.global_codex_home().to_path_buf()),
        };

        cli_overrides
            .apply_on_value(&mut merged_value)
            .map_err(|err: String| anyhow::anyhow!(err))
            .context("Failed to apply CLI config overrides")?;

        let config_toml: ConfigToml = merged_value.clone().try_into().map_err(|err| {
            anyhow::anyhow!(err).context("Failed to deserialize merged config into ConfigToml")
        })?;

        let config = Config::load_from_base_config_with_overrides(
            config_toml.clone(),
            config_overrides,
            agent_codex_home.clone(),
        )
        .with_context(|| {
            format!(
                "Failed to build Config for agent `{}`",
                agent_id.map(AgentId::as_str).unwrap_or("primary")
            )
        })?;

        let allowed_agents = config
            .multi_agent
            .agents
            .iter()
            .map(|agent| AgentId::parse(agent))
            .collect::<Result<Vec<_>>>()?;

        Ok(AgentContext::new(
            agent_id_owned,
            agent_codex_home,
            self.registry.global_codex_home().to_path_buf(),
            config_toml,
            config,
            allowed_agents,
        ))
    }
}

/// Convenience helper that loads an [`AgentContext`] using environment-derived
/// Codex paths.
pub async fn load_agent_context(
    agent_slug: Option<&str>,
    cli_overrides: &CliConfigOverrides,
    config_overrides: ConfigOverrides,
) -> Result<AgentContext> {
    AgentConfigLoader::from_env()?
        .load_by_slug(agent_slug, cli_overrides, config_overrides)
        .await
}

fn merge_toml_values(base: &mut TomlValue, overlay: &TomlValue) {
    if let TomlValue::Table(overlay_table) = overlay
        && let TomlValue::Table(base_table) = base
    {
        for (key, value) in overlay_table {
            if let Some(existing) = base_table.get_mut(key) {
                merge_toml_values(existing, value);
            } else {
                base_table.insert(key.clone(), value.clone());
            }
        }
    } else {
        *base = overlay.clone();
    }
}

pub mod orchestrator;
pub use orchestrator::ActiveDelegateSession;
pub use orchestrator::AgentOrchestrator;
pub use orchestrator::DelegateEvent;
pub use orchestrator::DelegatePrompt;
pub use orchestrator::DelegateRequest;
pub use orchestrator::DelegateRunId;
pub use orchestrator::DelegateSessionMode;
pub use orchestrator::DelegateSessionSummary;
pub use orchestrator::DetachedRunStatusSummary;
pub use orchestrator::DetachedRunSummary;
use orchestrator::MultiAgentDelegateAdapter;
pub use orchestrator::OrchestratorError;

pub fn delegate_tool_adapter(orchestrator: Arc<AgentOrchestrator>) -> Arc<dyn DelegateToolAdapter> {
    Arc::new(MultiAgentDelegateAdapter::new(orchestrator))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::ConfigOverrides;
    use codex_core::protocol::SandboxPolicy;
    use tempfile::tempdir;

    #[tokio::test]
    async fn parses_agent_id_and_loads_config() {
        let temp_home = tempdir().expect("tempdir");
        let global = temp_home.path().join("global");
        let agents_root = global.join("agents");
        fs::create_dir_all(&agents_root).expect("agents dir");

        // Seed global config.
        fs::create_dir_all(global.join("log")).expect("log dir");
        fs::create_dir_all(global.join("sessions")).expect("sessions dir");
        fs::create_dir_all(global.join("history")).expect("history dir");
        fs::create_dir_all(global.join("mcp")).expect("mcp dir");
        fs::create_dir_all(&agents_root).expect("agents dir");
        fs::create_dir_all(global.join("tmp")).expect("tmp dir");
        fs::write(global.join("config.toml"), "model = \"o1\"").expect("write global config");

        let loader = AgentConfigLoader::new(global.clone());
        let cli_overrides = CliConfigOverrides {
            raw_overrides: vec!["model=\"o2\"".to_string()],
        };

        let context = loader
            .load_by_slug(None, &cli_overrides, ConfigOverrides::default())
            .await
            .expect("load context");

        assert!(context.agent_id().is_none());
        assert_eq!(context.codex_home(), global.as_path());
        assert_eq!(context.config().model, "o2", "CLI override should win");
        assert!(context.allowed_agents().is_empty());

        let agent_id = AgentId::parse("rust_test_writer").expect("parse");
        let agent_dir = loader
            .registry
            .ensure_agent_dir(&agent_id)
            .expect("agent dir");
        fs::write(
            agent_dir.join("config.toml"),
            "sandbox_mode = \"danger-full-access\"",
        )
        .expect("write agent config");

        let context = loader
            .load(
                Some(&agent_id),
                &CliConfigOverrides::default(),
                ConfigOverrides::default(),
            )
            .await
            .expect("load agent context");

        assert_eq!(context.agent_id().unwrap().as_str(), "rust_test_writer");
        assert_eq!(context.codex_home(), agent_dir.as_path());
        assert_eq!(
            context.config().sandbox_policy,
            SandboxPolicy::DangerFullAccess
        );
        assert!(context.allowed_agents().is_empty());
    }

    #[tokio::test]
    async fn allowed_agents_follow_multi_agent_list() {
        let temp_home = tempdir().expect("tempdir");
        let global = temp_home.path().join("global");
        let agents_root = global.join("agents");
        fs::create_dir_all(global.join("log")).expect("log dir");
        fs::create_dir_all(global.join("sessions")).expect("sessions dir");
        fs::create_dir_all(global.join("history")).expect("history dir");
        fs::create_dir_all(global.join("mcp")).expect("mcp dir");
        fs::create_dir_all(global.join("tmp")).expect("tmp dir");
        fs::create_dir_all(&agents_root).expect("agents dir");

        fs::write(
            global.join("config.toml"),
            r#"
model = "gpt-5"

[multi_agent]
agents = ["ideas_provider", "critic"]
"#,
        )
        .expect("write global config");

        let loader = AgentConfigLoader::new(global.clone());
        let context = loader
            .load_by_slug(
                None,
                &CliConfigOverrides::default(),
                ConfigOverrides::default(),
            )
            .await
            .expect("load context with multi-agent list");

        let allowed: Vec<_> = context
            .allowed_agents()
            .iter()
            .map(|id| id.as_str().to_string())
            .collect();
        assert_eq!(allowed, ["ideas_provider", "critic"]);
        assert!(
            context.config().include_delegate_tool,
            "delegate tool automatically enabled when agents are configured"
        );
    }

    #[test]
    fn agent_id_rejects_invalid_characters() {
        assert!(AgentId::parse("Ok").is_err());
        assert!(AgentId::parse("with space").is_err());
        assert!(AgentId::parse("rust#1").is_err());
        assert!(AgentId::parse("").is_err());
        assert!(AgentId::parse("rust_test_writer").is_ok());
    }

    #[test]
    fn merge_toml_recursively_merges_tables() {
        use toml::value::Table;

        let mut base_table = Table::new();
        base_table.insert("model".into(), TomlValue::String("o1".into()));
        let mut base_nested = Table::new();
        base_nested.insert("value".into(), TomlValue::Integer(1));
        base_table.insert("nested".into(), TomlValue::Table(base_nested));
        let mut base = TomlValue::Table(base_table);

        let mut overlay_table = Table::new();
        let mut overlay_nested = Table::new();
        overlay_nested.insert("value".into(), TomlValue::Integer(2));
        overlay_nested.insert("extra".into(), TomlValue::Boolean(true));
        overlay_table.insert("nested".into(), TomlValue::Table(overlay_nested));
        overlay_table.insert("new".into(), TomlValue::String("field".into()));
        let overlay = TomlValue::Table(overlay_table);

        merge_toml_values(&mut base, &overlay);
        let nested = base
            .get("nested")
            .unwrap()
            .as_table()
            .expect("nested table");
        assert_eq!(nested.get("value").unwrap().as_integer(), Some(2));
        assert_eq!(nested.get("extra").unwrap().as_bool(), Some(true));
        assert_eq!(base.get("new").unwrap().as_str(), Some("field"));
    }
}
