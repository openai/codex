//! Plugin slash command extension.
//!
//! Handles /plugin subcommands: install, uninstall, enable, disable, list, validate, update.

use codex_plugin::cli::MarketplaceCommand;
use codex_plugin::cli::PluginCommand;
use codex_plugin::cli::parse_command;
use codex_plugin::installer::PluginSource;
use codex_plugin::registry::InstallScope;
use std::path::PathBuf;

/// Result of a plugin command execution.
#[derive(Debug)]
pub enum PluginCommandResult {
    /// Success message to display.
    Success(String),
    /// List of plugins to display.
    List(Vec<PluginListEntry>),
    /// Help text to display.
    Help(String),
    /// Error message to display.
    Error(String),
}

/// Entry in plugin list display.
#[derive(Debug, Clone)]
pub struct PluginListEntry {
    pub id: String,
    pub version: Option<String>,
    pub scope: String,
    pub enabled: bool,
}

/// Plugin manager context for TUI operations.
pub struct PluginManagerContext {
    pub codex_home: PathBuf,
    pub project_path: Option<PathBuf>,
}

impl PluginManagerContext {
    pub fn new(codex_home: PathBuf, project_path: Option<PathBuf>) -> Self {
        Self {
            codex_home,
            project_path,
        }
    }
}

/// Handle a plugin command string (without the leading /plugin).
///
/// Returns a result that can be displayed to the user.
pub async fn handle_plugin_command(args: &str, ctx: &PluginManagerContext) -> PluginCommandResult {
    let cmd = match parse_command(args) {
        Ok(c) => c,
        Err(e) => return PluginCommandResult::Error(format!("Invalid command: {e}")),
    };

    match cmd {
        PluginCommand::Install {
            source,
            marketplace,
            scope,
            force: _,
        } => {
            let mp = marketplace.unwrap_or_else(|| "user".to_string());
            handle_install(&source, &mp, scope, ctx).await
        }
        PluginCommand::Uninstall { plugin_id, scope } => {
            handle_uninstall(&plugin_id, scope, ctx).await
        }
        PluginCommand::Enable { plugin_id } => handle_enable(&plugin_id, ctx).await,
        PluginCommand::Disable { plugin_id } => handle_disable(&plugin_id, ctx).await,
        PluginCommand::List { scope, verbose: _ } => handle_list(scope, ctx).await,
        PluginCommand::Validate { path } => {
            let path_ref = path.as_ref().map(|p| p.as_path());
            handle_validate(path_ref, ctx).await
        }
        PluginCommand::Update { plugin_id, scope } => handle_update(&plugin_id, scope, ctx).await,
        PluginCommand::Marketplace(mp_cmd) => handle_marketplace(mp_cmd, ctx).await,
        PluginCommand::Help => PluginCommandResult::Help(help_text()),
    }
}

async fn handle_install(
    source_str: &str,
    marketplace: &str,
    scope: InstallScope,
    ctx: &PluginManagerContext,
) -> PluginCommandResult {
    let source = match PluginSource::parse(source_str) {
        Ok(s) => s,
        Err(e) => return PluginCommandResult::Error(format!("Invalid source: {e}")),
    };

    let registry = codex_plugin::registry::PluginRegistryV2::new(&ctx.codex_home);
    if let Err(e) = registry.load().await {
        return PluginCommandResult::Error(format!("Failed to load registry: {e}"));
    }
    let registry = std::sync::Arc::new(registry);

    let cache_dir = ctx.codex_home.join("plugins").join("cache");
    let installer =
        codex_plugin::installer::PluginInstaller::new(registry, cache_dir, &ctx.codex_home);

    let project_path = ctx.project_path.as_deref();

    match installer
        .install(&source, marketplace, scope, project_path)
        .await
    {
        Ok(entry) => {
            let version = entry.version.unwrap_or_else(|| "unknown".to_string());
            PluginCommandResult::Success(format!(
                "Installed plugin (version {version}) at scope {scope}"
            ))
        }
        Err(e) => PluginCommandResult::Error(format!("Install failed: {e}")),
    }
}

