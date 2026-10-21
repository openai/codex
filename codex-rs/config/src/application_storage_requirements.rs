//! Managed locations for newly created Desktop workspaces. These requirements
//! fix the storage settings; they do not restrict access to other directories.

use crate::AbsolutePathBuf;
use crate::config_toml::ConfigToml;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApplicationStorageRequirementsToml {
    #[serde(default, deserialize_with = "deserialize_absolute_path")]
    pub default_project_parent: Option<AbsolutePathBuf>,
    #[serde(default, deserialize_with = "deserialize_absolute_path")]
    pub projectless_workspace_root: Option<AbsolutePathBuf>,
}

impl ApplicationStorageRequirementsToml {
    pub(crate) fn is_empty(&self) -> bool {
        self.default_project_parent.is_none() && self.projectless_workspace_root.is_none()
    }

    pub(crate) fn apply_to_config(&self, config: &mut ConfigToml) {
        for (key, path) in [
            ("defaultProjectParent", &self.default_project_parent),
            ("projectlessWorkspaceRoot", &self.projectless_workspace_root),
        ] {
            if let Some(path) = path {
                config.desktop.get_or_insert_default().insert(
                    key.to_string(),
                    Value::String(path.to_string_lossy().into_owned()),
                );
            }
        }
    }

    pub(crate) fn exact_requirement_for_config_path(
        &self,
        segments: &[String],
    ) -> Option<&'static str> {
        [
            (
                self.default_project_parent.is_some(),
                "defaultProjectParent",
                "desktop.defaultProjectParent",
            ),
            (
                self.projectless_workspace_root.is_some(),
                "projectlessWorkspaceRoot",
                "desktop.projectlessWorkspaceRoot",
            ),
        ]
        .into_iter()
        .find_map(|(managed, key, name)| {
            let path = ["desktop", key];
            (managed && segments.iter().zip(path).all(|(left, right)| left == right))
                .then_some(name)
        })
    }
}

#[cfg(test)]
#[path = "application_storage_requirements_tests.rs"]
mod tests;

// Requirements must not resolve relative to a machine-specific config directory.
fn deserialize_absolute_path<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AbsolutePathBuf>, D::Error> {
    let path = std::path::PathBuf::deserialize(deserializer)?;
    AbsolutePathBuf::from_absolute_path_checked(path)
        .map(Some)
        .map_err(serde::de::Error::custom)
}
