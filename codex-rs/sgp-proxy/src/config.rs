use std::path::PathBuf;

use clap::Parser;
use clap::ValueEnum;

/// CLI arguments for the SGP proxy.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "codex-sgp-proxy",
    about = "Translation proxy: Responses API <-> Agentex JSON-RPC"
)]
pub struct Args {
    /// Port to listen on. If not set, an ephemeral port is used.
    #[arg(long)]
    pub port: Option<u16>,

    /// Path to a JSON file to write startup info (single line). Includes {"port": <u16>}.
    #[arg(long, value_name = "FILE")]
    pub server_info: Option<PathBuf>,

    /// Enable HTTP shutdown endpoint at GET /shutdown
    #[arg(long)]
    pub http_shutdown: bool,

    /// Agentex API base URL (e.g. https://agentex.example.com).
    #[arg(long)]
    pub agentex_url: String,

    /// Agentex agent ID to route requests to.
    #[arg(long)]
    pub agent_id: String,

    /// Task lifecycle mode.
    #[arg(long, value_enum, default_value_t = TaskLifecycleMode::PerSession)]
    pub task_lifecycle: TaskLifecycleMode,

    /// Comma-separated list of tool names that the Agentex agent handles
    /// internally. All other tool calls from the agent are emitted as
    /// FunctionCall items for Codex to execute locally.
    #[arg(long, value_delimiter = ',')]
    pub agent_tools: Vec<String>,
}

/// Controls whether each Codex session maps to a single Agentex task
/// (preserving cross-turn memory) or spawns a fresh task per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TaskLifecycleMode {
    /// First request creates a task; subsequent requests reuse it.
    PerSession,
    /// Every request creates a new task.
    PerRequest,
}
