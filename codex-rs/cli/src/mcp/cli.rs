use clap::Parser;

/// [experimental] Arguments for the MCP wizard.
#[derive(Debug, Parser, Clone, Default)]
pub struct WizardArgs {
    /// Optional template to preselect when starting the wizard.
    #[arg(long)]
    pub template: Option<String>,

    /// Server name (non-interactive mode).
    #[arg(long)]
    pub name: Option<String>,

    /// Command to launch the MCP server.
    #[arg(long)]
    pub command: Option<String>,

    /// Command arguments (repeatable).
    #[arg(long = "arg", value_name = "ARG")]
    pub args: Vec<String>,

    /// Environment variables KEY=VALUE (repeatable).
    #[arg(long = "env", value_parser = parse_env_pair, value_name = "KEY=VALUE")]
    pub env: Vec<(String, String)>,

    /// Startup timeout in milliseconds.
    #[arg(long = "startup-timeout-ms")]
    pub startup_timeout_ms: Option<u64>,

    /// Optional description for the server.
    #[arg(long)]
    pub description: Option<String>,

    /// Tags to associate with the server (repeatable).
    #[arg(long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,

    /// Authentication type (e.g. none, env, apikey).
    #[arg(long = "auth-type")]
    pub auth_type: Option<String>,

    /// Authentication secret reference.
    #[arg(long = "auth-secret-ref")]
    pub auth_secret_ref: Option<String>,

    /// Authentication environment variables KEY=VALUE (repeatable).
    #[arg(long = "auth-env", value_parser = parse_env_pair, value_name = "KEY=VALUE")]
    pub auth_env: Vec<(String, String)>,

    /// Healthcheck type (e.g. none, stdio, http).
    #[arg(long = "health-type")]
    pub health_type: Option<String>,

    /// Healthcheck command (for stdio type).
    #[arg(long = "health-command")]
    pub health_command: Option<String>,

    /// Healthcheck arguments (repeatable).
    #[arg(long = "health-arg", value_name = "ARG")]
    pub health_args: Vec<String>,

    /// Healthcheck timeout in milliseconds.
    #[arg(long = "health-timeout-ms")]
    pub health_timeout_ms: Option<u64>,

    /// Healthcheck interval in seconds.
    #[arg(long = "health-interval-seconds")]
    pub health_interval_seconds: Option<u64>,

    /// Healthcheck endpoint (for network types).
    #[arg(long = "health-endpoint")]
    pub health_endpoint: Option<String>,

    /// Healthcheck protocol (for network types).
    #[arg(long = "health-protocol")]
    pub health_protocol: Option<String>,

    /// Apply configuration without prompting.
    #[arg(long, default_value_t = false)]
    pub apply: bool,

    /// Output summary as JSON instead of launching interactive flow.
    #[arg(long, default_value_t = false)]
    pub json: bool,
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
