//! Skill source tracking.
//!
//! Each loaded skill carries provenance information describing where it
//! was discovered. This is used for precedence resolution and diagnostics.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Where a skill was loaded from.
///
/// Skills can originate from bundled defaults, project-local `.cocode/skills/`
/// directories, user-global `~/.cocode/skills/`, or plugin directories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillSource {
    /// A skill bundled with the binary.
    Bundled,

    /// A project-local skill found in `.cocode/skills/`.
    ProjectLocal {
        /// Absolute path to the skill directory.
        path: PathBuf,
    },

    /// A user-global skill found in `~/.cocode/skills/`.
    UserGlobal {
        /// Absolute path to the skill directory.
        path: PathBuf,
    },

    /// A skill provided by a plugin.
    Plugin {
        /// Name of the plugin that provided the skill.
        plugin_name: String,
    },
}

impl fmt::Display for SkillSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bundled => write!(f, "bundled"),
            Self::ProjectLocal { path } => write!(f, "project-local ({})", path.display()),
            Self::UserGlobal { path } => write!(f, "user-global ({})", path.display()),
            Self::Plugin { plugin_name } => write!(f, "plugin ({plugin_name})"),
        }
    }
}

/// Categorization of where a skill was loaded from.
///
/// This is a simplified version of [`SkillSource`] used when the exact
/// path is not needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadedFrom {
    /// From bundled skills compiled into the binary.
    BundledDir,

    /// From a project-local `.cocode/skills/` directory.
    ProjectSkillsDir,

    /// From the user-global `~/.cocode/skills/` directory.
    UserSkillsDir,

    /// From a plugin directory.
    PluginDir,
}

impl fmt::Display for LoadedFrom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BundledDir => write!(f, "bundled"),
            Self::ProjectSkillsDir => write!(f, "project skills"),
            Self::UserSkillsDir => write!(f, "user skills"),
            Self::PluginDir => write!(f, "plugin"),
        }
    }
}

impl From<&SkillSource> for LoadedFrom {
    fn from(source: &SkillSource) -> Self {
        match source {
            SkillSource::Bundled => Self::BundledDir,
            SkillSource::ProjectLocal { .. } => Self::ProjectSkillsDir,
            SkillSource::UserGlobal { .. } => Self::UserSkillsDir,
            SkillSource::Plugin { .. } => Self::PluginDir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_source_display() {
        assert_eq!(SkillSource::Bundled.to_string(), "bundled");

        let local = SkillSource::ProjectLocal {
            path: PathBuf::from("/project/.cocode/skills/commit"),
        };
        assert_eq!(
            local.to_string(),
            "project-local (/project/.cocode/skills/commit)"
        );

        let global = SkillSource::UserGlobal {
            path: PathBuf::from("/home/user/.cocode/skills/review"),
        };
        assert_eq!(
            global.to_string(),
            "user-global (/home/user/.cocode/skills/review)"
        );

        let plugin = SkillSource::Plugin {
            plugin_name: "my-plugin".to_string(),
        };
        assert_eq!(plugin.to_string(), "plugin (my-plugin)");
    }

    #[test]
    fn test_loaded_from_display() {
        assert_eq!(LoadedFrom::BundledDir.to_string(), "bundled");
        assert_eq!(LoadedFrom::ProjectSkillsDir.to_string(), "project skills");
        assert_eq!(LoadedFrom::UserSkillsDir.to_string(), "user skills");
        assert_eq!(LoadedFrom::PluginDir.to_string(), "plugin");
    }

    #[test]
    fn test_loaded_from_conversion() {
        assert_eq!(
            LoadedFrom::from(&SkillSource::Bundled),
            LoadedFrom::BundledDir
        );
        assert_eq!(
            LoadedFrom::from(&SkillSource::ProjectLocal {
                path: PathBuf::from("/x")
            }),
            LoadedFrom::ProjectSkillsDir
        );
        assert_eq!(
            LoadedFrom::from(&SkillSource::UserGlobal {
                path: PathBuf::from("/x")
            }),
            LoadedFrom::UserSkillsDir
        );
        assert_eq!(
            LoadedFrom::from(&SkillSource::Plugin {
                plugin_name: "p".to_string()
            }),
            LoadedFrom::PluginDir
        );
    }
}
