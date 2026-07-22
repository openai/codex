use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core_plugins::ConfiguredMarketplace;
use codex_core_plugins::OPENAI_BUNDLED_MARKETPLACE_NAME;
use codex_core_plugins::PluginInstallOutcome;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsConfigInput;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::allowed_configured_marketplace_names;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_core_plugins::installed_marketplaces::resolve_configured_marketplace_root;
use codex_core_plugins::marketplace::MarketplaceListError;
use codex_core_plugins::marketplace::MarketplacePluginAuthPolicy;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_core_plugins::marketplace::MarketplacePluginSource;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_login::CodexAuth;
use codex_login::auth::read_codex_api_key_from_env;
use codex_plugin::PluginId;
use codex_plugin::validate_plugin_segment;
use codex_protocol::auth::AuthMode;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::marketplace_cmd::MarketplaceCli;

const OPENAI_BUNDLED_ALPHA_MARKETPLACE_NAME: &str = "openai-bundled-alpha";
const OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME: &str = "openai-primary-runtime";

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin")]
pub struct PluginCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: PluginSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum PluginSubcommand {
    /// Install a plugin from a configured marketplace snapshot.
    ///
    /// Pass either `PLUGIN@MARKETPLACE` or pass `PLUGIN` with
    /// `--marketplace MARKETPLACE`.
    Add(AddPluginArgs),

    /// List plugins available from configured marketplace snapshots.
    List(ListPluginsArgs),

    /// Add, list, upgrade, or remove configured plugin marketplaces.
    Marketplace(MarketplaceCli),

    /// Remove an installed plugin from local config and cache.
    ///
    /// Pass either `PLUGIN@MARKETPLACE` or pass `PLUGIN` with
    /// `--marketplace MARKETPLACE`.
    Remove(RemovePluginArgs),
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin add",
    after_help = "Examples:\n  codex plugin add sample@debug\n  codex plugin add sample --marketplace debug"
)]
pub struct AddPluginArgs {
    /// Plugin selector to install: either PLUGIN@MARKETPLACE or PLUGIN with --marketplace.
    #[arg(value_name = "PLUGIN[@MARKETPLACE]")]
    plugin: String,

    /// Configured marketplace name to use when PLUGIN does not include @MARKETPLACE.
    #[arg(long = "marketplace", short = 'm', value_name = "MARKETPLACE")]
    marketplace_name: Option<String>,

    /// Output install result as JSON.
    #[arg(long = "json")]
    json: bool,
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin list",
    after_help = "Examples:\n  codex plugin list\n  codex plugin list --marketplace debug\n  codex plugin list --json\n  codex plugin list --available --json"
)]
pub struct ListPluginsArgs {
    /// Only list plugins from this configured marketplace name.
    #[arg(long = "marketplace", short = 'm', value_name = "MARKETPLACE")]
    marketplace_name: Option<String>,

    /// Output plugin list as JSON.
    #[arg(long = "json")]
    json: bool,

    /// Include uninstalled marketplace plugins in the JSON output.
    #[arg(long = "available", requires = "json")]
    available: bool,
}

#[derive(Debug, Parser)]
#[command(
    bin_name = "codex plugin remove",
    after_help = "Examples:\n  codex plugin remove sample@debug\n  codex plugin remove sample --marketplace debug"
)]
pub struct RemovePluginArgs {
    /// Plugin selector to remove: either PLUGIN@MARKETPLACE or PLUGIN with --marketplace.
    #[arg(value_name = "PLUGIN[@MARKETPLACE]")]
    plugin: String,

    /// Marketplace name to use when PLUGIN does not include @MARKETPLACE.
    #[arg(long = "marketplace", short = 'm', value_name = "MARKETPLACE")]
    marketplace_name: Option<String>,

    /// Output remove result as JSON.
    #[arg(long = "json")]
    json: bool,
}

