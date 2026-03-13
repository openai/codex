use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;

use codex_app_server_protocol::McpElicitationEnumSchema;
use codex_app_server_protocol::McpElicitationPrimitiveSchema;
use codex_app_server_protocol::McpElicitationSingleSelectEnumSchema;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::mcp::RequestId as McpRequestId;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::TextElement;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::ChatComposerConfig;
use crate::bottom_pane::InputResult;
use crate::bottom_pane::bottom_pane_view::BottomPaneView;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::measure_rows_height;
use crate::bottom_pane::selection_popup_common::menu_surface_inset;
use crate::bottom_pane::selection_popup_common::menu_surface_padding_height;
use crate::bottom_pane::selection_popup_common::render_menu_surface;
use crate::bottom_pane::selection_popup_common::render_rows;
use crate::render::renderable::Renderable;
use crate::text_formatting::format_json_compact;
use crate::text_formatting::truncate_text;

const ANSWER_PLACEHOLDER: &str = "Type your answer";
const OPTIONAL_ANSWER_PLACEHOLDER: &str = "Type your answer (optional)";
const FOOTER_SEPARATOR: &str = " | ";
const MIN_COMPOSER_HEIGHT: u16 = 3;
const MIN_OVERLAY_HEIGHT: u16 = 8;
const APPROVAL_FIELD_ID: &str = "__approval";
const APPROVAL_ACCEPT_ONCE_VALUE: &str = "accept";
const APPROVAL_ACCEPT_SESSION_VALUE: &str = "accept_session";
const APPROVAL_ACCEPT_ALWAYS_VALUE: &str = "accept_always";
const APPROVAL_DECLINE_VALUE: &str = "decline";
const APPROVAL_CANCEL_VALUE: &str = "cancel";
const APPROVAL_META_KIND_KEY: &str = "codex_approval_kind";
const APPROVAL_META_KIND_MCP_TOOL_CALL: &str = "mcp_tool_call";
const APPROVAL_META_KIND_TOOL_SUGGESTION: &str = "tool_suggestion";
const APPROVAL_PERSIST_KEY: &str = "persist";
const APPROVAL_PERSIST_SESSION_VALUE: &str = "session";
const APPROVAL_PERSIST_ALWAYS_VALUE: &str = "always";
const APPROVAL_TOOL_PARAMS_KEY: &str = "tool_params";
const APPROVAL_TOOL_PARAMS_DISPLAY_KEY: &str = "tool_params_display";
const APPROVAL_TOOL_PARAM_DISPLAY_LIMIT: usize = 3;
const APPROVAL_TOOL_PARAM_VALUE_TRUNCATE_GRAPHEMES: usize = 60;
const TOOL_TYPE_KEY: &str = "tool_type";
const TOOL_ID_KEY: &str = "tool_id";
const TOOL_NAME_KEY: &str = "tool_name";
const TOOL_SUGGEST_SUGGEST_TYPE_KEY: &str = "suggest_type";
const TOOL_SUGGEST_REASON_KEY: &str = "suggest_reason";
const TOOL_SUGGEST_INSTALL_URL_KEY: &str = "install_url";

#[derive(Clone, PartialEq, Default)]
struct ComposerDraft {
    text: String,
    text_elements: Vec<TextElement>,
    local_image_paths: Vec<PathBuf>,
    pending_pastes: Vec<(String, String)>,
}

impl ComposerDraft {
    fn text_with_pending(&self) -> String {
        if self.pending_pastes.is_empty() {
            return self.text.clone();
        }
        debug_assert!(
            !self.text_elements.is_empty(),
            "pending pastes should always have matching text elements"
        );
        let (expanded, _) = ChatComposer::expand_pending_pastes(
            &self.text,
            self.text_elements.clone(),
            &self.pending_pastes,
        );
        expanded
    }
}

#[derive(Clone, Debug, PartialEq)]
struct McpServerElicitationOption {
    label: String,
    description: Option<String>,
    value: Value,
}

