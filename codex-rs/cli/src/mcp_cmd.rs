use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_cli::mcp::cli::WizardArgs;
use codex_cli::mcp::wizard::WizardOutcome;
use codex_cli::mcp::wizard::build_non_interactive as build_wizard_non_interactive;
use codex_cli::mcp::wizard::confirm_apply as wizard_confirm_apply;
use codex_cli::mcp::wizard::render_json_summary as wizard_render_json;
use codex_cli::mcp::wizard::run_interactive as run_wizard_interactive;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_global_mcp_servers;
use codex_core::config::migrations::mcp::MigrationOptions;
use codex_core::config::migrations::mcp::{self};
use codex_core::config::write_global_mcp_servers;
use codex_core::config_types::McpServerConfig;
use codex_core::mcp::registry::McpRegistry;
use codex_core::mcp::registry::validate_server_name;
use codex_core::mcp::templates::TemplateCatalog;

/// [experimental] Launch Codex as an MCP server or manage configured MCP servers.
///
/// Subcommands:
/// - `serve`  — run the MCP server on stdio
/// - `list`   — list configured servers (with `--json`)
/// - `get`    — show a single server (with `--json`)
/// - `add`    — add a server launcher entry to `~/.codex/config.toml`
/// - `remove` — delete a server entry
#[derive(Debug, clap::Parser)]
pub struct McpCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub cmd: Option<McpSubcommand>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, clap::Subcommand)]
pub enum McpSubcommand {
    /// [experimental] Run the Codex MCP server (stdio transport).
    Serve,

    /// [experimental] List configured MCP servers.
    List(ListArgs),

    /// [experimental] Show details for a configured MCP server.
    Get(GetArgs),

    /// [experimental] Add a global MCP server entry.
    Add(AddArgs),

    /// [experimental] Remove a global MCP server entry.
    Remove(RemoveArgs),

    /// [experimental] Migrate MCP configuration to the latest schema.
    Migrate(MigrateArgs),

    /// [experimental] Launch the MCP configuration wizard (preview).
    Wizard(WizardArgs),
}

#[derive(Debug, clap::Parser)]
pub struct ListArgs {
    /// Output the configured servers as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct GetArgs {
    /// Name of the MCP server to display.
    pub name: String,

    /// Output the server configuration as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct AddArgs {
    /// Name for the MCP server configuration.
    pub name: String,

    /// Environment variables to set when launching the server.
    #[arg(long, value_parser = parse_env_pair, value_name = "KEY=VALUE")]
    pub env: Vec<(String, String)>,

    /// Command to launch the MCP server.
    #[arg(trailing_var_arg = true, num_args = 1..)]
    pub command: Vec<String>,
}

#[derive(Debug, clap::Parser)]
pub struct RemoveArgs {
    /// Name of the MCP server configuration to remove.
    pub name: String,
}

#[derive(Debug, clap::Parser)]
pub struct MigrateArgs {
    /// Apply migration changes instead of performing a dry-run preview.
    #[arg(long, default_value_t = false)]
    pub apply: bool,

