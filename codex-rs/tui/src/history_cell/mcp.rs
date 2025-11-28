use super::HistoryCell;
use crate::exec_cell::TOOL_CALL_MAX_LINES;
use crate::exec_cell::spinner;
use crate::render::line_utils::line_to_static;
use crate::render::line_utils::prefix_lines;
use crate::text_formatting::format_and_truncate_tool_result;
use crate::wrapping::RtOptions;
use crate::wrapping::word_wrap_line;
use base64::Engine;
use codex_common::format_env_display::format_env_display;
use codex_core::config::Config;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::protocol::McpAuthStatus;
use codex_core::protocol::McpInvocation;
use image::DynamicImage;
use image::ImageReader;
use mcp_types::EmbeddedResourceResource;
use mcp_types::Resource;
use mcp_types::ResourceLink;
use mcp_types::ResourceTemplate;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use std::collections::HashMap;
use std::io::Cursor;
use std::time::Duration;
use std::time::Instant;
use tracing::error;

/// Summary of MCP tools and resources configured for the session.
///
/// Renders a `/mcp` section header, connection metadata, and tool/resource listings grouped by
/// server. Sensitive values such as env vars and headers are masked.
///
/// # Output
///
/// ```plain
/// /mcp
///
/// 🔌  MCP Tools
///
///   • docs
///     • Status: enabled
///     • Auth: supported
///     • Command: docs-server
///     • Tools: list
/// ```
#[derive(Debug)]
pub(crate) struct McpToolsOutputCell {
    lines: Vec<Line<'static>>,
}

impl McpToolsOutputCell {
    pub(crate) fn new(
        config: &Config,
        tools: HashMap<String, mcp_types::Tool>,
        resources: HashMap<String, Vec<Resource>>,
        resource_templates: HashMap<String, Vec<ResourceTemplate>>,
        auth_statuses: &HashMap<String, McpAuthStatus>,
    ) -> Self {
        let mut lines: Vec<Line<'static>> = vec![
            "/mcp".magenta().into(),
            "".into(),
            vec!["🔌  ".into(), "MCP Tools".bold()].into(),
            "".into(),
        ];

        if tools.is_empty() {
            lines.push("  • No MCP tools available.".italic().into());
            lines.push("".into());
            return Self { lines };
        }

        let mut servers: Vec<_> = config.mcp_servers.iter().collect();
        servers.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (server, cfg) in servers {
            let prefix = format!("mcp__{server}__");
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
                lines.push(Line::from(""));
                continue;
            }
            lines.push(header.into());
            lines.push(vec!["    • Status: ".into(), "enabled".green()].into());
            lines.push(vec!["    • Auth: ".into(), auth_status.to_string().into()].into());

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

        Self { lines }
    }
}

impl HistoryCell for McpToolsOutputCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.lines.clone()
    }
}

pub(crate) fn new_mcp_tools_output(
    config: &Config,
    tools: HashMap<String, mcp_types::Tool>,
    resources: HashMap<String, Vec<Resource>>,
    resource_templates: HashMap<String, Vec<ResourceTemplate>>,
    auth_statuses: &HashMap<String, McpAuthStatus>,
) -> McpToolsOutputCell {
    McpToolsOutputCell::new(config, tools, resources, resource_templates, auth_statuses)
}

pub(crate) fn empty_mcp_output() -> McpToolsOutputCell {
    McpToolsOutputCell {
        lines: {
            let mut link_line: Line = vec![
                "    See the ".into(),
                "\u{1b}]8;;https://github.com/openai/codex/blob/main/docs/config.md#mcp_servers\u{7}MCP docs\u{1b}]8;;\u{7}".underlined(),
                " to configure them.".into(),
            ]
            .into();
            link_line = link_line.patch_style(Style::default().add_modifier(Modifier::DIM));

            vec![
                "/mcp".magenta().into(),
                "".into(),
                vec!["🔌  ".into(), "MCP Tools".bold()].into(),
                "".into(),
                "  • No MCP servers configured.".italic().into(),
                link_line,
            ]
        },
    }
}

