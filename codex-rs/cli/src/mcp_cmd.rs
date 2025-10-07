use std::collections::HashMap;
use std::fs;
use std::io::IsTerminal;
use std::io::Read;
use std::path::Path;

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
use codex_rmcp_client::delete_oauth_tokens;
use codex_rmcp_client::perform_oauth_login;

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
    pub subcommand: McpSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum McpSubcommand {
    /// [experimental] List configured MCP servers.
    List(ListArgs),

    /// [experimental] Show details for a configured MCP server.
    Get(GetArgs),

    /// [experimental] Add a global MCP server entry.
    Add(AddArgs),

    /// [experimental] Remove a global MCP server entry.
    Remove(RemoveArgs),

    /// [experimental] Authenticate with a configured MCP server via OAuth.
    /// Requires experimental_use_rmcp_client = true in config.toml.
    Login(LoginArgs),

    /// [experimental] Remove stored OAuth credentials for a server.
    /// Requires experimental_use_rmcp_client = true in config.toml.
    Logout(LogoutArgs),
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

    #[command(flatten)]
    pub server_args: AddMcpServerArgs,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpServerArgs {
    #[command(flatten)]
    pub transport: AddMcpTransportArgs,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpTransportArgs {
    #[command(flatten)]
    pub stdio: AddMcpStdioArgs,

    #[command(flatten)]
    pub streamable_http: AddMcpStreamableHttpArgs,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpStdioArgs {
    /// Environment variables to set when launching the server.
    #[arg(long, value_parser = parse_env_pair, value_name = "KEY=VALUE")]
    pub env: Vec<(String, String)>,

    /// Command to launch the MCP server.
    #[arg(trailing_var_arg = true, num_args = 0..)]
    pub command: Vec<String>,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpStreamableHttpArgs {
    /// URL for a streamable HTTP MCP server.
    #[arg(long, value_name = "URL")]
    pub url: Option<String>,

    #[command(flatten)]
    pub http_auth: StreamableHttpAuthArgs,
}

#[derive(Debug, clap::Args)]
pub struct StreamableHttpAuthArgs {
    /// Optional environment variable to read for a bearer token.
    #[arg(
        long = "bearer-token-env-var",
        value_name = "ENV_VAR",
        requires = "url",
        conflicts_with = "bearer_token_stdin"
    )]
    pub bearer_token_env_var: Option<String>,

    /// Read a bearer token for the HTTP server from stdin and persist it to ~/.codex/.env.
    #[arg(
        long = "with-bearer-token",
        requires = "url",
        help = "Read the MCP bearer token from stdin (e.g. `printenv GITHUB_API_KEY | codex mcp add github --url <URL> --with-bearer-token`)",
        conflicts_with = "bearer_token_env_var"
    )]
    pub bearer_token_stdin: bool,
}

#[derive(Debug, clap::Parser)]
pub struct RemoveArgs {
    /// Name of the MCP server configuration to remove.
    pub name: String,
}

#[derive(Debug, clap::Parser)]
pub struct LoginArgs {
    /// Name of the MCP server to authenticate with oauth.
    pub name: String,
}

#[derive(Debug, clap::Parser)]
pub struct LogoutArgs {
    /// Name of the MCP server to deauthenticate.
    pub name: String,
}

impl McpCli {
    pub async fn run(self) -> Result<()> {
        let McpCli {
            config_overrides,
            subcommand,
        } = self;

        match subcommand {
            McpSubcommand::List(args) => {
                run_list(&config_overrides, args).await?;
            }
            McpSubcommand::Get(args) => {
                run_get(&config_overrides, args).await?;
            }
            McpSubcommand::Add(args) => {
                run_add(&config_overrides, args).await?;
            }
            McpSubcommand::Remove(args) => {
                run_remove(&config_overrides, args).await?;
            }
            McpSubcommand::Login(args) => {
                run_login(&config_overrides, args).await?;
            }
            McpSubcommand::Logout(args) => {
                run_logout(&config_overrides, args).await?;
            }
        }

        Ok(())
    }
}

