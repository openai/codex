//! MCP tool-call, inventory, and output history cells.

use super::*;

#[derive(Debug)]
struct CompletedMcpToolCallWithImageOutput {
    _image: DynamicImage,
}
impl HistoryCell for CompletedMcpToolCallWithImageOutput {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec!["tool result (image output)".into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec![Line::from("tool result (image output)")]
    }
}
fn mcp_auth_status_label(status: McpAuthStatus) -> &'static str {
    match status {
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
    result: Option<Result<codex_protocol::mcp::CallToolResult, String>>,
    animations_enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct McpInvocation {
    pub(crate) server: String,
    pub(crate) tool: String,
    pub(crate) arguments: Option<serde_json::Value>,
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
        let image_cell = try_new_completed_mcp_tool_call_with_image_output(&result)
            .map(|cell| Box::new(cell) as Box<dyn HistoryCell>);
        self.duration = Some(duration);
        self.result = Some(result);
        image_cell
    }

    fn success(&self) -> Option<bool> {
        match self.result.as_ref() {
            Some(Ok(result)) => Some(!result.is_error.unwrap_or(false)),
            Some(Err(_)) => Some(false),
            None => None,
        }
    }

    pub(crate) fn mark_failed(&mut self) {
        let elapsed = self.start_time.elapsed();
        self.duration = Some(elapsed);
        self.result = Some(Err("interrupted".to_string()));
    }

    fn render_content_block(block: &serde_json::Value, width: usize) -> String {
        let content = match serde_json::from_value::<rmcp::model::Content>(block.clone()) {
            Ok(content) => content,
            Err(_) => {
                return format_and_truncate_tool_result(
                    &block.to_string(),
                    TOOL_CALL_MAX_LINES,
                    width,
                );
            }
        };

        match content.raw {
            rmcp::model::RawContent::Text(text) => {
                format_and_truncate_tool_result(&text.text, TOOL_CALL_MAX_LINES, width)
            }
            rmcp::model::RawContent::Image(_) => "<image content>".to_string(),
            rmcp::model::RawContent::Audio(_) => "<audio content>".to_string(),
            rmcp::model::RawContent::Resource(resource) => {
                let uri = match resource.resource {
                    rmcp::model::ResourceContents::TextResourceContents { uri, .. } => uri,
                    rmcp::model::ResourceContents::BlobResourceContents { uri, .. } => uri,
                };
                format!("embedded resource: {uri}")
            }
            rmcp::model::RawContent::ResourceLink(link) => format!("link: {}", link.uri),
        }
    }
}

