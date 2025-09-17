use std::cell::Cell;
use std::collections::BTreeMap;

use anyhow::Context;
use anyhow::anyhow;
use codex_core::mcp::templates::TemplateCatalog;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Row;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Table;
use ratatui::widgets::TableState;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use unicode_segmentation::UnicodeSegmentation;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::ScrollState;
use crate::mcp::types::AuthDraft;
use crate::mcp::types::HealthDraft;
use crate::mcp::types::McpWizardDraft;
use crate::mcp::types::TemplateSummary;
use crate::mcp::types::template_summaries;

const AUTH_TYPES: &[&str] = &["none", "env", "apikey", "oauth"];
const HEALTH_TYPES: &[&str] = &["none", "stdio", "http"];

pub(crate) struct McpWizardInit {
    pub app_event_tx: AppEventSender,
    pub catalog: TemplateCatalog,
    pub draft: Option<McpWizardDraft>,
    pub existing_name: Option<String>,
}

pub(crate) struct McpWizardView {
    app_event_tx: AppEventSender,
    catalog: TemplateCatalog,
    templates: Vec<TemplateSummary>,
    draft: McpWizardDraft,
    existing_name: Option<String>,
    screen: WizardScreen,
    field_scroll: ScrollState,
    field_visible_rows: Cell<usize>,
    template_scroll: ScrollState,
    variant_scroll: ScrollState,
    text_input: Option<TextInput>,
    variant_options: Vec<String>,
    variant_target: Option<FieldKind>,
    error_message: Option<String>,
    close_requested: bool,
}

impl McpWizardView {
    pub(crate) fn new(init: McpWizardInit) -> Self {
        let templates = template_summaries(&init.catalog);
        let draft = init.draft.unwrap_or_default();
        let needs_template = draft.template_id.is_none() && !templates.is_empty();
        let screen = if needs_template {
            WizardScreen::TemplateSelect
        } else {
            WizardScreen::Form
        };

        let mut field_scroll = ScrollState::new();
        if matches!(screen, WizardScreen::Form) {
            let entries = field_entries(&draft);
            field_scroll.clamp_selection(entries.iter().filter(|e| e.enabled).count());
            ensure_selection(&entries, &mut field_scroll);
        }

        let mut template_scroll = ScrollState::new();
        if !templates.is_empty() {
            template_scroll.clamp_selection(templates.len() + 1);
            if let Some(id) = draft.template_id.as_ref() {
                if let Some(idx) = templates.iter().position(|tpl| tpl.id == *id) {
                    template_scroll.selected_idx = Some(idx);
                } else {
                    template_scroll.selected_idx = Some(templates.len());
                }
            } else {
                template_scroll.selected_idx = Some(templates.len());
            }
        }

        Self {
            app_event_tx: init.app_event_tx,
            catalog: init.catalog,
            templates,
            draft,
            existing_name: init.existing_name,
            screen,
            field_scroll,
            field_visible_rows: Cell::new(1),
            template_scroll,
            variant_scroll: ScrollState::new(),
            text_input: None,
            variant_options: Vec::new(),
            variant_target: None,
            error_message: None,
            close_requested: false,
        }
    }

    fn submit(&mut self) {
        if let Err(err) = self.draft.validate() {
            self.error_message = Some(err.to_string());
            self.screen = WizardScreen::Form;
            return;
        }
        self.app_event_tx.send(AppEvent::ApplyMcpWizard {
            draft: self.draft.clone(),
            existing_name: self.existing_name.clone(),
        });
        self.close_requested = true;
    }

    fn start_text_edit(&mut self, field: FieldKind) {
        let current = field_value(&self.draft, field);
        self.text_input = Some(TextInput::new(current));
        self.screen = WizardScreen::TextEdit(field);
        self.error_message = None;
    }

    fn commit_text_edit(&mut self, field: FieldKind) {
        let input = match self.text_input.take() {
            Some(input) => input.content,
            None => return,
        };
        if let Err(err) = self.apply_text(field, &input) {
            self.error_message = Some(err.to_string());
            self.text_input = Some(TextInput::new(input));
            return;
        }
        self.screen = WizardScreen::Form;
        self.error_message = None;
        ensure_selection(&field_entries(&self.draft), &mut self.field_scroll);
    }

