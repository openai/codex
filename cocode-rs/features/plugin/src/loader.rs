//! Plugin discovery and loading.
//!
//! Scans directories for plugins (containing PLUGIN.toml) and loads their
//! contributions.

use crate::agent_loader::load_agents_from_dir;
use crate::command_loader::load_commands_from_dir;
use crate::contribution::PluginContribution;
use crate::contribution::PluginContributions;
use crate::error::Result;
use crate::error::plugin_error::InvalidManifestSnafu;
use crate::error::plugin_error::IoSnafu;
use crate::error::plugin_error::PathTraversalSnafu;
use crate::manifest::PLUGIN_TOML;
use crate::manifest::PluginManifest;
use crate::mcp_loader::load_mcp_servers_from_dir;
use crate::scope::PluginScope;

use cocode_skill::SkillLoadOutcome;
use cocode_skill::load_skills_from_dir;
use std::path::Path;
use std::path::PathBuf;
use tracing::debug;
use tracing::info;
use tracing::warn;
use walkdir::WalkDir;

/// Maximum depth to scan for plugins.
const MAX_SCAN_DEPTH: i32 = 3;

/// A loaded plugin with its manifest and contributions.
#[derive(Debug)]
pub struct LoadedPlugin {
    /// Plugin manifest.
    pub manifest: PluginManifest,

    /// Plugin directory.
    pub path: PathBuf,

    /// Scope the plugin was loaded from.
    pub scope: PluginScope,

    /// Loaded contributions.
    pub contributions: Vec<PluginContribution>,
}

impl LoadedPlugin {
    /// Get the plugin name.
    pub fn name(&self) -> &str {
        &self.manifest.plugin.name
    }

    /// Get the plugin version.
    pub fn version(&self) -> &str {
        &self.manifest.plugin.version
    }
}

/// Plugin loader that discovers and loads plugins from directories.
pub struct PluginLoader {
    /// Maximum depth to scan.
    max_depth: i32,
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self {
            max_depth: MAX_SCAN_DEPTH,
        }
    }
}

impl PluginLoader {
    /// Create a new plugin loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum scan depth.
    pub fn with_max_depth(mut self, depth: i32) -> Self {
        self.max_depth = depth;
        self
    }

    /// Scan a directory for plugins.
    ///
    /// Returns a list of paths to plugin directories (containing PLUGIN.toml).
    pub fn scan(&self, root: &Path) -> Vec<PathBuf> {
        if !root.is_dir() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let depth = self.max_depth.max(0) as usize;

        // Note: Symlinks are not followed to prevent potential security issues
        // with symlink attacks and to ensure plugins stay within their boundaries.
        let walker = WalkDir::new(root)
            .max_depth(depth)
            .follow_links(false)
            .into_iter();

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().is_dir() {
                let manifest_path = entry.path().join(PLUGIN_TOML);
                if manifest_path.is_file() {
                    results.push(entry.path().to_path_buf());
                }
            }
        }

