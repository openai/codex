use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use clap::ArgGroup;
use codex_core::config::Config;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::find_codex_home;
use codex_core::config::load_global_mcp_servers;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::mcp::auth::McpOAuthLoginSupport;
use codex_core::mcp::auth::compute_auth_statuses;
use codex_core::mcp::auth::oauth_login_support;
use codex_core::protocol::McpAuthStatus;
use codex_rmcp_client::delete_oauth_tokens;
use codex_rmcp_client::perform_oauth_login;
use codex_utils_cli::CliConfigOverrides;
use codex_utils_cli::format_env_display::format_env_display;

/// Subcommands:
/// - `list`   — list configured servers (with `--json`)
/// - `get`    — show a single server (with `--json`)
/// - `add`    — add a server launcher entry to `~/.codex/config.toml`
/// - `remove` — delete a server entry
/// - `login`  — authenticate with MCP server using OAuth
/// - `logout` — remove OAuth credentials for MCP server
#[derive(Debug, clap::Parser)]
pub struct McpCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: McpSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum McpSubcommand {
    List(ListArgs),
    Get(GetArgs),
    Add(AddArgs),
    Remove(RemoveArgs),
    Login(LoginArgs),
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
#[command(override_usage = "codex mcp add [OPTIONS] <NAME> (--url <URL> | -- <COMMAND>...)")]
pub struct AddArgs {
    /// Name for the MCP server configuration.
    pub name: String,

    #[command(flatten)]
    pub transport_args: AddMcpTransportArgs,
}

#[derive(Debug, clap::Args)]
#[command(
    group(
        ArgGroup::new("transport")
            .args(["command", "url"])
            .required(true)
            .multiple(false)
    )
)]
pub struct AddMcpTransportArgs {
    #[command(flatten)]
    pub stdio: Option<AddMcpStdioArgs>,

    #[command(flatten)]
    pub streamable_http: Option<AddMcpStreamableHttpArgs>,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpStdioArgs {
    /// Command to launch the MCP server.
    /// Use --url for a streamable HTTP server.
    #[arg(
            trailing_var_arg = true,
            num_args = 0..,
        )]
    pub command: Vec<String>,

    /// Environment variables to set when launching the server.
    /// Only valid with stdio servers.
    #[arg(
        long,
        value_parser = parse_env_pair,
        value_name = "KEY=VALUE",
    )]
    pub env: Vec<(String, String)>,
}

#[derive(Debug, clap::Args)]
pub struct AddMcpStreamableHttpArgs {
    /// URL for a streamable HTTP MCP server.
    #[arg(long)]
    pub url: String,

    /// Optional environment variable to read for a bearer token.
    /// Only valid with streamable HTTP servers.
    #[arg(
        long = "bearer-token-env-var",
        value_name = "ENV_VAR",
        requires = "url"
    )]
    pub bearer_token_env_var: Option<String>,
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

    /// Comma-separated list of OAuth scopes to request.
    #[arg(long, value_delimiter = ',', value_name = "SCOPE,SCOPE")]
    pub scopes: Vec<String>,
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