const DEFAULT_BEARER_TOKEN_ENV_PREFIX: &str = "CODEX_MCP";

async fn run_add(config_overrides: &CliConfigOverrides, add_args: AddArgs) -> Result<()> {
    // Validate any provided overrides even though they are not currently applied.
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let AddArgs { name, server_args } = add_args;
    let AddMcpServerArgs { transport } = server_args;
    let AddMcpTransportArgs {
        stdio,
        streamable_http,
    } = transport;
    let AddMcpStdioArgs { env, command } = stdio;
    let AddMcpStreamableHttpArgs { url, http_auth } = streamable_http;

    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .await
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let new_entry = if let Some(url) = url {
        if !env.is_empty() {
            bail!("--env is not supported when adding a streamable HTTP server");
        }
        if !command.is_empty() {
            bail!("command arguments are not supported when --url is provided");
        }

        let (mut env_var_name, should_read_stdin) = match auth_kind {
            Some(StreamableHttpAuthKind::EnvVar(var)) => (Some(var), false),
            Some(StreamableHttpAuthKind::Stdin) => (None, true),
            None => (None, false),
        };
        if http_auth.bearer_token_stdin {
            let token = read_bearer_token_from_stdin()?;
            let key = env_var_name
                .take()
                .unwrap_or_else(|| derive_bearer_token_env_var(&name));
            persist_env_var(&codex_home, &key, &token)?;
            env_var_name = Some(key);
        }

        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var: env_var_name,
            },
            startup_timeout_sec: None,
            tool_timeout_sec: None,
        }
    } else {
        if let Some(choice) = auth_kind {
            match choice {
                StreamableHttpAuthKind::EnvVar(_) => {
                    bail!("--bearer-token-env-var can only be used with --url");
                }
                StreamableHttpAuthKind::Stdin => {
                    bail!("--with-bearer-token can only be used with --url");
                }
            }
        }

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

        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: command_bin,
                args: command_args,
                env: env_map,
            },
            startup_timeout_sec: None,
            tool_timeout_sec: None,
        }
    };

    servers.insert(name.clone(), new_entry);

    write_global_mcp_servers(&codex_home, &servers)
        .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;

    println!("Added global MCP server '{name}'.");

    Ok(())
}

async fn run_remove(config_overrides: &CliConfigOverrides, remove_args: RemoveArgs) -> Result<()> {
    config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;

    let RemoveArgs { name } = remove_args;

    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .await
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

async fn run_login(config_overrides: &CliConfigOverrides, login_args: LoginArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .await
        .context("failed to load configuration")?;

    if !config.use_experimental_use_rmcp_client {
        bail!(
            "OAuth login is only supported when experimental_use_rmcp_client is true in config.toml."
        );
    }

    let LoginArgs { name } = login_args;

    let Some(server) = config.mcp_servers.get(&name) else {
        bail!("No MCP server named '{name}' found.");
    };

    let url = match &server.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => url.clone(),
        _ => bail!("OAuth login is only supported for streamable HTTP servers."),
    };

    perform_oauth_login(&name, &url, config.mcp_oauth_credentials_store_mode).await?;
    println!("Successfully logged in to MCP server '{name}'.");
    Ok(())
}

async fn run_logout(config_overrides: &CliConfigOverrides, logout_args: LogoutArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .await
        .context("failed to load configuration")?;

    let LogoutArgs { name } = logout_args;

    let server = config
        .mcp_servers
        .get(&name)
        .ok_or_else(|| anyhow!("No MCP server named '{name}' found in configuration."))?;

    let url = match &server.transport {
        McpServerTransportConfig::StreamableHttp { url, .. } => url.clone(),
        _ => bail!("OAuth logout is only supported for streamable_http transports."),
    };

    match delete_oauth_tokens(&name, &url, config.mcp_oauth_credentials_store_mode) {
        Ok(true) => println!("Removed OAuth credentials for '{name}'."),
        Ok(false) => println!("No OAuth credentials stored for '{name}'."),
        Err(err) => return Err(anyhow!("failed to delete OAuth credentials: {err}")),
    }

    Ok(())
}