pub async fn run_plugin_add(
    overrides: Vec<(String, toml::Value)>,
    args: AddPluginArgs,
) -> Result<()> {
    let PluginCommandContext {
        codex_home,
        plugins_input,
        manager,
    } = load_plugin_command_context(overrides).await?;
    let AddPluginArgs {
        plugin,
        marketplace_name,
        json,
    } = args;
    let PluginSelection {
        plugin_name,
        marketplace_name,
        ..
    } = parse_plugin_selection(plugin, marketplace_name)?;
    let marketplace = find_marketplace_for_plugin(
        &manager,
        codex_home.as_path(),
        &plugins_input,
        &marketplace_name,
        &plugin_name,
    )?;
    let outcome = manager
        .install_plugin(
            &plugins_input.config_layer_stack,
            PluginInstallRequest {
                plugin_name,
                marketplace_path: marketplace.path,
            },
        )
        .await?;

    if json {
        let output = JsonPluginAddOutput::from_outcome(outcome);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "Added plugin `{}` from marketplace `{}`.",
        outcome.plugin_id.plugin_name, outcome.plugin_id.marketplace_name
    );
    println!(
        "Installed plugin root: {}",
        outcome.installed_path.as_path().display()
    );

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonPluginAddOutput {
    plugin_id: String,
    name: String,
    marketplace_name: String,
    version: String,
    installed_path: String,
    auth_policy: &'static str,
}

impl JsonPluginAddOutput {
    fn from_outcome(outcome: PluginInstallOutcome) -> Self {
        Self {
            plugin_id: outcome.plugin_id.as_key(),
            name: outcome.plugin_id.plugin_name,
            marketplace_name: outcome.plugin_id.marketplace_name,
            version: outcome.plugin_version,
            installed_path: outcome.installed_path.as_path().display().to_string(),
            auth_policy: auth_policy_label(outcome.auth_policy),
        }
    }
}

pub async fn run_plugin_list(
    overrides: Vec<(String, toml::Value)>,
    args: ListPluginsArgs,
) -> Result<()> {
    let PluginCommandContext {
        codex_home,
        plugins_input,
        manager,
        ..
    } = load_plugin_command_context(overrides).await?;
    let outcome = manager
        .list_marketplaces_for_config(&plugins_input, &[], /*include_openai_curated*/ true)
        .context("failed to list marketplace plugins")?;
    ensure_configured_marketplace_snapshots_loaded(
        codex_home.as_path(),
        &plugins_input,
        &outcome.errors,
        /*marketplace_name*/ None,
    )?;

    let marketplaces = outcome
        .marketplaces
        .into_iter()
        .filter(|marketplace| {
            args.marketplace_name
                .as_ref()
                .is_none_or(|name| marketplace.name == *name)
        })
        .collect::<Vec<_>>();
    let marketplace_sources = configured_marketplace_sources(&plugins_input, codex_home.as_path());

    if args.json {
        let output = JsonPluginListOutput::from_marketplaces(
            marketplaces,
            args.available,
            &marketplace_sources,
        );
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if marketplaces.is_empty() {
        if let Some(marketplace_name) = args.marketplace_name {
            println!("No plugins found in marketplace `{marketplace_name}`.");
        } else {
            println!("No marketplace plugins found.");
        }
    } else {
        for (index, marketplace) in marketplaces.into_iter().enumerate() {
            let mut rows = Vec::new();
            let mut plugin_width = "PLUGIN".len();
            let mut status_width = "STATUS".len();
            let mut installed_version_width = "VERSION".len();
            let mut path_width = "PATH".len();

            for plugin in &marketplace.plugins {
                let state = if plugin.installed && plugin.enabled {
                    "installed, enabled"
                } else if plugin.installed {
                    "installed, disabled"
                } else {
                    "not installed"
                };
                let installed_version = plugin.installed_version.clone().unwrap_or_default();
                let path = match &plugin.source {
                    codex_core_plugins::marketplace::MarketplacePluginSource::Local { path } => {
                        path.as_path().display().to_string()
                    }
                    codex_core_plugins::marketplace::MarketplacePluginSource::Git {
                        url,
                        path,
                        ref_name,
                        sha,
                    } => {
                        let mut parts = vec![url.clone()];
                        if let Some(path) = path {
                            parts.push(format!("path `{path}`"));
                        }
                        if let Some(ref_name) = ref_name {
                            parts.push(format!("ref `{ref_name}`"));
                        }
                        if let Some(sha) = sha {
                            parts.push(format!("sha `{sha}`"));
                        }
                        parts.join(", ")
                    }
                    codex_core_plugins::marketplace::MarketplacePluginSource::Npm {
                        package,
                        version,
                        registry,
                    } => {
                        let mut parts = vec![package.clone()];
                        if let Some(version) = version {
                            parts.push(format!("version `{version}`"));
                        }
                        if let Some(registry) = registry {
                            parts.push(format!("registry `{registry}`"));
                        }
                        parts.join(", ")
                    }
                };
                plugin_width = plugin_width.max(plugin.id.len());
                status_width = status_width.max(state.len());
                installed_version_width = installed_version_width.max(installed_version.len());
                path_width = path_width.max(path.len());
                rows.push((plugin.id.clone(), state, installed_version, path));
            }

            if index > 0 {
                println!();
            }
            println!("Marketplace `{}`", marketplace.name);
            println!("{}", marketplace.path.as_path().display());
            println!();
            println!(
                "{:<plugin_width$}  {:<status_width$}  {:<installed_version_width$}  {:<path_width$}",
                "PLUGIN", "STATUS", "VERSION", "PATH"
            );
            for (plugin, status, installed_version, path) in rows {
                println!(
                    "{plugin:<plugin_width$}  {status:<status_width$}  {installed_version:<installed_version_width$}  {path:<path_width$}"
                );
            }
        }
    }

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonPluginListOutput {
    installed: Vec<JsonPluginListEntry>,
    available: Vec<JsonPluginListEntry>,
}

impl JsonPluginListOutput {
    fn from_marketplaces(
        marketplaces: Vec<codex_core_plugins::ConfiguredMarketplace>,
        include_available: bool,
        marketplace_sources: &HashMap<String, JsonMarketplaceSource>,
    ) -> Self {
        let mut installed = Vec::new();
        let mut available = Vec::new();

        for marketplace in marketplaces {
            let marketplace_source = marketplace_sources.get(&marketplace.name).cloned();
            for plugin in marketplace.plugins {
                let entry = JsonPluginListEntry::from_configured_plugin(
                    &marketplace.name,
                    marketplace_source.clone(),
                    plugin,
                );
                if entry.installed {
                    installed.push(entry);
                } else if include_available {
                    available.push(entry);
                }
            }
        }

        Self {
            installed,
            available,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonPluginListEntry {
    plugin_id: String,
    name: String,
    marketplace_name: String,
    version: Option<String>,
    installed: bool,
    enabled: bool,
    source: JsonPluginSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    marketplace_source: Option<JsonMarketplaceSource>,
    install_policy: &'static str,
    auth_policy: &'static str,
}

impl JsonPluginListEntry {
    fn from_configured_plugin(
        marketplace_name: &str,
        marketplace_source: Option<JsonMarketplaceSource>,
        plugin: codex_core_plugins::ConfiguredMarketplacePlugin,
    ) -> Self {
        let version = plugin.installed_version.or(plugin.local_version);
        Self {
            plugin_id: plugin.id,
            name: plugin.name,
            marketplace_name: marketplace_name.to_string(),
            version,
            installed: plugin.installed,
            enabled: plugin.enabled,
            source: JsonPluginSource::from_marketplace_source(plugin.source),
            marketplace_source,
            install_policy: install_policy_label(plugin.policy.installation),
            auth_policy: auth_policy_label(plugin.policy.authentication),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
enum JsonPluginSource {
    Local {
        path: String,
    },
    Git {
        url: String,
        #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
        ref_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sha: Option<String>,
    },
    GitSubdir {
        url: String,
        path: String,
        #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
        ref_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sha: Option<String>,
    },
    Npm {
        package: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        registry: Option<String>,
    },
}

impl JsonPluginSource {
    fn from_marketplace_source(source: MarketplacePluginSource) -> Self {
        match source {
            MarketplacePluginSource::Local { path } => Self::Local {
                path: path.as_path().display().to_string(),
            },
            MarketplacePluginSource::Git {
                url,
                path: Some(path),
                ref_name,
                sha,
            } => Self::GitSubdir {
                url,
                path,
                ref_name,
                sha,
            },
            MarketplacePluginSource::Git {
                url,
                path: None,
                ref_name,
                sha,
            } => Self::Git { url, ref_name, sha },
            MarketplacePluginSource::Npm {
                package,
                version,
                registry,
            } => Self::Npm {
                package,
                version,
                registry,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonMarketplaceSource {
    source_type: String,
    source: String,
}

pub(crate) fn configured_marketplace_sources(
    plugins_input: &PluginsConfigInput,
    codex_home: &Path,
) -> HashMap<String, JsonMarketplaceSource> {
    let Some(user_config) = plugins_input.config_layer_stack.effective_user_config() else {
        return HashMap::new();
    };
    let Some(marketplaces) = user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
    else {
        return HashMap::new();
    };
    let allowed_marketplace_names =
        allowed_configured_marketplace_names(&plugins_input.config_layer_stack, codex_home);

    marketplaces
        .iter()
        .filter(|(marketplace_name, _)| allowed_marketplace_names.contains(*marketplace_name))
        .filter_map(|(marketplace_name, marketplace)| {
            let source_type = marketplace
                .get("source_type")
                .and_then(toml::Value::as_str)?;
            let source = marketplace.get("source").and_then(toml::Value::as_str)?;
            Some((
                marketplace_name.clone(),
                JsonMarketplaceSource {
                    source_type: source_type.to_string(),
                    source: source.to_string(),
                },
            ))
        })
        .collect()
}

fn install_policy_label(policy: MarketplacePluginInstallPolicy) -> &'static str {
    match policy {
        MarketplacePluginInstallPolicy::NotAvailable => "NOT_AVAILABLE",
        MarketplacePluginInstallPolicy::Available => "AVAILABLE",
        MarketplacePluginInstallPolicy::InstalledByDefault => "INSTALLED_BY_DEFAULT",
    }
}

fn auth_policy_label(policy: MarketplacePluginAuthPolicy) -> &'static str {
    match policy {
        MarketplacePluginAuthPolicy::OnInstall => "ON_INSTALL",
        MarketplacePluginAuthPolicy::OnUse => "ON_USE",
    }
}

pub async fn run_plugin_remove(
    overrides: Vec<(String, toml::Value)>,
    args: RemovePluginArgs,
) -> Result<()> {
    let PluginCommandContext { manager, .. } = load_plugin_command_context(overrides).await?;
    let RemovePluginArgs {
        plugin,
        marketplace_name,
        json,
    } = args;
    let selection = parse_plugin_selection(plugin, marketplace_name)?;

    manager
        .uninstall_plugin(selection.plugin_key.clone())
        .await?;
    if json {
        let output = JsonPluginRemoveOutput::from_selection(selection);
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "Removed plugin `{}` from marketplace `{}`.",
        selection.plugin_name, selection.marketplace_name
    );

    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonPluginRemoveOutput {
    plugin_id: String,
    name: String,
    marketplace_name: String,
}

impl JsonPluginRemoveOutput {
    fn from_selection(selection: PluginSelection) -> Self {
        Self {
            plugin_id: selection.plugin_key,
            name: selection.plugin_name,
            marketplace_name: selection.marketplace_name,
        }
    }
}

struct PluginCommandContext {
    codex_home: PathBuf,
    plugins_input: PluginsConfigInput,
    manager: PluginsManager,
}

async fn load_plugin_command_context(
    overrides: Vec<(String, toml::Value)>,
) -> Result<PluginCommandContext> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let plugins_input = config.plugins_config_input();
    let manager = PluginsManager::new(codex_home.to_path_buf());
    manager.set_auth_mode(load_cli_auth_mode(&config).await);
    Ok(PluginCommandContext {
        codex_home: codex_home.to_path_buf(),
        plugins_input,
        manager,
    })
}

pub(crate) async fn load_cli_auth_mode(config: &Config) -> Option<AuthMode> {
    if let Some(api_key) = read_codex_api_key_from_env() {
        return Some(CodexAuth::from_api_key(&api_key).api_auth_mode());
    }

    let auth_route_config = config.auth_route_config();
    CodexAuth::from_auth_storage(
        &config.codex_home,
        config.cli_auth_credentials_store_mode,
        Some(&config.chatgpt_base_url),
        config.auth_keyring_backend_kind(),
        &auth_route_config,
    )
    .await
    .ok()
    .flatten()
    .map(|auth| auth.api_auth_mode())
}

struct PluginSelection {
    plugin_name: String,
    marketplace_name: String,
    plugin_key: String,
}

impl PluginSelection {
    fn from_plugin_id(plugin_id: PluginId) -> Self {
        let plugin_key = plugin_id.as_key();
        Self {
            plugin_name: plugin_id.plugin_name,
            marketplace_name: plugin_id.marketplace_name,
            plugin_key,
        }
    }
}

fn parse_plugin_selection(
    plugin: String,
    marketplace_name: Option<String>,
) -> Result<PluginSelection> {
    match (PluginId::parse(&plugin), marketplace_name) {
        (Ok(plugin_id), None) => Ok(PluginSelection::from_plugin_id(plugin_id)),
        (Ok(plugin_id), Some(marketplace_name)) => {
            if plugin_id.marketplace_name != marketplace_name {
                bail!(
                    "plugin id `{}` belongs to marketplace `{}`, but --marketplace specified `{}`",
                    plugin,
                    plugin_id.marketplace_name,
                    marketplace_name
                );
            }
            Ok(PluginSelection::from_plugin_id(plugin_id))
        }
        (Err(_), Some(marketplace_name)) => Ok(PluginSelection::from_plugin_id(PluginId::new(
            plugin,
            marketplace_name,
        )?)),
        (Err(_), None) => {
            bail!("plugin requires --marketplace unless passed as <plugin>@<marketplace>")
        }
    }
}

fn find_marketplace_for_plugin(
    manager: &PluginsManager,
    codex_home: &std::path::Path,
    plugins_input: &PluginsConfigInput,
    marketplace_name: &str,
    plugin_name: &str,
) -> Result<ConfiguredMarketplace> {
    let outcome = manager
        .list_marketplaces_for_config(plugins_input, &[], /*include_openai_curated*/ true)
        .context("failed to list marketplace plugins")?;
    ensure_configured_marketplace_snapshots_loaded(
        codex_home,
        plugins_input,
        &outcome.errors,
        Some(marketplace_name),
    )?;
    let matches = outcome
        .marketplaces
        .into_iter()
        .filter(|marketplace| marketplace.name == marketplace_name)
        .filter(|marketplace| {
            marketplace
                .plugins
                .iter()
                .any(|plugin| plugin.name == plugin_name)
        })
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => bail!("plugin `{plugin_name}` was not found in marketplace `{marketplace_name}`"),
        [marketplace] => Ok(marketplace.clone()),
        _ => bail!(
            "plugin `{plugin_name}` in marketplace `{marketplace_name}` matched multiple marketplace roots"
        ),
    }
}

pub(crate) struct ConfiguredMarketplaceSnapshotIssue {
    pub(crate) marketplace_name: String,
    pub(crate) path: PathBuf,
    pub(crate) message: String,
}

fn ensure_configured_marketplace_snapshots_loaded(
    codex_home: &std::path::Path,
    plugins_input: &PluginsConfigInput,
    load_errors: &[MarketplaceListError],
    marketplace_name: Option<&str>,
) -> Result<()> {
    let issues = configured_marketplace_snapshot_issues(
        codex_home,
        plugins_input,
        load_errors,
        marketplace_name,
    );
    if issues.is_empty() {
        return Ok(());
    }

    let issue_lines = issues
        .iter()
        .map(|issue| {
            format!(
                "- `{}` at {}: {}",
                issue.marketplace_name,
                issue.path.display(),
                issue.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    bail!("failed to load configured marketplace snapshot(s):\n{issue_lines}");
}

pub(crate) fn configured_marketplace_snapshot_issues(
    codex_home: &std::path::Path,
    plugins_input: &PluginsConfigInput,
    load_errors: &[MarketplaceListError],
    marketplace_name: Option<&str>,
) -> Vec<ConfiguredMarketplaceSnapshotIssue> {
    let Some(user_config) = plugins_input.config_layer_stack.effective_user_config() else {
        return Vec::new();
    };
    let Some(configured_marketplaces) = user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
    else {
        return Vec::new();
    };
    let allowed_marketplace_names =
        allowed_configured_marketplace_names(&plugins_input.config_layer_stack, codex_home);

    let default_install_root = marketplace_install_root(codex_home);
    let mut manifest_paths = Vec::new();
    let mut issues = Vec::new();
    for (configured_name, marketplace) in configured_marketplaces {
        if !allowed_marketplace_names.contains(configured_name) {
            continue;
        }
        if marketplace_name.is_some_and(|name| configured_name != name) {
            continue;
        }
        if !marketplace.is_table() {
            issues.push(ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: configured_name.clone(),
                path: PathBuf::from("<invalid config>"),
                message: "configured marketplace entry must be a table".to_string(),
            });
            continue;
        }
        if let Err(err) = validate_plugin_segment(configured_name, "marketplace name") {
            issues.push(ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: configured_name.clone(),
                path: PathBuf::from("<invalid config>"),
                message: err.to_string(),
            });
            continue;
        }
        if marketplace.get("source_type").and_then(toml::Value::as_str) == Some("local")
            && marketplace
                .get("source")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
        {
            issues.push(ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: configured_name.clone(),
                path: PathBuf::from("<invalid source>"),
                message: "configured local marketplace source is missing or empty".to_string(),
            });
            continue;
        }
        let Some(root) = resolve_configured_marketplace_root(
            configured_name,
            marketplace,
            &default_install_root,
        ) else {
            continue;
        };
        match find_marketplace_manifest_path(&root) {
            Some(path) => manifest_paths.push((configured_name.clone(), path)),
            None => {
                if is_implicit_system_marketplace_root(configured_name, codex_home, &root) {
                    continue;
                }
                issues.push(ConfiguredMarketplaceSnapshotIssue {
                    marketplace_name: configured_name.clone(),
                    path: root,
                    message: "marketplace root does not contain a supported manifest".to_string(),
                });
            }
        }
    }

    for error in load_errors {
        if let Some((configured_name, _)) = manifest_paths
            .iter()
            .find(|(_, path)| path.as_path() == error.path.as_path())
        {
            issues.push(ConfiguredMarketplaceSnapshotIssue {
                marketplace_name: configured_name.clone(),
                path: error.path.to_path_buf(),
                message: error.message.clone(),
            });
        }
    }
    issues
}

fn is_implicit_system_marketplace_root(
    marketplace_name: &str,
    _codex_home: &Path,
    root: &Path,
) -> bool {
    if matches!(
        marketplace_name,
        OPENAI_BUNDLED_MARKETPLACE_NAME | OPENAI_BUNDLED_ALPHA_MARKETPLACE_NAME
    ) && path_ends_with(root, &[".tmp", "bundled-marketplaces", marketplace_name])
    {
        return true;
    }

    marketplace_name == OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME
        && path_ends_with(
            root,
            &[
                "codex-runtimes",
                "codex-primary-runtime",
                "plugins",
                marketplace_name,
            ],
        )
}

fn path_ends_with(path: &Path, suffix: &[&str]) -> bool {
    let path_components = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    path_components.as_slice().ends_with(
        &suffix
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>(),
    )
}