        results
    }

    /// Load a single plugin from its directory.
    pub fn load(&self, dir: &Path, scope: PluginScope) -> Result<LoadedPlugin> {
        debug!(path = %dir.display(), scope = %scope, "Loading plugin");

        // Load manifest
        let manifest = PluginManifest::from_dir(dir)?;

        // Validate manifest
        if let Err(errors) = manifest.validate() {
            return Err(InvalidManifestSnafu {
                path: dir.to_path_buf(),
                message: errors.join("; "),
            }
            .build());
        }

        // Load contributions
        let contributions =
            self.load_contributions(dir, &manifest.contributions, &manifest.plugin.name)?;

        info!(
            name = %manifest.plugin.name,
            version = %manifest.plugin.version,
            scope = %scope,
            contributions = contributions.len(),
            "Loaded plugin"
        );

        Ok(LoadedPlugin {
            manifest,
            path: dir.to_path_buf(),
            scope,
            contributions,
        })
    }

    /// Validate that a path stays within the plugin directory.
    ///
    /// Returns the canonical path if valid, or an error for path traversal.
    fn validate_path(&self, plugin_dir: &Path, relative_path: &str) -> Result<PathBuf> {
        let full_path = plugin_dir.join(relative_path);

        // Canonicalize both paths to resolve symlinks and ..
        let canonical_plugin = match plugin_dir.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                return Err(IoSnafu {
                    path: plugin_dir.to_path_buf(),
                    message: e.to_string(),
                }
                .build());
            }
        };

        // The target path may not exist yet, so canonicalize the parent
        let canonical_full = if full_path.exists() {
            full_path.canonicalize().map_err(|e| {
                IoSnafu {
                    path: full_path.clone(),
                    message: e.to_string(),
                }
                .build()
            })?
        } else {
            // Path doesn't exist, return as-is (will fail later with appropriate error)
            full_path
        };

        // Check that the canonical path is within the plugin directory
        if !canonical_full.starts_with(&canonical_plugin) {
            return Err(PathTraversalSnafu {
                path: PathBuf::from(relative_path),
            }
            .build());
        }

        Ok(canonical_full)
    }

    /// Load contributions from a plugin.
    fn load_contributions(
        &self,
        plugin_dir: &Path,
        contributions: &PluginContributions,
        plugin_name: &str,
    ) -> Result<Vec<PluginContribution>> {
        let mut result = Vec::new();

        // Load skills
        for skill_path in &contributions.skills {
            let full_path = match self.validate_path(plugin_dir, skill_path) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        plugin = %plugin_name,
                        path = %skill_path,
                        error = %e,
                        "Invalid skill path in plugin"
                    );
                    continue;
                }
            };
            if full_path.is_dir() {
                let outcomes = load_skills_from_dir(&full_path);
                for outcome in outcomes {
                    match outcome {
                        SkillLoadOutcome::Success { skill, .. } => {
                            result.push(PluginContribution::Skill {
                                skill,
                                plugin_name: plugin_name.to_string(),
                            });
                        }
                        SkillLoadOutcome::Failed { path, error } => {
                            warn!(
                                plugin = %plugin_name,
                                path = %path.display(),
                                error = %error,
                                "Failed to load skill from plugin"
                            );
                        }
                    }
                }
            } else {
                debug!(
                    plugin = %plugin_name,
                    path = %full_path.display(),
                    "Skill path not found or not a directory"
                );
            }
        }

        // Load hooks
        for hook_path in &contributions.hooks {
            let full_path = match self.validate_path(plugin_dir, hook_path) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        plugin = %plugin_name,
                        path = %hook_path,
                        error = %e,
                        "Invalid hook path in plugin"
                    );
                    continue;
                }
            };
            if full_path.is_file() {
                match self.load_hooks_from_file(&full_path, plugin_name) {
                    Ok(hooks) => result.extend(hooks),
                    Err(e) => {
                        warn!(
                            plugin = %plugin_name,
                            path = %full_path.display(),
                            error = %e,
                            "Failed to load hooks from plugin"
                        );
                    }
                }
            } else {
                debug!(
                    plugin = %plugin_name,
                    path = %full_path.display(),
                    "Hook file not found"
                );
            }
        }

        // Load agents
        for agent_path in &contributions.agents {
            let full_path = match self.validate_path(plugin_dir, agent_path) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        plugin = %plugin_name,
                        path = %agent_path,
                        error = %e,
                        "Invalid agent path in plugin"
                    );
                    continue;
                }
            };
            if full_path.is_dir() {
                let agents = load_agents_from_dir(&full_path, plugin_name);
                result.extend(agents);
            } else {
                debug!(
                    plugin = %plugin_name,
                    path = %full_path.display(),
                    "Agent path not found or not a directory"
                );
            }
        }

        // Load commands
        for command_path in &contributions.commands {
            let full_path = match self.validate_path(plugin_dir, command_path) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        plugin = %plugin_name,
                        path = %command_path,
                        error = %e,
                        "Invalid command path in plugin"
                    );
                    continue;
                }
            };
            if full_path.is_dir() {
                let commands = load_commands_from_dir(&full_path, plugin_name);
                result.extend(commands);
            } else {
                debug!(
                    plugin = %plugin_name,
                    path = %full_path.display(),
                    "Command path not found or not a directory"
                );
            }
        }

        // Load MCP servers
        for mcp_path in &contributions.mcp_servers {
            let full_path = match self.validate_path(plugin_dir, mcp_path) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        plugin = %plugin_name,
                        path = %mcp_path,
                        error = %e,
                        "Invalid MCP server path in plugin"
                    );
                    continue;
                }
            };
            if full_path.is_dir() {
                let servers = load_mcp_servers_from_dir(&full_path, plugin_name);
                result.extend(servers);
            } else {
                debug!(
                    plugin = %plugin_name,
                    path = %full_path.display(),
                    "MCP server path not found or not a directory"
                );
            }
        }

        Ok(result)
    }

    /// Load hooks from a TOML file.
    fn load_hooks_from_file(
        &self,
        path: &Path,
        plugin_name: &str,
    ) -> Result<Vec<PluginContribution>> {
        // load_hooks_from_toml takes a path and handles reading internally
        match cocode_hooks::load_hooks_from_toml(path) {
            Ok(definitions) => Ok(definitions
                .into_iter()
                .map(|hook| PluginContribution::Hook {
                    hook,
                    plugin_name: plugin_name.to_string(),
                })
                .collect()),
            Err(e) => Err(InvalidManifestSnafu {
                path: path.to_path_buf(),
                message: format!("Failed to parse hooks: {e}"),
            }
            .build()),
        }
    }
}