/// Status line for active or completed MCP tool calls.
///
/// Shows a spinner or success/error bullet, the invoked server.tool signature, and wrapped snippets
/// of any returned text output. Used for both in-progress calls and final results.
///
/// # Output
///
/// ```plain
/// • Calling search.find_docs({"query":"q"})
///   └ <wrapped output>
/// ```
#[derive(Debug)]
pub(crate) struct McpToolCallCell {
    call_id: String,
    invocation: McpInvocation,
    start_time: Instant,
    duration: Option<Duration>,
    result: Option<Result<mcp_types::CallToolResult, String>>,
    animations_enabled: bool,
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
        result: Result<mcp_types::CallToolResult, String>,
    ) -> Option<Box<dyn HistoryCell>> {
        let image_cell = try_new_completed_mcp_tool_call_with_image_output(&result)
            .map(|cell| Box::new(cell) as Box<dyn HistoryCell>);
        self.duration = Some(duration);
        self.result = Some(result);
        image_cell
    }

    pub(crate) fn mark_failed(&mut self) {
        let elapsed = self.start_time.elapsed();
        self.duration = Some(elapsed);
        self.result = Some(Err("interrupted".to_string()));
    }

    fn success(&self) -> Option<bool> {
        match self.result.as_ref() {
            Some(Ok(result)) => Some(!result.is_error.unwrap_or(false)),
            Some(Err(_)) => Some(false),
            None => None,
        }
    }

    fn render_content_block(block: &mcp_types::ContentBlock, width: usize) -> String {
        match block {
            mcp_types::ContentBlock::TextContent(text) => {
                format_and_truncate_tool_result(&text.text, TOOL_CALL_MAX_LINES, width)
            }
            mcp_types::ContentBlock::ImageContent(_) => "<image content>".to_string(),
            mcp_types::ContentBlock::AudioContent(_) => "<audio content>".to_string(),
            mcp_types::ContentBlock::EmbeddedResource(resource) => {
                let uri = match &resource.resource {
                    EmbeddedResourceResource::TextResourceContents(text) => text.uri.clone(),
                    EmbeddedResourceResource::BlobResourceContents(blob) => blob.uri.clone(),
                };
                format!("embedded resource: {uri}")
            }
            mcp_types::ContentBlock::ResourceLink(ResourceLink { uri, .. }) => {
                format!("link: {uri}")
            }
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
            None => spinner(Some(self.start_time), self.animations_enabled),
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
            let wrapped = word_wrap_line(&invocation_line, opts);
            let body_lines: Vec<Line<'static>> = wrapped.iter().map(line_to_static).collect();
            lines.extend(prefix_lines(body_lines, "  └ ".dim(), "    ".into()));
        }

        let mut detail_lines: Vec<Line<'static>> = Vec::new();
        // Reserve four columns for the tree prefix ("  └ "/"    ") and ensure the wrapper still has
        // at least one cell to work with.
        let detail_wrap_width = (width as usize).saturating_sub(4).max(1);

        if let Some(result) = &self.result {
            match result {
                Ok(mcp_types::CallToolResult { content, .. }) => {
                    if !content.is_empty() {
                        for block in content {
                            let text = Self::render_content_block(block, detail_wrap_width);
                            for segment in text.split('\n') {
                                let line = Line::from(segment.to_string().dim());
                                let wrapped = word_wrap_line(
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
                    let wrapped = word_wrap_line(
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

pub(crate) fn new_active_mcp_tool_call(
    call_id: String,
    invocation: McpInvocation,
    animations_enabled: bool,
) -> McpToolCallCell {
    McpToolCallCell::new(call_id, invocation, animations_enabled)
}

/// Placeholder cell for MCP tool calls that yielded an image.
///
/// The image itself is handled elsewhere; this cell keeps the history entry consistent while image
/// rendering support is implemented.
#[derive(Debug)]
pub(crate) struct CompletedMcpToolCallWithImageOutput {
    _image: DynamicImage,
}

impl HistoryCell for CompletedMcpToolCallWithImageOutput {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec!["tool result (image output)".into()]
    }
}

/// If the first content is an image, return a new cell with the image.
/// TODO(rgwood-dd): Handle images properly even if they're not the first result.
fn try_new_completed_mcp_tool_call_with_image_output(
    result: &Result<mcp_types::CallToolResult, String>,
) -> Option<CompletedMcpToolCallWithImageOutput> {
    match result {
        Ok(mcp_types::CallToolResult { content, .. }) => {
            if let Some(mcp_types::ContentBlock::ImageContent(image)) = content.first() {
                let raw_data = match base64::engine::general_purpose::STANDARD.decode(&image.data) {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to decode image data: {e}");
                        return None;
                    }
                };
                let reader = match ImageReader::new(Cursor::new(raw_data)).with_guessed_format() {
                    Ok(reader) => reader,
                    Err(e) => {
                        error!("Failed to guess image format: {e}");
                        return None;
                    }
                };

                let image = match reader.decode() {
                    Ok(image) => image,
                    Err(e) => {
                        error!("Image decoding failed: {e}");
                        return None;
                    }
                };

                Some(CompletedMcpToolCallWithImageOutput { _image: image })
            } else {
                None
            }
        }
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_cell::new_mcp_tools_output;
    use codex_core::config::Config;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;
    use codex_core::config::types::McpServerConfig;
    use codex_core::config::types::McpServerTransportConfig;
    use codex_core::protocol::McpAuthStatus;
    use mcp_types::CallToolResult;
    use mcp_types::ContentBlock;
    use mcp_types::TextContent;
    use mcp_types::Tool;
    use mcp_types::ToolInputSchema;
    use serde_json::json;
    use std::time::Duration;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config")
    }

    #[test]
    fn tools_output_masks_sensitive_values() {
        let mut config = test_config();
        let mut env = std::collections::HashMap::new();
        env.insert("TOKEN".to_string(), "secret".to_string());
        let stdio_config = McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: vec![],
                env: Some(env),
                env_vars: vec!["APP_TOKEN".to_string()],
                cwd: None,
            },
            enabled: true,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
        };
        config.mcp_servers.insert("docs".to_string(), stdio_config);

        let mut headers = std::collections::HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer secret".to_string());
        let mut env_headers = std::collections::HashMap::new();
        env_headers.insert("X-API-Key".to_string(), "API_KEY_ENV".to_string());
        let http_config = McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com/mcp".to_string(),
                bearer_token_env_var: Some("MCP_TOKEN".to_string()),
                http_headers: Some(headers),
                env_http_headers: Some(env_headers),
            },
            enabled: true,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
        };
        config.mcp_servers.insert("http".to_string(), http_config);

        let mut tools: std::collections::HashMap<String, Tool> = std::collections::HashMap::new();
        tools.insert(
            "mcp__docs__list".to_string(),
            Tool {
                annotations: None,
                description: None,
                input_schema: ToolInputSchema {
                    properties: None,
                    required: None,
                    r#type: "object".to_string(),
                },
                name: "list".to_string(),
                output_schema: None,
                title: None,
            },
        );
        tools.insert(
            "mcp__http__ping".to_string(),
            Tool {
                annotations: None,
                description: None,
                input_schema: ToolInputSchema {
                    properties: None,
                    required: None,
                    r#type: "object".to_string(),
                },
                name: "ping".to_string(),
                output_schema: None,
                title: None,
            },
        );

        let auth_statuses: std::collections::HashMap<String, McpAuthStatus> =
            std::collections::HashMap::new();
        let cell = new_mcp_tools_output(
            &config,
            tools,
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
            &auth_statuses,
        );
        let rendered = cell.display_string(120);

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn active_call_snapshot() {
        let invocation = McpInvocation {
            server: "search".into(),
            tool: "find_docs".into(),
            arguments: Some(json!({
                "query": "ratatui styling",
                "limit": 3,
            })),
        };

        let cell = new_active_mcp_tool_call("call-1".into(), invocation, true);
        let rendered = cell.display_string(80);

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn completed_call_success_snapshot() {
        let invocation = McpInvocation {
            server: "search".into(),
            tool: "find_docs".into(),
            arguments: Some(json!({
                "query": "ratatui styling",
                "limit": 3,
            })),
        };

        let result = CallToolResult {
            content: vec![ContentBlock::TextContent(TextContent {
                annotations: None,
                text: "Found styling guidance in styles.md".into(),
                r#type: "text".into(),
            })],
            is_error: None,
            structured_content: None,
        };

        let mut cell = new_active_mcp_tool_call("call-2".into(), invocation, true);
        assert!(
            cell.complete(Duration::from_millis(1420), Ok(result))
                .is_none()
        );

        let rendered = cell.display_string(80);

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn completed_call_error_snapshot() {
        let invocation = McpInvocation {
            server: "search".into(),
            tool: "find_docs".into(),
            arguments: Some(json!({
                "query": "ratatui styling",
                "limit": 3,
            })),
        };

        let mut cell = new_active_mcp_tool_call("call-3".into(), invocation, true);
        assert!(
            cell.complete(Duration::from_secs(2), Err("network timeout".into()))
                .is_none()
        );

        let rendered = cell.display_string(80);

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn completed_call_multiple_outputs_snapshot() {
        let invocation = McpInvocation {
            server: "search".into(),
            tool: "find_docs".into(),
            arguments: Some(json!({
                "query": "ratatui styling",
                "limit": 3,
            })),
        };

        let result = CallToolResult {
            content: vec![
                ContentBlock::TextContent(TextContent {
                    annotations: None,
                    text: "Found styling guidance in styles.md and additional notes in CONTRIBUTING.md.".into(),
                    r#type: "text".into(),
                }),
                ContentBlock::ResourceLink(ResourceLink {
                    annotations: None,
                    description: Some("Link to styles documentation".into()),
                    mime_type: None,
                    name: "styles.md".into(),
                    size: None,
                    title: Some("Styles".into()),
                    r#type: "resource_link".into(),
                    uri: "file:///docs/styles.md".into(),
                }),
            ],
            is_error: None,
            structured_content: None,
        };

        let mut cell = new_active_mcp_tool_call("call-4".into(), invocation, true);
        assert!(
            cell.complete(Duration::from_millis(640), Ok(result))
                .is_none()
        );

        let rendered = cell.display_string(48);

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn completed_call_wrapped_outputs_snapshot() {
        let invocation = McpInvocation {
            server: "metrics".into(),
            tool: "get_nearby_metric".into(),
            arguments: Some(json!({
                "query": "very_long_query_that_needs_wrapping_to_display_properly_in_the_history",
                "limit": 1,
            })),
        };

        let result = CallToolResult {
            content: vec![ContentBlock::TextContent(TextContent {
                annotations: None,
                text: "Line one of the response, which is quite long and needs wrapping.\nLine two continues the response with more detail.".into(),
                r#type: "text".into(),
            })],
            is_error: None,
            structured_content: None,
        };

        let mut cell = new_active_mcp_tool_call("call-5".into(), invocation, true);
        assert!(
            cell.complete(Duration::from_millis(1280), Ok(result))
                .is_none()
        );

        let rendered = cell.display_string(40);

        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn completed_call_multiple_outputs_inline_snapshot() {
        let invocation = McpInvocation {
            server: "metrics".into(),
            tool: "summary".into(),
            arguments: Some(json!({
                "metric": "trace.latency",
                "window": "15m",
            })),
        };

        let result = CallToolResult {
            content: vec![
                ContentBlock::TextContent(TextContent {
                    annotations: None,
                    text: "Latency summary: p50=120ms, p95=480ms.".into(),
                    r#type: "text".into(),
                }),
                ContentBlock::TextContent(TextContent {
                    annotations: None,
                    text: "No anomalies detected.".into(),
                    r#type: "text".into(),
                }),
            ],
            is_error: None,
            structured_content: None,
        };

        let mut cell = new_active_mcp_tool_call("call-6".into(), invocation, true);
        assert!(
            cell.complete(Duration::from_millis(320), Ok(result))
                .is_none()
        );

        let rendered = cell.display_string(120);

        insta::assert_snapshot!(rendered);
    }
}
