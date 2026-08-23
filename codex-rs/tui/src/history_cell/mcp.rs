//! MCP tool-call, inventory, and output history cells.

use super::*;

use codex_protocol::mcp::is_node_repl_backed_server;

#[path = "mcp_result.rs"]
mod result;

use crate::style::StatusTone;
use crate::style::accent_style;
use crate::style::status_style;
use codex_app_server_protocol::McpServerConnectionStatus;
use result::McpResultKind;
use result::McpToolResult;

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;

#[derive(Debug)]
struct McpImageOutputCell;

impl HistoryCell for McpImageOutputCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec!["tool result (image output)".into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from("tool result (image output)")]
    }
}
fn mcp_auth_status_label(status: McpAuthStatus) -> &'static str {
    match status {
        McpAuthStatus::Unknown => "Unknown",
        McpAuthStatus::Unsupported => "Unsupported",
        McpAuthStatus::NotLoggedIn => "Not logged in",
        McpAuthStatus::BearerToken => "Bearer token",
        McpAuthStatus::OAuth => "OAuth",
    }
}
#[derive(Debug)]
pub(crate) struct McpToolCallCell {
    call_id: String,
    invocation: McpInvocation,
    start_time: Instant,
    duration: Option<Duration>,
    result: Option<Result<McpToolResult, String>>,
    animations_enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct McpInvocation {
    pub(crate) server: String,
    pub(crate) tool: String,
    pub(crate) arguments: Option<serde_json::Value>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum McpToolCallRenderMode {
    /// Compact presentation used in normal conversation history.
    Display,
    /// Complete invocation and result used by the Ctrl+T transcript.
    Transcript,
}

#[derive(serde::Deserialize)]
struct NodeReplExecOutput {
    exit_code: i64,
    output: String,
}

impl McpToolCallCell {
    pub(crate) fn new(
        call_id: String,
        invocation: McpInvocation,
        animations_enabled: bool,
    ) -> Self {
        Self {
            call_id,
            invocation,
            start_time: Instant::now(),
            duration: None,
            result: None,
            animations_enabled,
        }
    }

    pub(crate) fn call_id(&self) -> &str {
        &self.call_id
    }

    pub(crate) fn complete(
        &mut self,
        duration: Duration,
        result: Result<codex_protocol::mcp::CallToolResult, String>,
    ) -> Option<Box<dyn HistoryCell>> {
        let result = result.map(|result| McpToolResult::new(result, self.result_kind()));
        let image_cell = result
            .as_ref()
            .ok()
            .filter(|result| result.has_image)
            .map(|_| Box::new(McpImageOutputCell) as Box<dyn HistoryCell>);
        self.duration = Some(duration);
        self.result = Some(result);
        image_cell
    }

    fn success(&self) -> Option<bool> {
        match self.result.as_ref() {
            Some(Ok(result)) => Some(!result.is_error),
            Some(Err(_)) => Some(false),
            None => None,
        }
    }

    pub(crate) fn mark_failed(&mut self) {
        let elapsed = self.start_time.elapsed();
        self.duration = Some(elapsed);
        self.result = Some(Err("interrupted".to_string()));
    }

    fn result_kind(&self) -> McpResultKind {
        if is_node_repl_backed_server(&self.invocation.server) && self.invocation.tool == "js" {
            McpResultKind::NodeRepl
        } else {
            McpResultKind::Standard
        }
    }

    fn render_lines(&self, width: u16, mode: McpToolCallRenderMode) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let status = self.success();
        let node_repl = self.result_kind() == McpResultKind::NodeRepl;
        let compact = node_repl && mode == McpToolCallRenderMode::Display;
        let bullet = match status {
            Some(true) => "•".green().bold(),
            Some(false) => "•".red().bold(),
            None => activity_indicator(
                Some(self.start_time),
                MotionMode::from_animations_enabled(self.animations_enabled),
                ReducedMotionIndicator::StaticBullet,
            )
            .unwrap_or_else(|| "•".dim()),
        };
        let header_text = if status.is_some() {
            "Called"
        } else {
            "Calling"
        };

        let invocation_line = if compact {
            let title = self
                .invocation
                .arguments
                .as_ref()
                .and_then(|arguments| arguments.get("title"))
                .and_then(serde_json::Value::as_str)
                .map(|title| title.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|title| !title.is_empty())
                .map(|title| title.graphemes(true).take(80).collect::<String>())
                .unwrap_or_else(|| format!("{}.{}", self.invocation.server, self.invocation.tool));
            Line::from(title.cyan())
        } else {
            line_to_static(&format_mcp_invocation(&self.invocation))
        };
        let mut compact_spans = vec![bullet.clone(), " ".into(), header_text.bold(), " ".into()];
        let mut compact_header = Line::from(compact_spans.clone());
        let reserved = compact_header.width();

        let inline_invocation =
            invocation_line.width() <= (width as usize).saturating_sub(reserved);

        if inline_invocation {
            compact_header.extend(invocation_line.spans.clone());
            lines.push(compact_header);
        } else {
            compact_spans.pop(); // drop trailing space for standalone header
            lines.push(Line::from(compact_spans));

            let opts = RtOptions::new((width as usize).saturating_sub(4))
                .initial_indent("".into())
                .subsequent_indent("    ".into());
            let wrapped = adaptive_wrap_line(&invocation_line, opts);
            let body_lines: Vec<Line<'static>> = wrapped.iter().map(line_to_static).collect();
            lines.extend(prefix_lines(body_lines, "  └ ".dim(), "    ".into()));
        }

        let mut detail_lines: Vec<Line<'static>> = Vec::new();
        // Reserve four columns for the tree prefix ("  └ "/"    ") and ensure the wrapper still has at least one cell to work with.
        let detail_wrap_width = (width as usize).saturating_sub(4).max(1);

        if let Some(result) = &self.result {
            match result {
                Ok(McpToolResult { content, .. }) => {
                    if !content.is_empty() {
                        for block in content {
                            let text = if compact && status == Some(true) {
                                let meaningful_output = block.text().and_then(|text| {
                                    if text.starts_with("Script completed\n") {
                                        return text
                                            .split_once("\nOutput:\n")
                                            .map(|(_, output)| output.to_string());
                                    }
                                    serde_json::from_str::<NodeReplExecOutput>(text)
                                        .ok()
                                        .filter(|output| output.exit_code == 0)
                                        .map(|output| output.output)
                                });
                                match meaningful_output {
                                    Some(output) if output.is_empty() => continue,
                                    Some(output) => format_and_truncate_tool_result(
                                        &output,
                                        TOOL_CALL_MAX_LINES,
                                        detail_wrap_width,
                                    ),
                                    None => block.render(detail_wrap_width),
                                }
                            } else if node_repl
                                && mode == McpToolCallRenderMode::Transcript
                                && let Some(output) = block.text()
                            {
                                output.trim_end_matches('\n').to_string()
                            } else {
                                block.render(detail_wrap_width)
                            };
                            for segment in text.split('\n') {
                                let line = Line::from(segment.to_string().dim());
                                let wrapped = adaptive_wrap_line(
                                    &line,
                                    RtOptions::new(detail_wrap_width)
                                        .initial_indent("".into())
                                        .subsequent_indent("    ".into()),
                                );
                                detail_lines.extend(wrapped.iter().map(line_to_static));
                            }
                        }
                    }
                }
                Err(err) => {
                    let err_text = format!("Error: {err}");
                    let err_text = if node_repl && mode == McpToolCallRenderMode::Transcript {
                        err_text
                    } else {
                        format_and_truncate_tool_result(
                            &err_text,
                            TOOL_CALL_MAX_LINES,
                            width as usize,
                        )
                    };
                    let err_line = Line::from(err_text.dim());
                    let wrapped = adaptive_wrap_line(
                        &err_line,
                        RtOptions::new(detail_wrap_width)
                            .initial_indent("".into())
                            .subsequent_indent("    ".into()),
                    );
                    detail_lines.extend(wrapped.iter().map(line_to_static));
                }
            }
        }

        if !detail_lines.is_empty() {
            let initial_prefix: Span<'static> = if inline_invocation {
                "  └ ".dim()
            } else {
                "    ".into()
            };
            lines.extend(prefix_lines(detail_lines, initial_prefix, "    ".into()));
        }

        lines
    }
}