    fn apply_text(&mut self, field: FieldKind, value: &str) -> anyhow::Result<()> {
        match field {
            FieldKind::Name => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(anyhow!("Name cannot be empty"));
                }
                self.draft.name = trimmed.to_string();
            }
            FieldKind::Command => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    return Err(anyhow!("Command cannot be empty"));
                }
                self.draft.command = trimmed.to_string();
            }
            FieldKind::Args => {
                self.draft.args = split_items(value);
            }
            FieldKind::Env => {
                self.draft.env = parse_key_values(value)?;
            }
            FieldKind::Description => {
                let trimmed = value.trim();
                self.draft.description = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
            }
            FieldKind::Tags => {
                self.draft.tags = split_items(value);
            }
            FieldKind::StartupTimeout => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    self.draft.startup_timeout_ms = None;
                } else {
                    let timeout = trimmed.parse::<u64>().context("Expected a number")?;
                    self.draft.startup_timeout_ms = Some(timeout);
                }
            }
            FieldKind::AuthSecret => {
                if let Some(auth) = self.ensure_auth_slot() {
                    let trimmed = value.trim();
                    auth.secret_ref = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    };
                }
            }
            FieldKind::AuthEnv => {
                if let Some(auth) = self.ensure_auth_slot() {
                    auth.env = parse_key_values(value)?;
                }
            }
            FieldKind::HealthCommand => {
                let trimmed = value.trim();
                if let Some(health) = self.ensure_health_slot() {
                    health.command = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    };
                }
            }
            FieldKind::HealthArgs => {
                if let Some(health) = self.ensure_health_slot() {
                    health.args = split_items(value);
                }
            }
            FieldKind::HealthEndpoint => {
                if let Some(health) = self.ensure_health_slot() {
                    let trimmed = value.trim();
                    health.endpoint = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    };
                }
            }
            FieldKind::HealthTimeout => {
                if let Some(health) = self.ensure_health_slot() {
                    let trimmed = value.trim();
                    health.timeout_ms = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.parse::<u64>().context("Expected a number")?)
                    };
                }
            }
            FieldKind::HealthInterval => {
                if let Some(health) = self.ensure_health_slot() {
                    let trimmed = value.trim();
                    health.interval_seconds = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.parse::<u64>().context("Expected a number")?)
                    };
                }
            }
            FieldKind::Summary => {}
            FieldKind::Template => {}
            FieldKind::AuthType | FieldKind::HealthType => {}
        }
        Ok(())
    }

    fn ensure_auth_slot(&mut self) -> Option<&mut AuthDraft> {
        if self.draft.auth.is_none() {
            self.draft.auth = Some(AuthDraft::default());
        }
        self.draft.auth.as_mut()
    }

    fn ensure_health_slot(&mut self) -> Option<&mut HealthDraft> {
        if self.draft.health.is_none() {
            self.draft.health = Some(HealthDraft::default());
        }
        self.draft.health.as_mut()
    }

    fn start_variant_select(&mut self, field: FieldKind, options: Vec<String>) {
        self.variant_options = options;
        self.variant_target = Some(field);
        self.variant_scroll = ScrollState::new();
        let len = self.variant_options.len();
        self.variant_scroll.clamp_selection(len);
        let current = match field {
            FieldKind::AuthType => self
                .draft
                .auth
                .as_ref()
                .and_then(|auth| auth.kind.clone())
                .unwrap_or_else(|| "none".to_string()),
            FieldKind::HealthType => self
                .draft
                .health
                .as_ref()
                .and_then(|health| health.kind.clone())
                .unwrap_or_else(|| "none".to_string()),
            _ => String::new(),
        };
        if let Some(pos) = self.variant_options.iter().position(|opt| opt == &current) {
            self.variant_scroll.selected_idx = Some(pos);
        }
        self.screen = WizardScreen::VariantSelect;
    }

    fn commit_variant_select(&mut self) {
        let field = match self.variant_target.take() {
            Some(field) => field,
            None => {
                self.screen = WizardScreen::Form;
                return;
            }
        };
        let idx = self.variant_scroll.selected_idx.unwrap_or(0);
        if let Some(value) = self.variant_options.get(idx).cloned() {
            match field {
                FieldKind::AuthType => {
                    if value == "none" {
                        self.draft.auth = None;
                    } else if let Some(auth) = self.ensure_auth_slot() {
                        auth.kind = Some(value);
                    } else {
                        self.error_message = Some("Failed to prepare auth settings".to_string());
                    }
                }
                FieldKind::HealthType => {
                    if value == "none" {
                        self.draft.health = None;
                    } else if let Some(health) = self.ensure_health_slot() {
                        health.kind = Some(value.clone());
                        if value == "http" {
                            health.protocol = Some("http".to_string());
                            health.command = None;
                            health.args.clear();
                        } else {
                            health.endpoint = None;
                            health.protocol = None;
                        }
                    } else {
                        self.error_message = Some("Failed to prepare health settings".to_string());
                    }
                }
                FieldKind::Template => {}
                _ => {}
            }
        }
        self.screen = WizardScreen::Form;
        ensure_selection(&field_entries(&self.draft), &mut self.field_scroll);
    }

    fn activate_field(&mut self, field: FieldKind) {
        match field {
            FieldKind::Template => {
                if self.templates.is_empty() {
                    self.error_message = Some("No templates available".to_string());
                    return;
                }
                let total = self.templates.len() + 1;
                self.template_scroll.clamp_selection(total);
                if let Some(id) = self.draft.template_id.as_ref() {
                    if let Some(pos) = self.templates.iter().position(|tpl| tpl.id == *id) {
                        self.template_scroll.selected_idx = Some(pos);
                    } else {
                        self.template_scroll.selected_idx = Some(self.templates.len());
                    }
                } else {
                    self.template_scroll.selected_idx = Some(self.templates.len());
                }
                self.screen = WizardScreen::TemplateSelect;
            }
            FieldKind::AuthType => {
                self.start_variant_select(
                    FieldKind::AuthType,
                    AUTH_TYPES.iter().map(|s| s.to_string()).collect(),
                );
            }
            FieldKind::HealthType => {
                self.start_variant_select(
                    FieldKind::HealthType,
                    HEALTH_TYPES.iter().map(|s| s.to_string()).collect(),
                );
            }
            FieldKind::Summary => {
                self.screen = WizardScreen::Summary;
            }
            _ => self.start_text_edit(field),
        }
    }

    fn apply_template_choice(&mut self) {
        let idx = self.template_scroll.selected_idx.unwrap_or(0);
        if idx >= self.templates.len() {
            self.draft.template_id = None;
            self.screen = WizardScreen::Form;
            return;
        }
        let template_id = self.templates[idx].id.clone();
        if let Some(cfg) = self.catalog.instantiate(&template_id) {
            self.draft.apply_template_config(&cfg);
            self.draft.template_id = Some(template_id);
            if self.draft.command.trim().is_empty() {
                self.draft.command = cfg.command;
            }
            self.screen = WizardScreen::Form;
            ensure_selection(&field_entries(&self.draft), &mut self.field_scroll);
        } else {
            self.error_message = Some(format!("Template '{template_id}' not found"));
            self.screen = WizardScreen::Form;
        }
    }

    fn render_template_select(&self, area: Rect, buf: &mut Buffer) {
        let mut rows: Vec<Row> = self
            .templates
            .iter()
            .map(|tpl| {
                let summary = match (&tpl.summary, &tpl.category) {
                    (Some(summary), Some(category)) if !category.is_empty() => {
                        format!("{summary} ({category})")
                    }
                    (Some(summary), _) => summary.clone(),
                    (None, Some(category)) if !category.is_empty() => {
                        format!("({category})")
                    }
                    _ => String::new(),
                };
                Row::new(vec![tpl.id.clone(), summary])
            })
            .collect();
        rows.push(Row::new(vec![
            "manual".to_string(),
            "Start from scratch".to_string(),
        ]));

        let mut state = TableState::default();
        state.select(self.template_scroll.selected_idx);
        *state.offset_mut() = self.template_scroll.scroll_top;

        let table = Table::new(
            rows,
            [Constraint::Percentage(30), Constraint::Percentage(70)],
        )
        .header(Row::new(vec!["Template".bold(), "Summary".bold()]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Select MCP Template"),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        StatefulWidget::render(table, area, buf, &mut state);

        let footer = Paragraph::new("Enter to select • Esc to cancel".dim())
            .block(Block::default().borders(Borders::NONE));
        Widget::render(
            footer,
            Rect {
                x: area.x,
                y: area.bottom().saturating_sub(1),
                width: area.width,
                height: 1,
            },
            buf,
        );
    }

    fn render_form(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Min(area.height.saturating_sub(5)),
            Constraint::Length(5),
        ])
        .split(area);

        let entries = field_entries(&self.draft);
        let rows: Vec<Row> = entries
            .iter()
            .map(|entry| {
                let style = if !entry.enabled {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    Span::styled(entry.label.to_string(), style),
                    Span::styled(entry.value.clone(), style),
                ])
            })
            .collect();
        let mut state = TableState::default();
        state.select(self.field_scroll.selected_idx);
        *state.offset_mut() = self.field_scroll.scroll_top;

        let table = Table::new(
            rows,
            [Constraint::Percentage(35), Constraint::Percentage(65)],
        )
        .header(Row::new(vec!["Field".bold(), "Value".bold()]))
        .block(Block::default().borders(Borders::ALL).title("MCP Wizard"))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let table_area = chunks[0];
        StatefulWidget::render(table, table_area, buf, &mut state);

        let visible_rows = table_area.height.saturating_sub(3) as usize;
        self.field_visible_rows.set(visible_rows.max(1));

        let mut footer_lines: Vec<Line> = Vec::new();
        if let Some(err) = self.error_message.as_ref() {
            footer_lines.push(err.as_str().red().into());
        }
        footer_lines.push(Line::from(vec![
            "Keys: ".dim(),
            "↑/↓".cyan(),
            " move  ".dim(),
            "Enter".cyan(),
            " edit/select  ".dim(),
            "s".cyan(),
            " summary  ".dim(),
            "Esc".cyan(),
            " close".dim(),
        ]));

        let footer = Paragraph::new(footer_lines)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(Wrap { trim: true });
        Widget::render(footer, chunks[1], buf);
    }

    fn render_variant_select(&self, area: Rect, buf: &mut Buffer) {
        let rows: Vec<Row> = self
            .variant_options
            .iter()
            .map(|opt| Row::new(vec![opt.clone()]))
            .collect();
        let mut state = TableState::default();
        state.select(self.variant_scroll.selected_idx);
        *state.offset_mut() = self.variant_scroll.scroll_top;
        let title = match self.variant_target {
            Some(FieldKind::AuthType) => "Select authentication",
            Some(FieldKind::HealthType) => "Select health check",
            _ => "Select option",
        };
        let table = Table::new(rows, [Constraint::Percentage(100)])
            .block(Block::default().borders(Borders::ALL).title(title))
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        StatefulWidget::render(table, area, buf, &mut state);

        let footer =
            Paragraph::new("Enter to confirm • Esc to cancel".dim()).block(Block::default());
        Widget::render(
            footer,
            Rect {
                x: area.x,
                y: area.bottom().saturating_sub(1),
                width: area.width,
                height: 1,
            },
            buf,
        );
    }

    fn render_text_edit(&self, field: FieldKind, area: Rect, buf: &mut Buffer) {
        let input = self
            .text_input
            .as_ref()
            .map(|i| i.content.clone())
            .unwrap_or_default();
        let mut lines = vec![Line::from(vec![field_label(field).bold()])];
        if let Some(err) = self.error_message.as_ref() {
            lines.push(err.as_str().red().into());
        }
        lines.push("Enter to save • Esc to cancel".dim().into());
        lines.push(Line::from(""));
        lines.push(Line::from(input));
        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Edit"))
            .wrap(Wrap { trim: false });
        Widget::render(paragraph, area, buf);
    }

    fn render_summary(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = self.draft.summary_lines();
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            "Enter".cyan(),
            " apply  ".dim(),
            "b".cyan(),
            " back  ".dim(),
            "Esc".cyan(),
            " cancel".dim(),
        ]));
        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Review"))
            .wrap(Wrap { trim: true });
        Widget::render(paragraph, area, buf);
    }
}