    /// Migrate even when the schema version is already up-to-date.
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

impl McpCli {
    pub async fn run(self, codex_linux_sandbox_exe: Option<PathBuf>) -> Result<()> {
        let McpCli {
            config_overrides,
            cmd,
        } = self;
        let subcommand = cmd.unwrap_or(McpSubcommand::Serve);

        match subcommand {
            McpSubcommand::Serve => {
                codex_mcp_server::run_main(codex_linux_sandbox_exe, config_overrides).await?;
            }
            McpSubcommand::List(args) => {
                run_list(&config_overrides, args)?;
            }
            McpSubcommand::Get(args) => {
                run_get(&config_overrides, args)?;
            }
            McpSubcommand::Add(args) => {
                run_add(&config_overrides, args)?;
            }
            McpSubcommand::Remove(args) => {
                run_remove(&config_overrides, args)?;
            }
            McpSubcommand::Migrate(args) => {
                run_migrate(&config_overrides, args)?;
            }
            McpSubcommand::Wizard(args) => {
                run_wizard(&config_overrides, args)?;
            }
        }

        Ok(())
    }
}

fn run_add(config_overrides: &CliConfigOverrides, add_args: AddArgs) -> Result<()> {
    // Validate any provided overrides even though they are not currently applied.
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let AddArgs { name, env, command } = add_args;

    validate_server_name(&name)?;

    let mut command_parts = command.into_iter();
    let command_bin = command_parts
        .next()
        .ok_or_else(|| anyhow!("command is required"))?;
    let command_args: Vec<String> = command_parts.collect();

    let env_map = if env.is_empty() {
        None
    } else {
        let mut map = HashMap::new();
        for (key, value) in env {
            map.insert(key, value);
        }
        Some(map)
    };

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let new_entry = McpServerConfig {
        command: command_bin,
        args: command_args,
        env: env_map,
        startup_timeout_ms: None,
        ..McpServerConfig::default()
    };

    servers.insert(name.clone(), new_entry);

    write_global_mcp_servers(&codex_home, &servers)
        .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;

    println!("Added global MCP server '{name}'.");

    Ok(())
}

fn run_remove(config_overrides: &CliConfigOverrides, remove_args: RemoveArgs) -> Result<()> {
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let RemoveArgs { name } = remove_args;

    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let removed = servers.remove(&name).is_some();

    if removed {
        write_global_mcp_servers(&codex_home, &servers)
            .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;
    }

    if removed {
        println!("Removed global MCP server '{name}'.");
    } else {
        println!("No MCP server named '{name}' found.");
    }

    Ok(())
}

fn run_migrate(config_overrides: &CliConfigOverrides, args: MigrateArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    if !config.experimental_mcp_overhaul && !args.force {
        bail!(
            "MCP overhaul features are disabled. Enable `experimental.mcp_overhaul=true` or rerun with --force."
        );
    }

    let options = MigrationOptions {
        dry_run: !args.apply,
        force: args.force,
    };

    let report = mcp::migrate_to_v2(&config.codex_home, &options).with_context(|| {
        format!(
            "failed to migrate configuration at {}",
            config.codex_home.display()
        )
    })?;

    if options.dry_run {
        println!(
            "Dry run complete (from schema v{} → v{}). Changes detected: {}",
            report.from_version, report.to_version, report.changes_detected
        );
    } else {
        println!(
            "Migration applied (schema v{} → v{}). Backup created: {}",
            report.from_version, report.to_version, report.backed_up
        );
    }

    if !report.notes.is_empty() {
        for note in report.notes {
            println!("• {note}");
        }
    }

    Ok(())
}

fn run_wizard(config_overrides: &CliConfigOverrides, args: WizardArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    if !config.experimental_mcp_overhaul {
        bail!(
            "MCP overhaul features are disabled. Enable `experimental.mcp_overhaul=true` to use the wizard."
        );
    }

    let templates = TemplateCatalog::load_default().unwrap_or_else(|err| {
        tracing::warn!("Failed to load MCP templates: {err}");
        TemplateCatalog::empty()
    });
    let registry = McpRegistry::new(&config, templates.clone());

    let has_non_interactive_inputs = args.name.is_some()
        || args.command.is_some()
        || !args.args.is_empty()
        || !args.env.is_empty()
        || args.startup_timeout_ms.is_some()
        || args.description.is_some()
        || !args.tags.is_empty()
        || args.auth_type.is_some()
        || args.auth_secret_ref.is_some()
        || !args.auth_env.is_empty()
        || args.health_type.is_some()
        || args.health_command.is_some()
        || !args.health_args.is_empty()
        || args.health_timeout_ms.is_some()
        || args.health_interval_seconds.is_some()
        || args.health_endpoint.is_some()
        || args.health_protocol.is_some();

    if args.json {
        if has_non_interactive_inputs {
            let outcome = build_wizard_non_interactive(&registry, &args)?;
            println!("{}", wizard_render_json(&outcome)?);
        } else {
            let summary = serde_json::json!({
                "experimental_overhaul": registry.experimental_enabled(),
                "server_count": registry.servers().count(),
                "template_ids": templates
                    .templates()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
                "preselected_template": args.template,
            });
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        return Ok(());
    }

    let outcome = if has_non_interactive_inputs {
        build_wizard_non_interactive(&registry, &args)?
    } else {
        run_wizard_interactive(&registry, args.template.as_deref())?
    };

    let mut applied = false;
    let mut summary_shown = false;

    if args.apply {
        print_wizard_summary(&outcome);
        summary_shown = true;
        registry
            .upsert_server(&outcome.name, outcome.server.clone())
            .context("failed to persist MCP server")?;
        applied = true;
    } else if wizard_confirm_apply(&outcome)? {
        summary_shown = true;
        registry
            .upsert_server(&outcome.name, outcome.server.clone())
            .context("failed to persist MCP server")?;
        applied = true;
    }

    if applied {
        println!(
            "Saved server '{name}' to {path}",
            name = outcome.name,
            path = registry.codex_home().display()
        );
        if !summary_shown {
            print_wizard_summary(&outcome);
        }
    } else {
        println!("No changes saved.");
    }

    Ok(())
}

fn print_wizard_summary(outcome: &WizardOutcome) {
    println!("Configuration summary:");
    for (key, value) in outcome.summary() {
        println!("  {key}: {value}");
    }
}

fn run_list(config_overrides: &CliConfigOverrides, list_args: ListArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    let templates = TemplateCatalog::load_default().unwrap_or_else(|err| {
        tracing::warn!("Failed to load MCP templates: {err}");
        TemplateCatalog::empty()
    });
    let registry = McpRegistry::new(&config, templates);

    let mut entries: Vec<_> = registry.servers().collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));