async fn handle_uninstall(
    plugin_id: &str,
    scope: Option<InstallScope>,
    ctx: &PluginManagerContext,
) -> PluginCommandResult {
    let scope = scope.unwrap_or(InstallScope::User);

    let registry = codex_plugin::registry::PluginRegistryV2::new(&ctx.codex_home);
    if let Err(e) = registry.load().await {
        return PluginCommandResult::Error(format!("Failed to load registry: {e}"));
    }
    let registry = std::sync::Arc::new(registry);

    let cache_dir = ctx.codex_home.join("plugins").join("cache");
    let installer =
        codex_plugin::installer::PluginInstaller::new(registry, cache_dir, &ctx.codex_home);

    let project_path = ctx.project_path.as_deref();

    match installer.uninstall(plugin_id, scope, project_path).await {
        Ok(()) => {
            PluginCommandResult::Success(format!("Uninstalled {plugin_id} from scope {scope}"))
        }
        Err(e) => PluginCommandResult::Error(format!("Uninstall failed: {e}")),
    }
}

async fn handle_enable(plugin_id: &str, ctx: &PluginManagerContext) -> PluginCommandResult {
    let settings = codex_plugin::settings::PluginSettings::new(&ctx.codex_home);
    settings.enable(plugin_id).await;

    match settings.save().await {
        Ok(()) => PluginCommandResult::Success(format!("Enabled plugin: {plugin_id}")),
        Err(e) => PluginCommandResult::Error(format!("Failed to save settings: {e}")),
    }
}

async fn handle_disable(plugin_id: &str, ctx: &PluginManagerContext) -> PluginCommandResult {
    let settings = codex_plugin::settings::PluginSettings::new(&ctx.codex_home);
    settings.disable(plugin_id).await;

    match settings.save().await {
        Ok(()) => PluginCommandResult::Success(format!("Disabled plugin: {plugin_id}")),
        Err(e) => PluginCommandResult::Error(format!("Failed to save settings: {e}")),
    }
}

async fn handle_list(
    scope: Option<InstallScope>,
    ctx: &PluginManagerContext,
) -> PluginCommandResult {
    let registry = codex_plugin::registry::PluginRegistryV2::new(&ctx.codex_home);
    if let Err(e) = registry.load().await {
        return PluginCommandResult::Error(format!("Failed to load registry: {e}"));
    }

    let settings = codex_plugin::settings::PluginSettings::new(&ctx.codex_home);

    let plugins = registry.list(scope).await;

    let mut entries = Vec::new();
    for (plugin_id, entry) in plugins {
        let enabled = settings.is_enabled(&plugin_id).await;
        entries.push(PluginListEntry {
            id: plugin_id,
            version: entry.version,
            scope: entry.scope.to_string(),
            enabled,
        });
    }

    PluginCommandResult::List(entries)
}

async fn handle_validate(
    path: Option<&std::path::Path>,
    ctx: &PluginManagerContext,
) -> PluginCommandResult {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => ctx
            .project_path
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
    };

    let registry = codex_plugin::registry::PluginRegistryV2::new(&ctx.codex_home);
    if let Err(e) = registry.load().await {
        return PluginCommandResult::Error(format!("Failed to load registry: {e}"));
    }
    let registry = std::sync::Arc::new(registry);

    let cache_dir = ctx.codex_home.join("plugins").join("cache");
    let installer =
        codex_plugin::installer::PluginInstaller::new(registry, cache_dir, &ctx.codex_home);

    match installer.validate(&path).await {
        Ok(manifest) => PluginCommandResult::Success(format!(
            "Valid plugin: {} (version {})",
            manifest.name,
            manifest.version.unwrap_or_else(|| "unknown".to_string())
        )),
        Err(e) => PluginCommandResult::Error(format!("Validation failed: {e}")),
    }
}

