//! Temporary cleanup-hook allowlist shared by local and executor plugin discovery.
//! This narrowly authorizes known MCP cleanup calls; it does not verify plugin signatures.

use codex_config::HookHandlerConfig;
use codex_protocol::protocol::HookEventName;

struct BundledHook {
    plugin_id: &'static str,
    events: &'static [HookEventName],
    target: BundledHookTarget,
}

enum BundledHookTarget {
    McpServer {
        server: &'static str,
        tool: &'static str,
    },
    App {
        server: &'static str,
        connector_id: &'static str,
        tool: &'static str,
    },
}

// Keep unsigned plugin exceptions together so they can be removed as signing lands.
const ALLOWLISTED_BUNDLED_HOOKS: &[BundledHook] = &[
    BundledHook {
        plugin_id: "browser@openai-bundled",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::McpServer {
            server: "node_repl",
            tool: "turn_ended",
        },
    },
    BundledHook {
        plugin_id: "chrome@openai-bundled",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::McpServer {
            server: "node_repl",
            tool: "turn_ended",
        },
    },
    BundledHook {
        plugin_id: "chrome-dev@openai-bundled",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::McpServer {
            server: "node_repl",
            tool: "turn_ended",
        },
    },
    BundledHook {
        plugin_id: "chrome-internal@openai-bundled",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::McpServer {
            server: "node_repl",
            tool: "turn_ended",
        },
    },
    BundledHook {
        plugin_id: "computer-use@openai-bundled",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::McpServer {
            server: "node_repl",
            tool: "turn_ended",
        },
    },
    BundledHook {
        plugin_id: "unified-computer-use@openai-bundled",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::McpServer {
            server: "cua_repl",
            tool: "turn_ended",
        },
    },
    BundledHook {
        plugin_id: "browser@openai-curated-remote",
        events: &[
            HookEventName::Stop,
            HookEventName::Interrupt,
            HookEventName::SubagentStop,
        ],
        target: BundledHookTarget::App {
            server: "codex_apps",
            connector_id: "connector_openai_browser",
            tool: "browser.turn_ended",
        },
    },
];

/// Matches a temporary unsigned cleanup exception. App targets additionally require the
/// connector identity for this handler's server and tool from the caller's enabled tool
/// catalog; callers without that catalog must pass `None`.
pub fn is_allowlisted_bundled_cleanup_hook(
    plugin_id: &str,
    event: HookEventName,
    matcher: Option<&str>,
    handler: &HookHandlerConfig,
    app_connector_id: Option<&str>,
) -> bool {
    let HookHandlerConfig::McpTool {
        server,
        tool,
        input,
        ..
    } = handler
    else {
        return false;
    };

    matcher.is_none()
        && ALLOWLISTED_BUNDLED_HOOKS.iter().any(|hook| {
            hook.plugin_id == plugin_id
                && hook.events.contains(&event)
                && match hook.target {
                    BundledHookTarget::McpServer {
                        server: expected_server,
                        tool: expected_tool,
                    } => server == expected_server && tool == expected_tool,
                    BundledHookTarget::App {
                        server: expected_server,
                        connector_id,
                        tool: expected_tool,
                    } => {
                        // Raw Apps tool names can collide; require the registered connector.
                        server == expected_server
                            && tool == expected_tool
                            && input.is_empty()
                            && app_connector_id == Some(connector_id)
                    }
                }
        })
}