impl BottomPaneView for McpWizardView {
    fn handle_key_event(&mut self, _pane: &mut BottomPane, key_event: KeyEvent) {
        match self.screen {
            WizardScreen::TemplateSelect => match key_event.code {
                KeyCode::Esc => {
                    self.close_requested = true;
                }
                KeyCode::Up => {
                    let total = self.templates.len() + 1;
                    self.template_scroll.move_up_wrap(total);
                    self.template_scroll.ensure_visible(total, 8);
                }
                KeyCode::Down => {
                    let total = self.templates.len() + 1;
                    self.template_scroll.move_down_wrap(total);
                    self.template_scroll.ensure_visible(total, 8);
                }
                KeyCode::Enter => {
                    self.apply_template_choice();
                }
                _ => {}
            },
            WizardScreen::Form => match key_event.code {
                KeyCode::Esc => {
                    self.close_requested = true;
                }
                KeyCode::Char('s') => {
                    self.screen = WizardScreen::Summary;
                }
                KeyCode::Enter => {
                    if let Some(entry) = current_enabled_entry(&self.draft, &self.field_scroll) {
                        self.activate_field(entry.kind);
                    }
                }
                KeyCode::Up => {
                    move_selection(
                        &self.draft,
                        &mut self.field_scroll,
                        Direction::Up,
                        self.field_visible_rows.get(),
                    );
                    self.error_message = None;
                }
                KeyCode::Down => {
                    move_selection(
                        &self.draft,
                        &mut self.field_scroll,
                        Direction::Down,
                        self.field_visible_rows.get(),
                    );
                    self.error_message = None;
                }
                KeyCode::PageUp => {
                    for _ in 0..self.field_visible_rows.get().saturating_sub(1) {
                        move_selection(
                            &self.draft,
                            &mut self.field_scroll,
                            Direction::Up,
                            self.field_visible_rows.get(),
                        );
                    }
                }
                KeyCode::PageDown => {
                    for _ in 0..self.field_visible_rows.get().saturating_sub(1) {
                        move_selection(
                            &self.draft,
                            &mut self.field_scroll,
                            Direction::Down,
                            self.field_visible_rows.get(),
                        );
                    }
                }
                _ => {}
            },
            WizardScreen::VariantSelect => match key_event.code {
                KeyCode::Esc => {
                    self.screen = WizardScreen::Form;
                }
                KeyCode::Up => {
                    let len = self.variant_options.len();
                    self.variant_scroll.move_up_wrap(len);
                    self.variant_scroll.ensure_visible(len, 8);
                }
                KeyCode::Down => {
                    let len = self.variant_options.len();
                    self.variant_scroll.move_down_wrap(len);
                    self.variant_scroll.ensure_visible(len, 8);
                }
                KeyCode::Enter => {
                    self.commit_variant_select();
                }
                _ => {}
            },
            WizardScreen::TextEdit(field) => match key_event.code {
                KeyCode::Esc => {
                    self.text_input = None;
                    self.screen = WizardScreen::Form;
                }
                KeyCode::Enter => {
                    self.commit_text_edit(field);
                }
                _ => {
                    if let Some(input) = self.text_input.as_mut() {
                        input.handle_key(key_event);
                    }
                }
            },
            WizardScreen::Summary => match key_event.code {
                KeyCode::Enter => self.submit(),
                KeyCode::Char('b') => {
                    self.screen = WizardScreen::Form;
                }
                KeyCode::Esc => {
                    self.close_requested = true;
                }
                _ => {}
            },
        }
    }