impl HistoryCell for McpToolCallCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.render_lines(width, McpToolCallRenderMode::Display)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.render_lines(width, McpToolCallRenderMode::Transcript)
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let header_text = if self.success().is_some() {
            "Called"
        } else {
            "Calling"
        };
        let mut lines = vec![Line::from(format!(
            "{header_text} {}",
            format_mcp_invocation(&self.invocation)
        ))];

        if let Some(result) = &self.result {
            match result {
                Ok(McpToolResult { content, .. }) => {
                    for block in content {
                        let text = block.render(RAW_TOOL_OUTPUT_WIDTH);
                        lines.extend(raw_lines_from_source(&text));
                    }
                }
                Err(err) => lines.push(Line::from(format!("Error: {err}"))),
            }
        }

        lines
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        if !self.animations_enabled || self.result.is_some() {
            return None;
        }
        Some((self.start_time.elapsed().as_millis() / 50) as u64)
    }
}

pub(crate) fn new_active_mcp_tool_call(
    call_id: String,
    invocation: McpInvocation,
    animations_enabled: bool,
) -> McpToolCallCell {
    McpToolCallCell::new(call_id, invocation, animations_enabled)
}
/// Render a summary of configured MCP servers from the current `Config`.
pub(crate) fn empty_mcp_output() -> WebHyperlinkHistoryCell {
    let mut docs_line = HyperlinkLine::new(Line::from("    See the "));
    docs_line.push_span(
        "MCP docs".underlined(),
        Some("https://developers.openai.com/codex/mcp"),
    );
    docs_line.push_span(" to configure them.".into(), /*destination*/ None);

    let lines = vec![
        HyperlinkLine::new("/mcp".magenta().into()),
        HyperlinkLine::from(""),
        HyperlinkLine::new(vec!["🔌  ".into(), "MCP Tools".bold()].into()),
        HyperlinkLine::from(""),
        HyperlinkLine::new("  • No MCP servers configured.".italic().into()),
        docs_line.style(Style::default().add_modifier(Modifier::DIM)),
    ];

    WebHyperlinkHistoryCell::new_hyperlink_lines(lines)
}

