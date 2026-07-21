use crate::ClaSource;
use crate::CurSource;
use crate::RewriteProfile;
use crate::detect::plugins;
use crate::detect::sessions::detect_recent_cla_sessions_with_limits;
use crate::detect::sessions::detect_recent_cur_sessions_with_limits;
use crate::model::ExternalAgentSessionImportLimits;
use crate::sessions::ExternalAgentSessionMigration;
use crate::sessions::SessionMetadataMode;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use crate::model::MigrationDetails;
use crate::scope::MigrationScope;
use crate::source_cla;
use crate::source_cur;

pub(super) use crate::InstructionSourceGroup;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MarketplaceImportSource {
    pub(super) source: String,
    pub(super) ref_name: Option<String>,
}

pub(super) struct DetectedSourcePlugins {
    pub(super) description: String,
    pub(super) details: MigrationDetails,
}

pub(super) struct PluginDetectionContext<'a> {
    pub(super) external_agent_home: &'a Path,
    pub(super) source_settings: &'a Path,
    pub(super) source_root: &'a Path,
    pub(super) repo_root: Option<&'a Path>,
    pub(super) settings: Option<&'a JsonValue>,
    pub(super) configured_plugin_ids: &'a HashSet<String>,
    pub(super) configured_marketplace_plugins: &'a BTreeMap<String, HashSet<String>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ExternalAgentSource {
    #[default]
    Cla,
    Cur,
}

impl ExternalAgentSource {
    pub(super) fn from_migration_source(migration_source: Option<&str>) -> Self {
        if migration_source
            .is_some_and(|source| source.eq_ignore_ascii_case(CurSource::MIGRATION_SOURCE))
        {
            Self::Cur
        } else {
            Self::Cla
        }
    }