    fn is_complete(&self) -> bool {
        self.close_requested
    }

    fn on_ctrl_c(&mut self, _pane: &mut BottomPane) -> CancellationEvent {
        self.close_requested = true;
        CancellationEvent::Handled
    }

    fn desired_height(&self, _width: u16) -> u16 {
        18
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        match self.screen {
            WizardScreen::TemplateSelect => self.render_template_select(area, buf),
            WizardScreen::Form => self.render_form(area, buf),
            WizardScreen::VariantSelect => self.render_variant_select(area, buf),
            WizardScreen::TextEdit(field) => self.render_text_edit(field, area, buf),
            WizardScreen::Summary => self.render_summary(area, buf),
        }
    }
}

#[derive(Copy, Clone)]
enum WizardScreen {
    TemplateSelect,
    Form,
    VariantSelect,
    TextEdit(FieldKind),
    Summary,
}

#[derive(Copy, Clone, Debug)]
enum FieldKind {
    Template,
    Name,
    Command,
    Args,
    Env,
    Description,
    Tags,
    StartupTimeout,
    AuthType,
    AuthSecret,
    AuthEnv,
    HealthType,
    HealthCommand,
    HealthArgs,
    HealthEndpoint,
    HealthTimeout,
    HealthInterval,
    Summary,
}