#[cfg(test)]
/// Render MCP tools grouped by connection using the fully-qualified tool names.
pub(crate) fn new_mcp_tools_output(
    config: &Config,
    tools: HashMap<String, codex_protocol::mcp::Tool>,
    resources: HashMap<String, Vec<Resource>>,
    resource_templates: HashMap<String, Vec<ResourceTemplate>>,
    auth_statuses: &HashMap<String, McpAuthStatus>,
) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = vec![
        "/mcp".magenta().into(),
        "".into(),
        vec!["🔌  ".into(), "MCP Tools".bold()].into(),
        "".into(),
    ];

    if tools.is_empty() {
        lines.push("  • No MCP tools available.".italic().into());
        lines.push("".into());
    }

    let effective_servers = config.mcp_servers.get().clone();
    let mut servers: Vec<_> = effective_servers.iter().collect();
    servers.sort_by_key(|(server, _)| *server);

    for (server, cfg) in servers {
        let prefix = qualified_mcp_tool_name_prefix(server);
        let mut names: Vec<String> = tools
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| k[prefix.len()..].to_string())
            .collect();
        names.sort();

        let auth_status = auth_statuses
            .get(server.as_str())
            .copied()
            .unwrap_or(McpAuthStatus::Unsupported);
        let mut header: Vec<Span<'static>> = vec!["  • ".into(), server.clone().into()];
        if !cfg.enabled {
            header.push(" ".into());
            header.push("(disabled)".red());
            lines.push(header.into());
            if let Some(reason) = cfg.disabled_reason.as_ref().map(ToString::to_string) {
                lines.push(vec!["    • Reason: ".into(), reason.dim()].into());
            }
            lines.push(Line::from(""));
            continue;
        }
        lines.push(header.into());
        lines.push(vec!["    • Status: ".into(), "enabled".green()].into());
        lines.push(
            vec![
                "    • Auth: ".into(),
                mcp_auth_status_label(auth_status).into(),
            ]
            .into(),
        );

        match &cfg.transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => {
                let args_suffix = if args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", args.join(" "))
                };
                let cmd_display = format!("{command}{args_suffix}");
                lines.push(vec!["    • Command: ".into(), cmd_display.into()].into());

                if let Some(cwd) = cwd.as_ref() {
                    lines.push(vec!["    • Cwd: ".into(), cwd.to_string().into()].into());
                }

                let env_display = format_env_display(env.as_ref(), env_vars);
                if env_display != "-" {
                    lines.push(vec!["    • Env: ".into(), env_display.into()].into());
                }
            }
            McpServerTransportConfig::StreamableHttp {
                url,
                http_headers,
                env_http_headers,
                ..
            } => {
                lines.push(vec!["    • URL: ".into(), url.clone().into()].into());
                if let Some(headers) = http_headers.as_ref()
                    && !headers.is_empty()
                {
                    let mut pairs: Vec<_> = headers.iter().collect();
                    pairs.sort_by_key(|(name, _)| *name);
                    let display = pairs
                        .into_iter()
                        .map(|(name, _)| format!("{name}=*****"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(vec!["    • HTTP headers: ".into(), display.into()].into());
                }
                if let Some(headers) = env_http_headers.as_ref()
                    && !headers.is_empty()
                {
                    let mut pairs: Vec<_> = headers.iter().collect();
                    pairs.sort_by_key(|(name, _)| *name);
                    let display = pairs
                        .into_iter()
                        .map(|(name, var)| format!("{name}={var}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(vec!["    • Env HTTP headers: ".into(), display.into()].into());
                }
            }
        }

        if names.is_empty() {
            lines.push("    • Tools: (none)".into());
        } else {
            lines.push(vec!["    • Tools: ".into(), names.join(", ").into()].into());
        }

        let server_resources: Vec<Resource> =
            resources.get(server.as_str()).cloned().unwrap_or_default();
        if server_resources.is_empty() {
            lines.push("    • Resources: (none)".into());
        } else {
            let mut spans: Vec<Span<'static>> = vec!["    • Resources: ".into()];

            for (idx, resource) in server_resources.iter().enumerate() {
                if idx > 0 {
                    spans.push(", ".into());
                }

                let label = resource.title.as_ref().unwrap_or(&resource.name);
                spans.push(label.clone().into());
                spans.push(" ".into());
                spans.push(format!("({})", resource.uri).dim());
            }

            lines.push(spans.into());
        }

        let server_templates: Vec<ResourceTemplate> = resource_templates
            .get(server.as_str())
            .cloned()
            .unwrap_or_default();
        if server_templates.is_empty() {
            lines.push("    • Resource templates: (none)".into());
        } else {
            let mut spans: Vec<Span<'static>> = vec!["    • Resource templates: ".into()];

            for (idx, template) in server_templates.iter().enumerate() {
                if idx > 0 {
                    spans.push(", ".into());
                }

                let label = template.title.as_ref().unwrap_or(&template.name);
                spans.push(label.clone().into());
                spans.push(" ".into());
                spans.push(format!("({})", template.uri_template).dim());
            }

            lines.push(spans.into());
        }

        lines.push(Line::from(""));
    }

    PlainHistoryCell { lines }
}

/// Build the `/mcp` history cell from app-server `McpServerStatus` responses.
///
/// The server list comes directly from the app-server status response, sorted
/// alphabetically. The TUI deliberately does not enrich these rows from
/// client-local config because the app-server owns the remote MCP state.
///
/// Normal output is a compact connection summary. Full detail preserves the
/// tool, auth, resource, and resource-template inventory.
pub(crate) fn new_mcp_tools_output_from_statuses(
    statuses: &[McpServerStatus],
    detail: McpServerStatusDetail,
) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = vec![
        "/mcp".magenta().into(),
        "".into(),
        vec!["🔌  ".into(), "MCP Tools".bold()].into(),
        "".into(),
    ];

    let mut statuses = statuses.iter().collect::<Vec<_>>();
    statuses.sort_by(|a, b| a.name.cmp(&b.name));

    let has_any_tools = statuses.iter().any(|status| !status.tools.is_empty());
    if !has_any_tools && matches!(detail, McpServerStatusDetail::Full) {
        lines.push("  • No MCP tools available.".italic().into());
        lines.push("".into());
    }

    for status in statuses {
        let (label, style) = match status.runtime_status {
            Some(McpServerConnectionStatus::Connected) => {
                ("connected", status_style(StatusTone::Success))
            }
            Some(McpServerConnectionStatus::Starting) => ("starting", accent_style()),
            Some(McpServerConnectionStatus::AuthenticationRequired) => (
                "authentication required",
                status_style(StatusTone::Attention),
            ),
            Some(McpServerConnectionStatus::Failed) => {
                ("failed", status_style(StatusTone::Failure))
            }
            Some(McpServerConnectionStatus::NotStarted) => ("not started", Style::default().dim()),
            Some(McpServerConnectionStatus::Disabled) => ("disabled", Style::default().dim()),
            Some(McpServerConnectionStatus::Cancelled) => ("cancelled", Style::default().dim()),
            None if matches!(
                status.auth_status,
                codex_app_server_protocol::McpAuthStatus::NotLoggedIn
            ) =>
            {
                (
                    "authentication required",
                    status_style(StatusTone::Attention),
                )
            }
            None => ("unknown", Style::default().dim()),
        };
        let count = status.tools.len();
        let unit = if count == 1 { "tool" } else { "tools" };
        lines.push(
            vec![
                "  • ".set_style(style),
                status.name.clone().bold(),
                ": ".into(),
                label.set_style(style),
                format!(" ({count} {unit})").dim(),
            ]
            .into(),
        );
        if matches!(detail, McpServerStatusDetail::ToolsAndAuthOnly) {
            continue;
        }
        let auth_status = match status.auth_status {
            codex_app_server_protocol::McpAuthStatus::Unknown => McpAuthStatus::Unknown,
            codex_app_server_protocol::McpAuthStatus::Unsupported => McpAuthStatus::Unsupported,
            codex_app_server_protocol::McpAuthStatus::NotLoggedIn => McpAuthStatus::NotLoggedIn,
            codex_app_server_protocol::McpAuthStatus::BearerToken => McpAuthStatus::BearerToken,
            codex_app_server_protocol::McpAuthStatus::OAuth => McpAuthStatus::OAuth,
        };
        lines.push(
            vec![
                "    • Auth: ".into(),
                mcp_auth_status_label(auth_status).into(),
            ]
            .into(),
        );

        let mut names = status.tools.keys().cloned().collect::<Vec<_>>();
        names.sort();
        if names.is_empty() {
            lines.push("    • Tools: (none)".into());
        } else {
            lines.push(vec!["    • Tools: ".into(), names.join(", ").into()].into());
        }

        if matches!(detail, McpServerStatusDetail::Full) {
            let server_resources = status.resources.clone();
            if server_resources.is_empty() {
                lines.push("    • Resources: (none)".into());
            } else {
                let mut spans: Vec<Span<'static>> = vec!["    • Resources: ".into()];

                for (idx, resource) in server_resources.iter().enumerate() {
                    if idx > 0 {
                        spans.push(", ".into());
                    }

                    let label = resource.title.as_ref().unwrap_or(&resource.name);
                    spans.push(label.clone().into());
                    spans.push(" ".into());
                    spans.push(format!("({})", resource.uri).dim());
                }

                lines.push(spans.into());
            }

            let server_templates = status.resource_templates.clone();
            if server_templates.is_empty() {
                lines.push("    • Resource templates: (none)".into());
            } else {
                let mut spans: Vec<Span<'static>> = vec!["    • Resource templates: ".into()];

                for (idx, template) in server_templates.iter().enumerate() {
                    if idx > 0 {
                        spans.push(", ".into());
                    }

                    let label = template.title.as_ref().unwrap_or(&template.name);
                    spans.push(label.clone().into());
                    spans.push(" ".into());
                    spans.push(format!("({})", template.uri_template).dim());
                }

                lines.push(spans.into());
            }
        }

        lines.push(Line::from(""));
    }

    if matches!(detail, McpServerStatusDetail::ToolsAndAuthOnly) {
        lines.push("".into());
        lines.push("  Use /mcp verbose for tools and resources.".dim().into());
    }

    PlainHistoryCell { lines }
}
/// A transient history cell that shows an animated spinner while the MCP
/// inventory RPC is in flight.
///
/// Inserted as the `active_cell` by `ChatWidget::add_mcp_output()` and removed
/// once the fetch completes. The app removes committed copies from transcript
/// history, while `ChatWidget::clear_mcp_inventory_loading()` only clears the
/// in-flight `active_cell`.
#[derive(Debug)]
pub(crate) struct McpInventoryLoadingCell {
    start_time: Instant,
    animations_enabled: bool,
}

