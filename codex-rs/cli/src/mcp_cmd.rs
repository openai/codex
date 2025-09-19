use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_global_mcp_servers;
use codex_core::config::write_global_mcp_servers;
use codex_core::config_types::McpServerConfig;
use codex_core::git_info::resolve_root_git_project_for_trust;

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

    /// Write this server to the project's `.codex/config.toml` instead of global config.
    #[arg(long)]
    pub project: bool,
}

#[derive(Debug, clap::Parser)]
pub struct RemoveArgs {
    /// Name of the MCP server configuration to remove.
    pub name: String,

    /// Remove from the project's `.codex/config.toml` instead of global config.
    #[arg(long)]
    pub project: bool,
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
        }

        Ok(())
    }
}

fn run_add(config_overrides: &CliConfigOverrides, add_args: AddArgs) -> Result<()> {
    // Validate any provided overrides even though they are not currently applied.
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let AddArgs {
        name,
        env,
        command,
        project,
    } = add_args;

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

    if project {
        let cwd = std::env::current_dir().context("failed to get current directory")?;
        let project_root = resolve_root_git_project_for_trust(&cwd).unwrap_or(cwd);
        add_project_mcp_server(
            &project_root,
            &name,
            McpServerConfig {
                command: command_bin,
                args: command_args,
                env: env_map,
                startup_timeout_ms: None,
            },
        )?;
        println!(
            "Added project MCP server '{name}' in {}/.codex.",
            project_root.display()
        );
        return Ok(());
    }

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let new_entry = McpServerConfig {
        command: command_bin,
        args: command_args,
        env: env_map,
        startup_timeout_ms: None,
    };

    servers.insert(name.clone(), new_entry);

    write_global_mcp_servers(&codex_home, &servers)
        .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;

    println!("Added global MCP server '{name}'.");

    Ok(())
}

fn run_remove(config_overrides: &CliConfigOverrides, remove_args: RemoveArgs) -> Result<()> {
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let RemoveArgs { name, project } = remove_args;

    validate_server_name(&name)?;

    if project {
        let cwd = std::env::current_dir().context("failed to get current directory")?;
        let project_root = resolve_root_git_project_for_trust(&cwd).unwrap_or(cwd);
        let removed = remove_project_mcp_server(&project_root, &name)?;
        if removed {
            println!(
                "Removed project MCP server '{name}' in {}/.codex.",
                project_root.display()
            );
        } else {
            println!("No MCP server named '{name}' found.");
        }
        return Ok(());
    }

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

fn ensure_project_codex_path(project_root: &std::path::Path) -> Result<std::path::PathBuf> {
    let codex_dir = project_root.join(".codex");
    std::fs::create_dir_all(&codex_dir)
        .with_context(|| format!("failed to create {}", codex_dir.display()))?;
    Ok(codex_dir.join("config.toml"))
}

fn add_project_mcp_server(
    project_root: &std::path::Path,
    name: &str,
    entry: McpServerConfig,
) -> Result<()> {
    use toml_edit::{Array as TomlArray, DocumentMut, Item as TomlItem, Table as TomlTable, value};

    let path = ensure_project_codex_path(project_root)?;
    let mut doc = match std::fs::read_to_string(&path) {
        Ok(contents) => contents.parse::<DocumentMut>().map_err(|e| anyhow!(e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(e) => return Err(anyhow!(e)),
    };

    if !doc.as_table().contains_key("mcp_servers") || doc["mcp_servers"].as_table().is_none() {
        let mut table = TomlTable::new();
        table.set_implicit(true);
        doc["mcp_servers"] = TomlItem::Table(table);
    }

    let mut entry_tbl = TomlTable::new();
    entry_tbl.set_implicit(false);
    entry_tbl["command"] = value(entry.command);

    if !entry.args.is_empty() {
        let mut args = TomlArray::new();
        for a in entry.args {
            args.push(a);
        }
        entry_tbl["args"] = TomlItem::Value(args.into());
    }

    if let Some(env) = entry.env {
        if !env.is_empty() {
            let mut env_tbl = TomlTable::new();
            env_tbl.set_implicit(false);
            let mut pairs: Vec<_> = env.into_iter().collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(&b));
            for (k, v) in pairs {
                env_tbl.insert(&k, value(v));
            }
            entry_tbl["env"] = TomlItem::Table(env_tbl);
        }
    }

    if let Some(timeout) = entry.startup_timeout_ms {
        let timeout = i64::try_from(timeout).context("startup_timeout_ms too large")?;
        entry_tbl["startup_timeout_ms"] = value(timeout);
    }

    doc["mcp_servers"][name] = TomlItem::Table(entry_tbl);
    std::fs::write(&path, doc.to_string())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn remove_project_mcp_server(project_root: &std::path::Path, name: &str) -> Result<bool> {
    use toml_edit::DocumentMut;
    let path = project_root.join(".codex").join("config.toml");
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(anyhow!(e)),
    };

    let mut doc = contents.parse::<DocumentMut>().map_err(|e| anyhow!(e))?;
    let Some(mcp_tbl) = doc["mcp_servers"].as_table_mut() else {
        return Ok(false);
    };
    let removed = mcp_tbl.remove(name).is_some();
    if removed {
        std::fs::write(&path, doc.to_string())
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(removed)
}

fn run_list(config_overrides: &CliConfigOverrides, list_args: ListArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .context("failed to load configuration")?;

    let mut entries: Vec<_> = config.mcp_servers.iter().collect();
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
                    "command": cfg.command,
                    "args": cfg.args,
                    "env": env,
                    "startup_timeout_ms": cfg.startup_timeout_ms,
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

    let Some(server) = config.mcp_servers.get(&get_args.name) else {
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
        }))?;
        println!("{output}");
        return Ok(());
    }

    println!("{}", get_args.name);
    println!("  command: {}", server.command);
    let args = if server.args.is_empty() {
        "-".to_string()
    } else {
        server.args.join(" ")
    };
    println!("  args: {args}");
    let env_display = match server.env.as_ref() {
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
    println!("  env: {env_display}");
    if let Some(timeout) = server.startup_timeout_ms {
        println!("  startup_timeout_ms: {timeout}");
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

fn validate_server_name(name: &str) -> Result<()> {
    let is_valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if is_valid {
        Ok(())
    } else {
        bail!("invalid server name '{name}' (use letters, numbers, '-', '_')");
    }
}
