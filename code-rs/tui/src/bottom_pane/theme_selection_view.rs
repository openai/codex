#![allow(dead_code)]

use code_core::config_types::ThemeName;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use unicode_segmentation::UnicodeSegmentation;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
// Cleanup: remove unused imports to satisfy warning-as-error policy
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::chatwidget::BackgroundOrderTicket;
use crate::theme::{custom_theme_is_dark, map_theme_for_palette, palette_mode, PaletteMode};
use crate::thread_spawner;

use super::BottomPane;
use super::bottom_pane_view::BottomPaneView;

/// Interactive UI for selecting appearance (Theme & Spinner)
pub(crate) struct ThemeSelectionView {
    original_theme: ThemeName, // Theme to restore on cancel
    current_theme: ThemeName,  // Currently displayed theme
    selected_theme_index: usize,
    // Spinner tab state
    _original_spinner: String,
    current_spinner: String,
    selected_spinner_index: usize,
    // UI mode/state
    mode: Mode,
    overview_selected_index: usize, // 0 = Theme, 1 = Spinner
    // Revert points when backing out of detail views
    revert_theme_on_back: ThemeName,
    revert_spinner_on_back: String,
    // One-shot flags to show selection at top on first render of detail views
    just_entered_themes: bool,
    just_entered_spinner: bool,
    app_event_tx: AppEventSender,
    tail_ticket: BackgroundOrderTicket,
    before_ticket: BackgroundOrderTicket,
    is_complete: bool,
}

impl ThemeSelectionView {
    pub fn new(
        current_theme: ThemeName,
        app_event_tx: AppEventSender,
        tail_ticket: BackgroundOrderTicket,
        before_ticket: BackgroundOrderTicket,
    ) -> Self {
        let current_theme = map_theme_for_palette(current_theme, custom_theme_is_dark());
        let themes = Self::get_theme_options();
        let selected_theme_index = themes
            .iter()
            .position(|(t, _, _)| *t == current_theme)
            .unwrap_or(0);

        // Initialize spinner selection from current runtime spinner
        let spinner_names = crate::spinner::spinner_names();
        let current_spinner_name = crate::spinner::current_spinner().name.clone();
        let selected_spinner_index = spinner_names
            .iter()
            .position(|n| *n == current_spinner_name)
            .unwrap_or(0);

        Self {
            original_theme: current_theme,
            current_theme,
            selected_theme_index,
            _original_spinner: current_spinner_name.clone(),
            current_spinner: current_spinner_name.clone(),
            selected_spinner_index,
            mode: Mode::Overview,
            overview_selected_index: 0,
            revert_theme_on_back: current_theme,
            revert_spinner_on_back: current_spinner_name,
            just_entered_themes: false,
            just_entered_spinner: false,
            app_event_tx,
            tail_ticket,
            before_ticket,
            is_complete: false,
        }
    }

    fn get_theme_options() -> Vec<(ThemeName, &'static str, &'static str)> {
        if matches!(palette_mode(), PaletteMode::Ansi16) {
            return vec![
                (
                    ThemeName::LightPhotonAnsi16,
                    "Light (16-color)",
                    "High-contrast light palette for limited terminals",
                ),
                (
                    ThemeName::DarkCarbonAnsi16,
                    "Dark (16-color)",
                    "High-contrast dark palette for limited terminals",
                ),
            ];
        }

        let mut v = vec![
            // Light themes (at top)
            (
                ThemeName::LightPhoton,
                "Light - Photon",
                "Clean professional light theme",
            ),
            (
                ThemeName::LightPrismRainbow,
                "Light - Prism Rainbow",
                "Vibrant rainbow accents",
            ),
            (
                ThemeName::LightVividTriad,
                "Light - Vivid Triad",
                "Cyan, pink, amber triad",
            ),
            (
                ThemeName::LightPorcelain,
                "Light - Porcelain",
                "Refined porcelain tones",
            ),
            (
                ThemeName::LightSandbar,
                "Light - Sandbar",
                "Warm sandy beach colors",
            ),
            (
                ThemeName::LightGlacier,
                "Light - Glacier",
                "Cool glacier blues",
            ),
            (
                ThemeName::DarkPaperLightPro,
                "Light - Paper Pro",
                "Premium paper-like",
            ),
            // Dark themes (below)
            (
                ThemeName::DarkCarbonNight,
                "Dark - Carbon Night",
                "Sleek modern dark theme",
            ),
            (
                ThemeName::DarkShinobiDusk,
                "Dark - Shinobi Dusk",
                "Japanese-inspired twilight",
            ),
            (
                ThemeName::DarkOledBlackPro,
                "Dark - OLED Black Pro",
                "True black for OLED displays",
            ),
            (
                ThemeName::DarkAmberTerminal,
                "Dark - Amber Terminal",
                "Retro amber CRT aesthetic",
            ),
            (
                ThemeName::DarkAuroraFlux,
                "Dark - Aurora Flux",
                "Northern lights inspired",
            ),
            (
                ThemeName::DarkCharcoalRainbow,
                "Dark - Charcoal Rainbow",
                "High-contrast accessible",
            ),
            (
                ThemeName::DarkZenGarden,
                "Dark - Zen Garden",
                "Calm and peaceful",
            ),
        ];
        // Append custom theme if available (use saved label and light/dark prefix)
        if let Some(label0) = crate::theme::custom_theme_label() {
            if matches!(palette_mode(), PaletteMode::Ansi16) {
                return v;
            }
            // Sanitize any leading Light/Dark prefix the model may have included
            let mut label = label0.trim().to_string();
            for pref in ["Light - ", "Dark - ", "Light ", "Dark "] {
                if label.starts_with(pref) {
                    label = label[pref.len()..].trim().to_string();
                    break;
                }
            }
            let name = if crate::theme::custom_theme_is_dark().unwrap_or(false) {
                format!("Dark - {}", label)
            } else {
                format!("Light - {}", label)
            };
            v.push((
                ThemeName::Custom,
                Box::leak(name.into_boxed_str()),
                "Your saved custom theme",
            ));
        }
        v
    }

    fn allow_custom_theme_generation() -> bool {
        !matches!(palette_mode(), PaletteMode::Ansi16)
    }

    fn move_selection_up(&mut self) {
        if matches!(self.mode, Mode::Themes) {
            let options = Self::get_theme_options();
            if self.selected_theme_index > 0 {
                self.selected_theme_index -= 1;
                self.current_theme = options[self.selected_theme_index].0;
                self.app_event_tx
                    .send(AppEvent::PreviewTheme(self.current_theme));
            }
        } else {
            let names = crate::spinner::spinner_names();
            if self.selected_spinner_index > 0 {
                self.selected_spinner_index -= 1;
                if let Some(name) = names.get(self.selected_spinner_index) {
                    self.current_spinner = name.clone();
                    self.app_event_tx
                        .send(AppEvent::PreviewSpinner(self.current_spinner.clone()));
                }
            }
        }
    }

    fn move_selection_down(&mut self) {
        if matches!(self.mode, Mode::Themes) {
            let options = Self::get_theme_options();
            let allow_extra_row = Self::allow_custom_theme_generation();
            let limit = if allow_extra_row {
                options.len()
            } else {
                options.len().saturating_sub(1)
            };
            if self.selected_theme_index + 1 <= limit {
                self.selected_theme_index += 1;
                if self.selected_theme_index < options.len() {
                    self.current_theme = options[self.selected_theme_index].0;
                    self.app_event_tx
                        .send(AppEvent::PreviewTheme(self.current_theme));
                }
            }
        } else {
            let names = crate::spinner::spinner_names();
            // Allow moving onto the extra pseudo-row (Generate your own…)
            if self.selected_spinner_index + 1 <= names.len() {
                self.selected_spinner_index += 1;
                if self.selected_spinner_index < names.len() {
                    if let Some(name) = names.get(self.selected_spinner_index) {
                        self.current_spinner = name.clone();
                        self.app_event_tx
                            .send(AppEvent::PreviewSpinner(self.current_spinner.clone()));
                    }
                } else {
                    // On the pseudo-row: do not change current spinner preview
                }
            }
        }
    }

    fn confirm_theme(&mut self) {
        self.app_event_tx
            .send(AppEvent::UpdateTheme(self.current_theme));
        self.revert_theme_on_back = self.current_theme;
        self.mode = Mode::Overview;
    }

    fn confirm_spinner(&mut self) {
        self.app_event_tx
            .send(AppEvent::UpdateSpinner(self.current_spinner.clone()));
        self.revert_spinner_on_back = self.current_spinner.clone();
        self.mode = Mode::Overview;
    }

    fn cancel_detail(&mut self) {
        match self.mode {
            Mode::Themes => {
                if self.current_theme != self.revert_theme_on_back {
                    self.current_theme = self.revert_theme_on_back;
                    self.app_event_tx
                        .send(AppEvent::PreviewTheme(self.current_theme));
                }
            }
            Mode::Spinner => {
                if self.current_spinner != self.revert_spinner_on_back {
                    self.current_spinner = self.revert_spinner_on_back.clone();
                    self.app_event_tx
                        .send(AppEvent::PreviewSpinner(self.current_spinner.clone()));
                }
            }
            Mode::Overview => {}
            Mode::CreateSpinner(_) => {}
            Mode::CreateTheme(_) => {}
        }
        self.mode = Mode::Overview;
    }

    fn send_tail(&self, message: impl Into<String>) {
        self.app_event_tx
            .send_background_event_with_ticket(&self.tail_ticket, message);
    }

    fn send_before_next_output(&self, message: impl Into<String>) {
        self.app_event_tx.send_background_before_next_output_with_ticket(
            &self.before_ticket,
            message,
        );
    }