async fn handle_update(
    plugin_id: &str,
    scope: Option<InstallScope>,
    ctx: &PluginManagerContext,
) -> PluginCommandResult {
    let scope = scope.unwrap_or(InstallScope::User);

    let registry = codex_plugin::registry::PluginRegistryV2::new(&ctx.codex_home);
    if let Err(e) = registry.load().await {
        return PluginCommandResult::Error(format!("Failed to load registry: {e}"));
    }
    let registry = std::sync::Arc::new(registry);

    let cache_dir = ctx.codex_home.join("plugins").join("cache");
    let installer =
        codex_plugin::installer::PluginInstaller::new(registry, cache_dir, &ctx.codex_home);

    let project_path = ctx.project_path.as_deref();

    match installer.update(plugin_id, scope, project_path).await {
        Ok(entry) => {
            let version = entry.version.unwrap_or_else(|| "unknown".to_string());
            PluginCommandResult::Success(format!("Updated {plugin_id} to version {version}"))
        }
        Err(e) => PluginCommandResult::Error(format!("Update failed: {e}")),
    }
}

async fn handle_marketplace(
    cmd: MarketplaceCommand,
    ctx: &PluginManagerContext,
) -> PluginCommandResult {
    let registry = codex_plugin::registry::PluginRegistryV2::new(&ctx.codex_home);
    if let Err(e) = registry.load().await {
        return PluginCommandResult::Error(format!("Failed to load registry: {e}"));
    }
    let registry = std::sync::Arc::new(registry);

    let manager = codex_plugin::marketplace::MarketplaceManager::new(registry, &ctx.codex_home);
    let _ = manager.load().await; // Ignore error if no marketplaces exist

    match cmd {
        MarketplaceCommand::Add { name, source } => {
            // Parse source string to MarketplaceSource
            let mp_source = parse_marketplace_source(&source);

            match manager.add(&name, mp_source).await {
                Ok(()) => {
                    if let Err(e) = manager.save().await {
                        return PluginCommandResult::Error(format!(
                            "Added but failed to save: {e}"
                        ));
                    }
                    PluginCommandResult::Success(format!("Added marketplace: {name}"))
                }
                Err(e) => PluginCommandResult::Error(format!("Failed to add marketplace: {e}")),
            }
        }
        MarketplaceCommand::Remove { name } => match manager.remove(&name).await {
            Ok(()) => {
                if let Err(e) = manager.save().await {
                    return PluginCommandResult::Error(format!("Removed but failed to save: {e}"));
                }
                PluginCommandResult::Success(format!("Removed marketplace: {name}"))
            }
            Err(e) => PluginCommandResult::Error(format!("Failed to remove marketplace: {e}")),
        },
        MarketplaceCommand::List => {
            let marketplaces = manager.list().await;

            if marketplaces.is_empty() {
                PluginCommandResult::Success("No marketplaces registered".to_string())
            } else {
                let list = marketplaces
                    .iter()
                    .map(|(name, entry)| format_marketplace_entry(name, entry))
                    .collect::<Vec<_>>()
                    .join("\n");
                PluginCommandResult::Success(format!("Registered marketplaces:\n{list}"))
            }
        }
        MarketplaceCommand::Update { name } => {
            if let Some(mp_name) = name {
                // Update specific marketplace
                match manager.update(&mp_name).await {
                    Ok(manifest) => {
                        if let Err(e) = manager.save().await {
                            return PluginCommandResult::Error(format!(
                                "Updated but failed to save: {e}"
                            ));
                        }
                        PluginCommandResult::Success(format!(
                            "Updated {mp_name}: {} plugins available",
                            manifest.plugins.len()
                        ))
                    }
                    Err(e) => {
                        PluginCommandResult::Error(format!("Failed to update {mp_name}: {e}"))
                    }
                }
            } else {
                // Update all marketplaces
                let results = manager.update_all().await;
                if let Err(e) = manager.save().await {
                    return PluginCommandResult::Error(format!("Updated but failed to save: {e}"));
                }

                let mut success_count = 0;
                let mut errors = Vec::new();
                for (mp_name, result) in results {
                    match result {
                        Ok(manifest) => {
                            success_count += 1;
                            tracing::info!(
                                "Updated {}: {} plugins",
                                mp_name,
                                manifest.plugins.len()
                            );
                        }
                        Err(e) => {
                            errors.push(format!("{mp_name}: {e}"));
                        }
                    }
                }

                if errors.is_empty() {
                    PluginCommandResult::Success(format!("Updated {success_count} marketplace(s)"))
                } else {
                    PluginCommandResult::Error(format!(
                        "Updated {success_count}, failed: {}",
                        errors.join("; ")
                    ))
                }
            }
        }
    }
}