async fn run_add(config_overrides: &CliConfigOverrides, add_args: AddArgs) -> Result<()> {
    // Validate any provided overrides even though they are not currently applied.
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let AddArgs {
        name,
        transport_args,
    } = add_args;

    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .await
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let transport = match transport_args {
        AddMcpTransportArgs {
            stdio: Some(stdio), ..
        } => {
            let mut command_parts = stdio.command.into_iter();
            let command_bin = command_parts
                .next()
                .ok_or_else(|| anyhow!("command is required"))?;
            let command_args: Vec<String> = command_parts.collect();

            let env_map = if stdio.env.is_empty() {
                None
            } else {
                Some(stdio.env.into_iter().collect::<HashMap<_, _>>())
            };
            McpServerTransportConfig::Stdio {
                command: command_bin,
                args: command_args,
                env: env_map,
                env_vars: Vec::new(),
                cwd: None,
            }
        }
        AddMcpTransportArgs {
            streamable_http:
                Some(AddMcpStreamableHttpArgs {
                    url,
                    bearer_token_env_var,
                }),
            ..
        } => McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers: None,
            env_http_headers: None,
        },
        AddMcpTransportArgs { .. } => bail!("exactly one of --command or --url must be provided"),
    };

    let new_entry = McpServerConfig {
        transport: transport.clone(),
        enabled: true,
        required: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
    };

    servers.insert(name.clone(), new_entry);

    ConfigEditsBuilder::new(&codex_home)
        .replace_mcp_servers(&servers)
        .apply()
        .await
        .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;

    println!("Added global MCP server '{name}'.");

    match oauth_login_support(&transport).await {
        McpOAuthLoginSupport::Supported(oauth_config) => {
            println!("Detected OAuth support. Starting OAuth flow…");
            perform_oauth_login(
                &name,
                &oauth_config.url,
                config.mcp_oauth_credentials_store_mode,
                oauth_config.http_headers,
                oauth_config.env_http_headers,
                &Vec::new(),
                config.mcp_oauth_callback_port,
            )
            .await?;
            println!("Successfully logged in.");
        }
        McpOAuthLoginSupport::Unsupported => {}
        McpOAuthLoginSupport::Unknown(_) => println!(
            "MCP server may or may not require login. Run `codex mcp login {name}` to login."
        ),
    }

    Ok(())
}

async fn run_remove(config_overrides: &CliConfigOverrides, remove_args: RemoveArgs) -> Result<()> {
    config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;

    let RemoveArgs { name } = remove_args;

    validate_server_name(&name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let mut servers = load_global_mcp_servers(&codex_home)
        .await
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let removed = servers.remove(&name).is_some();

    if removed {
        ConfigEditsBuilder::new(&codex_home)
            .replace_mcp_servers(&servers)
            .apply()
            .await
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
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let LoginArgs { name, scopes } = login_args;

    let Some(server) = config.mcp_servers.get().get(&name) else {
        bail!("No MCP server named '{name}' found.");
    };

    let (url, http_headers, env_http_headers) = match &server.transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            http_headers,
            env_http_headers,
            ..
        } => (url.clone(), http_headers.clone(), env_http_headers.clone()),
        _ => bail!("OAuth login is only supported for streamable HTTP servers."),
    };

    let mut scopes = scopes;
    if scopes.is_empty() {
        scopes = server.scopes.clone().unwrap_or_default();
    }

    perform_oauth_login(
        &name,
        &url,
        config.mcp_oauth_credentials_store_mode,
        http_headers,
        env_http_headers,
        &scopes,
        config.mcp_oauth_callback_port,
    )
    .await?;
    println!("Successfully logged in to MCP server '{name}'.");
    Ok(())
}

