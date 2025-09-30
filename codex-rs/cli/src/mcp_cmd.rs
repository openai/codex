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
use codex_core::config_types::McpServerTransportConfig;

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
}

#[derive(Debug, clap::Parser)]
pub struct RemoveArgs {
    /// Name of the MCP server configuration to remove.
    pub name: String,
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
        transport: McpServerTransportConfig::Stdio {
            command: command_bin,
            args: command_args,
            env: env_map,
        },
        startup_timeout_sec: None,
        tool_timeout_sec: None,
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
                let transport = match &cfg.transport {
                    McpServerTransportConfig::Stdio { command, args, env } => serde_json::json!({
                        "type": "stdio",
                        "command": command,
                        "args": args,
                        "env": env,
                    }),
                    McpServerTransportConfig::StreamableHttp { url, bearer_token } => {
                        serde_json::json!({
                            "type": "streamable_http",
                            "url": url,
                            "bearer_token": bearer_token,
                        })
                    }
                };

                serde_json::json!({
                    "name": name,
                    "transport": transport,
                    "startup_timeout_sec": cfg
                        .startup_timeout_sec
                        .map(|timeout| timeout.as_secs_f64()),
                    "tool_timeout_sec": cfg
                        .tool_timeout_sec
                        .map(|timeout| timeout.as_secs_f64()),
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

    let mut stdio_rows: Vec<[String; 4]> = Vec::new();
    let mut http_rows: Vec<[String; 3]> = Vec::new();

    for (name, cfg) in entries {
        match &cfg.transport {
            McpServerTransportConfig::Stdio { command, args, env } => {
                let args_display = if args.is_empty() {
                    "-".to_string()
                } else {
                    args.join(" ")
                };
                let env_display = match env.as_ref() {
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
                stdio_rows.push([name.clone(), command.clone(), args_display, env_display]);
            }
            McpServerTransportConfig::StreamableHttp { url, bearer_token } => {
                let has_bearer = if bearer_token.is_some() {
                    "True"
                } else {
                    "False"
                };
                http_rows.push([name.clone(), url.clone(), has_bearer.into()]);
            }
        }
    }

    if !stdio_rows.is_empty() {
        let mut widths = ["Name".len(), "Command".len(), "Args".len(), "Env".len()];
        for row in &stdio_rows {
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

        for row in &stdio_rows {
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
    }

    if !stdio_rows.is_empty() && !http_rows.is_empty() {
        println!();
    }

    if !http_rows.is_empty() {
        let mut widths = ["Name".len(), "Url".len(), "Has Bearer Token".len()];
        for row in &http_rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        println!(
            "{:<name_w$}  {:<url_w$}  {:<token_w$}",
            "Name",
            "Url",
            "Has Bearer Token",
            name_w = widths[0],
            url_w = widths[1],
            token_w = widths[2],
        );

        for row in &http_rows {
            println!(
                "{:<name_w$}  {:<url_w$}  {:<token_w$}",
                row[0],
                row[1],
                row[2],
                name_w = widths[0],
                url_w = widths[1],
                token_w = widths[2],
            );
        }
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
        let transport = match &server.transport {
            McpServerTransportConfig::Stdio { command, args, env } => serde_json::json!({
                "type": "stdio",
                "command": command,
                "args": args,
                "env": env,
            }),
            McpServerTransportConfig::StreamableHttp { url, bearer_token } => serde_json::json!({
                "type": "streamable_http",
                "url": url,
                "bearer_token": bearer_token,
            }),
        };
        let output = serde_json::to_string_pretty(&serde_json::json!({
            "name": get_args.name,
            "transport": transport,
            "startup_timeout_sec": server
                .startup_timeout_sec
                .map(|timeout| timeout.as_secs_f64()),
            "tool_timeout_sec": server
                .tool_timeout_sec
                .map(|timeout| timeout.as_secs_f64()),
        }))?;
        println!("{output}");
        return Ok(());
    }

    println!("{}", get_args.name);
    match &server.transport {
        McpServerTransportConfig::Stdio { command, args, env } => {
            println!("  transport: stdio");
            println!("  command: {command}");
            let args_display = if args.is_empty() {
                "-".to_string()
            } else {
                args.join(" ")
            };
            println!("  args: {args_display}");
            let env_display = match env.as_ref() {
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
        }
        McpServerTransportConfig::StreamableHttp { url, bearer_token } => {
            println!("  transport: streamable_http");
            println!("  url: {url}");
            let bearer = bearer_token.as_deref().unwrap_or("-");
            println!("  bearer_token: {bearer}");
        }
    }
    if let Some(timeout) = server.startup_timeout_sec {
        println!("  startup_timeout_sec: {}", timeout.as_secs_f64());
    }
    if let Some(timeout) = server.tool_timeout_sec {
        println!("  tool_timeout_sec: {}", timeout.as_secs_f64());
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
