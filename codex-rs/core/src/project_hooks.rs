use crate::config::types::ProjectHookCommand;
use crate::config::types::ProjectHookConfig;
use crate::config::types::ProjectHookEvent;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectHook {
    pub name: Option<String>,
    pub event: ProjectHookEvent,
    pub run: ProjectHookCommand,
    pub cwd: Option<PathBuf>,
    pub resolved_cwd: Option<AbsolutePathBuf>,
    pub env: Option<HashMap<String, String>>,
    pub timeout_ms: Option<u64>,
    pub run_in_background: bool,
}

impl ProjectHook {
    fn from_config(config: &ProjectHookConfig, project_root: &Path) -> Self {
        let resolved_cwd = config
            .cwd
            .as_ref()
            .and_then(|cwd| AbsolutePathBuf::resolve_path_against_base(cwd, project_root).ok());

        Self {
            name: config.name.clone(),
            event: config.event,
            run: config.run.clone(),
            cwd: config.cwd.clone(),
            resolved_cwd,
            env: config.env.clone(),
            timeout_ms: config.timeout_ms,
            run_in_background: config.run_in_background,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectHooks {
    hooks: Vec<ProjectHook>,
}

impl ProjectHooks {
    pub fn from_configs(configs: Option<&[ProjectHookConfig]>, project_root: &Path) -> Self {
        Self {
            hooks: configs
                .unwrap_or_default()
                .iter()
                .map(|config| ProjectHook::from_config(config, project_root))
                .collect(),
        }
    }

    /// Create ProjectHooks by merging global hooks (from ~/.codex/) and project hooks.
    /// Global hooks run first, then project hooks. Both run when present.
    pub fn from_global_and_project_configs(
        global_configs: Option<&[ProjectHookConfig]>,
        global_root: &Path,
        project_configs: Option<&[ProjectHookConfig]>,
        project_root: &Path,
    ) -> Self {
        let mut hooks: Vec<ProjectHook> = Vec::new();

        // Add global hooks first (from ~/.codex/hooks)
        if let Some(globals) = global_configs {
            hooks.extend(
                globals
                    .iter()
                    .map(|config| ProjectHook::from_config(config, global_root)),
            );
        }

        // Add project hooks second (from <project>/.codex/hooks)
        if let Some(projects) = project_configs {
            hooks.extend(
                projects
                    .iter()
                    .map(|config| ProjectHook::from_config(config, project_root)),
            );
        }

        Self { hooks }
    }

    pub fn hooks(&self) -> &[ProjectHook] {
        &self.hooks
    }
}