#[derive(Clone)]
struct FieldEntry {
    kind: FieldKind,
    label: &'static str,
    value: String,
    enabled: bool,
}

fn field_entries(draft: &McpWizardDraft) -> Vec<FieldEntry> {
    let mut entries = Vec::new();
    entries.push(FieldEntry {
        kind: FieldKind::Template,
        label: "Template",
        value: draft
            .template_id
            .clone()
            .unwrap_or_else(|| "manual".to_string()),
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::Name,
        label: "Name",
        value: draft.name.clone(),
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::Command,
        label: "Command",
        value: draft.command.clone(),
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::Args,
        label: "Args",
        value: if draft.args.is_empty() {
            "-".to_string()
        } else {
            draft.args.join(" ")
        },
        enabled: true,
    });
    let env_count = draft.env.len();
    entries.push(FieldEntry {
        kind: FieldKind::Env,
        label: "Env",
        value: if env_count == 0 {
            "0 entries".to_string()
        } else {
            format!(
                "{env_count} entr{}",
                if env_count == 1 { "y" } else { "ies" }
            )
        },
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::Description,
        label: "Description",
        value: draft.description.clone().unwrap_or_else(|| "-".to_string()),
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::Tags,
        label: "Tags",
        value: if draft.tags.is_empty() {
            "-".to_string()
        } else {
            draft.tags.join(", ")
        },
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::StartupTimeout,
        label: "Startup timeout (ms)",
        value: draft
            .startup_timeout_ms
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        enabled: true,
    });

    let auth_kind = draft
        .auth
        .as_ref()
        .and_then(|auth| auth.kind.clone())
        .unwrap_or_else(|| "none".to_string());
    let auth_enabled = auth_kind != "none";
    entries.push(FieldEntry {
        kind: FieldKind::AuthType,
        label: "Auth type",
        value: auth_kind,
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::AuthSecret,
        label: "Auth secret",
        value: draft
            .auth
            .as_ref()
            .and_then(|auth| auth.secret_ref.clone())
            .unwrap_or_else(|| "-".to_string()),
        enabled: auth_enabled,
    });
    entries.push(FieldEntry {
        kind: FieldKind::AuthEnv,
        label: "Auth env",
        value: draft
            .auth
            .as_ref()
            .map(|auth| {
                let count = auth.env.len();
                if count == 0 {
                    "0 entries".to_string()
                } else {
                    format!("{count} entr{}", if count == 1 { "y" } else { "ies" })
                }
            })
            .unwrap_or_else(|| "0 entries".to_string()),
        enabled: auth_enabled,
    });

    let health_kind = draft
        .health
        .as_ref()
        .and_then(|health| health.kind.clone())
        .unwrap_or_else(|| "none".to_string());
    let health_stdio = health_kind == "stdio";
    let health_http = health_kind == "http";
    let health_enabled = health_kind != "none";

    entries.push(FieldEntry {
        kind: FieldKind::HealthType,
        label: "Health type",
        value: health_kind,
        enabled: true,
    });
    entries.push(FieldEntry {
        kind: FieldKind::HealthCommand,
        label: "Health command",
        value: draft
            .health
            .as_ref()
            .and_then(|health| health.command.clone())
            .unwrap_or_else(|| "-".to_string()),
        enabled: health_stdio,
    });
    entries.push(FieldEntry {
        kind: FieldKind::HealthArgs,
        label: "Health args",
        value: draft
            .health
            .as_ref()
            .map(|health| {
                if health.args.is_empty() {
                    "-".to_string()
                } else {
                    health.args.join(" ")
                }
            })
            .unwrap_or_else(|| "-".to_string()),
        enabled: health_stdio,
    });
    entries.push(FieldEntry {
        kind: FieldKind::HealthEndpoint,
        label: "Health endpoint",
        value: draft
            .health
            .as_ref()
            .and_then(|health| health.endpoint.clone())
            .unwrap_or_else(|| "-".to_string()),
        enabled: health_http,
    });
    entries.push(FieldEntry {
        kind: FieldKind::HealthTimeout,
        label: "Health timeout (ms)",
        value: draft
            .health
            .as_ref()
            .and_then(|health| health.timeout_ms)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        enabled: health_enabled,
    });
    entries.push(FieldEntry {
        kind: FieldKind::HealthInterval,
        label: "Health interval (s)",
        value: draft
            .health
            .as_ref()
            .and_then(|health| health.interval_seconds)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        enabled: health_enabled,
    });
    entries.push(FieldEntry {
        kind: FieldKind::Summary,
        label: "Review & apply",
        value: "→".to_string(),
        enabled: true,
    });
    entries
}