#[derive(Clone, Debug, PartialEq)]
enum McpServerElicitationFieldInput {
    Select {
        options: Vec<McpServerElicitationOption>,
        default_idx: Option<usize>,
    },
    Text {
        secret: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct McpServerElicitationField {
    id: String,
    label: String,
    prompt: String,
    required: bool,
    input: McpServerElicitationFieldInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum McpServerElicitationResponseMode {
    FormContent,
    ApprovalAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolSuggestionToolType {
    Connector,
    Plugin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolSuggestionType {
    Install,
    Enable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolSuggestionRequest {
    pub(crate) tool_type: ToolSuggestionToolType,
    pub(crate) suggest_type: ToolSuggestionType,
    pub(crate) suggest_reason: String,
    pub(crate) tool_id: String,
    pub(crate) tool_name: String,
    pub(crate) install_url: String,
}

#[derive(Clone, Debug, PartialEq)]
struct McpToolApprovalDisplayParam {
    name: String,
    value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct McpServerElicitationFormRequest {
    thread_id: ThreadId,
    server_name: String,
    request_id: McpRequestId,
    message: String,
    approval_display_params: Vec<McpToolApprovalDisplayParam>,
    response_mode: McpServerElicitationResponseMode,
    fields: Vec<McpServerElicitationField>,
    tool_suggestion: Option<ToolSuggestionRequest>,
}

#[derive(Default)]
struct McpServerElicitationAnswerState {
    selection: ScrollState,
    draft: ComposerDraft,
    answer_committed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterTip {
    text: String,
    highlight: bool,
}

impl FooterTip {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            highlight: false,
        }
    }

    fn highlighted(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            highlight: true,
        }
    }
}

impl McpServerElicitationFormRequest {
    pub(crate) fn from_event(
        thread_id: ThreadId,
        request: ElicitationRequestEvent,
    ) -> Option<Self> {
        let ElicitationRequest::Form {
            meta,
            message,
            requested_schema,
        } = request.request
        else {
            return None;
        };

        let tool_suggestion = parse_tool_suggestion_request(meta.as_ref());
        let is_tool_approval = meta
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|meta| meta.get(APPROVAL_META_KIND_KEY))
            .and_then(Value::as_str)
            == Some(APPROVAL_META_KIND_MCP_TOOL_CALL);
        let is_empty_object_schema = requested_schema.as_object().is_some_and(|schema| {
            schema.get("type").and_then(Value::as_str) == Some("object")
                && schema
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_some_and(serde_json::Map::is_empty)
        });
        let is_tool_approval_action =
            is_tool_approval && (requested_schema.is_null() || is_empty_object_schema);
        let approval_display_params = if is_tool_approval_action {
            parse_tool_approval_display_params(meta.as_ref())
        } else {
            Vec::new()
        };

        let (response_mode, fields) = if tool_suggestion.is_some()
            && (requested_schema.is_null() || is_empty_object_schema)
        {
            (McpServerElicitationResponseMode::FormContent, Vec::new())
        } else if requested_schema.is_null() || (is_tool_approval && is_empty_object_schema) {
            let mut options = vec![McpServerElicitationOption {
                label: "Allow".to_string(),
                description: Some("Run the tool and continue.".to_string()),
                value: Value::String(APPROVAL_ACCEPT_ONCE_VALUE.to_string()),
            }];
            if is_tool_approval_action
                && tool_approval_supports_persist_mode(
                    meta.as_ref(),
                    APPROVAL_PERSIST_SESSION_VALUE,
                )
            {
                options.push(McpServerElicitationOption {
                    label: "Allow for this session".to_string(),
                    description: Some(
                        "Run the tool and remember this choice for this session.".to_string(),
                    ),
                    value: Value::String(APPROVAL_ACCEPT_SESSION_VALUE.to_string()),
                });
            }
            if is_tool_approval_action
                && tool_approval_supports_persist_mode(meta.as_ref(), APPROVAL_PERSIST_ALWAYS_VALUE)
            {
                options.push(McpServerElicitationOption {
                    label: "Always allow".to_string(),
                    description: Some(
                        "Run the tool and remember this choice for future tool calls.".to_string(),
                    ),
                    value: Value::String(APPROVAL_ACCEPT_ALWAYS_VALUE.to_string()),
                });
            }
            if is_tool_approval_action {
                options.push(McpServerElicitationOption {
                    label: "Cancel".to_string(),
                    description: Some("Cancel this tool call".to_string()),
                    value: Value::String(APPROVAL_CANCEL_VALUE.to_string()),
                });
            } else {
                options.extend([
                    McpServerElicitationOption {
                        label: "Deny".to_string(),
                        description: Some("Decline this tool call and continue.".to_string()),
                        value: Value::String(APPROVAL_DECLINE_VALUE.to_string()),
                    },
                    McpServerElicitationOption {
                        label: "Cancel".to_string(),
                        description: Some("Cancel this tool call".to_string()),
                        value: Value::String(APPROVAL_CANCEL_VALUE.to_string()),
                    },
                ]);
            }
            (
                McpServerElicitationResponseMode::ApprovalAction,
                vec![McpServerElicitationField {
                    id: APPROVAL_FIELD_ID.to_string(),
                    label: String::new(),
                    prompt: String::new(),
                    required: true,
                    input: McpServerElicitationFieldInput::Select {
                        options,
                        default_idx: Some(0),
                    },
                }],
            )
        } else {
            (
                McpServerElicitationResponseMode::FormContent,
                parse_fields_from_schema(&requested_schema)?,
            )
        };

        Some(Self {
            thread_id,
            server_name: request.server_name,
            request_id: request.id,
            message,
            approval_display_params,
            response_mode,
            fields,
            tool_suggestion,
        })
    }

    pub(crate) fn tool_suggestion(&self) -> Option<&ToolSuggestionRequest> {
        self.tool_suggestion.as_ref()
    }

    pub(crate) fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    pub(crate) fn server_name(&self) -> &str {
        self.server_name.as_str()
    }

    pub(crate) fn request_id(&self) -> &McpRequestId {
        &self.request_id
    }
}

fn parse_tool_suggestion_request(meta: Option<&Value>) -> Option<ToolSuggestionRequest> {
    let meta = meta?.as_object()?;
    if meta.get(APPROVAL_META_KIND_KEY).and_then(Value::as_str)
        != Some(APPROVAL_META_KIND_TOOL_SUGGESTION)
    {
        return None;
    }

    let tool_type = match meta.get(TOOL_TYPE_KEY).and_then(Value::as_str) {
        Some("connector") => ToolSuggestionToolType::Connector,
        Some("plugin") => ToolSuggestionToolType::Plugin,
        _ => return None,
    };
    let suggest_type = match meta
        .get(TOOL_SUGGEST_SUGGEST_TYPE_KEY)
        .and_then(Value::as_str)
    {
        Some("install") => ToolSuggestionType::Install,
        Some("enable") => ToolSuggestionType::Enable,
        _ => return None,
    };

    Some(ToolSuggestionRequest {
        tool_type,
        suggest_type,
        suggest_reason: meta
            .get(TOOL_SUGGEST_REASON_KEY)
            .and_then(Value::as_str)?
            .to_string(),
        tool_id: meta.get(TOOL_ID_KEY).and_then(Value::as_str)?.to_string(),
        tool_name: meta.get(TOOL_NAME_KEY).and_then(Value::as_str)?.to_string(),
        install_url: meta
            .get(TOOL_SUGGEST_INSTALL_URL_KEY)
            .and_then(Value::as_str)?
            .to_string(),
    })
}

fn tool_approval_supports_persist_mode(meta: Option<&Value>, expected_mode: &str) -> bool {
    let Some(persist) = meta
        .and_then(Value::as_object)
        .and_then(|meta| meta.get(APPROVAL_PERSIST_KEY))
    else {
        return false;
    };

    match persist {
        Value::String(value) => value == expected_mode,
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value == expected_mode),
        _ => false,
    }
}

fn parse_tool_approval_display_params(meta: Option<&Value>) -> Vec<McpToolApprovalDisplayParam> {
    let Some(meta) = meta.and_then(Value::as_object) else {
        return Vec::new();
    };

    let display_params = meta
        .get(APPROVAL_TOOL_PARAMS_DISPLAY_KEY)
        .and_then(Value::as_array)
        .map(|display_params| {
            display_params
                .iter()
                .filter_map(parse_tool_approval_display_param)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !display_params.is_empty() {
        return display_params;
    }

    let mut fallback_params = meta
        .get(APPROVAL_TOOL_PARAMS_KEY)
        .and_then(Value::as_object)
        .map(|tool_params| {
            tool_params
                .iter()
                .map(|(name, value)| McpToolApprovalDisplayParam {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    fallback_params.sort_by(|left, right| left.name.cmp(&right.name));
    fallback_params
}

fn parse_tool_approval_display_param(value: &Value) -> Option<McpToolApprovalDisplayParam> {
    let value = value.as_object()?;
    let name = value.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(McpToolApprovalDisplayParam {
        name: name.to_string(),
        value: value.get("value")?.clone(),
    })
}

fn format_tool_approval_display_message(
    message: &str,
    approval_display_params: &[McpToolApprovalDisplayParam],
) -> String {
    let message = message.trim();
    if approval_display_params.is_empty() {
        return message.to_string();
    }

    let mut sections = Vec::new();
    if !message.is_empty() {
        sections.push(message.to_string());
    }
    let param_lines = approval_display_params
        .iter()
        .take(APPROVAL_TOOL_PARAM_DISPLAY_LIMIT)
        .map(format_tool_approval_display_param_line)
        .collect::<Vec<_>>();
    if !param_lines.is_empty() {
        sections.push(param_lines.join("\n"));
    }
    let mut message = sections.join("\n\n");
    message.push('\n');
    message
}

fn format_tool_approval_display_param_line(param: &McpToolApprovalDisplayParam) -> String {
    format!(
        "{}: {}",
        param.name,
        format_tool_approval_display_param_value(&param.value)
    )
}

fn format_tool_approval_display_param_value(value: &Value) -> String {
    let formatted = match value {
        Value::String(text) => text.split_whitespace().collect::<Vec<_>>().join(" "),
        _ => {
            let compact_json = value.to_string();
            format_json_compact(&compact_json).unwrap_or(compact_json)
        }
    };
    truncate_text(&formatted, APPROVAL_TOOL_PARAM_VALUE_TRUNCATE_GRAPHEMES)
}

fn parse_fields_from_schema(requested_schema: &Value) -> Option<Vec<McpServerElicitationField>> {
    let schema = requested_schema.as_object()?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return None;
    }
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    let properties = schema.get("properties")?.as_object()?;
    let mut fields = Vec::new();
    for (id, property_schema) in properties {
        let property =
            serde_json::from_value::<McpElicitationPrimitiveSchema>(property_schema.clone())
                .ok()?;
        fields.push(parse_field(id, property, required.contains(id))?);
    }
    if fields.is_empty() {
        return None;
    }
    Some(fields)
}

fn parse_field(
    id: &str,
    property: McpElicitationPrimitiveSchema,
    required: bool,
) -> Option<McpServerElicitationField> {
    match property {
        McpElicitationPrimitiveSchema::String(schema) => {
            let label = schema.title.unwrap_or_else(|| id.to_string());
            let prompt = schema.description.unwrap_or_else(|| label.clone());
            Some(McpServerElicitationField {
                id: id.to_string(),
                label,
                prompt,
                required,
                input: McpServerElicitationFieldInput::Text { secret: false },
            })
        }
        McpElicitationPrimitiveSchema::Boolean(schema) => {
            let label = schema.title.unwrap_or_else(|| id.to_string());
            let prompt = schema.description.unwrap_or_else(|| label.clone());
            let default_idx = schema.default.map(|value| if value { 0 } else { 1 });
            let options = [true, false]
                .into_iter()
                .map(|value| {
                    let label = if value { "True" } else { "False" }.to_string();
                    McpServerElicitationOption {
                        label,
                        description: None,
                        value: Value::Bool(value),
                    }
                })
                .collect();
            Some(McpServerElicitationField {
                id: id.to_string(),
                label,
                prompt,
                required,
                input: McpServerElicitationFieldInput::Select {
                    options,
                    default_idx,
                },
            })
        }
        McpElicitationPrimitiveSchema::Enum(McpElicitationEnumSchema::Legacy(schema)) => {
            let label = schema.title.unwrap_or_else(|| id.to_string());
            let prompt = schema.description.unwrap_or_else(|| label.clone());
            let default_idx = schema
                .default
                .as_ref()
                .and_then(|value| schema.enum_.iter().position(|entry| entry == value));
            let enum_names = schema.enum_names.unwrap_or_default();
            let options = schema
                .enum_
                .into_iter()
                .enumerate()
                .map(|(idx, value)| McpServerElicitationOption {
                    label: enum_names
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| value.clone()),
                    description: None,
                    value: Value::String(value),
                })
                .collect();
            Some(McpServerElicitationField {
                id: id.to_string(),
                label,
                prompt,
                required,
                input: McpServerElicitationFieldInput::Select {
                    options,
                    default_idx,
                },
            })
        }
        McpElicitationPrimitiveSchema::Enum(McpElicitationEnumSchema::SingleSelect(schema)) => {
            parse_single_select_field(id, schema, required)
        }
        McpElicitationPrimitiveSchema::Number(_)
        | McpElicitationPrimitiveSchema::Enum(McpElicitationEnumSchema::MultiSelect(_)) => None,
    }
}

fn parse_single_select_field(
    id: &str,
    schema: McpElicitationSingleSelectEnumSchema,
    required: bool,
) -> Option<McpServerElicitationField> {
    match schema {
        McpElicitationSingleSelectEnumSchema::Untitled(schema) => {
            let label = schema.title.unwrap_or_else(|| id.to_string());
            let prompt = schema.description.unwrap_or_else(|| label.clone());
            let default_idx = schema
                .default
                .as_ref()
                .and_then(|value| schema.enum_.iter().position(|entry| entry == value));
            let options = schema
                .enum_
                .into_iter()
                .map(|value| McpServerElicitationOption {
                    label: value.clone(),
                    description: None,
                    value: Value::String(value),
                })
                .collect();
            Some(McpServerElicitationField {
                id: id.to_string(),
                label,
                prompt,
                required,
                input: McpServerElicitationFieldInput::Select {
                    options,
                    default_idx,
                },
            })
        }
        McpElicitationSingleSelectEnumSchema::Titled(schema) => {
            let label = schema.title.unwrap_or_else(|| id.to_string());
            let prompt = schema.description.unwrap_or_else(|| label.clone());
            let default_idx = schema.default.as_ref().and_then(|value| {
                schema
                    .one_of
                    .iter()
                    .position(|entry| entry.const_.as_str() == value)
            });
            let options = schema
                .one_of
                .into_iter()
                .map(|entry| McpServerElicitationOption {
                    label: entry.title,
                    description: None,
                    value: Value::String(entry.const_),
                })
                .collect();
            Some(McpServerElicitationField {
                id: id.to_string(),
                label,
                prompt,
                required,
                input: McpServerElicitationFieldInput::Select {
                    options,
                    default_idx,
                },
            })
        }
    }
}

pub(crate) struct McpServerElicitationOverlay {
    app_event_tx: AppEventSender,
    request: McpServerElicitationFormRequest,
    queue: VecDeque<McpServerElicitationFormRequest>,
    composer: ChatComposer,
    answers: Vec<McpServerElicitationAnswerState>,
    current_idx: usize,
    done: bool,
    validation_error: Option<String>,
}

impl McpServerElicitationOverlay {
    pub(crate) fn new(
        request: McpServerElicitationFormRequest,
        app_event_tx: AppEventSender,
        has_input_focus: bool,
        enhanced_keys_supported: bool,
        disable_paste_burst: bool,
    ) -> Self {
        let mut composer = ChatComposer::new_with_config(
            has_input_focus,
            app_event_tx.clone(),
            enhanced_keys_supported,
            ANSWER_PLACEHOLDER.to_string(),
            disable_paste_burst,
            ChatComposerConfig::plain_text(),
        );
        composer.set_footer_hint_override(Some(Vec::new()));
        let mut overlay = Self {
            app_event_tx,
            request,
            queue: VecDeque::new(),
            composer,
            answers: Vec::new(),
            current_idx: 0,
            done: false,
            validation_error: None,
        };
        overlay.reset_for_request();
        overlay.restore_current_draft();
        overlay
    }

    fn reset_for_request(&mut self) {
        self.answers = self
            .request
            .fields
            .iter()
            .map(|field| {
                let mut selection = ScrollState::new();
                let (draft, answer_committed) = match &field.input {
                    McpServerElicitationFieldInput::Select { default_idx, .. } => {
                        selection.selected_idx = default_idx.or(Some(0));
                        (ComposerDraft::default(), default_idx.is_some())
                    }
                    McpServerElicitationFieldInput::Text { .. } => {
                        (ComposerDraft::default(), false)
                    }
                };
                McpServerElicitationAnswerState {
                    selection,
                    draft,
                    answer_committed,
                }
            })
            .collect();
        self.current_idx = 0;
        self.validation_error = None;
        self.composer
            .set_text_content(String::new(), Vec::new(), Vec::new());
    }

    fn field_count(&self) -> usize {
        self.request.fields.len()
    }

    fn current_index(&self) -> usize {
        self.current_idx
    }

    fn current_field(&self) -> Option<&McpServerElicitationField> {
        self.request.fields.get(self.current_index())
    }

    fn current_answer(&self) -> Option<&McpServerElicitationAnswerState> {
        self.answers.get(self.current_index())
    }

    fn current_answer_mut(&mut self) -> Option<&mut McpServerElicitationAnswerState> {
        let idx = self.current_idx;
        self.answers.get_mut(idx)
    }

    fn capture_composer_draft(&self) -> ComposerDraft {
        ComposerDraft {
            text: self.composer.current_text(),
            text_elements: self.composer.text_elements(),
            local_image_paths: self
                .composer
                .local_images()
                .into_iter()
                .map(|img| img.path)
                .collect(),
            pending_pastes: self.composer.pending_pastes(),
        }
    }

    fn restore_current_draft(&mut self) {
        self.composer
            .set_placeholder_text(self.answer_placeholder().to_string());
        self.composer.set_footer_hint_override(Some(Vec::new()));
        if self.current_field_is_select() {
            self.composer
                .set_text_content(String::new(), Vec::new(), Vec::new());
            self.composer.move_cursor_to_end();
            return;
        }
        let Some(answer) = self.current_answer() else {
            self.composer
                .set_text_content(String::new(), Vec::new(), Vec::new());
            self.composer.move_cursor_to_end();
            return;
        };
        let draft = answer.draft.clone();
        self.composer
            .set_text_content(draft.text, draft.text_elements, draft.local_image_paths);
        self.composer.set_pending_pastes(draft.pending_pastes);
        self.composer.move_cursor_to_end();
    }

    fn save_current_draft(&mut self) {
        if self.current_field_is_select() {
            return;
        }
        let draft = self.capture_composer_draft();
        if let Some(answer) = self.current_answer_mut() {
            if answer.answer_committed && answer.draft != draft {
                answer.answer_committed = false;
            }
            answer.draft = draft;
        }
    }

    fn clear_current_draft(&mut self) {
        if self.current_field_is_select() {
            return;
        }
        if let Some(answer) = self.current_answer_mut() {
            answer.draft = ComposerDraft::default();
            answer.answer_committed = false;
        }
        self.composer
            .set_text_content(String::new(), Vec::new(), Vec::new());
        self.composer.move_cursor_to_end();
    }

    fn answer_placeholder(&self) -> &'static str {
        self.current_field().map_or(ANSWER_PLACEHOLDER, |field| {
            if field.required {
                ANSWER_PLACEHOLDER
            } else {
                OPTIONAL_ANSWER_PLACEHOLDER
            }
        })
    }

    fn current_field_is_select(&self) -> bool {
        matches!(
            self.current_field().map(|field| &field.input),
            Some(McpServerElicitationFieldInput::Select { .. })
        )
    }

    fn current_field_is_secret(&self) -> bool {
        matches!(
            self.current_field().map(|field| &field.input),
            Some(McpServerElicitationFieldInput::Text { secret: true })
        )
    }

    fn selected_option_index(&self) -> Option<usize> {
        self.current_answer()
            .and_then(|answer| answer.selection.selected_idx)
    }

    fn options_len(&self) -> usize {
        self.current_options().len()
    }

    fn current_options(&self) -> &[McpServerElicitationOption] {
        match self.current_field().map(|field| &field.input) {
            Some(McpServerElicitationFieldInput::Select { options, .. }) => options.as_slice(),
            _ => &[],
        }
    }

    fn option_rows(&self) -> Vec<GenericDisplayRow> {
        let selected_idx = self.selected_option_index();
        self.current_options()
            .iter()
            .enumerate()
            .map(|(idx, option)| {
                let prefix = if selected_idx.is_some_and(|selected| selected == idx) {
                    '›'
                } else {
                    ' '
                };
                let number = idx + 1;
                let prefix_label = format!("{prefix} {number}. ");
                let wrap_indent = UnicodeWidthStr::width(prefix_label.as_str());
                GenericDisplayRow {
                    name: format!("{prefix_label}{}", option.label),
                    description: option.description.clone(),
                    wrap_indent: Some(wrap_indent),
                    ..Default::default()
                }
            })
            .collect()
    }

    fn wrapped_prompt_lines(&self, width: u16) -> Vec<String> {
        textwrap::wrap(&self.current_prompt_text(), width.max(1) as usize)
            .into_iter()
            .map(|line| line.to_string())
            .collect()
    }

    fn current_prompt_text(&self) -> String {
        let request_message = format_tool_approval_display_message(
            &self.request.message,
            &self.request.approval_display_params,
        );
        let Some(field) = self.current_field() else {
            return request_message;
        };
        let mut sections = Vec::new();
        if !request_message.trim().is_empty() {
            sections.push(request_message);
        }
        let field_prompt = if field.label.trim().is_empty()
            || field.prompt.trim().is_empty()
            || field.label == field.prompt
        {
            if field.prompt.trim().is_empty() {
                field.label.clone()
            } else {
                field.prompt.clone()
            }
        } else {
            format!("{}\n{}", field.label, field.prompt)
        };
        if !field_prompt.trim().is_empty() {
            sections.push(field_prompt);
        }
        sections.join("\n\n")
    }

    fn footer_tips(&self) -> Vec<FooterTip> {
        let mut tips = Vec::new();
        let is_last_field = self.current_index().saturating_add(1) >= self.field_count();
        if self.current_field_is_select() {
            if self.field_count() == 1 {
                tips.push(FooterTip::highlighted("enter to submit"));
            } else if is_last_field {
                tips.push(FooterTip::highlighted("enter to submit all"));
            } else {
                tips.push(FooterTip::new("enter to submit answer"));
            }
        } else if self.field_count() == 1 {
            tips.push(FooterTip::highlighted("enter to submit"));
        } else if is_last_field {
            tips.push(FooterTip::highlighted("enter to submit all"));
        } else {
            tips.push(FooterTip::new("enter to submit answer"));
        }
        if self.field_count() > 1 {
            if self.current_field_is_select() {
                tips.push(FooterTip::new("←/→ to navigate fields"));
            } else {
                tips.push(FooterTip::new("ctrl + p / ctrl + n change field"));
            }
        }
        tips.push(FooterTip::new("esc to cancel"));
        tips
    }

    fn footer_tip_lines(&self, width: u16) -> Vec<Vec<FooterTip>> {
        let mut tips = Vec::new();
        if let Some(error) = self.validation_error.as_ref() {
            tips.push(FooterTip::highlighted(error.clone()));
        }
        tips.extend(self.footer_tips());
        wrap_footer_tips(width, tips)
    }

    fn options_required_height(&self, width: u16) -> u16 {
        let rows = self.option_rows();
        if rows.is_empty() {
            return 0;
        }
        let mut state = self
            .current_answer()
            .map(|answer| answer.selection)
            .unwrap_or_default();
        if state.selected_idx.is_none() {
            state.selected_idx = Some(0);
        }
        measure_rows_height(&rows, &state, rows.len(), width.max(1))
    }

    fn input_height(&self, width: u16) -> u16 {
        if self.current_field_is_select() {
            return self.options_required_height(width);
        }
        self.composer
            .desired_height(width.max(1))
            .clamp(MIN_COMPOSER_HEIGHT, MIN_COMPOSER_HEIGHT.saturating_add(5))
    }

    fn move_field(&mut self, next: bool) {
        let len = self.field_count();
        if len == 0 {
            return;
        }
        self.save_current_draft();
        let offset = if next { 1 } else { len.saturating_sub(1) };
        self.current_idx = (self.current_idx + offset) % len;
        self.validation_error = None;
        self.restore_current_draft();
    }

    fn jump_to_field(&mut self, idx: usize) {
        if idx >= self.field_count() {
            return;
        }
        self.save_current_draft();
        self.current_idx = idx;
        self.restore_current_draft();
    }

    fn field_value(&self, idx: usize) -> Option<Value> {
        let field = self.request.fields.get(idx)?;
        let answer = self.answers.get(idx)?;
        match &field.input {
            McpServerElicitationFieldInput::Select { options, .. } => {
                if !answer.answer_committed {
                    return None;
                }
                let selected_idx = answer.selection.selected_idx?;
                options.get(selected_idx).map(|option| option.value.clone())
            }
            McpServerElicitationFieldInput::Text { .. } => {
                if !answer.answer_committed {
                    return None;
                }
                let text = answer.draft.text_with_pending();
                let text = text.trim();
                (!text.is_empty()).then(|| Value::String(text.to_string()))
            }
        }
    }

    fn required_unanswered_count(&self) -> usize {
        self.request
            .fields
            .iter()
            .enumerate()
            .filter(|(idx, field)| field.required && self.field_value(*idx).is_none())
            .count()
    }

    fn first_required_unanswered_index(&self) -> Option<usize> {
        self.request
            .fields
            .iter()
            .enumerate()
            .find(|(idx, field)| field.required && self.field_value(*idx).is_none())
            .map(|(idx, _)| idx)
    }

    fn is_current_field_answered(&self) -> bool {
        self.field_value(self.current_index()).is_some()
    }

    fn option_index_for_digit(&self, ch: char) -> Option<usize> {
        let digit = ch.to_digit(10)?;
        if digit == 0 {
            return None;
        }
        let idx = (digit - 1) as usize;
        (idx < self.options_len()).then_some(idx)
    }

    fn select_current_option(&mut self, committed: bool) {
        let options_len = self.options_len();
        if let Some(answer) = self.current_answer_mut() {
            answer.selection.clamp_selection(options_len);
            answer.answer_committed = committed;
        }
    }

    fn clear_selection(&mut self) {
        if let Some(answer) = self.current_answer_mut() {
            answer.selection.reset();
            answer.answer_committed = false;
        }
    }

    fn dispatch_cancel(&self) {
        self.app_event_tx.send(AppEvent::SubmitThreadOp {
            thread_id: self.request.thread_id,
            op: Op::ResolveElicitation {
                server_name: self.request.server_name.clone(),
                request_id: self.request.request_id.clone(),
                decision: ElicitationAction::Cancel,
                content: None,
                meta: None,
            },
        });
    }

    fn submit_answers(&mut self) {
        self.save_current_draft();
        if let Some(idx) = self.first_required_unanswered_index() {
            self.validation_error = Some("Answer required fields before submitting.".to_string());
            self.jump_to_field(idx);
            return;
        }
        self.validation_error = None;
        if self.request.response_mode == McpServerElicitationResponseMode::ApprovalAction {
            let (decision, meta) = match self.field_value(0).as_ref().and_then(Value::as_str) {
                Some(APPROVAL_ACCEPT_ONCE_VALUE) => (ElicitationAction::Accept, None),
                Some(APPROVAL_ACCEPT_SESSION_VALUE) => (
                    ElicitationAction::Accept,
                    Some(serde_json::json!({
                        APPROVAL_PERSIST_KEY: APPROVAL_PERSIST_SESSION_VALUE,
                    })),
                ),
                Some(APPROVAL_ACCEPT_ALWAYS_VALUE) => (
                    ElicitationAction::Accept,
                    Some(serde_json::json!({
                        APPROVAL_PERSIST_KEY: APPROVAL_PERSIST_ALWAYS_VALUE,
                    })),
                ),
                Some(APPROVAL_DECLINE_VALUE) => (ElicitationAction::Decline, None),
                Some(APPROVAL_CANCEL_VALUE) => (ElicitationAction::Cancel, None),
                _ => (ElicitationAction::Cancel, None),
            };
            self.app_event_tx.send(AppEvent::SubmitThreadOp {
                thread_id: self.request.thread_id,
                op: Op::ResolveElicitation {
                    server_name: self.request.server_name.clone(),
                    request_id: self.request.request_id.clone(),
                    decision,
                    content: None,
                    meta,
                },
            });
            if let Some(next) = self.queue.pop_front() {
                self.request = next;
                self.reset_for_request();
                self.restore_current_draft();
            } else {
                self.done = true;
            }
            return;
        }
        let content = self
            .request
            .fields
            .iter()
            .enumerate()
            .filter_map(|(idx, field)| self.field_value(idx).map(|value| (field.id.clone(), value)))
            .collect::<serde_json::Map<_, _>>();
        self.app_event_tx.send(AppEvent::SubmitThreadOp {
            thread_id: self.request.thread_id,
            op: Op::ResolveElicitation {
                server_name: self.request.server_name.clone(),
                request_id: self.request.request_id.clone(),
                decision: ElicitationAction::Accept,
                content: Some(Value::Object(content)),
                meta: None,
            },
        });
        if let Some(next) = self.queue.pop_front() {
            self.request = next;
            self.reset_for_request();
            self.restore_current_draft();
        } else {
            self.done = true;
        }
    }

    fn go_next_or_submit(&mut self) {
        if self.current_index() + 1 >= self.field_count() {
            self.submit_answers();
        } else {
            self.move_field(true);
        }
    }

    fn apply_submission_to_draft(&mut self, text: String, text_elements: Vec<TextElement>) {
        let local_image_paths = self
            .composer
            .local_images()
            .into_iter()
            .map(|img| img.path)
            .collect::<Vec<_>>();
        if let Some(answer) = self.current_answer_mut() {
            answer.draft = ComposerDraft {
                text: text.clone(),
                text_elements: text_elements.clone(),
                local_image_paths: local_image_paths.clone(),
                pending_pastes: Vec::new(),
            };
            answer.answer_committed = !text.trim().is_empty();
        }
        self.composer
            .set_text_content(text, text_elements, local_image_paths);
        self.composer.move_cursor_to_end();
        self.composer.set_footer_hint_override(Some(Vec::new()));
    }

    fn handle_composer_input_result(&mut self, result: InputResult) -> bool {
        match result {
            InputResult::Submitted {
                text,
                text_elements,
            }
            | InputResult::Queued {
                text,
                text_elements,
            } => {
                self.apply_submission_to_draft(text, text_elements);
                self.validation_error = None;
                self.go_next_or_submit();
                true
            }
            _ => false,
        }
    }

    fn render_prompt(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let answered = self.is_current_field_answered();
        for (offset, line) in self.wrapped_prompt_lines(area.width).iter().enumerate() {
            let y = area.y.saturating_add(offset as u16);
            if y >= area.y + area.height {
                break;
            }
            let line = if answered {
                Line::from(line.clone())
            } else {
                Line::from(line.clone()).cyan()
            };
            Paragraph::new(line).render(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }

    fn render_input(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        if self.current_field_is_select() {
            let rows = self.option_rows();
            let mut state = self
                .current_answer()
                .map(|answer| answer.selection)
                .unwrap_or_default();
            if state.selected_idx.is_none() && !rows.is_empty() {
                state.selected_idx = Some(0);
            }
            state.ensure_visible(rows.len(), area.height as usize);
            render_rows(area, buf, &rows, &state, rows.len().max(1), "No options");
            return;
        }
        if self.current_field_is_secret() {
            self.composer.render_with_mask(area, buf, Some('*'));
        } else {
            self.composer.render(area, buf);
        }
    }

    fn render_footer(&self, area: Rect, input_area_height: u16, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let options_hidden = self.current_field_is_select()
            && input_area_height > 0
            && self.options_required_height(area.width) > input_area_height;
        let option_tip = if options_hidden {
            let selected = self.selected_option_index().unwrap_or(0).saturating_add(1);
            let total = self.options_len();
            Some(FooterTip::new(format!("option {selected}/{total}")))
        } else {
            None
        };
        let mut tip_lines = self.footer_tip_lines(area.width);
        if let Some(prefix) = option_tip {
            let mut tips = vec![prefix];
            if let Some(first_line) = tip_lines.first_mut() {
                let mut first = Vec::new();
                std::mem::swap(first_line, &mut first);
                tips.extend(first);
                *first_line = tips;
            } else {
                tip_lines.push(tips);
            }
        }
        for (row_idx, tips) in tip_lines.into_iter().take(area.height as usize).enumerate() {
            let mut spans = Vec::new();
            for (tip_idx, tip) in tips.into_iter().enumerate() {
                if tip_idx > 0 {
                    spans.push(FOOTER_SEPARATOR.into());
                }
                if tip.highlight {
                    spans.push(tip.text.cyan().bold().not_dim());
                } else {
                    spans.push(tip.text.into());
                }
            }
            let line = Line::from(spans).dim();
            Paragraph::new(line).render(
                Rect {
                    x: area.x,
                    y: area.y.saturating_add(row_idx as u16),
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}

impl Renderable for McpServerElicitationOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        let outer = Rect::new(0, 0, width, u16::MAX);
        let inner = menu_surface_inset(outer);
        let inner_width = inner.width.max(1);
        let height = 1u16
            .saturating_add(self.wrapped_prompt_lines(inner_width).len() as u16)
            .saturating_add(self.input_height(inner_width))
            .saturating_add(self.footer_tip_lines(inner_width).len() as u16)
            .saturating_add(menu_surface_padding_height());
        height.max(MIN_OVERLAY_HEIGHT)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let content_area = render_menu_surface(area, buf);
        if content_area.width == 0 || content_area.height == 0 {
            return;
        }
        let prompt_lines = self.wrapped_prompt_lines(content_area.width);
        let footer_lines = self.footer_tip_lines(content_area.width);
        let mut remaining = content_area.height;

        let progress_height = u16::from(remaining > 0);
        remaining = remaining.saturating_sub(progress_height);

        let footer_height = (footer_lines.len() as u16).min(remaining.saturating_sub(1));
        remaining = remaining.saturating_sub(footer_height);

        let min_input_height = if self.current_field_is_select() {
            u16::from(remaining > 0)
        } else {
            MIN_COMPOSER_HEIGHT.min(remaining)
        };
        let mut input_height = min_input_height;
        remaining = remaining.saturating_sub(input_height);

        let prompt_height = (prompt_lines.len() as u16).min(remaining);
        remaining = remaining.saturating_sub(prompt_height);
        input_height = input_height.saturating_add(remaining);

        let progress_area = Rect {
            x: content_area.x,
            y: content_area.y,
            width: content_area.width,
            height: progress_height,
        };
        let prompt_area = Rect {
            x: content_area.x,
            y: progress_area.y.saturating_add(progress_area.height),
            width: content_area.width,
            height: prompt_height,
        };
        let input_area = Rect {
            x: content_area.x,
            y: prompt_area.y.saturating_add(prompt_area.height),
            width: content_area.width,
            height: input_height,
        };
        let footer_area = Rect {
            x: content_area.x,
            y: input_area.y.saturating_add(input_area.height),
            width: content_area.width,
            height: footer_height,
        };

        let unanswered = self.required_unanswered_count();
        let progress_line = if self.field_count() > 0 {
            let idx = self.current_index() + 1;
            let total = self.field_count();
            let base = format!("Field {idx}/{total}");
            if unanswered > 0 {
                Line::from(format!("{base} ({unanswered} required unanswered)").dim())
            } else {
                Line::from(base.dim())
            }
        } else {
            Line::from("No fields".dim())
        };
        Paragraph::new(progress_line).render(progress_area, buf);
        self.render_prompt(prompt_area, buf);
        self.render_input(input_area, buf);
        self.render_footer(footer_area, input_area.height, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if self.current_field_is_select() {
            return None;
        }
        let content_area = menu_surface_inset(area);
        if content_area.width == 0 || content_area.height == 0 {
            return None;
        }
        let prompt_lines = self.wrapped_prompt_lines(content_area.width);
        let footer_lines = self.footer_tip_lines(content_area.width);
        let mut remaining = content_area.height;
        remaining = remaining.saturating_sub(u16::from(remaining > 0));
        let footer_height = (footer_lines.len() as u16).min(remaining.saturating_sub(1));
        remaining = remaining.saturating_sub(footer_height);
        let min_input_height = MIN_COMPOSER_HEIGHT.min(remaining);
        let mut input_height = min_input_height;
        remaining = remaining.saturating_sub(input_height);
        let prompt_height = (prompt_lines.len() as u16).min(remaining);
        remaining = remaining.saturating_sub(prompt_height);
        input_height = input_height.saturating_add(remaining);
        let input_area = Rect {
            x: content_area.x,
            y: content_area
                .y
                .saturating_add(1)
                .saturating_add(prompt_height),
            width: content_area.width,
            height: input_height,
        };
        self.composer.cursor_pos(input_area)
    }
}

impl BottomPaneView for McpServerElicitationOverlay {
    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }

        if matches!(key_event.code, KeyCode::Esc) {
            self.dispatch_cancel();
            self.done = true;
            return;
        }

        match key_event {
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_field(false);
                return;
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_field(true);
                return;
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } if self.current_field_is_select() => {
                self.move_field(false);
                return;
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } if self.current_field_is_select() => {
                self.move_field(true);
                return;
            }
            _ => {}
        }

        if self.current_field_is_select() {
            self.validation_error = None;
            let options_len = self.options_len();
            match key_event.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(answer) = self.current_answer_mut() {
                        answer.selection.move_up_wrap(options_len);
                        answer.answer_committed = false;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(answer) = self.current_answer_mut() {
                        answer.selection.move_down_wrap(options_len);
                        answer.answer_committed = false;
                    }
                }
                KeyCode::Backspace | KeyCode::Delete => self.clear_selection(),
                KeyCode::Char(' ') => self.select_current_option(true),
                KeyCode::Enter => {
                    if self.selected_option_index().is_some() {
                        self.select_current_option(true);
                    }
                    self.go_next_or_submit();
                }
                KeyCode::Char(ch) => {
                    if let Some(option_idx) = self.option_index_for_digit(ch) {
                        if let Some(answer) = self.current_answer_mut() {
                            answer.selection.selected_idx = Some(option_idx);
                        }
                        self.select_current_option(true);
                        self.go_next_or_submit();
                    }
                }
                _ => {}
            }
            return;
        }

        let before = self.capture_composer_draft();
        let (result, _) = self.composer.handle_key_event(key_event);
        let submitted = self.handle_composer_input_result(result);
        if submitted {
            return;
        }
        let after = self.capture_composer_draft();
        if before != after {
            self.validation_error = None;
            if let Some(answer) = self.current_answer_mut() {
                answer.answer_committed = false;
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if !self.current_field_is_select() && !self.composer.current_text_with_pending().is_empty()
        {
            self.clear_current_draft();
            return CancellationEvent::Handled;
        }
        self.dispatch_cancel();
        self.done = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.done
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() || self.current_field_is_select() {
            return false;
        }
        self.validation_error = None;
        if let Some(answer) = self.current_answer_mut() {
            answer.answer_committed = false;
        }
        self.composer.handle_paste(pasted)
    }

    fn flush_paste_burst_if_due(&mut self) -> bool {
        self.composer.flush_paste_burst_if_due()
    }

    fn is_in_paste_burst(&self) -> bool {
        self.composer.is_in_paste_burst()
    }

    fn try_consume_mcp_server_elicitation_request(
        &mut self,
        request: McpServerElicitationFormRequest,
    ) -> Option<McpServerElicitationFormRequest> {
        self.queue.push_back(request);
        None
    }
}

fn wrap_footer_tips(width: u16, tips: Vec<FooterTip>) -> Vec<Vec<FooterTip>> {
    let max_width = width.max(1) as usize;
    let separator_width = UnicodeWidthStr::width(FOOTER_SEPARATOR);
    if tips.is_empty() {
        return vec![Vec::new()];
    }

    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut used = 0usize;

    for tip in tips {
        let tip_width = UnicodeWidthStr::width(tip.text.as_str()).min(max_width);
        let extra = if current.is_empty() {
            tip_width
        } else {
            separator_width.saturating_add(tip_width)
        };
        if !current.is_empty() && used.saturating_add(extra) > max_width {
            lines.push(current);
            current = Vec::new();
            used = 0;
        }
        if current.is_empty() {
            used = tip_width;
        } else {
            used = used
                .saturating_add(separator_width)
                .saturating_add(tip_width);
        }
        current.push(tip);
    }

    if current.is_empty() {
        lines.push(Vec::new());
    } else {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::render::renderable::Renderable;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::sync::mpsc::unbounded_channel;

    fn test_sender() -> (AppEventSender, UnboundedReceiver<AppEvent>) {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        (AppEventSender::new(tx_raw), rx)
    }

    fn form_request(
        message: &str,
        requested_schema: Value,
        meta: Option<Value>,
    ) -> ElicitationRequestEvent {
        ElicitationRequestEvent {
            turn_id: Some("turn-1".to_string()),
            server_name: "server-1".to_string(),
            id: McpRequestId::String("request-1".to_string()),
            request: ElicitationRequest::Form {
                meta,
                message: message.to_string(),
                requested_schema,
            },
        }
    }

    fn empty_object_schema() -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
        })
    }

    fn tool_approval_meta(
        persist_modes: &[&str],
        tool_params: Option<Value>,
        tool_params_display: Option<Vec<(&str, Value)>>,
    ) -> Option<Value> {
        let mut meta = serde_json::Map::from_iter([(
            APPROVAL_META_KIND_KEY.to_string(),
            Value::String(APPROVAL_META_KIND_MCP_TOOL_CALL.to_string()),
        )]);
        if !persist_modes.is_empty() {
            meta.insert(
                APPROVAL_PERSIST_KEY.to_string(),
                Value::Array(
                    persist_modes
                        .iter()
                        .map(|mode| Value::String((*mode).to_string()))
                        .collect(),
                ),
            );
        }
        if let Some(tool_params) = tool_params {
            meta.insert(APPROVAL_TOOL_PARAMS_KEY.to_string(), tool_params);
        }
        if let Some(tool_params_display) = tool_params_display {
            meta.insert(
                APPROVAL_TOOL_PARAMS_DISPLAY_KEY.to_string(),
                Value::Array(
                    tool_params_display
                        .into_iter()
                        .map(|(name, value)| {
                            serde_json::json!({
                                "name": name,
                                "value": value,
                            })
                        })
                        .collect(),
                ),
            );
        }
        Some(Value::Object(meta))
    }

    fn snapshot_buffer(buf: &Buffer) -> String {
        let mut lines = Vec::new();
        for y in 0..buf.area().height {
            let mut row = String::new();
            for x in 0..buf.area().width {
                row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(row);
        }
        lines.join("\n")
    }

    fn render_snapshot(overlay: &McpServerElicitationOverlay, area: Rect) -> String {
        let mut buf = Buffer::empty(area);
        overlay.render(area, &mut buf);
        snapshot_buffer(&buf)
    }

    #[test]
    fn parses_boolean_form_request() {
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request(
                "Allow this request?",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                            "description": "Approve the pending action.",
                        }
                    },
                    "required": ["confirmed"],
                }),
                None,
            ),
        )
        .expect("expected supported form");

        assert_eq!(
            request,
            McpServerElicitationFormRequest {
                thread_id,
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                message: "Allow this request?".to_string(),
                approval_display_params: Vec::new(),
                response_mode: McpServerElicitationResponseMode::FormContent,
                fields: vec![McpServerElicitationField {
                    id: "confirmed".to_string(),
                    label: "Confirm".to_string(),
                    prompt: "Approve the pending action.".to_string(),
                    required: true,
                    input: McpServerElicitationFieldInput::Select {
                        options: vec![
                            McpServerElicitationOption {
                                label: "True".to_string(),
                                description: None,
                                value: Value::Bool(true),
                            },
                            McpServerElicitationOption {
                                label: "False".to_string(),
                                description: None,
                                value: Value::Bool(false),
                            },
                        ],
                        default_idx: None,
                    },
                }],
                tool_suggestion: None,
            }
        );
    }

    #[test]
    fn unsupported_numeric_form_falls_back() {
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Pick a number",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "title": "Count",
                        }
                    },
                }),
                None,
            ),
        );

        assert_eq!(request, None);
    }

    #[test]
    fn missing_schema_uses_approval_actions() {
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request("Allow this request?", Value::Null, None),
        )
        .expect("expected approval fallback");

        assert_eq!(
            request,
            McpServerElicitationFormRequest {
                thread_id,
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                message: "Allow this request?".to_string(),
                approval_display_params: Vec::new(),
                response_mode: McpServerElicitationResponseMode::ApprovalAction,
                fields: vec![McpServerElicitationField {
                    id: APPROVAL_FIELD_ID.to_string(),
                    label: String::new(),
                    prompt: String::new(),
                    required: true,
                    input: McpServerElicitationFieldInput::Select {
                        options: vec![
                            McpServerElicitationOption {
                                label: "Allow".to_string(),
                                description: Some("Run the tool and continue.".to_string()),
                                value: Value::String(APPROVAL_ACCEPT_ONCE_VALUE.to_string()),
                            },
                            McpServerElicitationOption {
                                label: "Deny".to_string(),
                                description: Some(
                                    "Decline this tool call and continue.".to_string(),
                                ),
                                value: Value::String(APPROVAL_DECLINE_VALUE.to_string()),
                            },
                            McpServerElicitationOption {
                                label: "Cancel".to_string(),
                                description: Some("Cancel this tool call".to_string()),
                                value: Value::String(APPROVAL_CANCEL_VALUE.to_string()),
                            },
                        ],
                        default_idx: Some(0),
                    },
                }],
                tool_suggestion: None,
            }
        );
    }

    #[test]
    fn empty_tool_approval_schema_uses_approval_actions() {
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request(
                "Allow this request?",
                empty_object_schema(),
                tool_approval_meta(&[], None, None),
            ),
        )
        .expect("expected approval fallback");

        assert_eq!(
            request,
            McpServerElicitationFormRequest {
                thread_id,
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                message: "Allow this request?".to_string(),
                approval_display_params: Vec::new(),
                response_mode: McpServerElicitationResponseMode::ApprovalAction,
                fields: vec![McpServerElicitationField {
                    id: APPROVAL_FIELD_ID.to_string(),
                    label: String::new(),
                    prompt: String::new(),
                    required: true,
                    input: McpServerElicitationFieldInput::Select {
                        options: vec![
                            McpServerElicitationOption {
                                label: "Allow".to_string(),
                                description: Some("Run the tool and continue.".to_string()),
                                value: Value::String(APPROVAL_ACCEPT_ONCE_VALUE.to_string()),
                            },
                            McpServerElicitationOption {
                                label: "Cancel".to_string(),
                                description: Some("Cancel this tool call".to_string()),
                                value: Value::String(APPROVAL_CANCEL_VALUE.to_string()),
                            },
                        ],
                        default_idx: Some(0),
                    },
                }],
                tool_suggestion: None,
            }
        );
    }

    #[test]
    fn tool_suggestion_meta_is_parsed_into_request_payload() {
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Suggest Google Calendar",
                empty_object_schema(),
                Some(serde_json::json!({
                    "codex_approval_kind": "tool_suggestion",
                    "tool_type": "connector",
                    "suggest_type": "install",
                    "suggest_reason": "Plan and reference events from your calendar",
                    "tool_id": "connector_2128aebfecb84f64a069897515042a44",
                    "tool_name": "Google Calendar",
                    "install_url": "https://example.test/google-calendar",
                })),
            ),
        )
        .expect("expected tool suggestion form");

        assert_eq!(
            request.tool_suggestion(),
            Some(&ToolSuggestionRequest {
                tool_type: ToolSuggestionToolType::Connector,
                suggest_type: ToolSuggestionType::Install,
                suggest_reason: "Plan and reference events from your calendar".to_string(),
                tool_id: "connector_2128aebfecb84f64a069897515042a44".to_string(),
                tool_name: "Google Calendar".to_string(),
                install_url: "https://example.test/google-calendar".to_string(),
            })
        );
    }

    #[test]
    fn empty_unmarked_schema_falls_back() {
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request("Empty form", empty_object_schema(), None),
        );

        assert_eq!(request, None);
    }

    #[test]
    fn tool_approval_display_params_prefer_explicit_display_order() {
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Allow Calendar to create an event",
                empty_object_schema(),
                tool_approval_meta(
                    &[],
                    Some(serde_json::json!({
                        "zeta": 3,
                        "alpha": 1,
                    })),
                    Some(vec![
                        ("Calendar", Value::String("primary".to_string())),
                        ("Title", Value::String("Roadmap review".to_string())),
                    ]),
                ),
            ),
        )
        .expect("expected approval fallback");

        assert_eq!(
            request.approval_display_params,
            vec![
                McpToolApprovalDisplayParam {
                    name: "Calendar".to_string(),
                    value: Value::String("primary".to_string()),
                },
                McpToolApprovalDisplayParam {
                    name: "Title".to_string(),
                    value: Value::String("Roadmap review".to_string()),
                },
            ]
        );
    }

    #[test]
    fn submit_sends_accept_with_typed_content() {
        let (tx, mut rx) = test_sender();
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request(
                "Allow this request?",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                            "description": "Approve the pending action.",
                        }
                    },
                    "required": ["confirmed"],
                }),
                None,
            ),
        )
        .expect("expected supported form");
        let mut overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        overlay.select_current_option(true);
        overlay.submit_answers();

        let event = rx.try_recv().expect("expected resolution");
        let AppEvent::SubmitThreadOp {
            thread_id: resolved_thread_id,
            op,
        } = event
        else {
            panic!("expected SubmitThreadOp");
        };
        assert_eq!(resolved_thread_id, thread_id);
        assert_eq!(
            op,
            Op::ResolveElicitation {
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                decision: ElicitationAction::Accept,
                content: Some(serde_json::json!({
                    "confirmed": true,
                })),
                meta: None,
            }
        );
    }

    #[test]
    fn empty_tool_approval_schema_session_choice_sets_persist_meta() {
        let (tx, mut rx) = test_sender();
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request(
                "Allow this request?",
                empty_object_schema(),
                tool_approval_meta(
                    &[
                        APPROVAL_PERSIST_SESSION_VALUE,
                        APPROVAL_PERSIST_ALWAYS_VALUE,
                    ],
                    None,
                    None,
                ),
            ),
        )
        .expect("expected approval fallback");
        let mut overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        if let Some(answer) = overlay.current_answer_mut() {
            answer.selection.selected_idx = Some(1);
        }
        overlay.select_current_option(true);
        overlay.submit_answers();

        let event = rx.try_recv().expect("expected resolution");
        let AppEvent::SubmitThreadOp {
            thread_id: resolved_thread_id,
            op,
        } = event
        else {
            panic!("expected SubmitThreadOp");
        };
        assert_eq!(resolved_thread_id, thread_id);
        assert_eq!(
            op,
            Op::ResolveElicitation {
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                decision: ElicitationAction::Accept,
                content: None,
                meta: Some(serde_json::json!({
                    APPROVAL_PERSIST_KEY: APPROVAL_PERSIST_SESSION_VALUE,
                })),
            }
        );
    }

    #[test]
    fn empty_tool_approval_schema_always_allow_sets_persist_meta() {
        let (tx, mut rx) = test_sender();
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request(
                "Allow this request?",
                empty_object_schema(),
                tool_approval_meta(
                    &[
                        APPROVAL_PERSIST_SESSION_VALUE,
                        APPROVAL_PERSIST_ALWAYS_VALUE,
                    ],
                    None,
                    None,
                ),
            ),
        )
        .expect("expected approval fallback");
        let mut overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        if let Some(answer) = overlay.current_answer_mut() {
            answer.selection.selected_idx = Some(2);
        }
        overlay.select_current_option(true);
        overlay.submit_answers();

        let event = rx.try_recv().expect("expected resolution");
        let AppEvent::SubmitThreadOp {
            thread_id: resolved_thread_id,
            op,
        } = event
        else {
            panic!("expected SubmitThreadOp");
        };
        assert_eq!(resolved_thread_id, thread_id);
        assert_eq!(
            op,
            Op::ResolveElicitation {
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                decision: ElicitationAction::Accept,
                content: None,
                meta: Some(serde_json::json!({
                    APPROVAL_PERSIST_KEY: APPROVAL_PERSIST_ALWAYS_VALUE,
                })),
            }
        );
    }

    #[test]
    fn ctrl_c_cancels_elicitation() {
        let (tx, mut rx) = test_sender();
        let thread_id = ThreadId::default();
        let request = McpServerElicitationFormRequest::from_event(
            thread_id,
            form_request(
                "Allow this request?",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                            "description": "Approve the pending action.",
                        }
                    },
                    "required": ["confirmed"],
                }),
                None,
            ),
        )
        .expect("expected supported form");
        let mut overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        assert_eq!(overlay.on_ctrl_c(), CancellationEvent::Handled);

        let event = rx.try_recv().expect("expected resolution");
        let AppEvent::SubmitThreadOp {
            thread_id: resolved_thread_id,
            op,
        } = event
        else {
            panic!("expected SubmitThreadOp");
        };
        assert_eq!(resolved_thread_id, thread_id);
        assert_eq!(
            op,
            Op::ResolveElicitation {
                server_name: "server-1".to_string(),
                request_id: McpRequestId::String("request-1".to_string()),
                decision: ElicitationAction::Cancel,
                content: None,
                meta: None,
            }
        );
    }

    #[test]
    fn queues_requests_fifo() {
        let (tx, _rx) = test_sender();
        let first = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "First",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                        }
                    },
                }),
                None,
            ),
        )
        .expect("expected supported form");
        let second = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Second",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                        }
                    },
                }),
                None,
            ),
        )
        .expect("expected supported form");
        let third = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Third",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                        }
                    },
                }),
                None,
            ),
        )
        .expect("expected supported form");
        let mut overlay = McpServerElicitationOverlay::new(first, tx, true, false, false);

        overlay.try_consume_mcp_server_elicitation_request(second);
        overlay.try_consume_mcp_server_elicitation_request(third);
        overlay.select_current_option(true);
        overlay.submit_answers();

        assert_eq!(overlay.request.message, "Second");

        overlay.select_current_option(true);
        overlay.submit_answers();

        assert_eq!(overlay.request.message, "Third");
    }

    #[test]
    fn boolean_form_snapshot() {
        let (tx, _rx) = test_sender();
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Allow this request?",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "confirmed": {
                            "type": "boolean",
                            "title": "Confirm",
                            "description": "Approve the pending action.",
                        }
                    },
                    "required": ["confirmed"],
                }),
                None,
            ),
        )
        .expect("expected supported form");
        let overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        insta::assert_snapshot!(
            "mcp_server_elicitation_boolean_form",
            render_snapshot(&overlay, Rect::new(0, 0, 120, 16))
        );
    }

    #[test]
    fn approval_form_tool_approval_snapshot() {
        let (tx, _rx) = test_sender();
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Allow this request?",
                empty_object_schema(),
                tool_approval_meta(&[], None, None),
            ),
        )
        .expect("expected approval fallback");
        let overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        insta::assert_snapshot!(
            "mcp_server_elicitation_approval_form_without_schema",
            render_snapshot(&overlay, Rect::new(0, 0, 120, 16))
        );
    }

    #[test]
    fn approval_form_tool_approval_with_persist_options_snapshot() {
        let (tx, _rx) = test_sender();
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Allow this request?",
                empty_object_schema(),
                tool_approval_meta(
                    &[
                        APPROVAL_PERSIST_SESSION_VALUE,
                        APPROVAL_PERSIST_ALWAYS_VALUE,
                    ],
                    None,
                    None,
                ),
            ),
        )
        .expect("expected approval fallback");
        let overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        insta::assert_snapshot!(
            "mcp_server_elicitation_approval_form_with_session_persist",
            render_snapshot(&overlay, Rect::new(0, 0, 120, 16))
        );
    }

    #[test]
    fn approval_form_tool_approval_with_param_summary_snapshot() {
        let (tx, _rx) = test_sender();
        let request = McpServerElicitationFormRequest::from_event(
            ThreadId::default(),
            form_request(
                "Allow Calendar to create an event",
                empty_object_schema(),
                tool_approval_meta(
                    &[],
                    Some(serde_json::json!({
                        "calendar_id": "primary",
                        "title": "Roadmap review",
                        "notes": "This is a deliberately long note that should truncate before it turns the approval body into a giant wall of text in the TUI overlay.",
                        "ignored_after_limit": "fourth param",
                    })),
                    Some(vec![
                        ("Calendar", Value::String("primary".to_string())),
                        ("Title", Value::String("Roadmap review".to_string())),
                        (
                            "Notes",
                            Value::String("This is a deliberately long note that should truncate before it turns the approval body into a giant wall of text in the TUI overlay.".to_string()),
                        ),
                        ("Ignored", Value::String("fourth param".to_string())),
                    ]),
                ),
            ),
        )
        .expect("expected approval fallback");
        let overlay = McpServerElicitationOverlay::new(request, tx, true, false, false);

        insta::assert_snapshot!(
            "mcp_server_elicitation_approval_form_with_param_summary",
            render_snapshot(&overlay, Rect::new(0, 0, 120, 16))
        );
    }
}