impl McpInventoryLoadingCell {
    pub(crate) fn new(animations_enabled: bool) -> Self {
        Self {
            start_time: Instant::now(),
            animations_enabled,
        }
    }
}

impl HistoryCell for McpInventoryLoadingCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![
            vec![
                activity_indicator(
                    Some(self.start_time),
                    MotionMode::from_animations_enabled(self.animations_enabled),
                    ReducedMotionIndicator::StaticBullet,
                )
                .unwrap_or_else(|| "•".dim()),
                " ".into(),
                "Loading MCP inventory".bold(),
                "…".dim(),
            ]
            .into(),
        ]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from("Loading MCP inventory...")]
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        if !self.animations_enabled {
            return None;
        }
        Some((self.start_time.elapsed().as_millis() / 50) as u64)
    }
}

/// Convenience constructor for [`McpInventoryLoadingCell`].
pub(crate) fn new_mcp_inventory_loading(animations_enabled: bool) -> McpInventoryLoadingCell {
    McpInventoryLoadingCell::new(animations_enabled)
}
fn format_mcp_invocation(invocation: &McpInvocation) -> Line<'_> {
    let args_str = invocation
        .arguments
        .as_ref()
        .map(|v: &serde_json::Value| {
            // Use compact form to keep things short but readable.
            serde_json::to_string(v).unwrap_or_else(|_| v.to_string())
        })
        .unwrap_or_default();

    let invocation_spans = vec![
        invocation.server.as_str().cyan(),
        ".".into(),
        invocation.tool.as_str().cyan(),
        "(".into(),
        args_str.dim(),
        ")".into(),
    ];
    invocation_spans.into()
}
