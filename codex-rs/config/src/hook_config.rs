use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

pub use codex_protocol::hooks::HookEventsToml;
pub use codex_protocol::hooks::HookHandlerConfig;
pub use codex_protocol::hooks::MatcherGroup;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HooksFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub hooks: HookEventsToml,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HooksToml {
    #[serde(flatten)]
    pub events: HookEventsToml,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub state: BTreeMap<String, HookStateToml>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HookStateToml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_hash: Option<String>,
    /// Approved plugin contents; kept separate for declaration-only clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_content_hash: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedHooksRequirementsToml {
    pub managed_dir: Option<PathBuf>,
    pub windows_managed_dir: Option<PathBuf>,
    #[serde(flatten)]
    pub hooks: Box<HookEventsToml>,
}

impl ManagedHooksRequirementsToml {
    pub fn is_empty(&self) -> bool {
        let Self {
            managed_dir,
            windows_managed_dir,
            hooks,
        } = self;
        managed_dir.is_none() && windows_managed_dir.is_none() && hooks.is_empty()
    }

    pub fn handler_count(&self) -> usize {
        self.hooks.handler_count()
    }

    pub fn managed_dir_for_current_platform(&self) -> Option<&Path> {
        #[cfg(windows)]
        {
            self.windows_managed_dir.as_deref()
        }

        #[cfg(not(windows))]
        {
            self.managed_dir.as_deref()
        }
    }
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