fn ensure_selection(entries: &[FieldEntry], scroll: &mut ScrollState) {
    if entries.is_empty() {
        scroll.selected_idx = None;
        scroll.scroll_top = 0;
        return;
    }
    let enabled_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.enabled)
        .map(|(idx, _)| idx)
        .collect();
    if enabled_indices.is_empty() {
        scroll.selected_idx = None;
        scroll.scroll_top = 0;
        return;
    }
    let current = scroll.selected_idx.unwrap_or(0);
    if !enabled_indices.contains(&current) {
        scroll.selected_idx = Some(enabled_indices[0]);
        scroll.scroll_top = 0;
    }
}

fn current_enabled_entry(draft: &McpWizardDraft, scroll: &ScrollState) -> Option<FieldEntry> {
    let entries = field_entries(draft);
    scroll
        .selected_idx
        .and_then(|idx| entries.get(idx).cloned())
        .filter(|entry| entry.enabled)
}

#[derive(Copy, Clone)]
enum Direction {
    Up,
    Down,
}

fn move_selection(
    draft: &McpWizardDraft,
    scroll: &mut ScrollState,
    direction: Direction,
    visible_rows: usize,
) {
    let entries = field_entries(draft);
    if entries.is_empty() {
        scroll.selected_idx = None;
        return;
    }
    let mut idx = scroll.selected_idx.unwrap_or(0);
    let len = entries.len();
    loop {
        idx = match direction {
            Direction::Up => idx.checked_sub(1).unwrap_or(len - 1),
            Direction::Down => {
                if idx + 1 >= len {
                    0
                } else {
                    idx + 1
                }
            }
        };
        if entries[idx].enabled {
            scroll.selected_idx = Some(idx);
            break;
        }
        if idx == scroll.selected_idx.unwrap_or(0) {
            break;
        }
    }
    scroll.ensure_visible(len, visible_rows.max(1));
}

