use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use clap::Parser;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_core::config::Config;
use codex_core::config::find_codex_home;
use codex_core::plugins_manager_for_config;
use codex_core_plugins::ConfiguredMarketplace;
use codex_core_plugins::OPENAI_BUNDLED_MARKETPLACE_NAME;
use codex_core_plugins::PluginInstallOutcome;
use codex_core_plugins::PluginInstallRequest;
use codex_core_plugins::PluginsConfigInput;
use codex_core_plugins::PluginsManager;
use codex_core_plugins::RemotePluginInstallRequest;
use codex_core_plugins::allowed_configured_marketplace_names;
use codex_core_plugins::installed_marketplaces::marketplace_install_root;
use codex_core_plugins::installed_marketplaces::resolve_configured_marketplace_root;
use codex_core_plugins::marketplace::MarketplaceListError;
use codex_core_plugins::marketplace::MarketplacePluginAuthPolicy;
use codex_core_plugins::marketplace::MarketplacePluginInstallPolicy;
use codex_core_plugins::marketplace::MarketplacePluginSource;
use codex_core_plugins::marketplace::find_marketplace_manifest_path;
use codex_core_plugins::remote;
use codex_core_plugins::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use codex_core_plugins::remote::RemoteMarketplace;
use codex_core_plugins::remote::RemoteMarketplaceSource;
use codex_core_plugins::remote::RemotePluginCatalogCacheMode;
use codex_core_plugins::remote::RemotePluginSummary;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_plugin::PluginId;
use codex_plugin::validate_plugin_segment;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

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
    /// Install a plugin from a configured or remote marketplace.
    ///
    /// Pass either `PLUGIN@MARKETPLACE` or pass `PLUGIN` with
    /// `--marketplace MARKETPLACE`.
    Add(AddPluginArgs),

    /// List plugins available from configured and remote marketplaces.
    List(ListPluginsArgs),

    /// Add, list, upgrade, or remove configured plugin marketplaces.
    Marketplace(MarketplaceCli),

    /// Uninstall a plugin and remove its local cache.
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

    /// Marketplace name to use when PLUGIN does not include @MARKETPLACE.
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
    /// Only list plugins from this marketplace name.
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
    let context = load_plugin_command_context(overrides).await?;
    let AddPluginArgs {
        plugin,
        marketplace_name,
        json,
    } = args;
    let selection = parse_plugin_selection(plugin, marketplace_name)?;
    let outcome = if selection.marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME {
        let mut listing = fetch_remote_marketplaces(
            &context,
            Some(&selection.marketplace_name),
            RemotePluginCatalogCacheMode::PreferFreshCache,
        )
        .await?;
        // A newly published plugin may be missing from an otherwise fresh catalog cache.
        if listing.catalog_cache_used
            && !listing
                .marketplaces
                .iter()
                .flat_map(|marketplace| &marketplace.plugins)
                .any(|plugin| plugin.name == selection.plugin_name)
        {
            listing = fetch_remote_marketplaces(
                &context,
                Some(&selection.marketplace_name),
                RemotePluginCatalogCacheMode::ForceRefetch,
            )
            .await?;
        }
        let plugin = resolve_remote_plugin(listing.marketplaces, &selection)?;
        context
            .manager
            .install_remote_plugin(
                &context.plugins_input,
                context.auth.as_ref(),
                RemotePluginInstallRequest {
                    marketplace_name: selection.marketplace_name,
                    remote_plugin_id: plugin.remote_plugin_id,
                    install_attempt_id: None,
                },
                /*on_effective_plugins_changed*/ None,
            )
            .await?
            .installed
    } else {
        let marketplace = find_marketplace_for_plugin(
            &context.manager,
            context.codex_home.as_path(),
            &context.plugins_input,
            &selection.marketplace_name,
            &selection.plugin_name,
        )?;
        context
            .manager
            .install_plugin(
                &context.plugins_input.config_layer_stack,
                PluginInstallRequest {
                    plugin_name: selection.plugin_name,
                    marketplace_path: marketplace.path,
                },
            )
            .await?
    };
    let output = JsonPluginAddOutput::from_outcome(outcome);

    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "Added plugin `{}` from marketplace `{}`.",
        output.name, output.marketplace_name
    );
    println!("Installed plugin root: {}", output.installed_path);

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
    let context = load_plugin_command_context(overrides).await?;
    let remote_listing = fetch_remote_marketplaces(
        &context,
        args.marketplace_name.as_deref(),
        RemotePluginCatalogCacheMode::PreferFreshCache,
    )
    .await?;
    let PluginCommandContext {
        codex_home,
        plugins_input,
        manager,
        ..
    } = context;
    let outcome = manager
        .list_marketplaces_for_config(
            &plugins_input,
            &[],
            /*include_openai_curated*/ !remote_listing.uses_global_catalog,
        )
        .context("failed to list marketplace plugins")?;
    ensure_configured_marketplace_snapshots_loaded(
        codex_home.as_path(),
        &plugins_input,
        &outcome.errors,
        args.marketplace_name.as_deref(),
    )?;

    let marketplace_sources = configured_marketplace_sources(&plugins_input, codex_home.as_path());
    let marketplaces = outcome
        .marketplaces
        .into_iter()
        .map(|marketplace| {
            let source = marketplace_sources.get(&marketplace.name).cloned();
            PluginListMarketplace {
                plugins: marketplace
                    .plugins
                    .into_iter()
                    .map(|plugin| {
                        PluginListEntry::from_configured_plugin(
                            &marketplace.name,
                            source.clone(),
                            plugin,
                        )
                    })
                    .collect(),
                name: marketplace.name,
                path: Some(marketplace.path),
            }
        })
        .chain(
            remote_listing
                .marketplaces
                .into_iter()
                .map(PluginListMarketplace::from),
        )
        .filter(|marketplace| {
            args.marketplace_name
                .as_ref()
                .is_none_or(|name| marketplace.name == *name)
        })
        .collect::<Vec<_>>();

    if args.json {
        let output = JsonPluginListOutput::from_marketplaces(marketplaces, args.available);
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

            for plugin in &marketplace.plugins {
                let state = if plugin.installed && plugin.enabled {
                    "installed, enabled"
                } else if plugin.installed {
                    "installed, disabled"
                } else {
                    "not installed"
                };
                let installed_version = plugin.display_version.clone().unwrap_or_default();
                let path = match &plugin.source {
                    JsonPluginSource::Remote { id } => id.clone(),
                    JsonPluginSource::Local { path } => path.clone(),
                    JsonPluginSource::Git { url, ref_name, sha }
                    | JsonPluginSource::GitSubdir {
                        url, ref_name, sha, ..
                    } => {
                        let mut parts = vec![url.clone()];
                        if let JsonPluginSource::GitSubdir { path, .. } = &plugin.source {
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
                    JsonPluginSource::Npm {
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
                plugin_width = plugin_width.max(plugin.plugin_id.len());
                status_width = status_width.max(state.len());
                installed_version_width = installed_version_width.max(installed_version.len());
                rows.push((plugin.plugin_id.clone(), state, installed_version, path));
            }

            if index > 0 {
                println!();
            }
            println!("Marketplace `{}`", marketplace.name);
            if let Some(path) = &marketplace.path {
                println!("{}", path.display());
            } else {
                println!("Remote catalog");
            }
            println!();
            println!(
                "{:<plugin_width$}  {:<status_width$}  {:<installed_version_width$}  SOURCE",
                "PLUGIN", "STATUS", "VERSION"
            );
            for (plugin, status, installed_version, path) in rows {
                println!(
                    "{plugin:<plugin_width$}  {status:<status_width$}  {installed_version:<installed_version_width$}  {path}"
                );
            }
        }
    }

    Ok(())
}

struct PluginListMarketplace {
    name: String,
    path: Option<AbsolutePathBuf>,
    plugins: Vec<PluginListEntry>,
}

impl From<RemoteMarketplace> for PluginListMarketplace {
    fn from(marketplace: RemoteMarketplace) -> Self {
        Self {
            plugins: marketplace
                .plugins
                .into_iter()
                .map(|plugin| {
                    let version = plugin.local_version.or(plugin.version);
                    PluginListEntry {
                        plugin_id: plugin.id,
                        name: plugin.name,
                        marketplace_name: marketplace.name.clone(),
                        display_version: version.clone(),
                        version,
                        installed: plugin.installed,
                        enabled: plugin.enabled,
                        source: JsonPluginSource::Remote {
                            id: plugin.remote_plugin_id,
                        },
                        marketplace_source: None,
                        install_policy: match plugin.install_policy {
                            PluginInstallPolicy::NotAvailable => "NOT_AVAILABLE",
                            PluginInstallPolicy::Available => "AVAILABLE",
                            PluginInstallPolicy::InstalledByDefault => "INSTALLED_BY_DEFAULT",
                        },
                        auth_policy: match plugin.auth_policy {
                            PluginAuthPolicy::OnInstall => "ON_INSTALL",
                            PluginAuthPolicy::OnUse => "ON_USE",
                        },
                    }
                })
                .collect(),
            name: marketplace.name,
            path: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonPluginListOutput {
    installed: Vec<PluginListEntry>,
    available: Vec<PluginListEntry>,
}

impl JsonPluginListOutput {
    fn from_marketplaces(
        marketplaces: Vec<PluginListMarketplace>,
        include_available: bool,
    ) -> Self {
        let mut installed = Vec::new();
        let mut available = Vec::new();

        for marketplace in marketplaces {
            for entry in marketplace.plugins {
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
struct PluginListEntry {
    plugin_id: String,
    name: String,
    marketplace_name: String,
    version: Option<String>,
    #[serde(skip)]
    display_version: Option<String>,
    installed: bool,
    enabled: bool,
    source: JsonPluginSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    marketplace_source: Option<JsonMarketplaceSource>,
    install_policy: &'static str,
    auth_policy: &'static str,
}

impl PluginListEntry {
    fn from_configured_plugin(
        marketplace_name: &str,
        marketplace_source: Option<JsonMarketplaceSource>,
        plugin: codex_core_plugins::ConfiguredMarketplacePlugin,
    ) -> Self {
        let display_version = plugin.installed_version;
        let version = display_version.clone().or(plugin.local_version);
        Self {
            plugin_id: plugin.id,
            name: plugin.name,
            marketplace_name: marketplace_name.to_string(),
            version,
            display_version,
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
    Remote {
        id: String,
    },
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
    let effective_config = plugins_input.config_layer_stack.effective_config();
    let Some(marketplaces) = effective_config
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
    let context = load_plugin_command_context(overrides).await?;
    let RemovePluginArgs {
        plugin,
        marketplace_name,
        json,
    } = args;
    let selection = parse_plugin_selection(plugin, marketplace_name)?;

    if selection.marketplace_name == REMOTE_GLOBAL_MARKETPLACE_NAME {
        ensure!(
            context.plugins_input.plugins_enabled,
            "remote plugins are not enabled"
        );
        let auth = context.auth.as_ref();
        // Installed plugins may no longer appear in the directory or curated collection.
        let marketplaces = context
            .manager
            .build_and_cache_remote_installed_plugin_marketplaces(
                &context.plugins_input,
                auth,
                &[REMOTE_GLOBAL_MARKETPLACE_NAME],
                /*on_effective_plugins_changed*/ None,
            )
            .await?;
        let plugin = resolve_remote_plugin(marketplaces, &selection)?;
        let outcome = context
            .manager
            .uninstall_remote_plugin(
                &context.plugins_input,
                auth,
                &plugin.remote_plugin_id,
                /*on_effective_plugins_changed*/ None,
            )
            .await?;
        if let Some(err) = outcome.cache_removal_error {
            return Err(err.into());
        }
    } else {
        context
            .manager
            .uninstall_plugin(selection.plugin_key.clone())
            .await?;
    }
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
    manager: Arc<PluginsManager>,
    auth: Option<CodexAuth>,
}

async fn load_plugin_command_context(
    overrides: Vec<(String, toml::Value)>,
) -> Result<PluginCommandContext> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;
    let plugins_input = config.plugins_config_input();
    let auth_manager = load_cli_auth_manager(&config).await?;
    let manager = Arc::new(plugins_manager_for_config(
        &config,
        Arc::clone(&auth_manager),
    ));
    Ok(PluginCommandContext {
        codex_home: codex_home.to_path_buf(),
        plugins_input,
        manager,
        auth: auth_manager.auth().await,
    })
}

pub(crate) async fn load_cli_auth_manager(config: &Config) -> Result<Arc<AuthManager>> {
    Ok(AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ true).await?)
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

#[derive(Default)]
struct RemoteMarketplaceListing {
    marketplaces: Vec<RemoteMarketplace>,
    // A successful global catalog replaces the local curated catalog even when it is empty.
    uses_global_catalog: bool,
    catalog_cache_used: bool,
}

async fn fetch_remote_marketplaces(
    context: &PluginCommandContext,
    marketplace_name: Option<&str>,
    cache_mode: RemotePluginCatalogCacheMode,
) -> Result<RemoteMarketplaceListing> {
    if marketplace_name.is_some_and(|name| name != REMOTE_GLOBAL_MARKETPLACE_NAME) {
        return Ok(RemoteMarketplaceListing::default());
    }
    if !context.plugins_input.plugins_enabled {
        ensure!(marketplace_name.is_none(), "remote plugins are not enabled");
        return Ok(RemoteMarketplaceListing::default());
    }
    let auth = context.auth.as_ref();
    if !auth.is_some_and(CodexAuth::uses_codex_backend) {
        ensure!(
            marketplace_name.is_none(),
            "chatgpt authentication required for remote plugin catalog"
        );
        return Ok(RemoteMarketplaceListing::default());
    }
    let service = context.plugins_input.remote_plugin_service_config();
    let result = if context.plugins_input.remote_plugin_enabled {
        remote::fetch_remote_marketplaces(
            &service,
            auth,
            &[RemoteMarketplaceSource::Global],
            /*catalog_cache_root*/ Some(context.codex_home.as_path()),
            cache_mode,
        )
        .await
        .map(|outcome| RemoteMarketplaceListing {
            marketplaces: outcome.marketplaces,
            uses_global_catalog: true,
            catalog_cache_used: outcome.catalog_cache_used,
        })
    } else {
        remote::fetch_openai_curated_remote_collection_marketplace(
            &service,
            auth,
            /*catalog_cache_root*/ Some(context.codex_home.as_path()),
            cache_mode,
        )
        .await
        .map(|outcome| RemoteMarketplaceListing {
            marketplaces: outcome.marketplace.into_iter().collect(),
            uses_global_catalog: false,
            catalog_cache_used: outcome.catalog_cache_used,
        })
    };
    match result {
        Ok(listing) => Ok(listing),
        Err(err) if marketplace_name.is_none() => {
            eprintln!("Warning: failed to list remote marketplace plugins: {err}");
            Ok(RemoteMarketplaceListing::default())
        }
        Err(err) => Err(err).context("failed to list remote marketplace plugins"),
    }
}

fn resolve_remote_plugin(
    marketplaces: Vec<RemoteMarketplace>,
    selection: &PluginSelection,
) -> Result<RemotePluginSummary> {
    let matches = marketplaces
        .into_iter()
        .flat_map(|marketplace| marketplace.plugins)
        .filter(|plugin| plugin.name == selection.plugin_name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [plugin] => Ok(plugin.clone()),
        [] => bail!(
            "plugin `{}` was not found in remote marketplace `{}`",
            selection.plugin_name,
            selection.marketplace_name
        ),
        _ => bail!(
            "plugin `{}` matched multiple remote plugins",
            selection.plugin_key
        ),
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
    let effective_config = plugins_input.config_layer_stack.effective_config();
    let Some(configured_marketplaces) = effective_config
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