    if list_args.json {
        let json_entries: Vec<_> = entries
            .into_iter()
            .map(|(name, cfg)| {
                let env = cfg.env.as_ref().map(|env| {
                    env.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<BTreeMap<_, _>>()
                });
                serde_json::json!({
                    "name": name,
                    "display_name": cfg.display_name,
                    "category": cfg.category,
                    "template_id": cfg.template_id,
                    "description": cfg.description,
                    "command": cfg.command,
                    "args": cfg.args,
                    "env": env,
                    "startup_timeout_ms": cfg.startup_timeout_ms,
                    "auth": cfg.auth,
                    "healthcheck": cfg.healthcheck,
                    "tags": cfg.tags,
                    "created_at": cfg.created_at,
                    "last_verified_at": cfg.last_verified_at,
                    "metadata": cfg.metadata,
                    "experimental_overhaul": registry.experimental_enabled(),
                })
            })
            .collect();
        let output = serde_json::to_string_pretty(&json_entries)?;
        println!("{output}");
        return Ok(());
    }

    if entries.is_empty() {
        println!("No MCP servers configured yet. Try `codex mcp add my-tool -- my-command`.");
        return Ok(());
    }

    let mut rows: Vec<[String; 4]> = Vec::new();
    for (name, cfg) in entries {
        let args = if cfg.args.is_empty() {
            "-".to_string()
        } else {
            cfg.args.join(" ")
        };

        let env = match cfg.env.as_ref() {
            None => "-".to_string(),
            Some(map) if map.is_empty() => "-".to_string(),
            Some(map) => {
                let mut pairs: Vec<_> = map.iter().collect();
                pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                pairs
                    .into_iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };

        rows.push([name.clone(), cfg.command.clone(), args, env]);
    }

    let mut widths = ["Name".len(), "Command".len(), "Args".len(), "Env".len()];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    println!(
        "{:<name_w$}  {:<cmd_w$}  {:<args_w$}  {:<env_w$}",
        "Name",
        "Command",
        "Args",
        "Env",
        name_w = widths[0],
        cmd_w = widths[1],
        args_w = widths[2],
        env_w = widths[3],
    );

    for row in rows {
        println!(
            "{:<name_w$}  {:<cmd_w$}  {:<args_w$}  {:<env_w$}",
            row[0],
            row[1],
            row[2],
            row[3],
            name_w = widths[0],
            cmd_w = widths[1],
            args_w = widths[2],
            env_w = widths[3],
        );
    }

    Ok(())
}

fn run_get(config_overrides: &CliConfigOverrides, get_args: GetArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    let templates = TemplateCatalog::load_default().unwrap_or_else(|err| {
        tracing::warn!("Failed to load MCP templates: {err}");
        TemplateCatalog::empty()
    });
    let registry = McpRegistry::new(&config, templates);

    let Some(server) = registry.server(&get_args.name) else {
        bail!("No MCP server named '{name}' found.", name = get_args.name);
    };

    if get_args.json {
        let env = server.env.as_ref().map(|env| {
            env.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>()
        });
        let output = serde_json::to_string_pretty(&serde_json::json!({
            "name": get_args.name,
            "command": server.command,
            "args": server.args,
            "env": env,
            "startup_timeout_ms": server.startup_timeout_ms,
            "display_name": server.display_name,
            "category": server.category,
            "template_id": server.template_id,
            "description": server.description,
            "auth": server.auth,
            "healthcheck": server.healthcheck,
            "tags": server.tags,
            "created_at": server.created_at,
            "last_verified_at": server.last_verified_at,
            "metadata": server.metadata,
            "experimental_overhaul": registry.experimental_enabled(),
        }))?;
        println!("{output}");
        return Ok(());
    };

    println!("{}", get_args.name);
    println!("  command: {}", server.command);
    let args = if server.args.is_empty() {
        "-".to_string()
    } else {
        server.args.join(" ")
    };
    println!("  args: {args}");

    match server.env.as_ref() {
        None => println!("  env: -"),
        Some(map) if map.is_empty() => println!("  env: -"),
        Some(map) => {
            let mut pairs: Vec<_> = map.iter().collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (k, v) in pairs {
                println!("  env: {k}={v}");
            }
        }
    }

    if let Some(timeout) = server.startup_timeout_ms {
        println!("  startup_timeout_ms: {timeout}");
    }
    if let Some(display_name) = &server.display_name {
        println!("  display_name: {display_name}");
    }
    if let Some(category) = &server.category {
        println!("  category: {category}");
    }
    if let Some(template_id) = &server.template_id {
        println!("  template_id: {template_id}");
    }
    if let Some(description) = &server.description {
        println!("  description: {description}");
    }

    println!("  remove: codex mcp remove {}", get_args.name);

    Ok(())
}

fn parse_env_pair(raw: &str) -> Result<(String, String), String> {
    let mut parts = raw.splitn(2, '=');
    let key = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "environment entries must be in KEY=VALUE form".to_string())?;
    let value = parts
        .next()
        .map(str::to_string)
        .ok_or_else(|| "environment entries must be in KEY=VALUE form".to_string())?;

    Ok((key.to_string(), value))
}