async fn run_list(config_overrides: &CliConfigOverrides, list_args: ListArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .await
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
                    McpServerTransportConfig::StreamableHttp {
                        url,
                        bearer_token_env_var,
                    } => {
                        serde_json::json!({
                            "type": "streamable_http",
                            "url": url,
                            "bearer_token_env_var": bearer_token_env_var,
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
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
            } => {
                http_rows.push([
                    name.clone(),
                    url.clone(),
                    bearer_token_env_var.clone().unwrap_or("-".to_string()),
                ]);
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
        let mut widths = ["Name".len(), "Url".len(), "Bearer Token Env Var".len()];
        for row in &http_rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        println!(
            "{:<name_w$}  {:<url_w$}  {:<token_w$}",
            "Name",
            "Url",
            "Bearer Token Env Var",
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

async fn run_get(config_overrides: &CliConfigOverrides, get_args: GetArgs) -> Result<()> {
    let overrides = config_overrides.parse_overrides().map_err(|e| anyhow!(e))?;
    let config = Config::load_with_cli_overrides(overrides, ConfigOverrides::default())
        .await
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
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
            } => serde_json::json!({
                "type": "streamable_http",
                "url": url,
                "bearer_token_env_var": bearer_token_env_var,
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
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
        } => {
            println!("  transport: streamable_http");
            println!("  url: {url}");
            let env_var = bearer_token_env_var.as_deref().unwrap_or("-");
            println!("  bearer_token_env_var: {env_var}");
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

fn read_bearer_token_from_stdin() -> Result<String> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        bail!(
            "--with-bearer-token expects the bearer token on stdin. Try piping it, e.g. `printenv GITHUB_API_KEY | codex mcp add <name> --url <url> --with-bearer-token`."
        );
    }

    eprintln!("Reading MCP bearer token from stdin...");
    let mut buffer = String::new();
    stdin
        .read_to_string(&mut buffer)
        .context("failed to read bearer token from stdin")?;
    let token = buffer.trim().to_string();
    if token.is_empty() {
        bail!("No bearer token provided via stdin.");
    }

    Ok(token)
}

fn derive_bearer_token_env_var(server_name: &str) -> String {
    let mut normalized = String::new();
    for ch in server_name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_uppercase());
        } else {
            normalized.push('_');
        }
    }
    format!("{DEFAULT_BEARER_TOKEN_ENV_PREFIX}_{normalized}_BEARER_TOKEN")
}

fn persist_env_var(codex_home: &Path, key: &str, value: &str) -> Result<()> {
    let dotenv_path = codex_home.join(".env");
    if let Some(parent) = dotenv_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let mut lines = Vec::new();
    let mut replaced = false;
    if let Ok(existing) = fs::read_to_string(&dotenv_path) {
        for raw_line in existing.lines() {
            if let Some((existing_key, had_export)) = parse_env_key(raw_line)
                && existing_key == key
            {
                let prefix = if had_export { "export " } else { "" };
                lines.push(format!("{prefix}{key}={value}"));
                replaced = true;
                continue;
            }
            lines.push(raw_line.to_string());
        }
    }

    if !replaced {
        lines.push(format!("{key}={value}"));
    }

    let mut contents = lines.join("\n");
    if !contents.ends_with('\n') {
        contents.push('\n');
    }

    fs::write(&dotenv_path, contents)
        .with_context(|| format!("failed to update {}", dotenv_path.display()))?;
    Ok(())
}

fn parse_env_key(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (trimmed, had_export) = if let Some(stripped) = trimmed.strip_prefix("export ") {
        (stripped, true)
    } else {
        (trimmed, false)
    };
    let (key, _) = trimmed.split_once('=')?;
    Some((key.trim().to_string(), had_export))
}