fn field_label(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::Template => "Template",
        FieldKind::Name => "Name",
        FieldKind::Command => "Command",
        FieldKind::Args => "Args",
        FieldKind::Env => "Env",
        FieldKind::Description => "Description",
        FieldKind::Tags => "Tags",
        FieldKind::StartupTimeout => "Startup timeout (ms)",
        FieldKind::AuthType => "Auth type",
        FieldKind::AuthSecret => "Auth secret",
        FieldKind::AuthEnv => "Auth env",
        FieldKind::HealthType => "Health type",
        FieldKind::HealthCommand => "Health command",
        FieldKind::HealthArgs => "Health args",
        FieldKind::HealthEndpoint => "Health endpoint",
        FieldKind::HealthTimeout => "Health timeout (ms)",
        FieldKind::HealthInterval => "Health interval (s)",
        FieldKind::Summary => "Review & apply",
    }
}

fn field_value(draft: &McpWizardDraft, field: FieldKind) -> String {
    match field {
        FieldKind::Template => draft
            .template_id
            .clone()
            .unwrap_or_else(|| "manual".to_string()),
        FieldKind::Name => draft.name.clone(),
        FieldKind::Command => draft.command.clone(),
        FieldKind::Args => draft.args.join(" "),
        FieldKind::Env => format_key_values(&draft.env),
        FieldKind::Description => draft.description.clone().unwrap_or_default(),
        FieldKind::Tags => draft.tags.join(", "),
        FieldKind::StartupTimeout => draft
            .startup_timeout_ms
            .map(|v| v.to_string())
            .unwrap_or_default(),
        FieldKind::AuthType => draft
            .auth
            .as_ref()
            .and_then(|auth| auth.kind.clone())
            .unwrap_or_else(|| "none".to_string()),
        FieldKind::AuthSecret => draft
            .auth
            .as_ref()
            .and_then(|auth| auth.secret_ref.clone())
            .unwrap_or_default(),
        FieldKind::AuthEnv => draft
            .auth
            .as_ref()
            .map(|auth| format_key_values(&auth.env))
            .unwrap_or_default(),
        FieldKind::HealthType => draft
            .health
            .as_ref()
            .and_then(|health| health.kind.clone())
            .unwrap_or_else(|| "none".to_string()),
        FieldKind::HealthCommand => draft
            .health
            .as_ref()
            .and_then(|health| health.command.clone())
            .unwrap_or_default(),
        FieldKind::HealthArgs => draft
            .health
            .as_ref()
            .map(|health| health.args.join(" "))
            .unwrap_or_default(),
        FieldKind::HealthEndpoint => draft
            .health
            .as_ref()
            .and_then(|health| health.endpoint.clone())
            .unwrap_or_default(),
        FieldKind::HealthTimeout => draft
            .health
            .as_ref()
            .and_then(|health| health.timeout_ms)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        FieldKind::HealthInterval => draft
            .health
            .as_ref()
            .and_then(|health| health.interval_seconds)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        FieldKind::Summary => String::new(),
    }
}

fn split_items(input: &str) -> Vec<String> {
    input
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_key_values(input: &str) -> anyhow::Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for raw in input
        .split(['\n', ','])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let mut parts = raw.splitn(2, '=');
        let key = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Entries must be KEY=VALUE"))?;
        let value = parts
            .next()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Entries must be KEY=VALUE"))?;
        map.insert(key.to_string(), value);
    }
    Ok(map)
}

fn format_key_values(map: &BTreeMap<String, String>) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

struct TextInput {
    content: String,
    cursor: usize,
}

impl TextInput {
    fn new(initial: String) -> Self {
        Self {
            cursor: initial.len(),
            content: initial,
        }
    }

    fn handle_key(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.content.len(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Char(c) if !event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_char(c)
            }
            _ => {}
        }
    }

    fn insert_char(&mut self, ch: char) {
        let mut buf = [0; 4];
        let s = ch.encode_utf8(&mut buf);
        self.content.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = prev_grapheme_boundary(&self.content, self.cursor);
        self.content.drain(prev..self.cursor);
        self.cursor = prev;
    }

    fn delete(&mut self) {
        if self.cursor >= self.content.len() {
            return;
        }
        let next = next_grapheme_boundary(&self.content, self.cursor);
        self.content.drain(self.cursor..next);
    }

    fn move_left(&mut self) {
        self.cursor = prev_grapheme_boundary(&self.content, self.cursor);
    }

    fn move_right(&mut self) {
        self.cursor = next_grapheme_boundary(&self.content, self.cursor);
    }
}

fn prev_grapheme_boundary(s: &str, idx: usize) -> usize {
    let mut boundary = 0;
    for (i, _) in s[..idx].grapheme_indices(true) {
        boundary = i;
    }
    boundary
}