impl HistoryCell for McpToolCallCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let status = self.success();
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

        let invocation_line = line_to_static(&format_mcp_invocation(self.invocation.clone()));
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
                Ok(codex_protocol::mcp::CallToolResult { content, .. }) => {
                    if !content.is_empty() {
                        for block in content {
                            let text = Self::render_content_block(block, detail_wrap_width);
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
                    let err_text = format_and_truncate_tool_result(
                        &format!("Error: {err}"),
                        TOOL_CALL_MAX_LINES,
                        width as usize,
                    );
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

    fn raw_lines(&self) -> Vec<Line<'static>> {
        let header_text = if self.success().is_some() {
            "Called"
        } else {
            "Calling"
        };
        let mut lines = vec![Line::from(format!(
            "{header_text} {}",
            format_mcp_invocation(self.invocation.clone())
        ))];

        if let Some(result) = &self.result {
            match result {
                Ok(codex_protocol::mcp::CallToolResult { content, .. }) => {
                    for block in content {
                        let text = Self::render_content_block(block, RAW_TOOL_OUTPUT_WIDTH);
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
/// Returns an additional history cell if an MCP tool result includes a decodable image.
///
/// This intentionally returns at most one cell: the first image in `CallToolResult.content` that
/// successfully base64-decodes and parses as an image. This is used as a lightweight “image output
/// exists” affordance separate from the main MCP tool call cell.
///
/// Manual testing tip:
/// - Run the rmcp stdio test server (`codex-rs/rmcp-client/src/bin/test_stdio_server.rs`) and
///   register it as an MCP server via `codex mcp add`.
/// - Use its `image_scenario` tool with cases like `text_then_image`,
///   `invalid_base64_then_image`, or `invalid_image_bytes_then_image` to ensure this path triggers
///   even when the first block is not a valid image.
fn try_new_completed_mcp_tool_call_with_image_output(
    result: &Result<codex_protocol::mcp::CallToolResult, String>,
) -> Option<CompletedMcpToolCallWithImageOutput> {
    let image = result
        .as_ref()
        .ok()?
        .content
        .iter()
        .find_map(decode_mcp_image)?;

    Some(CompletedMcpToolCallWithImageOutput { _image: image })
}

/// Decodes an MCP `ImageContent` block into an in-memory image.
///
/// Returns `None` when the block is not an image, when base64 decoding fails, when the format
/// cannot be inferred, or when the image decoder rejects the bytes.
fn decode_mcp_image(block: &serde_json::Value) -> Option<DynamicImage> {
    let content = serde_json::from_value::<rmcp::model::Content>(block.clone()).ok()?;
    let rmcp::model::RawContent::Image(image) = content.raw else {
        return None;
    };
    let base64_data = if let Some(data_url) = image.data.strip_prefix("data:") {
        data_url.split_once(',')?.1
    } else {
        image.data.as_str()
    };
    let raw_data = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| {
            error!("Failed to decode image data: {e}");
            e
        })
        .ok()?;
    let reader = ImageReader::new(Cursor::new(raw_data))
        .with_guessed_format()
        .map_err(|e| {
            error!("Failed to guess image format: {e}");
            e
        })
        .ok()?;

    reader
        .decode()
        .map_err(|e| {
            error!("Image decoding failed: {e}");
            e
        })
        .ok()
}
/// Render a summary of configured MCP servers from the current `Config`.
pub(crate) fn empty_mcp_output() -> PlainHistoryCell {
    let lines: Vec<Line<'static>> = vec![
        "/mcp".magenta().into(),
        "".into(),
        vec!["🔌  ".into(), "MCP Tools".bold()].into(),
        "".into(),
        "  • No MCP servers configured.".italic().into(),
        Line::from(vec![
            "    See the ".into(),
            "\u{1b}]8;;https://developers.openai.com/codex/mcp\u{7}MCP docs\u{1b}]8;;\u{7}"
                .underlined(),
            " to configure them.".into(),
        ])
        .style(Style::default().add_modifier(Modifier::DIM)),
    ];

    PlainHistoryCell { lines }
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
    servers.sort_by(|(a, _), (b, _)| a.cmp(b));

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
                    lines.push(vec!["    • Cwd: ".into(), cwd.display().to_string().into()].into());
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
                    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
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
                    pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
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
/// alphabetically. Local config is only used to enrich returned servers with
/// transport details such as command, URL, cwd, and environment display.
///
/// This mirrors the layout of [`new_mcp_tools_output`] but sources data from
/// the paginated RPC response rather than the in-process `McpManager`. The
/// `detail` flag controls whether resources and resource templates are rendered.
pub(crate) fn new_mcp_tools_output_from_statuses(
    config: &Config,
    statuses: &[McpServerStatus],
    detail: McpServerStatusDetail,
) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = vec![
        "/mcp".magenta().into(),
        "".into(),
        vec!["🔌  ".into(), "MCP Tools".bold()].into(),
        "".into(),
    ];

    let mut statuses_by_name = HashMap::new();
    for status in statuses {
        statuses_by_name.insert(status.name.as_str(), status);
    }

    let mut server_names: Vec<String> = statuses.iter().map(|status| status.name.clone()).collect();
    server_names.sort();

    let has_any_tools = statuses.iter().any(|status| !status.tools.is_empty());
    if !has_any_tools {
        lines.push("  • No MCP tools available.".italic().into());
        lines.push("".into());
    }

    for server in server_names {
        let cfg = config.mcp_servers.get().get(server.as_str());
        let status = statuses_by_name.get(server.as_str()).copied();
        let header: Vec<Span<'static>> = vec!["  • ".into(), server.clone().into()];

        lines.push(header.into());
        if matches!(detail, McpServerStatusDetail::Full) {
            let enabled = cfg.map(|cfg| cfg.enabled).unwrap_or(true);
            let status_text = if enabled {
                "enabled".green()
            } else {
                "disabled".red()
            };
            lines.push(vec!["    • Status: ".into(), status_text].into());
            if let Some(reason) = cfg.and_then(|cfg| cfg.disabled_reason.as_ref()) {
                lines.push(vec!["    • Reason: ".into(), reason.to_string().dim()].into());
            }
        }
        let auth_status = status
            .map(|status| match status.auth_status {
                codex_app_server_protocol::McpAuthStatus::Unsupported => McpAuthStatus::Unsupported,
                codex_app_server_protocol::McpAuthStatus::NotLoggedIn => McpAuthStatus::NotLoggedIn,
                codex_app_server_protocol::McpAuthStatus::BearerToken => McpAuthStatus::BearerToken,
                codex_app_server_protocol::McpAuthStatus::OAuth => McpAuthStatus::OAuth,
            })
            .unwrap_or(McpAuthStatus::Unsupported);
        lines.push(
            vec![
                "    • Auth: ".into(),
                mcp_auth_status_label(auth_status).into(),
            ]
            .into(),
        );

        if let Some(cfg) = cfg {
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
                        lines.push(
                            vec!["    • Cwd: ".into(), cwd.display().to_string().into()].into(),
                        );
                    }

                    let env_display = format_env_display(env.as_ref(), env_vars.as_slice());
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
                        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
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
                        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
                        let display = pairs
                            .into_iter()
                            .map(|(name, var)| format!("{name}={var}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        lines.push(vec!["    • Env HTTP headers: ".into(), display.into()].into());
                    }
                }
            }
        }

        let mut names = status
            .map(|status| status.tools.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        names.sort();
        if names.is_empty() {
            lines.push("    • Tools: (none)".into());
        } else {
            lines.push(vec!["    • Tools: ".into(), names.join(", ").into()].into());
        }

        if matches!(detail, McpServerStatusDetail::Full) {
            let server_resources = status
                .map(|status| status.resources.clone())
                .unwrap_or_default();
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

            let server_templates = status
                .map(|status| status.resource_templates.clone())
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
        }

        lines.push(Line::from(""));
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
fn format_mcp_invocation<'a>(invocation: McpInvocation) -> Line<'a> {
    let args_str = invocation
        .arguments
        .as_ref()
        .map(|v: &serde_json::Value| {
            // Use compact form to keep things short but readable.
            serde_json::to_string(v).unwrap_or_else(|_| v.to_string())
        })
        .unwrap_or_default();

    let invocation_spans = vec![
        invocation.server.clone().cyan(),
        ".".into(),
        invocation.tool.cyan(),
        "(".into(),
        args_str.dim(),
        ")".into(),
    ];
    invocation_spans.into()
}
