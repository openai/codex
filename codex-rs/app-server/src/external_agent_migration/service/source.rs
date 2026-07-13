use codex_external_agent_migration::RewriteProfile;
use codex_external_agent_migration::sessions::ExternalAgentSessionMigration;
use codex_external_agent_migration::sessions::detect_recent_cla_sessions;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use super::MigrationDetails;
use super::source_cla;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InstructionSourceGroup {
    pub(super) scope: PathBuf,
    pub(super) sources: Vec<PathBuf>,
}

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
    pub(super) settings: Option<&'a JsonValue>,
    pub(super) configured_plugin_ids: &'a HashSet<String>,
    pub(super) configured_marketplace_plugins: &'a BTreeMap<String, HashSet<String>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ExternalAgentSource {
    #[default]
    Cla,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceFeature {
    Config,
    Plugins,
    Sessions,
}

impl ExternalAgentSource {
    pub(super) fn config_dir(self) -> &'static str {
        match self {
            Self::Cla => source_cla::CONFIG_DIR,
        }
    }

    pub(super) fn supports(self, feature: SourceFeature) -> bool {
        match (self, feature) {
            (
                Self::Cla,
                SourceFeature::Config | SourceFeature::Plugins | SourceFeature::Sessions,
            ) => true,
        }
    }

    pub(super) fn settings_file_name(self, project_scope: bool) -> &'static str {
        match (self, project_scope) {
            (Self::Cla, _) => "settings.json",
        }
    }

    pub(super) fn effective_settings(
        self,
        source_settings: &Path,
    ) -> io::Result<Option<JsonValue>> {
        match self {
            Self::Cla => source_cla::effective_settings(source_settings),
        }
    }

    pub(super) fn detect_plugins(
        self,
        context: PluginDetectionContext<'_>,
    ) -> io::Result<Option<DetectedSourcePlugins>> {
        match self {
            Self::Cla => Ok(source_cla::detect_plugins(&context)),
        }
    }

    pub(super) fn can_detect_plugins(self, settings: Option<&JsonValue>) -> bool {
        match self {
            Self::Cla => source_cla::can_detect_plugins(settings),
        }
    }

    pub(super) fn detect_recent_sessions(
        self,
        external_agent_home: &Path,
        codex_home: &Path,
    ) -> io::Result<Vec<ExternalAgentSessionMigration>> {
        match self {
            Self::Cla => detect_recent_cla_sessions(external_agent_home, codex_home),
        }
    }

    pub(super) fn marketplace_import_sources(
        self,
        external_agent_home: &Path,
        source_root: &Path,
        source_settings: &Path,
    ) -> io::Result<BTreeMap<String, MarketplaceImportSource>> {
        match self {
            Self::Cla => Ok(source_cla::effective_settings(source_settings)?
                .as_ref()
                .map(|settings| {
                    source_cla::marketplace_import_sources(
                        settings,
                        external_agent_home,
                        source_root,
                    )
                })
                .unwrap_or_default()),
        }
    }

    pub(super) fn append_config(
        self,
        root: &mut toml::map::Map<String, TomlValue>,
        settings: &serde_json::Map<String, JsonValue>,
    ) {
        match self {
            Self::Cla => source_cla::append_config(root, settings),
        }
    }

    pub(super) fn build_mcp_config(
        self,
        source_root: &Path,
        external_agent_home: &Path,
        settings: Option<&JsonValue>,
    ) -> io::Result<TomlValue> {
        match self {
            Self::Cla => source_cla::build_mcp_config(source_root, external_agent_home, settings),
        }
    }

    pub(super) fn mcp_source_path(self, source_root: PathBuf) -> PathBuf {
        match self {
            Self::Cla => source_root,
        }
    }

    pub(super) fn repo_instruction_source_groups(
        self,
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        match self {
            Self::Cla => source_cla::repo_instruction_source_groups(repo_root),
        }
    }

    pub(super) fn home_instruction_sources(
        self,
        external_agent_home: &Path,
    ) -> io::Result<Vec<PathBuf>> {
        match self {
            Self::Cla => source_cla::home_instruction_sources(external_agent_home),
        }
    }

    pub(super) fn read_instruction_source(self, path: &Path) -> io::Result<String> {
        match self {
            Self::Cla => source_cla::read_instruction_source(path),
        }
    }

    pub(super) fn import_commands(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => source_cla::import_source_commands(source_commands, target_skills),
        }
    }

    pub(super) fn count_missing_commands(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<usize> {
        match self {
            Self::Cla => source_cla::count_missing_source_commands(source_commands, target_skills),
        }
    }

    pub(super) fn missing_command_names(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => source_cla::missing_source_command_names(source_commands, target_skills),
        }
    }

    pub(super) fn import_subagents(
        self,
        source_agents: &Path,
        target_agents: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => source_cla::import_source_subagents(source_agents, target_agents),
        }
    }

    pub(super) fn hook_event_names(
        self,
        source_dir: &Path,
        target_hooks: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cla => source_cla::source_hook_event_names(source_dir, target_hooks),
        }
    }

    pub(super) fn import_hooks(self, source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
        match self {
            Self::Cla => source_cla::import_source_hooks(source_dir, target_hooks),
        }
    }

    pub(super) fn rewrite_profile(self) -> RewriteProfile {
        match self {
            Self::Cla => source_cla::REWRITE_PROFILE,
        }
    }
}