/// Load plugins from multiple root directories.
///
/// Scans each root for plugins and loads them. Returns all successfully
/// loaded plugins.
pub fn load_plugins_from_roots(roots: &[(PathBuf, PluginScope)]) -> Vec<LoadedPlugin> {
    let loader = PluginLoader::new();
    let mut plugins = Vec::new();

    for (root, scope) in roots {
        if !root.is_dir() {
            debug!(
                root = %root.display(),
                scope = %scope,
                "Plugin root does not exist or is not a directory"
            );
            continue;
        }

        let plugin_dirs = loader.scan(root);
        debug!(
            root = %root.display(),
            scope = %scope,
            count = plugin_dirs.len(),
            "Scanned for plugins"
        );

        for dir in plugin_dirs {
            match loader.load(&dir, *scope) {
                Ok(plugin) => plugins.push(plugin),
                Err(e) => {
                    warn!(
                        path = %dir.display(),
                        scope = %scope,
                        error = %e,
                        "Failed to load plugin"
                    );
                }
            }
        }
    }

    info!(total = plugins.len(), "Plugin loading complete");

    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_empty_dir() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let loader = PluginLoader::new();
        let results = loader.scan(tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn test_scan_finds_plugin() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let plugin_dir = tmp.path().join("my-plugin");
        fs::create_dir_all(&plugin_dir).expect("mkdir");
        fs::write(
            plugin_dir.join("PLUGIN.toml"),
            r#"
[plugin]
name = "my-plugin"
version = "1.0.0"
description = "Test plugin"
"#,
        )
        .expect("write");

        let loader = PluginLoader::new();
        let results = loader.scan(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], plugin_dir);
    }

    #[test]
    fn test_load_plugin() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let plugin_dir = tmp.path().join("test-plugin");
        fs::create_dir_all(&plugin_dir).expect("mkdir");
        fs::write(
            plugin_dir.join("PLUGIN.toml"),
            r#"
[plugin]
name = "test-plugin"
version = "0.1.0"
description = "A test plugin"
"#,
        )
        .expect("write");

        let loader = PluginLoader::new();
        let plugin = loader
            .load(&plugin_dir, PluginScope::Project)
            .expect("load");

        assert_eq!(plugin.name(), "test-plugin");
        assert_eq!(plugin.version(), "0.1.0");
        assert_eq!(plugin.scope, PluginScope::Project);
    }

    #[test]
    fn test_load_plugin_with_skills() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let plugin_dir = tmp.path().join("skill-plugin");
        let skills_dir = plugin_dir.join("skills").join("my-skill");
        fs::create_dir_all(&skills_dir).expect("mkdir");

        fs::write(
            plugin_dir.join("PLUGIN.toml"),
            r#"
[plugin]
name = "skill-plugin"
version = "0.1.0"
description = "Plugin with skills"

[contributions]
skills = ["skills/"]
"#,
        )
        .expect("write");

        fs::write(
            skills_dir.join("SKILL.toml"),
            r#"
name = "my-skill"
description = "A skill from a plugin"
prompt_inline = "Do something"
"#,
        )
        .expect("write skill");

        let loader = PluginLoader::new();
        let plugin = loader.load(&plugin_dir, PluginScope::User).expect("load");

        assert_eq!(plugin.name(), "skill-plugin");
        assert_eq!(plugin.contributions.len(), 1);
        assert!(plugin.contributions[0].is_skill());
        assert_eq!(plugin.contributions[0].name(), "my-skill");
    }

    #[test]
    fn test_load_nonexistent_plugin() {
        let loader = PluginLoader::new();
        let result = loader.load(Path::new("/nonexistent"), PluginScope::Project);
        assert!(result.is_err());
    }
}