async fn run_logout(config_overrides: &CliConfigOverrides, logout_args: LogoutArgs) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let LogoutArgs { name } = logout_args;

    let server = config
        .mcp_servers
        .get()
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
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let mut entries: Vec<_> = config.mcp_servers.iter().collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    let auth_statuses = compute_auth_statuses(
        config.mcp_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
    )
    .await;

    if list_args.json {
        let json_entries: Vec<_> = entries
            .into_iter()
            .map(|(name, cfg)| {
                let auth_status = auth_statuses
                    .get(name.as_str())
                    .map(|entry| entry.auth_status)
                    .unwrap_or(McpAuthStatus::Unsupported);
                let transport = match &cfg.transport {
                    McpServerTransportConfig::Stdio {
                        command,
                        args,
                        env,
                        env_vars,
                        cwd,
                    } => serde_json::json!({
                        "type": "stdio",
                        "command": command,
                        "args": args,
                        "env": env,
                        "env_vars": env_vars,
                        "cwd": cwd,
                    }),
                    McpServerTransportConfig::StreamableHttp {
                        url,
                        bearer_token_env_var,
                        http_headers,
                        env_http_headers,
                    } => {
                        serde_json::json!({
                            "type": "streamable_http",
                            "url": url,
                            "bearer_token_env_var": bearer_token_env_var,
                            "http_headers": http_headers,
                            "env_http_headers": env_http_headers,
                        })
                    }
                };

                serde_json::json!({
                    "name": name,
                    "enabled": cfg.enabled,
                    "disabled_reason": cfg.disabled_reason.as_ref().map(ToString::to_string),
                    "transport": transport,
                    "startup_timeout_sec": cfg
                        .startup_timeout_sec
                        .map(|timeout| timeout.as_secs_f64()),
                    "tool_timeout_sec": cfg
                        .tool_timeout_sec
                        .map(|timeout| timeout.as_secs_f64()),
                    "auth_status": auth_status,
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

    let mut stdio_rows: Vec<[String; 7]> = Vec::new();
    let mut http_rows: Vec<[String; 5]> = Vec::new();

    for (name, cfg) in entries {
        match &cfg.transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => {
                let args_display = if args.is_empty() {
                    "-".to_string()
                } else {
                    args.join(" ")
                };
                let env_display = format_env_display(env.as_ref(), env_vars);
                let cwd_display = cwd
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "-".to_string());
                let status = format_mcp_status(cfg);
                let auth_status = auth_statuses
                    .get(name.as_str())
                    .map(|entry| entry.auth_status)
                    .unwrap_or(McpAuthStatus::Unsupported)
                    .to_string();
                stdio_rows.push([
                    name.clone(),
                    command.clone(),
                    args_display,
                    env_display,
                    cwd_display,
                    status,
                    auth_status,
                ]);
            }
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                ..
            } => {
                let status = format_mcp_status(cfg);
                let auth_status = auth_statuses
                    .get(name.as_str())
                    .map(|entry| entry.auth_status)
                    .unwrap_or(McpAuthStatus::Unsupported)
                    .to_string();
                let bearer_token_display =
                    bearer_token_env_var.as_deref().unwrap_or("-").to_string();
                http_rows.push([
                    name.clone(),
                    url.clone(),
                    bearer_token_display,
                    status,
                    auth_status,
                ]);
            }
        }
    }

    if !stdio_rows.is_empty() {
        let mut widths = [
            "Name".len(),
            "Command".len(),
            "Args".len(),
            "Env".len(),
            "Cwd".len(),
            "Status".len(),
            "Auth".len(),
        ];
        for row in &stdio_rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        println!(
            "{name:<name_w$}  {command:<cmd_w$}  {args:<args_w$}  {env:<env_w$}  {cwd:<cwd_w$}  {status:<status_w$}  {auth:<auth_w$}",
            name = "Name",
            command = "Command",
            args = "Args",
            env = "Env",
            cwd = "Cwd",
            status = "Status",
            auth = "Auth",
            name_w = widths[0],
            cmd_w = widths[1],
            args_w = widths[2],
            env_w = widths[3],
            cwd_w = widths[4],
            status_w = widths[5],
            auth_w = widths[6],
        );

        for row in &stdio_rows {
            println!(
                "{name:<name_w$}  {command:<cmd_w$}  {args:<args_w$}  {env:<env_w$}  {cwd:<cwd_w$}  {status:<status_w$}  {auth:<auth_w$}",
                name = row[0].as_str(),
                command = row[1].as_str(),
                args = row[2].as_str(),
                env = row[3].as_str(),
                cwd = row[4].as_str(),
                status = row[5].as_str(),
                auth = row[6].as_str(),
                name_w = widths[0],
                cmd_w = widths[1],
                args_w = widths[2],
                env_w = widths[3],
                cwd_w = widths[4],
                status_w = widths[5],
                auth_w = widths[6],
            );
        }
    }

    if !stdio_rows.is_empty() && !http_rows.is_empty() {
        println!();
    }

    if !http_rows.is_empty() {
        let mut widths = [
            "Name".len(),
            "Url".len(),
            "Bearer Token Env Var".len(),
            "Status".len(),
            "Auth".len(),
        ];
        for row in &http_rows {
            for (i, cell) in row.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        println!(
            "{name:<name_w$}  {url:<url_w$}  {token:<token_w$}  {status:<status_w$}  {auth:<auth_w$}",
            name = "Name",
            url = "Url",
            token = "Bearer Token Env Var",
            status = "Status",
            auth = "Auth",
            name_w = widths[0],
            url_w = widths[1],
            token_w = widths[2],
            status_w = widths[3],
            auth_w = widths[4],
        );

        for row in &http_rows {
            println!(
                "{name:<name_w$}  {url:<url_w$}  {token:<token_w$}  {status:<status_w$}  {auth:<auth_w$}",
                name = row[0].as_str(),
                url = row[1].as_str(),
                token = row[2].as_str(),
                status = row[3].as_str(),
                auth = row[4].as_str(),
                name_w = widths[0],
                url_w = widths[1],
                token_w = widths[2],
                status_w = widths[3],
                auth_w = widths[4],
            );
        }
    }

    Ok(())
}

async fn run_get(config_overrides: &CliConfigOverrides, get_args: GetArgs) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let config = Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let Some(server) = config.mcp_servers.get().get(&get_args.name) else {
        bail!("No MCP server named '{name}' found.", name = get_args.name);
    };

    if get_args.json {
        let transport = match &server.transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => serde_json::json!({
                "type": "stdio",
                "command": command,
                "args": args,
                "env": env,
                "env_vars": env_vars,
                "cwd": cwd,
            }),
            McpServerTransportConfig::StreamableHttp {
                url,
                bearer_token_env_var,
                http_headers,
                env_http_headers,
            } => serde_json::json!({
                "type": "streamable_http",
                "url": url,
                "bearer_token_env_var": bearer_token_env_var,
                "http_headers": http_headers,
                "env_http_headers": env_http_headers,
            }),
        };
        let output = serde_json::to_string_pretty(&serde_json::json!({
            "name": get_args.name,
            "enabled": server.enabled,
            "disabled_reason": server.disabled_reason.as_ref().map(ToString::to_string),
            "transport": transport,
            "enabled_tools": server.enabled_tools.clone(),
            "disabled_tools": server.disabled_tools.clone(),
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

    if !server.enabled {
        if let Some(reason) = server.disabled_reason.as_ref() {
            println!("{name} (disabled: {reason})", name = get_args.name);
        } else {
            println!("{name} (disabled)", name = get_args.name);
        }
        return Ok(());
    }

    println!("{}", get_args.name);
    println!("  enabled: {}", server.enabled);
    let format_tool_list = |tools: &Option<Vec<String>>| -> String {
        match tools {
            Some(list) if list.is_empty() => "[]".to_string(),
            Some(list) => list.join(", "),
            None => "-".to_string(),
        }
    };
    if server.enabled_tools.is_some() {
        let enabled_tools_display = format_tool_list(&server.enabled_tools);
        println!("  enabled_tools: {enabled_tools_display}");
    }
    if server.disabled_tools.is_some() {
        let disabled_tools_display = format_tool_list(&server.disabled_tools);
        println!("  disabled_tools: {disabled_tools_display}");
    }
    match &server.transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            println!("  transport: stdio");
            println!("  command: {command}");
            let args_display = if args.is_empty() {
                "-".to_string()
            } else {
                args.join(" ")
            };
            println!("  args: {args_display}");
            let cwd_display = cwd
                .as_ref()
                .map(|path| path.display().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "-".to_string());
            println!("  cwd: {cwd_display}");
            let env_display = format_env_display(env.as_ref(), env_vars);
            println!("  env: {env_display}");
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } => {
            println!("  transport: streamable_http");
            println!("  url: {url}");
            let bearer_token_display = bearer_token_env_var.as_deref().unwrap_or("-");
            println!("  bearer_token_env_var: {bearer_token_display}");
            let headers_display = match http_headers {
                Some(map) if !map.is_empty() => {
                    let mut pairs: Vec<_> = map.iter().collect();
                    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                    pairs
                        .into_iter()
                        .map(|(k, _)| format!("{k}=*****"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
                _ => "-".to_string(),
            };
            println!("  http_headers: {headers_display}");
            let env_headers_display = match env_http_headers {
                Some(map) if !map.is_empty() => {
                    let mut pairs: Vec<_> = map.iter().collect();
                    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                    pairs
                        .into_iter()
                        .map(|(k, var)| format!("{k}={var}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
                _ => "-".to_string(),
            };
            println!("  env_http_headers: {env_headers_display}");
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

fn format_mcp_status(config: &McpServerConfig) -> String {
    if config.enabled {
        "enabled".to_string()
    } else if let Some(reason) = config.disabled_reason.as_ref() {
        format!("disabled: {reason}")
    } else {
        "disabled".to_string()
    }
}