    /// Spawn a background task that creates a custom spinner using the LLM with a JSON schema
    fn kickoff_spinner_creation(
        &self,
        user_prompt: String,
        progress_tx: std::sync::mpsc::Sender<ProgressMsg>,
    ) {
        let tx = self.app_event_tx.clone();
        let before_ticket = self.before_ticket.clone();
        let fallback_tx = self.app_event_tx.clone();
        let fallback_ticket = self.before_ticket.clone();
        let completion_tx = progress_tx.clone();
        if thread_spawner::spawn_lightweight("spinner-create", move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tx.send_background_before_next_output_with_ticket(
                        &before_ticket,
                        format!("Failed to start runtime: {}", e),
                    );
                    return;
                }
            };
            let _ = rt.block_on(async move {
                // Load current config (CLI-style) and construct a one-off ModelClient
                let cfg = match code_core::config::Config::load_with_cli_overrides(vec![], code_core::config::ConfigOverrides::default()) {
                    Ok(c) => c,
                    Err(e) => {
                        tx.send_background_before_next_output_with_ticket(
                            &before_ticket,
                            format!("Config error: {}", e),
                        );
                        return;
                    }
                };
                // Use the same auth preference as the active Codex session.
                // When logged in with ChatGPT, prefer ChatGPT auth; otherwise fall back to API key.
                let preferred_auth = if cfg.using_chatgpt_auth {
                    code_protocol::mcp_protocol::AuthMode::ChatGPT
                } else {
                    code_protocol::mcp_protocol::AuthMode::ApiKey
                };
                let auth_mgr = code_core::AuthManager::shared_with_mode_and_originator(
                    cfg.code_home.clone(),
                    preferred_auth,
                    cfg.responses_originator_header.clone(),
                );
                let client = code_core::ModelClient::new(
                    std::sync::Arc::new(cfg.clone()),
                    Some(auth_mgr),
                    None,
                    cfg.model_provider.clone(),
                    code_core::config_types::ReasoningEffort::Low,
                    cfg.model_reasoning_summary,
                    cfg.model_text_verbosity,
                    uuid::Uuid::new_v4(),
                    // Enable debug logs for targeted triage of stream issues
                    std::sync::Arc::new(std::sync::Mutex::new(code_core::debug_logger::DebugLogger::new(true).unwrap_or_else(|_| code_core::debug_logger::DebugLogger::new(false).expect("debug logger")))),
                );

                // Build developer guidance and input
                let developer = "You are performing a custom task to create a terminal spinner.\n\nRequirements:\n- Output JSON ONLY, no prose.\n- `interval` is the delay in milliseconds between frames; MUST be between 50 and 300 inclusive.\n- `frames` is an array of strings; each element is a frame displayed sequentially at the given interval.\n- The spinner SHOULD have between 2 and 60 frames.\n- Each frame SHOULD be between 1 and 30 characters wide. ALL frames MUST be the SAME width (same number of characters). If you propose frames with varying widths, PAD THEM ON THE LEFT with spaces so they are uniform.\n- You MAY use both ASCII and Unicode characters (e.g., box drawing, braille, arrows). Use EMOJIS ONLY if the user explicitly requests emojis in their prompt.\n- Be creative! You have the full range of Unicode to play with!\n".to_string();
                let mut input: Vec<code_protocol::models::ResponseItem> = Vec::new();
                input.push(code_protocol::models::ResponseItem::Message { id: None, role: "developer".to_string(), content: vec![code_protocol::models::ContentItem::InputText { text: developer }] });
                input.push(code_protocol::models::ResponseItem::Message { id: None, role: "user".to_string(), content: vec![code_protocol::models::ContentItem::InputText { text: user_prompt }] });

                // JSON schema for structured output
                let schema = serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "minLength": 1, "maxLength": 40, "description": "Display name for the spinner (1 - 3 words, shown in the UI)."},
                        "interval": {"type": "integer", "minimum": 50, "maximum": 300, "description": "Delay between frames in milliseconds (50 - 300)."},
                        "frames": {
                            "type": "array",
                            "items": {"type": "string", "minLength": 1, "maxLength": 30},
                            "minItems": 2,
                            "maxItems": 60,
                            "description": "2 - 60 frames, 1 - 30 characters each (every frame should be the same length of characters)."
                        }
                    },
                    "required": ["name", "interval", "frames"],
                    "additionalProperties": false
                });
                let format = code_core::TextFormat { r#type: "json_schema".to_string(), name: Some("custom_spinner".to_string()), strict: Some(true), schema: Some(schema) };

                let mut prompt = code_core::Prompt::default();
                prompt.input = input;
                prompt.store = true;
                prompt.text_format = Some(format);
                prompt.set_log_tag("ui/theme_spinner");

                // Stream and collect final JSON
                use futures::StreamExt;
                let _ = progress_tx.send(ProgressMsg::ThinkingDelta("(connecting to model)".to_string()));
                let mut stream = match client.stream(&prompt).await {
                    Ok(s) => s,
                    Err(e) => {
                        tx.send_background_before_next_output_with_ticket(
                            &before_ticket,
                            format!("Request error: {}", e),
                        );
                        tracing::info!("spinner request error: {}", e);
                        return;
                    }
                };
                let mut out = String::new();
                let mut think_sum = String::new();
                // Capture the last transport/stream error so we can surface it to the UI
                let mut last_err: Option<String> = None;
                while let Some(ev) = stream.next().await {
                    match ev {
                        Ok(code_core::ResponseEvent::Created) => { tracing::info!("LLM: created"); let _ = progress_tx.send(ProgressMsg::SetStatus("(starting generation)".to_string())); }
                        Ok(code_core::ResponseEvent::ReasoningSummaryDelta { delta, .. }) => { tracing::info!(target: "spinner", "LLM[thinking]: {}", delta); let _ = progress_tx.send(ProgressMsg::ThinkingDelta(delta.clone())); think_sum.push_str(&delta); }
                        Ok(code_core::ResponseEvent::ReasoningContentDelta { delta, .. }) => { tracing::info!(target: "spinner", "LLM[reasoning]: {}", delta); }
                        Ok(code_core::ResponseEvent::OutputTextDelta { delta, .. }) => { tracing::info!(target: "spinner", "LLM[delta]: {}", delta); let _ = progress_tx.send(ProgressMsg::OutputDelta(delta.clone())); out.push_str(&delta); }
                        Ok(code_core::ResponseEvent::OutputItemDone { item, .. }) => {
                            if let code_protocol::models::ResponseItem::Message { content, .. } = item {
                                for c in content { if let code_protocol::models::ContentItem::OutputText { text } = c { out.push_str(&text); } }
                            }
                            tracing::info!(target: "spinner", "LLM[item_done]");
                        }
                        Ok(code_core::ResponseEvent::Completed { .. }) => { tracing::info!("LLM: completed"); break; }
                        Err(e) => {
                            let msg = format!("{}", e);
                            tracing::info!("LLM stream error: {}", msg);
                            last_err = Some(msg);
                            break; // Stop consuming after a terminal transport error
                        }
                        _ => {}
                    }
                }

                let _ = progress_tx.send(ProgressMsg::RawOutput(out.clone()));

                // If we received no content at all, surface the transport error explicitly
                if out.trim().is_empty() {
                    let err = last_err
                        .map(|e| format!(
                            "model stream error: {} | raw_out_len={} think_len={}",
                            e,
                            out.len(),
                            think_sum.len()
                        ))
                        .unwrap_or_else(|| format!(
                            "model stream returned no content | raw_out_len={} think_len={}",
                            out.len(),
                            think_sum.len()
                        ));
                    let _ = progress_tx.send(ProgressMsg::CompletedErr { error: err, _raw_snippet: String::new() });
                    return;
                }

                // Parse JSON; on failure, attempt to salvage a top-level object and log raw output
                let v: serde_json::Value = match serde_json::from_str(&out) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::info!(target: "spinner", "Strict JSON parse failed: {}", e);
                        tracing::info!(target: "spinner", "Raw output: {}", out);
                        fn extract_first_json_object(s: &str) -> Option<String> {
                            let mut depth = 0usize;
                            let mut in_str = false;
                            let mut esc = false;
                            let mut start: Option<usize> = None;
                            for (i, ch) in s.char_indices() {
                                if in_str {
                                    if esc { esc = false; }
                                    else if ch == '\\' { esc = true; }
                                    else if ch == '"' { in_str = false; }
                                    continue;
                                }
                                match ch {
                                    '"' => in_str = true,
                                    '{' => { if depth == 0 { start = Some(i); } depth += 1; },
                                    '}' => { if depth > 0 { depth -= 1; if depth == 0 { let end = i + ch.len_utf8(); return start.map(|st| s[st..end].to_string()); } } },
                                    _ => {}
                                }
                            }
                            None
                        }
                        if let Some(obj) = extract_first_json_object(&out) {
                            match serde_json::from_str::<serde_json::Value>(&obj) {
                                Ok(v) => v,
                                Err(e2) => {
                                    let _ = progress_tx.send(ProgressMsg::CompletedErr {
                                        error: format!("{}", e2),
                                        _raw_snippet: out.chars().take(200).collect::<String>(),
                                    });
                                    return;
                                }
                            }
                        } else {
                            // Prefer a clearer message if we saw a transport error
                            let msg = last_err
                                .map(|le| format!("model stream error: {}", le))
                                .unwrap_or_else(|| format!("{}", e));
                            let _ = progress_tx.send(ProgressMsg::CompletedErr {
                                error: msg,
                                _raw_snippet: out.chars().take(200).collect::<String>(),
                            });
                            return;
                        }
                    }
                };
                let interval = v.get("interval").and_then(|x| x.as_u64()).unwrap_or(120).clamp(50, 300);
                let display_name = v
                    .get("name")
                    .and_then(|x| x.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Custom")
                    .to_string();
                let mut frames: Vec<String> = v
                    .get("frames")
                    .and_then(|x| x.as_array())
                    .map(|arr| arr.iter().filter_map(|f| f.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();

                // Enforce frame width limit (truncate to first 20 characters)
                const MAX_CHARS: usize = 20;
                frames = frames
                    .into_iter()
                    .map(|f| f.chars().take(MAX_CHARS).collect::<String>())
                    .filter(|f| !f.is_empty())
                    .collect();

                // Enforce count 2–50
                if frames.len() > 50 { frames.truncate(50); }
                if frames.len() < 2 { let _ = progress_tx.send(ProgressMsg::CompletedErr { error: "too few frames after validation".to_string(), _raw_snippet: out.chars().take(200).collect::<String>() }); return; }

                // Normalize: left-pad frames to equal length if needed
                let max_len = frames.iter().map(|f| f.chars().count()).max().unwrap_or(0);
                let norm_frames: Vec<String> = frames
                    .into_iter()
                    .map(|f| {
                        let cur = f.chars().count();
                        if cur >= max_len { f } else { format!("{}{}", " ".repeat(max_len - cur), f) }
                    })
                    .collect();

                // Persist + activate
                let _ = progress_tx.send(ProgressMsg::CompletedOk { name: display_name, interval, frames: norm_frames });
            });
        })
        .is_none()
        {
            let _ = completion_tx.send(ProgressMsg::CompletedErr {
                error: "background worker unavailable".to_string(),
                _raw_snippet: String::new(),
            });
            fallback_tx.send_background_before_next_output_with_ticket(
                &fallback_ticket,
                "Failed to generate spinner preview: background worker unavailable".to_string(),
            );
            return;
        }
    }

    /// Spawn a background task that creates a custom theme using the LLM.
    fn kickoff_theme_creation(
        &self,
        user_prompt: String,
        progress_tx: std::sync::mpsc::Sender<ProgressMsg>,
    ) {
        let tx = self.app_event_tx.clone();
        // Capture a compact example of the current theme as guidance
        fn color_to_hex(c: ratatui::style::Color) -> Option<String> {
            match c {
                ratatui::style::Color::Rgb(r, g, b) => {
                    Some(format!("#{:02X}{:02X}{:02X}", r, g, b))
                }
                _ => None,
            }
        }
        let cur = crate::theme::current_theme();
        let mut example = serde_json::json!({"name": "Current", "colors": {}});
        if let Some(v) = color_to_hex(cur.primary) {
            example["colors"]["primary"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.secondary) {
            example["colors"]["secondary"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.background) {
            example["colors"]["background"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.foreground) {
            example["colors"]["foreground"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.border) {
            example["colors"]["border"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.border_focused) {
            example["colors"]["border_focused"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.selection) {
            example["colors"]["selection"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.cursor) {
            example["colors"]["cursor"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.success) {
            example["colors"]["success"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.warning) {
            example["colors"]["warning"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.error) {
            example["colors"]["error"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.info) {
            example["colors"]["info"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.text) {
            example["colors"]["text"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.text_dim) {
            example["colors"]["text_dim"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.text_bright) {
            example["colors"]["text_bright"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.keyword) {
            example["colors"]["keyword"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.string) {
            example["colors"]["string"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.comment) {
            example["colors"]["comment"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.function) {
            example["colors"]["function"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.spinner) {
            example["colors"]["spinner"] = serde_json::Value::String(v);
        }
        if let Some(v) = color_to_hex(cur.progress) {
            example["colors"]["progress"] = serde_json::Value::String(v);
        }

        let before_ticket = self.before_ticket.clone();
        let fallback_tx = self.app_event_tx.clone();
        let fallback_ticket = self.before_ticket.clone();
        let completion_tx = progress_tx.clone();
        if thread_spawner::spawn_lightweight("theme-create", move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tx.send_background_before_next_output_with_ticket(
                        &before_ticket,
                        format!("Failed to start runtime: {}", e),
                    );
                    return;
                }
            };
            let _ = rt.block_on(async move {
                let cfg = match code_core::config::Config::load_with_cli_overrides(vec![], code_core::config::ConfigOverrides::default()) {
                    Ok(c) => c,
                    Err(e) => {
                        tx.send_background_before_next_output_with_ticket(
                            &before_ticket,
                            format!("Config error: {}", e),
                        );
                        return;
                    }
                };
                let auth_mgr = code_core::AuthManager::shared_with_mode_and_originator(
                    cfg.code_home.clone(),
                    code_protocol::mcp_protocol::AuthMode::ApiKey,
                    cfg.responses_originator_header.clone(),
                );
                let client = code_core::ModelClient::new(
                    std::sync::Arc::new(cfg.clone()),
                    Some(auth_mgr),
                    None,
                    cfg.model_provider.clone(),
                    cfg.model_reasoning_effort,
                    cfg.model_reasoning_summary,
                    cfg.model_text_verbosity,
                    uuid::Uuid::new_v4(),
                    std::sync::Arc::new(std::sync::Mutex::new(code_core::debug_logger::DebugLogger::new(false).unwrap_or_else(|_| code_core::debug_logger::DebugLogger::new(false).expect("debug logger")))),
                );

                // Prompt with example and detailed field usage to help the model choose appropriate colors
                let developer = format!(
                    "You are designing a TUI color theme for a terminal UI.\n\nOutput: Strict JSON only. Include fields: `name` (string), `is_dark` (boolean), and `colors` (object of hex strings #RRGGBB).\n\nImportant rules:\n- Include EVERY `colors` key below. If you are not changing a value, copy it from the Current example.\n- Ensure strong contrast and readability for text vs background and for dim/bright variants.\n- Favor accessible color contrast (WCAG-ish) where possible.\n\nColor semantics (how the UI uses them):\n- background: main screen background.\n- foreground: primary foreground accents for widgets.\n- text: normal body text; must be readable on background.\n- text_dim: secondary/description text; slightly lower contrast than text.\n- text_bright: headings/emphasis; higher contrast than text.\n- primary: primary action/highlight color for selected items/buttons.\n- secondary: secondary accents (less prominent than primary).\n- border: container borders/dividers; should be visible but subtle against background.\n- border_focused: border when focused/active; slightly stronger than border.\n- selection: background for selected list rows; must contrast with text.\n- cursor: text caret color in input fields; must contrast with background.\n- success/warning/error/info: status badges and notices.\n- keyword/string/comment/function: syntax highlight accents in code blocks.\n- spinner: glyph color for loading animations; should be visible on background.\n- progress: progress-bar foreground color.\n\nCurrent theme example (copy unchanged values from here):\n{}",
                    example.to_string()
                );
                let mut input: Vec<code_protocol::models::ResponseItem> = Vec::new();
                input.push(code_protocol::models::ResponseItem::Message { id: None, role: "developer".to_string(), content: vec![code_protocol::models::ContentItem::InputText { text: developer }] });
                input.push(code_protocol::models::ResponseItem::Message { id: None, role: "user".to_string(), content: vec![code_protocol::models::ContentItem::InputText { text: user_prompt }] });

                let schema = serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {"type": "string", "minLength": 1, "maxLength": 40},
                        "is_dark": {"type": "boolean"},
                        "colors": {
                            "type": "object",
                            "properties": {
                                "primary": {"type": "string"},
                                "secondary": {"type": "string"},
                                "background": {"type": "string"},
                                "foreground": {"type": "string"},
                                "border": {"type": "string"},
                                "border_focused": {"type": "string"},
                                "selection": {"type": "string"},
                                "cursor": {"type": "string"},
                                "success": {"type": "string"},
                                "warning": {"type": "string"},
                                "error": {"type": "string"},
                                "info": {"type": "string"},
                                "text": {"type": "string"},
                                "text_dim": {"type": "string"},
                                "text_bright": {"type": "string"},
                                "keyword": {"type": "string"},
                                "string": {"type": "string"},
                                "comment": {"type": "string"},
                                "function": {"type": "string"},
                                "spinner": {"type": "string"},
                                "progress": {"type": "string"}
                            },
                            "required": [
                                "primary", "secondary", "background", "foreground", "border",
                                "border_focused", "selection", "cursor", "success", "warning",
                                "error", "info", "text", "text_dim", "text_bright", "keyword",
                                "string", "comment", "function", "spinner", "progress"
                            ],
                            "additionalProperties": false
                        }
                    },
                    "required": ["name", "is_dark", "colors"],
                    "additionalProperties": false
                });
                let format = code_core::TextFormat { r#type: "json_schema".to_string(), name: Some("custom_theme".to_string()), strict: Some(true), schema: Some(schema) };

                let mut prompt = code_core::Prompt::default();
                prompt.input = input;
                prompt.store = true;
                prompt.text_format = Some(format);
                prompt.set_log_tag("ui/theme_builder");

                use futures::StreamExt;
                let _ = progress_tx.send(ProgressMsg::ThinkingDelta("(connecting to model)".to_string()));
                let mut stream = match client.stream(&prompt).await {
                    Ok(s) => s,
                    Err(e) => {
                        tx.send_background_before_next_output_with_ticket(
                            &before_ticket,
                            format!("Request error: {}", e),
                        );
                        return;
                    }
                };
                let mut out = String::new();
                // Capture the last transport/stream error so we can surface it to the UI
                let mut last_err: Option<String> = None;
                while let Some(ev) = stream.next().await {
                    match ev {
                        Ok(code_core::ResponseEvent::Created) => {
                            let _ = progress_tx.send(ProgressMsg::SetStatus("(starting generation)".to_string()));
                        }
                        Ok(code_core::ResponseEvent::ReasoningSummaryDelta { delta, .. }) => {
                            let _ = progress_tx.send(ProgressMsg::ThinkingDelta(delta));
                        }
                        Ok(code_core::ResponseEvent::ReasoningContentDelta { delta, .. }) => {
                            let _ = progress_tx.send(ProgressMsg::ThinkingDelta(delta));
                        }
                        Ok(code_core::ResponseEvent::OutputTextDelta { delta, .. }) => {
                            let _ = progress_tx.send(ProgressMsg::OutputDelta(delta.clone()));
                            out.push_str(&delta);
                        }
                        Ok(code_core::ResponseEvent::OutputItemDone { item, .. }) => {
                            if let code_protocol::models::ResponseItem::Message { content, .. } = item {
                                for c in content {
                                    if let code_protocol::models::ContentItem::OutputText { text } = c {
                                        out.push_str(&text);
                                    }
                                }
                            }
                        }
                        Ok(code_core::ResponseEvent::Completed { .. }) => break,
                        Err(e) => {
                            let msg = format!("{}", e);
                            let _ = progress_tx.send(ProgressMsg::ThinkingDelta(format!("(stream error: {})", msg)));
                            last_err = Some(msg);
                            break; // Stop consuming after a terminal transport error
                        }
                        _ => {}
                    }
                }

                let _ = progress_tx.send(ProgressMsg::RawOutput(out.clone()));
                // If we received no content at all, surface the transport error explicitly
                if out.trim().is_empty() {
                    let err = last_err
                        .map(|e| format!("model stream error: {}", e))
                        .unwrap_or_else(|| "model stream returned no content".to_string());
                    let _ = progress_tx.send(ProgressMsg::CompletedErr {
                        error: err,
                        _raw_snippet: String::new(),
                    });
                    return;
                }
                // Try strict parse first; if that fails, salvage the first JSON object in the text.
                let v: serde_json::Value = match serde_json::from_str(&out) {
                    Ok(v) => v,
                    Err(e) => {
                        // Attempt to extract the first top-level JSON object from the stream text
                        fn extract_first_json_object(s: &str) -> Option<String> {
                            let mut depth = 0usize;
                            let mut in_str = false;
                            let mut esc = false;
                            let mut start: Option<usize> = None;
                            for (i, ch) in s.char_indices() {
                                if in_str {
                                    if esc { esc = false; }
                                    else if ch == '\\' { esc = true; }
                                    else if ch == '"' { in_str = false; }
                                    continue;
                                }
                                match ch {
                                    '"' => in_str = true,
                                    '{' => { if depth == 0 { start = Some(i); } depth += 1; },
                                    '}' => { if depth > 0 { depth -= 1; if depth == 0 { let end = i + ch.len_utf8(); return start.map(|st| s[st..end].to_string()); } } },
                                    _ => {}
                                }
                            }
                            None
                        }
                        if let Some(obj) = extract_first_json_object(&out) {
                            match serde_json::from_str::<serde_json::Value>(&obj) {
                                Ok(v) => v,
                                Err(e2) => {
                                    let _ = progress_tx.send(ProgressMsg::CompletedErr {
                                        error: format!("{}", e2),
                                        _raw_snippet: out.chars().take(200).collect(),
                                    });
                                    return;
                                }
                            }
                        } else {
                            // Prefer a clearer message if we saw a transport error
                            let msg = last_err
                                .map(|le| format!("model stream error: {}", le))
                                .unwrap_or_else(|| format!("{}", e));
                            let _ = progress_tx.send(ProgressMsg::CompletedErr {
                                error: msg,
                                _raw_snippet: out.chars().take(200).collect(),
                            });
                            return;
                        }
                    }
                };
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Custom").trim().to_string();
                let is_dark = v.get("is_dark").and_then(|x| x.as_bool());
                let mut colors = code_core::config_types::ThemeColors::default();
                if let Some(map) = v.get("colors").and_then(|x| x.as_object()) {
                    let get = |k: &str| map.get(k).and_then(|x| x.as_str()).map(|s| s.trim().to_string());
                    colors.primary = get("primary");
                    colors.secondary = get("secondary");
                    colors.background = get("background");
                    colors.foreground = get("foreground");
                    colors.border = get("border");
                    colors.border_focused = get("border_focused");
                    colors.selection = get("selection");
                    colors.cursor = get("cursor");
                    colors.success = get("success");
                    colors.warning = get("warning");
                    colors.error = get("error");
                    colors.info = get("info");
                    colors.text = get("text");
                    colors.text_dim = get("text_dim");
                    colors.text_bright = get("text_bright");
                    colors.keyword = get("keyword");
                    colors.string = get("string");
                    colors.comment = get("comment");
                    colors.function = get("function");
                    colors.spinner = get("spinner");
                    colors.progress = get("progress");
                }
                let _ = progress_tx.send(ProgressMsg::CompletedThemeOk(name, colors, is_dark));
            });
        })
        .is_none()
        {
            let _ = completion_tx.send(ProgressMsg::CompletedErr {
                error: "background worker unavailable".to_string(),
                _raw_snippet: String::new(),
            });
            fallback_tx.send_background_before_next_output_with_ticket(
                &fallback_ticket,
                "Failed to generate theme: background worker unavailable".to_string(),
            );
            return;
        }
    }
}

enum Mode {
    Overview,
    Themes,
    Spinner,
    CreateSpinner(CreateState),
    CreateTheme(CreateThemeState),
}

struct CreateState {
    step: std::cell::Cell<CreateStep>,
    /// Freeform prompt describing the desired spinner
    prompt: String,
    /// While true, we render a loading indicator and disable input
    is_loading: std::cell::Cell<bool>,
    action_idx: usize, // 0 = Create/Save, 1 = Cancel/Retry
    /// Live stream messages from the background task
    rx: Option<std::sync::mpsc::Receiver<ProgressMsg>>,
    /// Accumulated thinking/output lines for live display (completed)
    thinking_lines: std::cell::RefCell<Vec<String>>,
    /// In‑progress line assembled from deltas
    thinking_current: std::cell::RefCell<String>,
    /// Parsed proposal waiting for review
    proposed_interval: std::cell::Cell<Option<u64>>,
    proposed_frames: std::cell::RefCell<Option<Vec<String>>>,
    proposed_name: std::cell::RefCell<Option<String>>,
    /// Last raw model output captured (for debugging parse errors)
    last_raw_output: std::cell::RefCell<Option<String>>,
}

struct CreateThemeState {
    step: std::cell::Cell<CreateStep>,
    prompt: String,
    is_loading: std::cell::Cell<bool>,
    action_idx: usize, // 0 = Create/Save, 1 = Cancel/Retry
    rx: Option<std::sync::mpsc::Receiver<ProgressMsg>>,
    thinking_lines: std::cell::RefCell<Vec<String>>,
    thinking_current: std::cell::RefCell<String>,
    proposed_name: std::cell::RefCell<Option<String>>,
    proposed_colors: std::cell::RefCell<Option<code_core::config_types::ThemeColors>>,
    preview_on: std::cell::Cell<bool>,
    review_focus_is_toggle: std::cell::Cell<bool>,
    last_raw_output: std::cell::RefCell<Option<String>>,
    proposed_is_dark: std::cell::Cell<Option<bool>>,
}

#[derive(Copy, Clone, PartialEq)]
enum CreateStep {
    Prompt,
    Action,
    Review,
}

enum ProgressMsg {
    ThinkingDelta(String),
    OutputDelta(String),
    RawOutput(String),
    SetStatus(String),
    CompletedOk {
        name: String,
        interval: u64,
        frames: Vec<String>,
    },
    CompletedThemeOk(String, code_core::config_types::ThemeColors, Option<bool>),
    // `_raw_snippet` is captured for potential future display/debugging
    CompletedErr {
        error: String,
        _raw_snippet: String,
    },
}

impl ThemeSelectionView {
    fn process_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent { code: KeyCode::Up, modifiers: KeyModifiers::NONE, .. } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                        }
                        CreateStep::Review => {
                            s.review_focus_is_toggle.set(true);
                        }
                    }
                } else {
                    match self.mode {
                        Mode::Overview => {
                            self.overview_selected_index =
                                self.overview_selected_index.saturating_sub(1) % 3;
                        }
                        _ => self.move_selection_up(),
                    }
                }
            }
            KeyEvent { code: KeyCode::Down, modifiers: KeyModifiers::NONE, .. } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                        }
                        CreateStep::Review => {
                            s.review_focus_is_toggle.set(false);
                        }
                    }
                } else {
                    match &self.mode {
                        Mode::Overview => {
                            self.overview_selected_index = (self.overview_selected_index + 1) % 3;
                        }
                        _ => self.move_selection_down(),
                    }
                }
            }
            KeyEvent { code: KeyCode::Left, modifiers: KeyModifiers::NONE, .. } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                        }
                        CreateStep::Review => {
                            s.review_focus_is_toggle.set(true);
                        }
                    }
                } else {
                    match self.mode {
                        Mode::Overview => {
                            self.overview_selected_index =
                                self.overview_selected_index.saturating_sub(1) % 3;
                        }
                        _ => self.move_selection_up(),
                    }
                }
            }
            KeyEvent { code: KeyCode::Right, modifiers: KeyModifiers::NONE, .. } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                        }
                        CreateStep::Review => {
                            s.review_focus_is_toggle.set(false);
                        }
                    }
                } else {
                    match &self.mode {
                        Mode::Overview => {
                            self.overview_selected_index = (self.overview_selected_index + 1) % 3;
                        }
                        _ => self.move_selection_down(),
                    }
                }
            }
            KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::NONE, .. } => {
                let current_mode = std::mem::replace(&mut self.mode, Mode::Overview);
                match current_mode {
                    Mode::Overview => {
                        match self.overview_selected_index {
                            0 => {
                                self.revert_theme_on_back = self.current_theme;
                                self.mode = Mode::Themes;
                                self.just_entered_themes = true;
                            }
                            1 => {
                                self.revert_spinner_on_back = self.current_spinner.clone();
                                self.mode = Mode::Spinner;
                                self.app_event_tx.send(AppEvent::ScheduleFrameIn(
                                    std::time::Duration::from_millis(120),
                                ));
                                self.just_entered_spinner = true;
                            }
                            _ => {
                                self.is_complete = true;
                                self.mode = Mode::Overview;
                            }
                        }
                    }
                    Mode::Themes => {
                        let count = Self::get_theme_options().len();
                        if Self::allow_custom_theme_generation()
                            && self.selected_theme_index >= count
                        {
                            self.app_event_tx
                                .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                            self.mode = Mode::CreateTheme(CreateThemeState {
                                step: std::cell::Cell::new(CreateStep::Prompt),
                                prompt: String::new(),
                                is_loading: std::cell::Cell::new(false),
                                action_idx: 0,
                                rx: None,
                                thinking_lines: std::cell::RefCell::new(Vec::new()),
                                thinking_current: std::cell::RefCell::new(String::new()),
                                proposed_name: std::cell::RefCell::new(None),
                                proposed_colors: std::cell::RefCell::new(None),
                                preview_on: std::cell::Cell::new(true),
                                review_focus_is_toggle: std::cell::Cell::new(true),
                                last_raw_output: std::cell::RefCell::new(None),
                                proposed_is_dark: std::cell::Cell::new(None),
                            });
                        } else {
                            self.confirm_theme()
                        }
                    }
                    Mode::Spinner => {
                        let names = crate::spinner::spinner_names();
                        if self.selected_spinner_index > names.len() {
                            self.selected_spinner_index = names.len().saturating_sub(1);
                        }
                        if self.selected_spinner_index >= names.len() {
                            self.mode = Mode::CreateSpinner(CreateState {
                                step: std::cell::Cell::new(CreateStep::Prompt),
                                prompt: String::new(),
                                is_loading: std::cell::Cell::new(false),
                                action_idx: 0,
                                rx: None,
                                thinking_lines: std::cell::RefCell::new(Vec::new()),
                                thinking_current: std::cell::RefCell::new(String::new()),
                                proposed_interval: std::cell::Cell::new(None),
                                proposed_frames: std::cell::RefCell::new(None),
                                proposed_name: std::cell::RefCell::new(None),
                                last_raw_output: std::cell::RefCell::new(None),
                            });
                        } else {
                            self.confirm_spinner()
                        }
                    }
                    Mode::CreateSpinner(mut s) => {
                        let mut go_overview = false;
                        match s.step.get() {
                            CreateStep::Prompt => {
                                if !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_interval.set(None);
                                    s.proposed_frames.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_spinner_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                            }
                            CreateStep::Action => {
                                if s.action_idx == 0 && !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_interval.set(None);
                                    s.proposed_frames.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_spinner_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                } else {
                                    go_overview = true;
                                }
                            }
                            CreateStep::Review => {
                                if s.action_idx == 0 {
                                    if let (Some(interval), Some(frames)) = (
                                        s.proposed_interval.get(),
                                        s.proposed_frames.borrow().clone(),
                                    ) {
                                        let display_name = s
                                            .proposed_name
                                            .borrow()
                                            .as_ref()
                                            .cloned()
                                            .unwrap_or_else(|| "Custom".to_string());
                                        if let Ok(home) = code_core::config::find_code_home() {
                                            let _ = code_core::config::set_custom_spinner(
                                                &home,
                                                "custom",
                                                &display_name,
                                                interval,
                                                &frames,
                                            );
                                        }
                                        crate::spinner::add_custom_spinner(
                                            "custom".to_string(),
                                            display_name.clone(),
                                            interval,
                                            frames,
                                        );
                                        crate::spinner::switch_spinner("custom");
                                        self.revert_spinner_on_back = "custom".to_string();
                                        self.current_spinner = "custom".to_string();
                                        self.app_event_tx
                                            .send(AppEvent::UpdateSpinner("custom".to_string()));
                                        self.send_tail("Custom spinner saved".to_string());
                                        go_overview = true;
                                    }
                                } else {
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_interval.set(None);
                                    s.proposed_frames.replace(None);
                                    s.step.set(CreateStep::Prompt);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                            }
                        }
                        if go_overview {
                            self.mode = Mode::Overview;
                        } else {
                            self.mode = Mode::CreateSpinner(s);
                        }
                    }
                    Mode::CreateTheme(mut s) => {
                        let mut go_overview = false;
                        match s.step.get() {
                            CreateStep::Prompt => {
                                if !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_name.replace(None);
                                    s.proposed_colors.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_theme_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                            }
                            CreateStep::Action => {
                                if s.action_idx == 0 && !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_name.replace(None);
                                    s.proposed_colors.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_theme_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                } else {
                                    go_overview = true;
                                }
                            }
                            CreateStep::Review => {
                                if s.review_focus_is_toggle.get() {
                                    let now_on = !s.preview_on.get();
                                    s.preview_on.set(now_on);
                                    if now_on {
                                        if let (Some(name), Some(colors)) = (
                                            s.proposed_name.borrow().clone(),
                                            s.proposed_colors.borrow().clone(),
                                        ) {
                                            crate::theme::set_custom_theme_colors(colors.clone());
                                            crate::theme::set_custom_theme_label(name.clone());
                                            crate::theme::init_theme(
                                                &code_core::config_types::ThemeConfig {
                                                    name: ThemeName::Custom,
                                                    colors,
                                                    label: Some(name),
                                                    is_dark: s.proposed_is_dark.get(),
                                                },
                                            );
                                        }
                                    } else {
                                        let fallback = if self.revert_theme_on_back == ThemeName::Custom {
                                            ThemeName::LightPhoton
                                        } else {
                                            self.revert_theme_on_back
                                        };
                                        self.app_event_tx.send(AppEvent::PreviewTheme(fallback));
                                    }
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                } else if s.action_idx == 0 {
                                    if let (Some(name), Some(colors)) = (
                                        s.proposed_name.borrow().clone(),
                                        s.proposed_colors.borrow().clone(),
                                    ) {
                                        if let Ok(home) = code_core::config::find_code_home() {
                                            let _ = code_core::config::set_custom_theme(
                                                &home,
                                                &name,
                                                &colors,
                                                s.preview_on.get(),
                                                s.proposed_is_dark.get(),
                                            );
                                        }
                                        crate::theme::set_custom_theme_label(name.clone());
                                        crate::theme::set_custom_theme_colors(colors.clone());
                                        crate::theme::set_custom_theme_is_dark(
                                            s.proposed_is_dark.get(),
                                        );
                                        if s.preview_on.get() {
                                        crate::theme::init_theme(
                                            &code_core::config_types::ThemeConfig {
                                                name: ThemeName::Custom,
                                                colors: colors.clone(),
                                                label: Some(name.clone()),
                                                is_dark: s.proposed_is_dark.get(),
                                            },
                                        );
                                            self.revert_theme_on_back = ThemeName::Custom;
                                            self.current_theme = ThemeName::Custom;
                                            self.app_event_tx
                                                .send(AppEvent::UpdateTheme(ThemeName::Custom));
                                        } else {
                                            self.app_event_tx
                                                .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                                        }
                                        if s.preview_on.get() {
                                            self.send_before_next_output(format!(
                                                "Set theme to {}",
                                                name
                                            ));
                                        } else {
                                            self.send_before_next_output(format!(
                                                "Saved custom theme {} (not active)",
                                                name
                                            ));
                                        }
                                        go_overview = true;
                                    }
                                } else {
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_name.replace(None);
                                    s.proposed_colors.replace(None);
                                    s.step.set(CreateStep::Prompt);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                    self.app_event_tx
                                        .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                                }
                            }
                        }
                        if go_overview {
                            self.mode = Mode::Overview;
                        } else {
                            self.mode = Mode::CreateTheme(s);
                        }
                    }
                }
            }
            KeyEvent { code: KeyCode::Esc, modifiers: KeyModifiers::NONE, .. } => match self.mode {
                Mode::Overview => self.is_complete = true,
                Mode::CreateSpinner(_) => {
                    self.mode = Mode::Spinner;
                }
                Mode::CreateTheme(_) => {
                    self.app_event_tx
                        .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                    self.mode = Mode::Themes;
                }
                _ => self.cancel_detail(),
            },
            KeyEvent { code: KeyCode::Char(c), modifiers, .. } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    if matches!(modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) {
                        match s.step.get() {
                            CreateStep::Prompt => s.prompt.push(c),
                            CreateStep::Action | CreateStep::Review => {}
                        }
                    }
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    if matches!(modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) {
                        match s.step.get() {
                            CreateStep::Prompt => s.prompt.push(c),
                            CreateStep::Action | CreateStep::Review => {}
                        }
                    }
                }
            }
            KeyEvent { code: KeyCode::Backspace, .. } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    match s.step.get() {
                        CreateStep::Prompt => {
                            if let Some((idx, _)) = s.prompt.grapheme_indices(true).last() {
                                s.prompt.truncate(idx);
                            } else {
                                s.prompt.clear();
                            }
                        }
                        CreateStep::Action | CreateStep::Review => {
                            return;
                        }
                    }
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    match s.step.get() {
                        CreateStep::Prompt => {
                            if let Some((idx, _)) = s.prompt.grapheme_indices(true).last() {
                                s.prompt.truncate(idx);
                            } else {
                                s.prompt.clear();
                            }
                        }
                        CreateStep::Action | CreateStep::Review => {
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn handle_key_event_direct(&mut self, key_event: KeyEvent) -> bool {
        let handled = matches!(
            key_event,
            KeyEvent { code: KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Enter | KeyCode::Esc, .. }
                | KeyEvent { code: KeyCode::Backspace, .. }
        ) || matches!(
            key_event,
            KeyEvent { code: KeyCode::Char(_), modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT, .. }
        );
        self.process_key_event(key_event);
        handled
    }
}

impl<'a> BottomPaneView<'a> for ThemeSelectionView {
    fn handle_paste(&mut self, text: String) -> super::bottom_pane_view::ConditionalUpdate {
        use super::bottom_pane_view::ConditionalUpdate;
        if let Mode::CreateSpinner(ref mut s) = self.mode {
            if s.is_loading.get() {
                return ConditionalUpdate::NoRedraw;
            }
            if matches!(s.step.get(), CreateStep::Prompt) {
                let paste = text.replace('\r', "\n");
                // The description is a single-line prompt; replace newlines with spaces.
                let paste = paste.replace('\n', " ");
                s.prompt.push_str(&paste);
                return ConditionalUpdate::NeedsRedraw;
            }
        } else if let Mode::CreateTheme(ref mut s) = self.mode {
            if s.is_loading.get() {
                return ConditionalUpdate::NoRedraw;
            }
            if matches!(s.step.get(), CreateStep::Prompt) {
                let paste = text.replace('\r', "\n");
                let paste = paste.replace('\n', " ");
                s.prompt.push_str(&paste);
                return ConditionalUpdate::NeedsRedraw;
            }
        }
        ConditionalUpdate::NoRedraw
    }
    fn desired_height(&self, _width: u16) -> u16 {
        match &self.mode {
            // Border (2) + inner padding (2) + 4 content rows = 8
            Mode::Overview => 8,
            // Detail lists: fixed 9 visible rows (max), shrink if fewer
            Mode::Themes => {
                let extra = if Self::allow_custom_theme_generation() { 1 } else { 0 };
                let n = (Self::get_theme_options().len() as u16) + extra;
                // Border(2) + padding(2) + title(1)+space(1) + list
                6 + n.min(9)
            }
            Mode::Spinner => {
                // +1 for the "Generate your own…" pseudo-row
                let n = (crate::spinner::spinner_names().len() as u16) + 1;
                // Border(2) + padding(2) + title(1)+space(1) + list
                6 + n.min(9)
            }
            // Title + spacer + 2 fields + buttons + help = 6 content rows
            // plus border(2) + padding(2) = 10; add 2 rows headroom for small terminals
            Mode::CreateSpinner(_) => 12,
            Mode::CreateTheme(_) => 12,
        }
    }

    fn handle_key_event(&mut self, _pane: &mut BottomPane<'a>, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // In create form, Up navigates fields/buttons
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                        }
                        CreateStep::Review => {
                            s.review_focus_is_toggle.set(true);
                        }
                    }
                } else {
                    match self.mode {
                        Mode::Overview => {
                            self.overview_selected_index =
                                self.overview_selected_index.saturating_sub(1) % 3;
                        }
                        _ => self.move_selection_up(),
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // In create form, Down navigates fields/buttons
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                        }
                        CreateStep::Review => {
                            s.review_focus_is_toggle.set(false);
                        }
                    }
                } else {
                    match &self.mode {
                        Mode::Overview => {
                            self.overview_selected_index = (self.overview_selected_index + 1) % 3;
                        }
                        _ => self.move_selection_down(),
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Treat Left like Up for navigation everywhere in this view
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx > 0 {
                                s.action_idx -= 1;
                            }
                        }
                        CreateStep::Review => {
                            // In review, Left focuses the toggle (mirrors Up)
                            s.review_focus_is_toggle.set(true);
                        }
                    }
                } else {
                    match self.mode {
                        Mode::Overview => {
                            self.overview_selected_index =
                                self.overview_selected_index.saturating_sub(1) % 3;
                        }
                        _ => self.move_selection_up(),
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Treat Right like Down for navigation everywhere in this view
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    let new_step = match s.step.get() {
                        CreateStep::Prompt => CreateStep::Action,
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Action
                        }
                        CreateStep::Review => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                            CreateStep::Review
                        }
                    };
                    s.step.set(new_step);
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    match s.step.get() {
                        CreateStep::Prompt => s.step.set(CreateStep::Action),
                        CreateStep::Action => {
                            if s.action_idx < 1 {
                                s.action_idx += 1;
                            }
                        }
                        CreateStep::Review => {
                            // In review, Right moves focus to buttons (mirrors Down)
                            s.review_focus_is_toggle.set(false);
                        }
                    }
                } else {
                    match &self.mode {
                        Mode::Overview => {
                            self.overview_selected_index = (self.overview_selected_index + 1) % 3;
                        }
                        _ => self.move_selection_down(),
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                // Take ownership of mode to avoid borrowing self while we may assign to self.mode
                let current_mode = std::mem::replace(&mut self.mode, Mode::Overview);
                match current_mode {
                    Mode::Overview => {
                        match self.overview_selected_index {
                            0 => {
                                self.revert_theme_on_back = self.current_theme;
                                self.mode = Mode::Themes;
                                self.just_entered_themes = true;
                            }
                            1 => {
                                self.revert_spinner_on_back = self.current_spinner.clone();
                                self.mode = Mode::Spinner;
                                self.app_event_tx.send(AppEvent::ScheduleFrameIn(
                                    std::time::Duration::from_millis(120),
                                ));
                                self.just_entered_spinner = true;
                            }
                            _ => {
                                // Close button
                                self.is_complete = true;
                                self.mode = Mode::Overview;
                            }
                        }
                    }
                    Mode::Themes => {
                        // If tail row selected (Generate your own…), open create form
                        let count = Self::get_theme_options().len();
                        if Self::allow_custom_theme_generation()
                            && self.selected_theme_index >= count
                        {
                            // Revert preview to the theme before entering Themes list for better legibility
                            self.app_event_tx
                                .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                            self.mode = Mode::CreateTheme(CreateThemeState {
                                step: std::cell::Cell::new(CreateStep::Prompt),
                                prompt: String::new(),
                                is_loading: std::cell::Cell::new(false),
                                action_idx: 0,
                                rx: None,
                                thinking_lines: std::cell::RefCell::new(Vec::new()),
                                thinking_current: std::cell::RefCell::new(String::new()),
                                proposed_name: std::cell::RefCell::new(None),
                                proposed_colors: std::cell::RefCell::new(None),
                                preview_on: std::cell::Cell::new(true),
                                review_focus_is_toggle: std::cell::Cell::new(true),
                                last_raw_output: std::cell::RefCell::new(None),
                                proposed_is_dark: std::cell::Cell::new(None),
                            });
                        } else {
                            // confirm_theme sets self.mode back to Overview
                            self.confirm_theme()
                        }
                    }
                    Mode::Spinner => {
                        // If tail row selected (Create your own…), open create form
                        let names = crate::spinner::spinner_names();
                        // Defensive: if selection somehow points to pseudo-row, clamp to current spinner index
                        if self.selected_spinner_index > names.len() {
                            self.selected_spinner_index = names.len().saturating_sub(1);
                        }
                        if self.selected_spinner_index >= names.len() {
                            self.mode = Mode::CreateSpinner(CreateState {
                                step: std::cell::Cell::new(CreateStep::Prompt),
                                prompt: String::new(),
                                is_loading: std::cell::Cell::new(false),
                                action_idx: 0,
                                rx: None,
                                thinking_lines: std::cell::RefCell::new(Vec::new()),
                                thinking_current: std::cell::RefCell::new(String::new()),
                                proposed_interval: std::cell::Cell::new(None),
                                proposed_frames: std::cell::RefCell::new(None),
                                proposed_name: std::cell::RefCell::new(None),
                                last_raw_output: std::cell::RefCell::new(None),
                            });
                        } else {
                            self.confirm_spinner()
                        }
                    }
                    Mode::CreateSpinner(mut s) => {
                        let mut go_overview = false;
                        match s.step.get() {
                            // Enter from Prompt submits immediately
                            CreateStep::Prompt => {
                                if !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_interval.set(None);
                                    s.proposed_frames.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_spinner_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                            }
                            CreateStep::Action => {
                                if s.action_idx == 0 && !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_interval.set(None);
                                    s.proposed_frames.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_spinner_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                } else {
                                    /* Cancel or already loading → return to overview */
                                    go_overview = true;
                                }
                            }
                            CreateStep::Review => {
                                if s.action_idx == 0 {
                                    // Save
                                    if let (Some(interval), Some(frames)) = (
                                        s.proposed_interval.get(),
                                        s.proposed_frames.borrow().clone(),
                                    ) {
                                        let display_name = s
                                            .proposed_name
                                            .borrow()
                                            .as_ref()
                                            .cloned()
                                            .unwrap_or_else(|| "Custom".to_string());
                                        if let Ok(home) = code_core::config::find_code_home() {
                                            let _ = code_core::config::set_custom_spinner(
                                                &home,
                                                "custom",
                                                &display_name,
                                                interval,
                                                &frames,
                                            );
                                        }
                                        crate::spinner::add_custom_spinner(
                                            "custom".to_string(),
                                            display_name.clone(),
                                            interval,
                                            frames,
                                        );
                                        crate::spinner::switch_spinner("custom");
                                        self.revert_spinner_on_back = "custom".to_string();
                                        self.current_spinner = "custom".to_string();
                                        self.app_event_tx
                                            .send(AppEvent::UpdateSpinner("custom".to_string()));
                                        self.send_tail("Custom spinner saved".to_string());
                                        go_overview = true;
                                    }
                                } else {
                                    // Retry -> return to input (Prompt) without running
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_interval.set(None);
                                    s.proposed_frames.replace(None);
                                    s.step.set(CreateStep::Prompt);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                            }
                        }
                        if go_overview {
                            self.mode = Mode::Overview;
                        } else {
                            self.mode = Mode::CreateSpinner(s);
                        }
                    }
                    Mode::CreateTheme(mut s) => {
                        let mut go_overview = false;
                        match s.step.get() {
                            CreateStep::Prompt => {
                                if !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_name.replace(None);
                                    s.proposed_colors.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_theme_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                }
                            }
                            CreateStep::Action => {
                                if s.action_idx == 0 && !s.is_loading.get() {
                                    let user_prompt = s.prompt.clone();
                                    s.is_loading.set(true);
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_name.replace(None);
                                    s.proposed_colors.replace(None);
                                    let (txp, rxp) = std::sync::mpsc::channel::<ProgressMsg>();
                                    s.rx = Some(rxp);
                                    self.kickoff_theme_creation(user_prompt, txp);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                } else {
                                    // Cancel
                                    go_overview = true;
                                }
                            }
                            CreateStep::Review => {
                                if s.review_focus_is_toggle.get() {
                                    // Toggle preview on/off
                                    let now_on = !s.preview_on.get();
                                    s.preview_on.set(now_on);
                                    if now_on {
                                        // Reapply preview
                                        if let (Some(name), Some(colors)) = (
                                            s.proposed_name.borrow().clone(),
                                            s.proposed_colors.borrow().clone(),
                                        ) {
                                            crate::theme::set_custom_theme_colors(colors.clone());
                                            crate::theme::set_custom_theme_label(name.clone());
                                            crate::theme::init_theme(
                                                &code_core::config_types::ThemeConfig {
                                                    name: ThemeName::Custom,
                                                    colors,
                                                    label: Some(name),
                                                    is_dark: s.proposed_is_dark.get(),
                                                },
                                            );
                                        }
                                    } else {
                                        // Revert to previous built-in or Photon if previous was Custom
                                        let fallback =
                                            if self.revert_theme_on_back == ThemeName::Custom {
                                                ThemeName::LightPhoton
                                            } else {
                                                self.revert_theme_on_back
                                            };
                                        self.app_event_tx.send(AppEvent::PreviewTheme(fallback));
                                    }
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                } else if s.action_idx == 0 {
                                    // Save
                                    if let (Some(name), Some(colors)) = (
                                        s.proposed_name.borrow().clone(),
                                        s.proposed_colors.borrow().clone(),
                                    ) {
                                        if let Ok(home) = code_core::config::find_code_home() {
                                            let _ = code_core::config::set_custom_theme(
                                                &home,
                                                &name,
                                                &colors,
                                                s.preview_on.get(),
                                                s.proposed_is_dark.get(),
                                            );
                                        }
                                        crate::theme::set_custom_theme_label(name.clone());
                                        crate::theme::set_custom_theme_colors(colors.clone());
                                        crate::theme::set_custom_theme_is_dark(
                                            s.proposed_is_dark.get(),
                                        );
                                        if s.preview_on.get() {
                                            // Keep preview and set active in UI if chosen
                                            crate::theme::init_theme(
                                                &code_core::config_types::ThemeConfig {
                                                    name: ThemeName::Custom,
                                                    colors: colors.clone(),
                                                    label: Some(name.clone()),
                                                    is_dark: s.proposed_is_dark.get(),
                                                },
                                            );
                                            self.revert_theme_on_back = ThemeName::Custom;
                                            self.current_theme = ThemeName::Custom;
                                            self.app_event_tx
                                                .send(AppEvent::UpdateTheme(ThemeName::Custom));
                                        } else {
                                            // Saved but not active: revert to previous theme visually
                                            self.app_event_tx.send(AppEvent::PreviewTheme(
                                                self.revert_theme_on_back,
                                            ));
                                        }
                                        // Informative status depending on whether we set active
                                        if s.preview_on.get() {
                                            self.send_before_next_output(format!(
                                                "Set theme to {}",
                                                name
                                            ));
                                        } else {
                                            self.send_before_next_output(format!(
                                                "Saved custom theme {} (not active)",
                                                name
                                            ));
                                        }
                                        go_overview = true;
                                    }
                                } else {
                                    // Retry -> back to input
                                    s.thinking_lines.borrow_mut().clear();
                                    s.thinking_current.borrow_mut().clear();
                                    s.proposed_name.replace(None);
                                    s.proposed_colors.replace(None);
                                    s.step.set(CreateStep::Prompt);
                                    self.app_event_tx.send(AppEvent::RequestRedraw);
                                    // Revert to previous theme while editing
                                    self.app_event_tx
                                        .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                                }
                            }
                        }
                        if go_overview {
                            self.mode = Mode::Overview;
                        } else {
                            self.mode = Mode::CreateTheme(s);
                        }
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => match self.mode {
                Mode::Overview => self.is_complete = true,
                Mode::CreateSpinner(_) => {
                    self.mode = Mode::Spinner;
                }
                Mode::CreateTheme(_) => {
                    // Revert preview to prior theme
                    self.app_event_tx
                        .send(AppEvent::PreviewTheme(self.revert_theme_on_back));
                    self.mode = Mode::Themes;
                }
                _ => self.cancel_detail(),
            },
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    // Accept typing when no modifiers or Shift is held
                    if matches!(modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) {
                        match s.step.get() {
                            CreateStep::Prompt => s.prompt.push(c),
                            CreateStep::Action | CreateStep::Review => {}
                        }
                    }
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    if matches!(modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT) {
                        match s.step.get() {
                            CreateStep::Prompt => s.prompt.push(c),
                            CreateStep::Action | CreateStep::Review => {}
                        }
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                if let Mode::CreateSpinner(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    match s.step.get() {
                        CreateStep::Prompt => {
                            s.prompt.pop();
                        }
                        CreateStep::Action | CreateStep::Review => {
                            return;
                        }
                    }
                } else if let Mode::CreateTheme(ref mut s) = self.mode {
                    if s.is_loading.get() {
                        return;
                    }
                    match s.step.get() {
                        CreateStep::Prompt => {
                            s.prompt.pop();
                        }
                        CreateStep::Action | CreateStep::Review => {
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.is_complete
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let options = Self::get_theme_options();
        let theme = crate::theme::current_theme();

        // Use full width and draw an outer window styled like the Diff overlay
        let render_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
        };
        Clear.render(render_area, buf);

        // Add one row of padding above the top border (clear + background)
        if render_area.y > 0 {
            let pad = Rect {
                x: render_area.x,
                y: render_area.y - 1,
                width: render_area.width,
                height: 1,
            };
            Clear.render(pad, buf);
            let pad_bg = Block::default().style(Style::default().bg(crate::colors::background()));
            pad_bg.render(pad, buf);
        }

        // Build a styled title with concise hints
        let t_dim = Style::default().fg(crate::colors::text_dim());
        let t_fg = Style::default().fg(crate::colors::text());
        let mut title_spans = vec![Span::styled(" ", t_dim), Span::styled("/theme", t_fg)];
        title_spans.extend_from_slice(&[
            Span::styled(" ——— ", t_dim),
            Span::styled("▲ ▼ ◀ ▶", t_fg),
            Span::styled(" select ", t_dim),
            Span::styled("——— ", t_dim),
            Span::styled("Enter", t_fg),
            Span::styled(" choose ", t_dim),
            Span::styled("——— ", t_dim),
            Span::styled("Esc", t_fg),
        ]);
        if matches!(self.mode, Mode::Overview) {
            title_spans.push(Span::styled(" close ", t_dim));
        } else {
            title_spans.push(Span::styled(" back ", t_dim));
        }

        let outer = Block::default()
            .borders(Borders::ALL)
            .title(Line::from(title_spans))
            .style(Style::default().bg(crate::colors::background()))
            .border_style(
                Style::default()
                    .fg(crate::colors::border())
                    .bg(crate::colors::background()),
            );
        let inner = outer.inner(render_area);
        outer.render(render_area, buf);

        // Paint inner content background as the normal theme background
        let inner_bg_style = Style::default().bg(crate::colors::background());
        for y in inner.y..inner.y + inner.height {
            for x in inner.x..inner.x + inner.width {
                buf[(x, y)].set_style(inner_bg_style);
            }
        }

        // Add one cell padding around the inside; body occupies full padded area
        let padded = inner.inner(ratatui::layout::Margin::new(1, 1));
        let body_area = padded;

        // Visible rows = available body height (already sized to ≤10)
        let available_height = body_area.height as usize;

        // Create body content
        let mut lines = Vec::new();
        if matches!(self.mode, Mode::Overview) {
            // Overview: two clear actions, also show current values
            let theme_label_owned = if self.current_theme == ThemeName::Custom {
                crate::theme::custom_theme_label().unwrap_or_else(|| "Custom".to_string())
            } else {
                Self::get_theme_options()
                    .iter()
                    .find(|(t, _, _)| *t == self.current_theme)
                    .map(|(_, name, _)| (*name).to_string())
                    .unwrap_or_else(|| "Theme".to_string())
            };
            // Row 0: Change Theme
            // Row 0: Theme
            {
                let selected = 0 == self.overview_selected_index;
                let mut spans = vec![Span::raw(" ")];
                if selected {
                    spans.push(Span::styled("› ", Style::default().fg(theme.keyword)));
                } else {
                    spans.push(Span::raw("  "));
                }
                let k = "Change Theme";
                if selected {
                    spans.push(Span::styled(
                        k,
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(k, Style::default().fg(theme.text)));
                }
                spans.push(Span::raw(" — "));
                spans.push(Span::styled(
                    theme_label_owned,
                    Style::default().fg(theme.text_dim),
                ));
                lines.push(Line::from(spans));
            }
            // Row 1: Spinner
            {
                let selected = 1 == self.overview_selected_index;
                let mut spans = vec![Span::raw(" ")];
                if selected {
                    spans.push(Span::styled("› ", Style::default().fg(theme.keyword)));
                } else {
                    spans.push(Span::raw("  "));
                }
                let k = "Change Spinner";
                if selected {
                    spans.push(Span::styled(
                        k,
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(k, Style::default().fg(theme.text)));
                }
                spans.push(Span::raw(" — "));
                let label = crate::spinner::spinner_label_for(&self.current_spinner);
                spans.push(Span::styled(label, Style::default().fg(theme.text_dim)));
                lines.push(Line::from(spans));
                // Spacer line before the Close button
                lines.push(Line::default());
            }
            // Row 2: Close button on its own line
            {
                let selected = 2 == self.overview_selected_index;
                let mut spans = vec![Span::raw(" ")];
                // Indicate selection with the same chevron prefix used above
                if selected {
                    spans.push(Span::styled("› ", Style::default().fg(theme.keyword)));
                } else {
                    spans.push(Span::raw("  "));
                }
                let sel = |b: bool| {
                    if b {
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    }
                };
                spans.push(Span::styled("[ Close ]", sel(selected)));
                lines.push(Line::from(spans));
            }
        } else if matches!(self.mode, Mode::Themes) {
            // Header: Choose Theme
            lines.push(Line::from(Span::styled(
                "Choose Theme",
                Style::default()
                    .fg(theme.text_bright)
                    .add_modifier(Modifier::BOLD),
            )));
            // Compute anchored window: top until middle, then center; bottom shows end
            let allow_custom = Self::allow_custom_theme_generation();
            let count = options.len() + if allow_custom { 1 } else { 0 };
            let visible = available_height.saturating_sub(1).min(9).max(1);
            let (start, _vis, _mid) = crate::util::list_window::anchored_window(
                self.selected_theme_index,
                count,
                visible,
            );
            let end = (start + visible).min(count);
            for i in start..end {
                let is_selected = i == self.selected_theme_index;
                if allow_custom && i >= options.len() {
                    // Pseudo-row: Generate your own…
                    let mut spans = vec![Span::raw(" ")];
                    if is_selected {
                        spans.push(Span::styled("› ", Style::default().fg(theme.keyword)));
                    } else {
                        spans.push(Span::raw("  "));
                    }
                    let label_style = if is_selected {
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_dim)
                    };
                    spans.push(Span::styled("Generate your own…", label_style));
                    lines.push(Line::from(spans));
                    continue;
                }
                let (theme_enum, name, description) = &options[i];
                let is_original = *theme_enum == self.original_theme;

                let prefix_selected = is_selected;
                let suffix = if is_original { " (original)" } else { "" };

                let mut spans = vec![Span::raw(" ")];
                if prefix_selected {
                    spans.push(Span::styled("› ", Style::default().fg(theme.keyword)));
                } else {
                    spans.push(Span::raw("  "));
                }

                if is_selected {
                    spans.push(Span::styled(
                        *name,
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::styled(*name, Style::default().fg(theme.text)));
                }

                spans.push(Span::styled(suffix, Style::default().fg(theme.text_dim)));

                if !suffix.is_empty() {
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::raw("  "));
                }

                spans.push(Span::styled(
                    *description,
                    Style::default().fg(theme.text_dim),
                ));

                lines.push(Line::from(spans));
            }
        } else if matches!(self.mode, Mode::CreateSpinner(_)) {
            // Inline form for custom spinner with visible selection & caret
            let theme = crate::theme::current_theme();
            if let Mode::CreateSpinner(s) = &self.mode {
                // Drain progress messages if streaming
                if let Some(rx) = &s.rx {
                    for _ in 0..100 {
                        // limit per render to keep UI snappy
                        match rx.try_recv() {
                            Ok(ProgressMsg::ThinkingDelta(d)) => {
                                if let Mode::CreateSpinner(ref sm) = self.mode {
                                    let mut cur = sm.thinking_current.borrow_mut();
                                    let mut hist = sm.thinking_lines.borrow_mut();
                                    cur.push_str(&d);
                                    if let Some(pos) = cur.rfind('\n') {
                                        // Split on last newline: commit completed portion, keep remainder
                                        let (complete, remainder) = cur.split_at(pos);
                                        if !complete.trim().is_empty() {
                                            hist.push(complete.trim().to_string());
                                        }
                                        *cur = remainder.trim_start_matches('\n').to_string();
                                        let keep = 10usize;
                                        let len = hist.len();
                                        if len > keep {
                                            hist.drain(0..len - keep);
                                        }
                                    }
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::OutputDelta(d)) => {
                                // Treat assistant text deltas the same as thinking: append to current; on newline commit to history
                                if let Mode::CreateSpinner(ref sm) = self.mode {
                                    let mut cur = sm.thinking_current.borrow_mut();
                                    let mut hist = sm.thinking_lines.borrow_mut();
                                    cur.push_str(&d);
                                    if let Some(pos) = cur.rfind('\n') {
                                        let (complete, remainder) = cur.split_at(pos);
                                        if !complete.trim().is_empty() {
                                            hist.push(complete.trim().to_string());
                                        }
                                        *cur = remainder.trim_start_matches('\n').to_string();
                                        let keep = 10usize;
                                        let len = hist.len();
                                        if len > keep {
                                            hist.drain(0..len - keep);
                                        }
                                    }
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::RawOutput(raw)) => {
                                if let Mode::CreateSpinner(ref sm) = self.mode {
                                    sm.last_raw_output.replace(Some(raw));
                                }
                            }
                            Ok(ProgressMsg::SetStatus(s)) => {
                                if let Mode::CreateSpinner(ref sm) = self.mode {
                                    let mut cur = sm.thinking_current.borrow_mut();
                                    cur.clear();
                                    cur.push_str(&s);
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::CompletedOk {
                                name,
                                interval,
                                frames,
                            }) => {
                                if let Mode::CreateSpinner(ref sm) = self.mode {
                                    sm.is_loading.set(false);
                                    sm.step.set(CreateStep::Review);
                                    sm.proposed_interval.set(Some(interval));
                                    sm.proposed_frames.replace(Some(frames));
                                    sm.proposed_name.replace(Some(name));
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::CompletedErr {
                                error,
                                _raw_snippet: _,
                            }) => {
                                if let Mode::CreateSpinner(ref sm) = self.mode {
                                    sm.is_loading.set(false);
                                    sm.step.set(CreateStep::Action);
                                    sm.thinking_lines
                                        .borrow_mut()
                                        .push(format!("Error: {}", error));
                                    sm.thinking_current.borrow_mut().clear();
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                            Ok(ProgressMsg::CompletedThemeOk(..)) => {}
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                        }
                    }
                }
                let mut form_lines = Vec::new();
                // While loading: replace the entire content with spinner + latest message
                if s.is_loading.get() {
                    form_lines.push(Line::from(Span::styled(
                        "Overview » Change Spinner » Create Custom",
                        Style::default()
                            .fg(theme.text_bright)
                            .add_modifier(Modifier::BOLD),
                    )));
                    // One blank line between title and spinner line
                    // Use an actually empty line to avoid wrap/spacing quirks
                    // with a single-space line under ratatui wrapping.
                    form_lines.push(Line::default());
                    use std::time::SystemTime;
                    use std::time::UNIX_EPOCH;
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let diamond = ["◇", "◆", "◇", "◆"];
                    let frame = diamond[((now_ms / 120) as usize) % diamond.len()].to_string();
                    form_lines.push(Line::from(vec![
                        Span::styled(frame, Style::default().fg(crate::colors::info())),
                        Span::styled(
                            " Generating spinner with AI…",
                            Style::default().fg(theme.text_bright),
                        ),
                    ]));
                    // Latest message only
                    // Show the latest in‑progress line if present, otherwise last completed line
                    let cur = s.thinking_current.borrow();
                    let latest = if !cur.trim().is_empty() {
                        cur.trim().to_string()
                    } else {
                        s.thinking_lines
                            .borrow()
                            .iter()
                            .rev()
                            .find(|l| !l.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| "Waiting for model…".to_string())
                    };
                    let mut latest_render = latest.to_string();
                    if !latest_render.ends_with('…') {
                        latest_render.push_str(" …");
                    }
                    form_lines.push(Line::from(Span::styled(
                        latest_render,
                        Style::default().fg(theme.text_dim),
                    )));
                    self.app_event_tx.send(AppEvent::ScheduleFrameIn(
                        std::time::Duration::from_millis(120),
                    ));
                    Paragraph::new(form_lines)
                        .alignment(Alignment::Left)
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .render(body_area, buf);
                    return;
                }

                // After completion (review)
                if matches!(s.step.get(), CreateStep::Review) {
                    // Theme review layout (header + toggle + buttons)
                    if let Mode::CreateTheme(ref st) = self.mode {
                        form_lines.push(Line::from(Span::styled(
                            "Overview » Change Theme » Create Custom",
                            Style::default()
                                .fg(theme.text_bright)
                                .add_modifier(Modifier::BOLD),
                        )));
                        form_lines.push(Line::default());
                        let name = st
                            .proposed_name
                            .borrow()
                            .clone()
                            .unwrap_or_else(|| "Custom".to_string());
                        let onoff = if st.preview_on.get() { "on" } else { "off" };
                        let sel = st.review_focus_is_toggle.get();
                        let style = if sel {
                            Style::default()
                                .fg(theme.primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.text)
                        };
                        form_lines.push(Line::from(Span::styled(
                            format!("Now showing {} [{}]", name, onoff),
                            style,
                        )));
                        form_lines.push(Line::default());
                        let mut spans: Vec<Span> = Vec::new();
                        // When toggle is focused, buttons are unselected
                        let primary_selected =
                            !st.review_focus_is_toggle.get() && s.action_idx == 0;
                        let secondary_selected =
                            !st.review_focus_is_toggle.get() && s.action_idx == 1;
                        let selbtn = |b: bool| {
                            if b {
                                Style::default()
                                    .fg(theme.primary)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme.text)
                            }
                        };
                        spans.push(Span::styled("[ Save ]", selbtn(primary_selected)));
                        spans.push(Span::raw("  "));
                        spans.push(Span::styled("[ Retry ]", selbtn(secondary_selected)));
                        form_lines.push(Line::from(spans));
                        Paragraph::new(form_lines)
                            .alignment(Alignment::Left)
                            .wrap(ratatui::widgets::Wrap { trim: false })
                            .render(body_area, buf);
                        return;
                    }
                    // Spinner review layout (header + preview + buttons)
                    form_lines.push(Line::from(Span::styled(
                        "Overview » Change Spinner » Create Custom",
                        Style::default()
                            .fg(theme.text_bright)
                            .add_modifier(Modifier::BOLD),
                    )));
                    // Theme review header with preview toggle when in theme mode
                    if let Mode::CreateTheme(ref st) = self.mode {
                        let name = st
                            .proposed_name
                            .borrow()
                            .clone()
                            .unwrap_or_else(|| "Custom".to_string());
                        let onoff = if st.preview_on.get() { "on" } else { "off" };
                        let sel = st.review_focus_is_toggle.get();
                        let style = if sel {
                            Style::default()
                                .fg(theme.primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.text)
                        };
                        form_lines.push(Line::from(Span::styled(
                            format!("Now showing {} [{}]", name, onoff),
                            style,
                        )));
                        form_lines.push(Line::default());
                    }
                    // Blank line between title and preview row
                    form_lines.push(Line::default());
                    // Preview styled like selection rows: border rules + spinner + label
                    if let (Some(interval), Some(frames)) = (
                        s.proposed_interval.get(),
                        s.proposed_frames.borrow().as_ref(),
                    ) {
                        use std::time::SystemTime;
                        use std::time::UNIX_EPOCH;
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let idx = if frames.is_empty() {
                            0
                        } else {
                            ((now_ms / interval) as usize) % frames.len()
                        };
                        let preview = frames.get(idx).cloned().unwrap_or_default();
                        // Spacer above the preview row
                        // Compute layout similar to the list row: left rule | spinner | label | right rule
                        let label = "Preview";
                        let max_frame_len: u16 = preview.chars().count() as u16;
                        let border = Style::default().fg(crate::colors::border());
                        let fg = Style::default().fg(crate::colors::info());
                        let x: u16 = max_frame_len.saturating_add(8);
                        let border_len = x.saturating_sub(max_frame_len);
                        let mut spans: Vec<Span> = Vec::new();
                        spans.push(Span::styled("─".repeat(border_len as usize), border));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(preview, fg));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(format!("{}...", label), fg));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled("─".repeat(border_len as usize), border));
                        form_lines.push(Line::from(spans));
                        self.app_event_tx.send(AppEvent::ScheduleFrameIn(
                            std::time::Duration::from_millis(interval),
                        ));
                    }
                    // Add spacing line between preview and buttons
                    // Spacing between preview and buttons
                    form_lines.push(Line::default());
                    // Buttons row moved to bottom
                    let mut spans: Vec<Span> = Vec::new();
                    // In Theme review: allow focusing the toggle line; if focused there, do not style buttons as selected
                    let primary_selected = if let Mode::CreateTheme(ref st) = self.mode {
                        !st.review_focus_is_toggle.get() && s.action_idx == 0
                    } else {
                        s.action_idx == 0
                    };
                    let secondary_selected = if let Mode::CreateTheme(ref st) = self.mode {
                        !st.review_focus_is_toggle.get() && s.action_idx == 1
                    } else {
                        s.action_idx == 1
                    };
                    let sel = |b: bool| {
                        if b {
                            Style::default()
                                .fg(theme.primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.text)
                        }
                    };
                    spans.push(Span::styled("[ Save ]", sel(primary_selected)));
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled("[ Retry ]", sel(secondary_selected)));
                    form_lines.push(Line::from(spans));
                    Paragraph::new(form_lines)
                        .alignment(Alignment::Left)
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .render(body_area, buf);
                    return;
                }

                // Default (idle): header, description input, border, and Generate/Cancel buttons
                form_lines.push(Line::from(Span::styled(
                    "Overview » Change Spinner » Create Custom",
                    Style::default()
                        .fg(theme.text_bright)
                        .add_modifier(Modifier::BOLD),
                )));
                // Blank line between title and content
                form_lines.push(Line::default());
                form_lines.push(Line::from(Span::styled(
                    "Code can generate a custom spinner just for you!",
                    Style::default().fg(theme.text),
                )));
                form_lines.push(Line::from(Span::styled(
                    "What sort of spinner would you like? (e.g. bouncing dot party, cat eating a pizza)",
                    Style::default().fg(theme.text_dim)
                )));
                // Exactly one blank line above Description
                form_lines.push(Line::default());
                // Show error above description if any
                if let Some(last) = s.thinking_lines.borrow().last().cloned() {
                    if last.starts_with("Error:") {
                        form_lines.push(Line::from(Span::styled(
                            last,
                            Style::default().fg(crate::colors::error()),
                        )));
                        if let Some(raw) = s.last_raw_output.borrow().as_ref() {
                            form_lines.push(Line::from(Span::styled(
                                "Model output (raw):",
                                Style::default().fg(theme.text_dim),
                            )));
                            for ln in raw.split('\n') {
                                form_lines.push(Line::from(Span::styled(
                                    ln.to_string(),
                                    Style::default().fg(theme.text),
                                )));
                            }
                        }
                        form_lines.push(Line::default());
                    }
                }
                let caret = Span::styled("▏", Style::default().fg(theme.info));
                let mut desc_spans: Vec<Span> = Vec::new();
                desc_spans.push(Span::styled(
                    "Description: ",
                    Style::default().fg(theme.keyword),
                ));
                let active = matches!(s.step.get(), CreateStep::Prompt);
                desc_spans.push(Span::styled(
                    s.prompt.clone(),
                    Style::default().fg(theme.text_bright),
                ));
                if active {
                    desc_spans.push(caret.clone());
                }
                form_lines.push(Line::from(desc_spans));
                form_lines.push(Line::from(Span::styled(
                    "─".repeat((body_area.width.saturating_sub(4)) as usize),
                    Style::default().fg(crate::colors::border()),
                )));
                // Buttons
                let mut spans: Vec<Span> = Vec::new();
                let on_actions = matches!(s.step.get(), CreateStep::Action);
                let primary_selected = on_actions && s.action_idx == 0;
                let secondary_selected = on_actions && s.action_idx == 1;
                let sel = |b: bool| {
                    if b {
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    }
                };
                spans.push(Span::styled("[ Generate... ]", sel(primary_selected)));
                spans.push(Span::raw("  "));
                spans.push(Span::styled("[ Cancel ]", sel(secondary_selected)));
                form_lines.push(Line::from(spans));

                Paragraph::new(form_lines)
                    .alignment(Alignment::Left)
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .render(body_area, buf);
            }
            return;
        } else if matches!(self.mode, Mode::CreateTheme(_)) {
            let theme = crate::theme::current_theme();
            if let Mode::CreateTheme(s) = &self.mode {
                if let Some(rx) = &s.rx {
                    for _ in 0..100 {
                        match rx.try_recv() {
                            Ok(ProgressMsg::ThinkingDelta(d)) | Ok(ProgressMsg::OutputDelta(d)) => {
                                if let Mode::CreateTheme(ref sm) = self.mode {
                                    let mut cur = sm.thinking_current.borrow_mut();
                                    let mut hist = sm.thinking_lines.borrow_mut();
                                    cur.push_str(&d);
                                    if let Some(pos) = cur.rfind('\n') {
                                        let (complete, remainder) = cur.split_at(pos);
                                        if !complete.trim().is_empty() {
                                            hist.push(complete.trim().to_string());
                                        }
                                        *cur = remainder.trim_start_matches('\n').to_string();
                                        let keep = 10usize;
                                        let len = hist.len();
                                        if len > keep {
                                            hist.drain(0..len - keep);
                                        }
                                    }
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::SetStatus(s)) => {
                                if let Mode::CreateTheme(ref sm) = self.mode {
                                    let mut cur = sm.thinking_current.borrow_mut();
                                    cur.clear();
                                    cur.push_str(&s);
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::CompletedThemeOk(name, colors, is_dark)) => {
                                if let Mode::CreateTheme(ref sm) = self.mode {
                                    sm.is_loading.set(false);
                                    sm.step.set(CreateStep::Review);
                                    sm.proposed_name.replace(Some(name.clone()));
                                    sm.proposed_colors.replace(Some(colors.clone()));
                                    sm.proposed_is_dark.set(is_dark);
                                    crate::theme::set_custom_theme_label(name.clone());
                                    crate::theme::set_custom_theme_is_dark(is_dark);
                                    crate::theme::init_theme(
                                        &code_core::config_types::ThemeConfig {
                                            name: ThemeName::Custom,
                                            colors: colors.clone(),
                                            label: Some(name),
                                            is_dark,
                                        },
                                    );
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Ok(ProgressMsg::CompletedOk { .. }) => {}
                            Ok(ProgressMsg::RawOutput(raw)) => {
                                if let Mode::CreateTheme(ref sm) = self.mode {
                                    sm.last_raw_output.replace(Some(raw));
                                }
                            }
                            Ok(ProgressMsg::CompletedErr { error, .. }) => {
                                if let Mode::CreateTheme(ref sm) = self.mode {
                                    sm.is_loading.set(false);
                                    sm.step.set(CreateStep::Action);
                                    sm.thinking_lines
                                        .borrow_mut()
                                        .push(format!("Error: {}", error));
                                    sm.thinking_current.borrow_mut().clear();
                                }
                                self.app_event_tx.send(AppEvent::RequestRedraw);
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                        }
                    }
                }
                let mut form_lines = Vec::new();
                if s.is_loading.get() {
                    form_lines.push(Line::from(Span::styled(
                        "Overview » Change Theme » Create Custom",
                        Style::default()
                            .fg(theme.text_bright)
                            .add_modifier(Modifier::BOLD),
                    )));
                    form_lines.push(Line::default());
                    use std::time::SystemTime;
                    use std::time::UNIX_EPOCH;
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let frames = ["◌", "◔", "◑", "◕", "●", "◕", "◑", "◔"];
                    let frame = frames[((now_ms / 100) as usize) % frames.len()].to_string();
                    form_lines.push(Line::from(vec![
                        Span::styled(frame, Style::default().fg(crate::colors::info())),
                        Span::styled(
                            " Generating theme with AI…",
                            Style::default().fg(theme.text_bright),
                        ),
                    ]));
                    let cur = s.thinking_current.borrow();
                    let latest = if !cur.trim().is_empty() {
                        cur.trim().to_string()
                    } else {
                        s.thinking_lines
                            .borrow()
                            .iter()
                            .rev()
                            .find(|l| !l.trim().is_empty())
                            .cloned()
                            .unwrap_or_else(|| "Waiting for model…".to_string())
                    };
                    let mut latest_render = latest.to_string();
                    if !latest_render.ends_with('…') {
                        latest_render.push_str(" …");
                    }
                    form_lines.push(Line::from(Span::styled(
                        latest_render,
                        Style::default().fg(theme.text_dim),
                    )));
                    self.app_event_tx.send(AppEvent::ScheduleFrameIn(
                        std::time::Duration::from_millis(100),
                    ));
                    Paragraph::new(form_lines)
                        .alignment(Alignment::Left)
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .render(body_area, buf);
                    return;
                }
                if matches!(s.step.get(), CreateStep::Review) {
                    // Header
                    form_lines.push(Line::from(Span::styled(
                        "Overview » Change Theme » Create Custom",
                        Style::default()
                            .fg(theme.text_bright)
                            .add_modifier(Modifier::BOLD),
                    )));
                    form_lines.push(Line::default());
                    // Toggle line: Now showing <Name> [on|off]
                    let name = s
                        .proposed_name
                        .borrow()
                        .clone()
                        .unwrap_or_else(|| "Custom".to_string());
                    let onoff = if s.preview_on.get() { "on" } else { "off" };
                    let toggle_style = if s.review_focus_is_toggle.get() {
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };
                    form_lines.push(Line::from(Span::styled(
                        format!("Now showing {} [{}]", name, onoff),
                        toggle_style,
                    )));
                    form_lines.push(Line::default());
                    // Buttons (Save / Retry)
                    let mut spans: Vec<Span> = Vec::new();
                    let save_sel = !s.review_focus_is_toggle.get() && s.action_idx == 0;
                    let retry_sel = !s.review_focus_is_toggle.get() && s.action_idx == 1;
                    let sel = |b: bool| {
                        if b {
                            Style::default()
                                .fg(theme.primary)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(theme.text)
                        }
                    };
                    spans.push(Span::styled("[ Save ]", sel(save_sel)));
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled("[ Retry ]", sel(retry_sel)));
                    form_lines.push(Line::from(spans));
                    Paragraph::new(form_lines)
                        .alignment(Alignment::Left)
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .render(body_area, buf);
                    return;
                }
                // Idle form
                form_lines.push(Line::from(Span::styled(
                    "Overview » Change Theme » Create Custom",
                    Style::default()
                        .fg(theme.text_bright)
                        .add_modifier(Modifier::BOLD),
                )));
                form_lines.push(Line::default());
                // If there was a recent error, show it once above description (with full raw output)
                if let Some(last) = s.thinking_lines.borrow().last().cloned() {
                    if last.starts_with("Error:") {
                        form_lines.push(Line::from(Span::styled(
                            last,
                            Style::default().fg(crate::colors::error()),
                        )));
                        if let Some(raw) = s.last_raw_output.borrow().as_ref() {
                            form_lines.push(Line::from(Span::styled(
                                "Model output (raw):",
                                Style::default().fg(theme.text_dim),
                            )));
                            for ln in raw.split('\n') {
                                form_lines.push(Line::from(Span::styled(
                                    ln.to_string(),
                                    Style::default().fg(theme.text),
                                )));
                            }
                        }
                        form_lines.push(Line::default());
                    }
                }
                form_lines.push(Line::from(Span::styled(
                    "Code can generate a custom theme just for you!",
                    Style::default().fg(theme.text),
                )));
                form_lines.push(Line::from(Span::styled(
                    "What should it look like? (e.g. Light Sunrise with Palm Trees, Dark River with Fireflies)",
                    Style::default().fg(theme.text_dim),
                )));
                form_lines.push(Line::default());
                let mut desc_spans: Vec<Span> = Vec::new();
                desc_spans.push(Span::styled(
                    "Description: ",
                    Style::default().fg(theme.keyword),
                ));
                let active = matches!(s.step.get(), CreateStep::Prompt);
                desc_spans.push(Span::styled(
                    s.prompt.clone(),
                    Style::default().fg(theme.text_bright),
                ));
                if active {
                    desc_spans.push(Span::styled("▏", Style::default().fg(theme.info)));
                }
                form_lines.push(Line::from(desc_spans));
                form_lines.push(Line::from(Span::styled(
                    "─".repeat((body_area.width.saturating_sub(4)) as usize),
                    Style::default().fg(crate::colors::border()),
                )));
                let mut spans: Vec<Span> = Vec::new();
                let on_actions = matches!(s.step.get(), CreateStep::Action);
                let primary_selected = on_actions && s.action_idx == 0;
                let secondary_selected = on_actions && s.action_idx == 1;
                let sel = |b: bool| {
                    if b {
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    }
                };
                spans.push(Span::styled("[ Generate... ]", sel(primary_selected)));
                spans.push(Span::raw("  "));
                spans.push(Span::styled("[ Cancel ]", sel(secondary_selected)));
                form_lines.push(Line::from(spans));
                Paragraph::new(form_lines)
                    .alignment(Alignment::Left)
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .render(body_area, buf);
                return;
            }
        } else {
            // Spinner: render one centered preview row per spinner, matching the composer title
            use std::time::SystemTime;
            use std::time::UNIX_EPOCH;
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let names = crate::spinner::spinner_names();
            // Include an extra pseudo-row for "Generate your own…"
            let count = names.len() + 1;
            // Reserve two rows (header + spacer)
            let visible = available_height.saturating_sub(2).min(9).max(1);
            let (start, _vis, _mid) = crate::util::list_window::anchored_window(
                self.selected_spinner_index,
                count,
                visible,
            );
            let end = (start + visible).min(count);

            // Compute fixed column widths globally so rows never jump when scrolling
            let max_frame_len: u16 = crate::spinner::global_max_frame_len() as u16;
            let mut max_label_len: u16 = 0;
            for name in names.iter() {
                let label = crate::spinner::spinner_label_for(name);
                max_label_len = max_label_len.max(label.chars().count() as u16);
            }

            // Render header (left-aligned) and spacer row
            let header_rect = Rect {
                x: body_area.x,
                y: body_area.y,
                width: body_area.width,
                height: 1,
            };
            let header = Line::from(Span::styled(
                "Overview » Change Spinner",
                Style::default()
                    .fg(theme.text_bright)
                    .add_modifier(Modifier::BOLD),
            ));
            Paragraph::new(header)
                .alignment(Alignment::Left)
                .render(header_rect, buf);
            if header_rect.y + 1 < body_area.y + body_area.height {
                let spacer = Rect {
                    x: body_area.x,
                    y: body_area.y + 1,
                    width: body_area.width,
                    height: 1,
                };
                Paragraph::new(Line::default()).render(spacer, buf);
            }

            for row_idx in 0..(end - start) {
                let i = start + row_idx;
                // rows start two below (header + spacer)
                let y = body_area.y + 2 + row_idx as u16;
                if y >= body_area.y + body_area.height {
                    break;
                }

                let row_rect = Rect {
                    x: body_area.x,
                    y,
                    width: body_area.width,
                    height: 1,
                };
                if i >= names.len() {
                    let mut spans = Vec::new();
                    let is_selected = i == self.selected_spinner_index;
                    // selector chevron
                    spans.push(Span::styled(
                        if is_selected { "› " } else { "  " }.to_string(),
                        Style::default().fg(if is_selected {
                            theme.keyword
                        } else {
                            theme.text
                        }),
                    ));
                    // label color: dim when not selected; primary + bold when selected
                    let label_style = if is_selected {
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text_dim)
                    };
                    spans.push(Span::styled("Generate your own…", label_style));
                    Paragraph::new(Line::from(spans)).render(row_rect, buf);
                    continue;
                }
                let name = &names[i];
                let is_selected = i == self.selected_spinner_index;
                let def = crate::spinner::find_spinner_by_name(name)
                    .unwrap_or(crate::spinner::current_spinner());
                let frame = crate::spinner::frame_at_time(def, now_ms);

                // Aligned columns (centered block):
                // selector (2) | left_rule | space | spinner (right‑aligned to max) | space | label (padded to max) | space | right_rule
                let border = if is_selected {
                    Style::default().fg(crate::colors::border())
                } else {
                    Style::default()
                        .fg(theme.text_dim)
                        .add_modifier(Modifier::DIM)
                };
                let fg = if is_selected {
                    Style::default().fg(crate::colors::info())
                } else {
                    Style::default()
                        .fg(theme.text_dim)
                        .add_modifier(Modifier::DIM)
                };
                let label = crate::spinner::spinner_label_for(name);

                // Use border-based alignment per spec
                let spinner_len = frame.chars().count() as u16;
                let text_len = (label.chars().count() as u16).saturating_add(3); // label + "..."
                let x: u16 = max_frame_len.saturating_add(8);
                let left_rule = x.saturating_sub(spinner_len);
                let right_rule = x.saturating_sub(text_len);

                let mut spans: Vec<Span> = Vec::new();
                // selector
                spans.push(Span::styled(
                    if is_selected { "› " } else { "  " }.to_string(),
                    Style::default().fg(if is_selected {
                        theme.keyword
                    } else {
                        theme.text
                    }),
                ));
                // left rule
                spans.push(Span::styled("─".repeat(left_rule as usize), border));
                // single space between left border and spinner
                spans.push(Span::raw(" "));
                // spinner
                spans.push(Span::styled(frame, fg));
                spans.push(Span::raw(" "));
                // label with dots
                spans.push(Span::styled(format!("{}... ", label), fg));
                // right rule (match left border logic: x - text_len)
                spans.push(Span::styled("─".repeat(right_rule as usize), border));
                Paragraph::new(Line::from(spans))
                    .alignment(Alignment::Left)
                    .render(row_rect, buf);
            }

            // Animate spinner previews while open
            self.app_event_tx
                .send(AppEvent::ScheduleFrameIn(std::time::Duration::from_millis(
                    100,
                )));

            // Done rendering spinners
            return;
        }

        // No explicit scroll info; list height is fixed to show boundaries naturally

        // Render the body content paragraph inside body area
        let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
        paragraph.render(body_area, buf);
    }
}
