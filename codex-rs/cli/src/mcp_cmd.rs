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
use codex_core::config_types::McpTransport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TransportArg {
    Stdio,
    Sse,
    Http,
}

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

    /// Transport to use when connecting to the MCP server.
    #[arg(long, default_value = "stdio", value_enum)]
    pub transport: TransportArg,

    /// Primary URL for HTTP/SSE transports.
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,

    /// Optional override for the URL used when sending JSON-RPC messages.
    #[arg(long = "messages-url", value_name = "URL")]
    pub messages_url: Option<String>,

    /// Extra HTTP headers to include with HTTP/SSE transports.
    #[arg(long, value_parser = parse_header_pair, value_name = "KEY=VALUE")]
    pub header: Vec<(String, String)>,

    /// Command to launch the MCP server.
    #[arg(trailing_var_arg = true, num_args = 0..)]
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

    let AddArgs {
        name,
        env,
        transport,
        url,
        messages_url,
        header,
        command,
    } = add_args;

    validate_server_name(&name)?;

    let env_map = if env.is_empty() {
        None
    } else {
        let mut map = HashMap::new();
        for (key, value) in env {
            map.insert(key, value);
        }
        Some(map)
    };

    let header_map = if header.is_empty() {
        None
    } else {
        let mut map = HashMap::new();
        for (key, value) in header {
            map.insert(key, value);
        }
        Some(map)
    };

    let transport = match transport {
        TransportArg::Stdio => McpTransport::Stdio,
        TransportArg::Sse => McpTransport::Sse,
        TransportArg::Http => McpTransport::Http,
    };
    let transport_str = transport_name(transport);

    let mut command_iter = command.into_iter();
    let (command_opt, command_args): (Option<String>, Vec<String>) = match transport {
        McpTransport::Stdio => {
            if url.is_some() {
                bail!("--url is only supported for transport=sse or transport=http");
            }
            if messages_url.is_some() {
                bail!("--messages-url is only supported for transport=sse or transport=http");
            }
            if header_map.is_some() {
                bail!("--header is only supported for transport=sse or transport=http");
            }

            let command_bin = command_iter
                .next()
                .ok_or_else(|| anyhow!("command is required when transport=stdio"))?;
            let args: Vec<String> = command_iter.collect();
            (Some(command_bin), args)
        }
        McpTransport::Sse | McpTransport::Http => {
            if command_iter.next().is_some() {
                bail!("command arguments are not supported for transport={transport_str}");
            }
            if env_map.is_some() {
                bail!("--env is only supported for transport=stdio");
            }
            if url.is_none() {
                bail!("--url is required when transport={transport_str}");
            }
            (None, Vec::new())
        }
    };

    let env_map = match transport {
        McpTransport::Stdio => env_map,
        _ => None,
    };

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let new_entry = McpServerConfig {
        transport,
        command: command_opt,
        args: command_args,
        env: env_map,
        url,
        messages_url,
        headers: header_map,
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
                let env = option_map_to_btreemap(&cfg.env);
                let headers = option_map_to_btreemap(&cfg.headers);
                serde_json::json!({
                    "name": name,
                    "transport": transport_name(cfg.transport),
                    "command": cfg.command,
                    "args": cfg.args,
                    "env": env,
                    "url": cfg.url,
                    "messages_url": cfg.messages_url,
                    "headers": headers,
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
        let transport = transport_name(cfg.transport).to_string();
        let target = match cfg.transport {
            McpTransport::Stdio => {
                let mut parts = Vec::new();
                if let Some(command) = &cfg.command {
                    parts.push(command.clone());
                }
                parts.extend(cfg.args.clone());
                if parts.is_empty() {
                    "-".to_string()
                } else {
                    parts.join(" ")
                }
            }
            _ => cfg.url.clone().unwrap_or_else(|| "-".to_string()),
        };

        let env_string = cfg.env.as_ref().filter(|m| !m.is_empty()).map(format_map);
        let headers_string = cfg
            .headers
            .as_ref()
            .filter(|m| !m.is_empty())
            .map(format_map);

        let details = match cfg.transport {
            McpTransport::Stdio => env_string.clone().unwrap_or_else(|| "-".to_string()),
            _ => {
                let mut parts = Vec::new();
                if let Some(messages_url) = &cfg.messages_url {
                    parts.push(format!("messages={messages_url}"));
                }
                if let Some(headers) = headers_string.clone() {
                    parts.push(format!("headers={headers}"));
                }
                if parts.is_empty() {
                    "-".to_string()
                } else {
                    parts.join("; ")
                }
            }
        };

        rows.push([name.clone(), transport, target, details]);
    }

    let mut widths = [
        "Name".len(),
        "Transport".len(),
        "Target".len(),
        "Details".len(),
    ];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.len());
        }
    }

    println!(
        "{:<name_w$}  {:<transport_w$}  {:<target_w$}  {:<details_w$}",
        "Name",
        "Transport",
        "Target",
        "Details",
        name_w = widths[0],
        transport_w = widths[1],
        target_w = widths[2],
        details_w = widths[3],
    );

    for row in rows {
        println!(
            "{:<name_w$}  {:<transport_w$}  {:<target_w$}  {:<details_w$}",
            row[0],
            row[1],
            row[2],
            row[3],
            name_w = widths[0],
            transport_w = widths[1],
            target_w = widths[2],
            details_w = widths[3],
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
    println!("  transport: {}", transport_name(server.transport));

    match server.transport {
        McpTransport::Stdio => {
            let command_display = server.command.as_deref().unwrap_or("-");
            println!("  command: {command_display}");
            let args = if server.args.is_empty() {
                "-".to_string()
            } else {
                server.args.join(" ")
            };
            println!("  args: {args}");
            let env_display = server
                .env
                .as_ref()
                .filter(|m| !m.is_empty())
                .map(format_map)
                .unwrap_or_else(|| "-".to_string());
            println!("  env: {env_display}");
        }
        McpTransport::Sse | McpTransport::Http => {
            let url_display = server.url.as_deref().unwrap_or("-");
            println!("  url: {url_display}");
            if let Some(messages_url) = &server.messages_url {
                println!("  messages_url: {messages_url}");
            }
            let headers_display = server
                .headers
                .as_ref()
                .filter(|m| !m.is_empty())
                .map(format_map)
                .unwrap_or_else(|| "-".to_string());
            println!("  headers: {headers_display}");
        }
    }

    if let Some(timeout) = server.startup_timeout_ms {
        println!("  startup_timeout_ms: {timeout}");
    }
    println!("  remove: codex mcp remove {}", get_args.name);

    Ok(())
}

fn parse_key_value_pair(raw: &str, kind: &str) -> Result<(String, String), String> {
    let mut parts = raw.splitn(2, '=');
    let key = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{kind} must be in KEY=VALUE form"))?;
    let value = parts
        .next()
        .map(str::to_string)
        .ok_or_else(|| format!("{kind} must be in KEY=VALUE form"))?;

    Ok((key.to_string(), value))
}

fn parse_env_pair(raw: &str) -> Result<(String, String), String> {
    parse_key_value_pair(raw, "environment entries")
}

fn parse_header_pair(raw: &str) -> Result<(String, String), String> {
    parse_key_value_pair(raw, "header entries")
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

fn transport_name(transport: McpTransport) -> &'static str {
    match transport {
        McpTransport::Stdio => "stdio",
        McpTransport::Sse => "sse",
        McpTransport::Http => "http",
    }
}

fn option_map_to_btreemap(
    map: &Option<HashMap<String, String>>,
) -> Option<BTreeMap<String, String>> {
    map.as_ref().map(|map| {
        map.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<BTreeMap<_, _>>()
    })
}

fn format_map(map: &HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = map.iter().collect();
    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}