    pub(super) fn config_dir(self) -> &'static str {
        match self {
            Self::Cla => ClaSource::CONFIG_DIR,
            Self::Cur => CurSource::CONFIG_DIR,
        }
    }

    pub(super) fn supports_memory(self) -> bool {
        match self {
            Self::Cla => true,
            Self::Cur => false,
        }
    }

    pub(super) fn settings_file_name(self, scope: &MigrationScope) -> &'static str {
        match (self, scope) {
            (Self::Cla, _) => ClaSource::SETTINGS_FILE,
            (Self::Cur, MigrationScope::Home) => CurSource::HOME_CONFIG_FILE,
            (Self::Cur, MigrationScope::Repository { .. }) => CurSource::PROJECT_CONFIG_FILE,
        }
    }

    pub(super) fn effective_settings(
        self,
        source_config_dir: &Path,
        source_settings: &Path,
    ) -> io::Result<Option<JsonValue>> {
        match self {
            Self::Cla => ClaSource::effective_settings(source_settings),
            Self::Cur => CurSource::effective_settings(source_config_dir, source_settings),
        }
    }

    pub(super) fn build_config(self, settings: &JsonValue) -> io::Result<TomlValue> {
        match self {
            Self::Cla => ClaSource::build_config(settings),
            Self::Cur => CurSource::build_config(settings),
        }
    }

    pub(super) fn plugin_migration(
        self,
        context: PluginDetectionContext<'_>,
    ) -> io::Result<Option<DetectedSourcePlugins>> {
        match self {
            Self::Cla => Ok(plugins::detect_cla_plugins(&context)),
            Self::Cur if context.repo_root.is_none() => plugins::detect_cur_plugins(&context),
            Self::Cur => Ok(None),
        }
    }

    pub(super) fn supports_plugin_migration(self, settings: Option<&JsonValue>) -> bool {
        match self {
            Self::Cla => plugins::can_detect_cla_plugins(settings),
            Self::Cur => true,
        }
    }

    pub(super) fn recent_sessions(
        self,
        external_agent_home: &Path,
        codex_home: &Path,
        limits: ExternalAgentSessionImportLimits,
    ) -> io::Result<Vec<ExternalAgentSessionMigration>> {
        match self {
            Self::Cla => {
                detect_recent_cla_sessions_with_limits(external_agent_home, codex_home, limits)
            }
            Self::Cur => {
                detect_recent_cur_sessions_with_limits(external_agent_home, codex_home, limits)
            }
        }
    }

    pub(super) fn session_metadata_mode(self) -> SessionMetadataMode {
        match self {
            Self::Cla => SessionMetadataMode::Embedded,
            Self::Cur => SessionMetadataMode::MigrationFallback,
        }
    }

    pub(super) fn connector_metadata_roots(self, external_agent_home: &Path) -> Vec<PathBuf> {
        match self {
            Self::Cla => ClaSource::connector_metadata_roots(external_agent_home),
            Self::Cur => Vec::new(),
        }
    }

    pub(super) fn marketplace_import_sources(
        self,
        external_agent_home: &Path,
        source_root: &Path,
        source_settings: &Path,
    ) -> io::Result<BTreeMap<String, MarketplaceImportSource>> {
        match self {
            Self::Cla => Ok(ClaSource::effective_settings(source_settings)?
                .as_ref()
                .map(|settings| {
                    source_cla::marketplace_import_sources(
                        settings,
                        external_agent_home,
                        source_root,
                    )
                })
                .unwrap_or_default()),
            Self::Cur => source_cur::marketplace_import_sources(external_agent_home),
        }
    }

    pub(super) fn build_mcp_config(
        self,
        source_root: &Path,
        source_config_dir: &Path,
        external_agent_home: &Path,
        settings: Option<&JsonValue>,
    ) -> io::Result<TomlValue> {
        match self {
            Self::Cla => ClaSource::build_mcp_config(source_root, external_agent_home, settings),
            Self::Cur => CurSource::build_mcp_config(source_config_dir),
        }
    }

    pub(super) fn mcp_source_path(
        self,
        source_root: PathBuf,
        source_config_dir: PathBuf,
    ) -> PathBuf {
        match self {
            Self::Cla => source_root,
            Self::Cur => source_config_dir.join("mcp.json"),
        }
    }

    pub(super) fn repo_instruction_source_groups(
        self,
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        match self {
            Self::Cla => ClaSource::repo_instruction_source_groups(repo_root),
            Self::Cur => CurSource::repo_instruction_source_groups(repo_root),
        }
    }

    pub(super) fn home_instruction_sources(
        self,
        external_agent_home: &Path,
    ) -> io::Result<Vec<PathBuf>> {
        match self {
            Self::Cla => ClaSource::home_instruction_sources(external_agent_home),
            Self::Cur => Ok(Vec::new()),
        }
    }

    pub(super) fn read_instruction_source(self, path: &Path) -> io::Result<String> {
        match self {
            Self::Cla => ClaSource::read_instruction_source(path),
            Self::Cur => CurSource::read_instruction_source(path),
        }
    }

    pub(super) fn import_commands(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => source_cla::import_source_commands(source_commands, target_skills),
            Self::Cur => source_cur::import_source_commands(source_commands, target_skills),
        }
    }

    pub(super) fn count_missing_commands(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<usize> {
        match self {
            Self::Cla => source_cla::count_missing_source_commands(source_commands, target_skills),
            Self::Cur => source_cur::count_missing_source_commands(source_commands, target_skills),
        }
    }

    pub(super) fn missing_command_names(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => source_cla::missing_source_command_names(source_commands, target_skills),
            Self::Cur => source_cur::missing_source_command_names(source_commands, target_skills),
        }
    }

    pub(super) fn import_subagents(
        self,
        source_agents: &Path,
        target_agents: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => ClaSource::import_subagents(source_agents, target_agents),
            Self::Cur => CurSource::import_subagents(source_agents, target_agents),
        }
    }

    pub(super) fn hook_event_names(
        self,
        source_dir: &Path,
        target_hooks: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => ClaSource::hook_event_names(source_dir, target_hooks),
            Self::Cur => CurSource::hook_event_names(source_dir, target_hooks),
        }
    }

    pub(super) fn import_hooks(self, source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
        match self {
            Self::Cla => ClaSource::import_hooks(source_dir, target_hooks),
            Self::Cur => CurSource::import_hooks(source_dir, target_hooks),
        }
    }

    pub(super) fn rewrite_profile(self) -> RewriteProfile {
        match self {
            Self::Cla => source_cla::REWRITE_PROFILE,
            Self::Cur => source_cur::REWRITE_PROFILE,
        }
    }
}
