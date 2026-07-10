use codex_external_agent_migration::RewriteProfile;
use serde_json::Value as JsonValue;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

use super::source_cl;
use super::source_cu;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InstructionSourceGroup {
    pub(super) scope: PathBuf,
    pub(super) sources: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ExternalAgentSource {
    #[default]
    Cl,
    Cu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceFeature {
    Config,
    Plugins,
    Sessions,
}

impl ExternalAgentSource {
    pub(super) fn from_api_source(source: Option<&str>) -> Self {
        if source.is_some_and(|source| source.eq_ignore_ascii_case("cursor")) {
            Self::Cu
        } else {
            Self::Cl
        }
    }

    pub(super) fn config_dir(self) -> &'static str {
        match self {
            Self::Cl => source_cl::CONFIG_DIR,
            Self::Cu => source_cu::CONFIG_DIR,
        }
    }

    pub(super) fn supports(self, feature: SourceFeature) -> bool {
        match (self, feature) {
            (
                Self::Cl,
                SourceFeature::Config | SourceFeature::Plugins | SourceFeature::Sessions,
            ) => true,
            (
                Self::Cu,
                SourceFeature::Config | SourceFeature::Plugins | SourceFeature::Sessions,
            ) => true,
        }
    }

    pub(super) fn settings_file_name(self, project_scope: bool) -> &'static str {
        match (self, project_scope) {
            (Self::Cl, _) => "settings.json",
            (Self::Cu, false) => source_cu::HOME_CONFIG_FILE,
            (Self::Cu, true) => source_cu::PROJECT_CONFIG_FILE,
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
            Self::Cl => source_cl::build_mcp_config(source_root, external_agent_home, settings),
            Self::Cu => source_cu::build_mcp_config(source_config_dir),
        }
    }

    pub(super) fn mcp_source_path(
        self,
        source_root: PathBuf,
        source_config_dir: PathBuf,
    ) -> PathBuf {
        match self {
            Self::Cl => source_root,
            Self::Cu => source_config_dir.join("mcp.json"),
        }
    }

    pub(super) fn repo_instruction_source_groups(
        self,
        repo_root: &Path,
    ) -> io::Result<Vec<InstructionSourceGroup>> {
        match self {
            Self::Cl => source_cl::repo_instruction_source_groups(repo_root),
            Self::Cu => source_cu::repo_instruction_source_groups(repo_root),
        }
    }

    pub(super) fn home_instruction_sources(
        self,
        external_agent_home: &Path,
    ) -> io::Result<Vec<PathBuf>> {
        match self {
            Self::Cl => source_cl::home_instruction_sources(external_agent_home),
            Self::Cu => source_cu::home_instruction_sources(external_agent_home),
        }
    }

    pub(super) fn read_instruction_source(self, path: &Path) -> io::Result<String> {
        match self {
            Self::Cl => source_cl::read_instruction_source(path),
            Self::Cu => source_cu::read_instruction_source(path),
        }
    }

    pub(super) fn import_commands(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cl => source_cl::import_source_commands(source_commands, target_skills),
            Self::Cu => source_cu::import_source_commands(source_commands, target_skills),
        }
    }

    pub(super) fn count_missing_commands(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<usize> {
        match self {
            Self::Cl => source_cl::count_missing_source_commands(source_commands, target_skills),
            Self::Cu => source_cu::count_missing_source_commands(source_commands, target_skills),
        }
    }

    pub(super) fn missing_command_names(
        self,
        source_commands: &Path,
        target_skills: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cl => source_cl::missing_source_command_names(source_commands, target_skills),
            Self::Cu => source_cu::missing_source_command_names(source_commands, target_skills),
        }
    }

    pub(super) fn import_subagents(
        self,
        source_agents: &Path,
        target_agents: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cl => source_cl::import_source_subagents(source_agents, target_agents),
            Self::Cu => source_cu::import_source_subagents(source_agents, target_agents),
        }
    }

    pub(super) fn hook_event_names(
        self,
        source_dir: &Path,
        target_hooks: &Path,
    ) -> io::Result<Vec<String>> {
        match self {
            Self::Cl => source_cl::source_hook_event_names(source_dir, target_hooks),
            Self::Cu => source_cu::source_hook_event_names(source_dir, target_hooks),
        }
    }

    pub(super) fn import_hooks(self, source_dir: &Path, target_hooks: &Path) -> io::Result<bool> {
        match self {
            Self::Cl => source_cl::import_source_hooks(source_dir, target_hooks),
            Self::Cu => source_cu::import_source_hooks(source_dir, target_hooks),
        }
    }

    pub(super) fn rewrite_profile(self) -> RewriteProfile {
        match self {
            Self::Cl => source_cl::REWRITE_PROFILE,
            Self::Cu => source_cu::REWRITE_PROFILE,
        }
    }
}