/// Parse a source string into MarketplaceSource.
fn parse_marketplace_source(source: &str) -> codex_plugin::marketplace::MarketplaceSource {
    use codex_plugin::marketplace::MarketplaceSource;

    // GitHub shorthand: owner/repo
    if source.contains('/') && !source.contains("://") && !source.starts_with('.') {
        return MarketplaceSource::GitHub {
            repo: source.to_string(),
            ref_spec: None,
            path: None,
        };
    }

    // Git URL
    if source.ends_with(".git") {
        return MarketplaceSource::Git {
            url: source.to_string(),
            ref_spec: None,
            path: None,
        };
    }

    // Local file/directory
    if source.starts_with("./") || source.starts_with('/') || source.starts_with("..") {
        let path = std::path::Path::new(source);
        if path.is_file() {
            return MarketplaceSource::File {
                path: source.to_string(),
            };
        } else {
            return MarketplaceSource::Directory {
                path: source.to_string(),
            };
        }
    }

    // Default to URL
    MarketplaceSource::Url {
        url: source.to_string(),
        headers: std::collections::HashMap::new(),
    }
}

/// Format a marketplace entry for display.
fn format_marketplace_entry(
    name: &str,
    entry: &codex_plugin::marketplace::MarketplaceEntry,
) -> String {
    use codex_plugin::marketplace::MarketplaceSource;

    let source_str = match &entry.source {
        MarketplaceSource::Url { url, .. } => format!("url: {url}"),
        MarketplaceSource::GitHub { repo, .. } => format!("github: {repo}"),
        MarketplaceSource::Git { url, .. } => format!("git: {url}"),
        MarketplaceSource::File { path } => format!("file: {path}"),
        MarketplaceSource::Directory { path } => format!("dir: {path}"),
    };

    let status = if entry.enabled { "enabled" } else { "disabled" };
    let updated = entry
        .last_updated
        .as_ref()
        .map(|t| format!(", updated: {t}"))
        .unwrap_or_default();

    format!("  - {name} ({source_str}) [{status}{updated}]")
}

/// Help text for plugin commands.
pub fn help_text() -> String {
    r#"Plugin Management Commands:

  /plugin install <source> [--marketplace <name>] [--scope <scope>]
      Install a plugin from source (github:owner/repo, npm:package, ./path)

  /plugin uninstall <plugin-id> [--scope <scope>]
      Uninstall a plugin

  /plugin enable <plugin-id>
      Enable a disabled plugin

  /plugin disable <plugin-id>
      Disable a plugin (keeps files)

  /plugin list [--scope <scope>]
      List installed plugins

  /plugin validate [<path>]
      Validate a plugin directory

  /plugin update <plugin-id> [--scope <scope>]
      Update a plugin to latest version

  /plugin marketplace list
      List registered marketplaces

Scopes: user, project, managed

Examples:
  /plugin install github:owner/my-plugin
  /plugin install npm:@scope/my-plugin --scope user
  /plugin install ./local/plugin --scope project
  /plugin list
  /plugin enable my-plugin@marketplace
"#
    .to_string()
}

/// Format plugin list for display.
pub fn format_plugin_list(entries: &[PluginListEntry]) -> String {
    if entries.is_empty() {
        return "No plugins installed.".to_string();
    }

    let mut lines = vec!["Installed plugins:".to_string()];
    for entry in entries {
        let status = if entry.enabled { "enabled" } else { "disabled" };
        let version = entry.version.as_deref().unwrap_or("?");
        lines.push(format!(
            "  {} (v{}) [{}, {}]",
            entry.id, version, entry.scope, status
        ));
    }
    lines.join("\n")
}