fn next_grapheme_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    if let Some((i, _)) = s[idx..].grapheme_indices(true).next() {
        return idx + i;
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::collections::BTreeMap;
    use tokio::sync::mpsc::unbounded_channel;

    fn render(view: &McpWizardView) -> String {
        let width = 78;
        let height = BottomPaneView::desired_height(view, width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        BottomPaneView::render(view, area, &mut buf);

        (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn make_sender() -> AppEventSender {
        let (tx, _rx) = unbounded_channel();
        AppEventSender::new(tx)
    }

    fn sample_draft() -> McpWizardDraft {
        let mut env = BTreeMap::new();
        env.insert("API_KEY".to_string(), "${SECRET}".to_string());

        let mut auth_env = BTreeMap::new();
        auth_env.insert("TOKEN".to_string(), "${TOKEN}".to_string());

        McpWizardDraft {
            name: "anthropic-mcp".to_string(),
            template_id: Some("anthropic/cli".to_string()),
            command: "anthropic-mcp".to_string(),
            args: vec![
                "--serve".to_string(),
                "--port".to_string(),
                "8080".to_string(),
            ],
            env,
            startup_timeout_ms: Some(15_000),
            description: Some("Anthropic MCP bridge".to_string()),
            tags: vec!["anthropic".to_string(), "beta".to_string()],
            auth: Some(AuthDraft {
                kind: Some("env".to_string()),
                secret_ref: Some("vault:anthropic".to_string()),
                env: auth_env,
            }),
            health: Some(HealthDraft {
                kind: Some("http".to_string()),
                command: None,
                args: vec![],
                timeout_ms: Some(5_000),
                interval_seconds: Some(60),
                endpoint: Some("http://127.0.0.1:9000/health".to_string()),
                protocol: Some("http/1.1".to_string()),
            }),
        }
    }

    fn empty_view() -> McpWizardView {
        McpWizardView::new(McpWizardInit {
            app_event_tx: make_sender(),
            catalog: TemplateCatalog::empty(),
            draft: None,
            existing_name: None,
        })
    }

    #[test]
    fn template_select_screen() {
        let mut view = empty_view();
        view.templates = vec![
            TemplateSummary {
                id: "anthropic/cli".to_string(),
                summary: Some("Anthropic CLI bridge".to_string()),
                category: Some("llm".to_string()),
            },
            TemplateSummary {
                id: "openai/web".to_string(),
                summary: Some("OpenAI Browser agent".to_string()),
                category: Some("llm".to_string()),
            },
        ];
        view.template_scroll
            .clamp_selection(view.templates.len() + 1);
        view.template_scroll.selected_idx = Some(0);
        view.screen = WizardScreen::TemplateSelect;

        assert_snapshot!("mcp_wizard_template_select", render(&view));
    }

    #[test]
    fn form_screen_with_error() {
        let mut view = McpWizardView::new(McpWizardInit {
            app_event_tx: make_sender(),
            catalog: TemplateCatalog::empty(),
            draft: Some(sample_draft()),
            existing_name: None,
        });
        view.screen = WizardScreen::Form;
        view.error_message = Some("Command must not be empty".to_string());
        ensure_selection(&field_entries(&view.draft), &mut view.field_scroll);
        view.field_scroll.selected_idx = Some(2);

        assert_snapshot!("mcp_wizard_form_error", render(&view));
    }

    #[test]
    fn variant_select_screen() {
        let mut view = McpWizardView::new(McpWizardInit {
            app_event_tx: make_sender(),
            catalog: TemplateCatalog::empty(),
            draft: Some(sample_draft()),
            existing_name: None,
        });
        view.screen = WizardScreen::VariantSelect;
        view.variant_target = Some(FieldKind::AuthType);
        view.variant_options = vec!["none".to_string(), "env".to_string(), "oauth".to_string()];
        view.variant_scroll
            .clamp_selection(view.variant_options.len());
        view.variant_scroll.selected_idx = Some(1);

        assert_snapshot!("mcp_wizard_variant_select", render(&view));
    }

    #[test]
    fn text_edit_screen() {
        let mut view = McpWizardView::new(McpWizardInit {
            app_event_tx: make_sender(),
            catalog: TemplateCatalog::empty(),
            draft: Some(sample_draft()),
            existing_name: None,
        });
        view.screen = WizardScreen::TextEdit(FieldKind::Env);
        view.error_message = Some("Expected KEY=VALUE pairs".to_string());
        view.text_input = Some(TextInput::new("API_KEY=\nORG_ID=1234".to_string()));

        assert_snapshot!("mcp_wizard_text_edit", render(&view));
    }

    #[test]
    fn summary_screen() {
        let mut view = McpWizardView::new(McpWizardInit {
            app_event_tx: make_sender(),
            catalog: TemplateCatalog::empty(),
            draft: Some(sample_draft()),
            existing_name: Some("existing".to_string()),
        });
        view.screen = WizardScreen::Summary;

        assert_snapshot!("mcp_wizard_summary", render(&view));
    }
}
