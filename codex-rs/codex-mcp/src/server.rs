use codex_builtin_mcps::BuiltinMcpServer;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;

/// The runtime launch strategy for an effective MCP server.
#[derive(Debug, Clone)]
pub(crate) enum McpServerLaunch {
    Configured(Box<McpServerConfig>),
    Builtin(BuiltinMcpServer),
}

/// MCP server after product-owned runtime additions have been applied.
#[derive(Debug, Clone)]
pub struct EffectiveMcpServer {
    launch: McpServerLaunch,
}

impl EffectiveMcpServer {
    pub fn configured(config: McpServerConfig) -> Self {
        Self {
            launch: McpServerLaunch::Configured(Box::new(config)),
        }
    }

    pub fn builtin(server: BuiltinMcpServer) -> Self {
        Self {
            launch: McpServerLaunch::Builtin(server),
        }
    }

    pub(crate) fn launch(&self) -> &McpServerLaunch {
        &self.launch
    }

    pub fn configured_config(&self) -> Option<&McpServerConfig> {
        match &self.launch {
            McpServerLaunch::Configured(config) => Some(config.as_ref()),
            McpServerLaunch::Builtin(_) => None,
        }
    }

    pub fn enabled(&self) -> bool {
        match &self.launch {
            McpServerLaunch::Configured(config) => config.enabled,
            McpServerLaunch::Builtin(_) => true,
        }
    }

    pub fn required(&self) -> bool {
        match &self.launch {
            McpServerLaunch::Configured(config) => config.required,
            McpServerLaunch::Builtin(_) => false,
        }
    }
}

/// Transport origin retained for metrics and diagnostics after server launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum McpServerOrigin {
    InProcess,
    Stdio,
    StreamableHttp(String),
}

impl McpServerOrigin {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InProcess => "in_process",
            Self::Stdio => "stdio",
            Self::StreamableHttp(origin) => origin,
        }
    }

    fn from_transport(transport: &McpServerTransportConfig) -> Option<Self> {
        match transport {
            McpServerTransportConfig::StreamableHttp { url, .. } => {
                let parsed = url::Url::parse(url).ok()?;
                Some(Self::StreamableHttp(parsed.origin().ascii_serialization()))
            }
            McpServerTransportConfig::Stdio { .. } => Some(Self::Stdio),
        }
    }
}

/// Semantic metadata that must survive after the server is launched.
#[derive(Debug, Clone)]
pub(crate) struct McpServerMetadata {
    pub pollutes_memory: bool,
    pub origin: Option<McpServerOrigin>,
    pub supports_parallel_tool_calls: bool,
}

impl From<&EffectiveMcpServer> for McpServerMetadata {
    fn from(server: &EffectiveMcpServer) -> Self {
        match server.launch() {
            McpServerLaunch::Configured(config) => Self {
                pollutes_memory: true,
                origin: McpServerOrigin::from_transport(&config.transport),
                supports_parallel_tool_calls: config.supports_parallel_tool_calls,
            },
            McpServerLaunch::Builtin(server) => Self {
                pollutes_memory: server.pollutes_memory(),
                origin: Some(McpServerOrigin::InProcess),
                supports_parallel_tool_calls: server.supports_parallel_tool_calls(),
            },
        }
    }
}
