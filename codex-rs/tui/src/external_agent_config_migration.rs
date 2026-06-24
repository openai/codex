use crate::diff_render::display_path_for;
use crate::external_agent_config_migration_model::ExternalAgentConfigMigrationGroupModel;
use crate::external_agent_config_migration_model::external_agent_config_migration_count_summary;
use crate::external_agent_config_migration_model::external_agent_config_migration_groups;
use crate::external_agent_config_migration_model::external_agent_config_migration_item_detail;
use crate::external_agent_config_migration_model::external_agent_config_migration_item_label;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::PluginsMigration;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::prelude::Stylize as _;
use ratatui::text::Line;
use tokio_stream::StreamExt;

mod render;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExternalAgentConfigMigrationOutcome {
    Proceed(Vec<ExternalAgentConfigMigrationItem>),
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusArea {
    Items,
    Actions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionMenuOption {
    Proceed,
    Customize,
    Skip,
    Back,
}

impl ActionMenuOption {
    fn label(self) -> &'static str {
        match self {
            Self::Proceed => "Import selected",
            Self::Customize => "Customize selection",
            Self::Skip => "Cancel",
            Self::Back => "Review selection",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MigrationView {
    Summary,
    Customize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MigrationSelection {
    item: ExternalAgentConfigMigrationItem,
    enabled: bool,
}

struct RenderLineEntry {
    item_idx: Option<usize>,
    kind: RenderLineKind,
    line: Line<'static>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderLineKind {
    Section,
    Item,
    ItemDetail,
}

pub(crate) async fn run_external_agent_config_migration_prompt(
    tui: &mut Tui,
    items: &[ExternalAgentConfigMigrationItem],
    selected_items: &[ExternalAgentConfigMigrationItem],
    error: Option<&str>,
) -> ExternalAgentConfigMigrationOutcome {
    let mut screen = ExternalAgentConfigMigrationScreen::new(
        tui.frame_requester(),
        items,
        selected_items,
        error.map(str::to_owned),
    );

    let _ = tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    });

    let events = tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw | TuiEvent::Resize => {
                    let _ = tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    });
                }
            }
        } else {
            screen.skip();
            break;
        }
    }

    screen.outcome()
}

struct ExternalAgentConfigMigrationScreen {
    request_frame: FrameRequester,
    items: Vec<MigrationSelection>,
    groups: Vec<ExternalAgentConfigMigrationGroupModel>,
    view: MigrationView,
    selected_item_idx: Option<usize>,
    scroll_top: usize,
    focus: FocusArea,
    highlighted_action: ActionMenuOption,
    done: bool,
    outcome: ExternalAgentConfigMigrationOutcome,
    error: Option<String>,
}

impl ExternalAgentConfigMigrationScreen {
    fn proceed_enabled(&self) -> bool {
        self.selected_count() > 0
    }

    fn first_available_action(&self) -> ActionMenuOption {
        match self.available_actions().first() {
            Some(action) => *action,
            None => ActionMenuOption::Back,
        }
    }

    fn last_available_action(&self) -> ActionMenuOption {
        match self.available_actions().last() {
            Some(action) => *action,
            None => ActionMenuOption::Back,
        }
    }

    fn previous_available_action(&self, action: ActionMenuOption) -> Option<ActionMenuOption> {
        let actions = self.available_actions();
        actions
            .iter()
            .position(|candidate| *candidate == action)
            .and_then(|idx| idx.checked_sub(1))
            .and_then(|idx| actions.get(idx))
            .copied()
    }

    fn next_available_action(&self, action: ActionMenuOption) -> Option<ActionMenuOption> {
        let actions = self.available_actions();
        actions
            .iter()
            .position(|candidate| *candidate == action)
            .and_then(|idx| actions.get(idx + 1))
            .copied()
    }

    fn available_actions(&self) -> Vec<ActionMenuOption> {
        match self.view {
            MigrationView::Summary => {
                let mut actions = Vec::new();
                if self.proceed_enabled() {
                    actions.push(ActionMenuOption::Proceed);
                }
                actions.extend([ActionMenuOption::Customize, ActionMenuOption::Skip]);
                actions
            }
            MigrationView::Customize => vec![ActionMenuOption::Back],
        }
    }

    fn normalize_highlighted_action(&mut self) {
        if !self.available_actions().contains(&self.highlighted_action) {
            self.highlighted_action = self.first_available_action();
        }
    }

    fn display_description(item: &ExternalAgentConfigMigrationItem) -> String {
        // App-server descriptions use migration vocabulary. Normalize that prefix so the TUI
        // consistently uses the user-facing import vocabulary.
        let description = item
            .description
            .strip_prefix("Migrate ")
            .map_or_else(|| item.description.clone(), |rest| format!("Import {rest}"));
        let Some(cwd) = item.cwd.as_deref() else {
            return description;
        };

        fn reformat_description(
            description: &str,
            prefix: &str,
            separator: &str,
            cwd: &std::path::Path,
        ) -> Option<String> {
            let remainder = description.strip_prefix(prefix)?;
            let (left, right) = remainder.split_once(separator)?;
            Some(format!(
                "{prefix}{}{}{}",
                display_path_for(std::path::Path::new(left), cwd),
                separator,
                display_path_for(std::path::Path::new(right), cwd)
            ))
        }

        if let Some(reformatted) = reformat_description(&description, "Import ", " into ", cwd) {
            return reformatted;
        }

        if let Some(reformatted) =
            reformat_description(&description, "Import skills from ", " to ", cwd)
        {
            return reformatted;
        }

        if let Some(reformatted) = reformat_description(&description, "Import ", " to ", cwd) {
            return reformatted;
        }

        if let Some(source) = description.strip_prefix("Import enabled plugins from ") {
            let description = format!(
                "Import enabled plugins from {}",
                display_path_for(std::path::Path::new(source), cwd)
            );
            if let Some(details) = &item.details {
                let marketplace_count = details.plugins.len();
                let plugin_count = details
                    .plugins
                    .iter()
                    .map(|plugin_group| plugin_group.plugin_names.len())
                    .sum::<usize>();
                return format!(
                    "{description} ({marketplace_count} {}, {plugin_count} {})",
                    if marketplace_count == 1 {
                        "marketplace"
                    } else {
                        "marketplaces"
                    },
                    if plugin_count == 1 {
                        "plugin"
                    } else {
                        "plugins"
                    }
                );
            }
            return description;
        }

        description
    }

    fn new(
        request_frame: FrameRequester,
        items: &[ExternalAgentConfigMigrationItem],
        selected_items: &[ExternalAgentConfigMigrationItem],
        error: Option<String>,
    ) -> Self {
        let groups = external_agent_config_migration_groups(items);
        let items = items
            .iter()
            .cloned()
            .map(|item| MigrationSelection {
                enabled: selected_items.contains(&item),
                item,
            })
            .collect::<Vec<_>>();
        let selected_item_idx = (!groups.is_empty()).then_some(0);
        let mut screen = Self {
            request_frame,
            items,
            groups,
            view: MigrationView::Summary,
            selected_item_idx,
            scroll_top: 0,
            focus: FocusArea::Actions,
            highlighted_action: ActionMenuOption::Proceed,
            done: false,
            outcome: ExternalAgentConfigMigrationOutcome::Skip,
            error,
        };
        screen.normalize_highlighted_action();
        screen
    }

    fn plugin_detail_lines(plugin_groups: &[PluginsMigration]) -> Vec<Line<'static>> {
        let mut lines = plugin_groups
            .iter()
            .take(3)
            .map(|plugin_group| {
                let mut plugin_names = plugin_group
                    .plugin_names
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>();
                let hidden_plugin_count = plugin_group
                    .plugin_names
                    .len()
                    .saturating_sub(plugin_names.len());
                if hidden_plugin_count > 0 {
                    plugin_names.push(format!("+{hidden_plugin_count} more"));
                }
                Line::from(format!(
                    "      • {}: {}",
                    plugin_group.marketplace_name,
                    plugin_names.join(", ")
                ))
            })
            .collect::<Vec<_>>();
        let hidden_marketplace_count = plugin_groups.len().saturating_sub(lines.len());
        if hidden_marketplace_count > 0 {
            lines.push(Line::from(format!(
                "      • +{hidden_marketplace_count} more marketplaces"
            )));
        }
        lines
    }

    fn is_done(&self) -> bool {
        self.done
    }

    fn outcome(&self) -> ExternalAgentConfigMigrationOutcome {
        self.outcome.clone()
    }

    fn finish_with(&mut self, outcome: ExternalAgentConfigMigrationOutcome) {
        self.outcome = outcome;
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn proceed(&mut self) {
        let selected = self.selected_items();
        self.finish_with(ExternalAgentConfigMigrationOutcome::Proceed(selected));
    }

    fn skip(&mut self) {
        self.finish_with(ExternalAgentConfigMigrationOutcome::Skip);
    }

    fn selected_items(&self) -> Vec<ExternalAgentConfigMigrationItem> {
        self.items
            .iter()
            .filter(|item| item.enabled)
            .map(|item| item.item.clone())
            .collect()
    }

    fn selected_count(&self) -> usize {
        self.items.iter().filter(|item| item.enabled).count()
    }

    fn group_selection_marker(
        &self,
        group: &ExternalAgentConfigMigrationGroupModel,
    ) -> &'static str {
        let enabled_count = group
            .item_indices
            .iter()
            .filter(|idx| self.items[**idx].enabled)
            .count();
        match enabled_count {
            0 => " ",
            count if count == group.item_indices.len() => "x",
            _ => "-",
        }
    }

    fn set_all_enabled(&mut self, enabled: bool) {
        for item in &mut self.items {
            item.enabled = enabled;
        }
        self.error = None;
        self.normalize_highlighted_action();
        self.request_frame.schedule_frame();
    }

    fn toggle_selected_item(&mut self) {
        if self.view != MigrationView::Customize || self.focus != FocusArea::Items {
            return;
        }
        let Some(selected_idx) = self.selected_item_idx else {
            return;
        };
        let Some(item) = self.items.get_mut(selected_idx) else {
            return;
        };
        item.enabled = !item.enabled;
        self.error = None;
        self.normalize_highlighted_action();
        self.request_frame.schedule_frame();
    }

    fn customize(&mut self) {
        self.view = MigrationView::Customize;
        self.selected_item_idx = (!self.items.is_empty()).then_some(0);
        self.scroll_top = 0;
        self.focus = FocusArea::Items;
        self.highlighted_action = ActionMenuOption::Back;
        self.request_frame.schedule_frame();
    }

    fn back_to_summary(&mut self) {
        self.view = MigrationView::Summary;
        self.selected_item_idx = (!self.groups.is_empty()).then_some(0);
        self.scroll_top = 0;
        self.focus = FocusArea::Actions;
        self.highlighted_action = self.first_available_action();
        self.request_frame.schedule_frame();
    }

    fn move_up(&mut self) {
        if self.view == MigrationView::Summary {
            self.focus = FocusArea::Actions;
            self.highlighted_action = self
                .previous_available_action(self.highlighted_action)
                .unwrap_or_else(|| self.last_available_action());
            self.request_frame.schedule_frame();
            return;
        }
        match self.focus {
            FocusArea::Items => match self.selected_item_idx {
                Some(0) => {
                    self.focus = FocusArea::Actions;
                    self.highlighted_action = self.last_available_action();
                }
                Some(idx) => {
                    self.selected_item_idx = Some(idx.saturating_sub(1));
                }
                None => {
                    self.focus = FocusArea::Actions;
                    self.highlighted_action = self.last_available_action();
                }
            },
            FocusArea::Actions => {
                if let Some(previous) = self.previous_available_action(self.highlighted_action) {
                    self.highlighted_action = previous;
                } else {
                    self.focus = FocusArea::Items;
                    if !self.items.is_empty() {
                        self.selected_item_idx = Some(self.items.len() - 1);
                    }
                }
            }
        }
        self.ensure_selected_item_visible();
        self.request_frame.schedule_frame();
    }

    fn move_down(&mut self) {
        if self.view == MigrationView::Summary {
            self.focus = FocusArea::Actions;
            self.highlighted_action = self
                .next_available_action(self.highlighted_action)
                .unwrap_or_else(|| self.first_available_action());
            self.request_frame.schedule_frame();
            return;
        }
        match self.focus {
            FocusArea::Items => match self.selected_item_idx {
                Some(idx) if idx + 1 < self.items.len() => {
                    self.selected_item_idx = Some(idx + 1);
                }
                _ => {
                    self.focus = FocusArea::Actions;
                    self.highlighted_action = self.first_available_action();
                }
            },
            FocusArea::Actions => {
                if let Some(next) = self.next_available_action(self.highlighted_action) {
                    self.highlighted_action = next;
                } else {
                    self.focus = FocusArea::Items;
                    if !self.items.is_empty() {
                        self.selected_item_idx = Some(0);
                    }
                }
            }
        }
        self.ensure_selected_item_visible();
        self.request_frame.schedule_frame();
    }

    fn confirm_selection(&mut self) {
        match self.focus {
            FocusArea::Items => self.toggle_selected_item(),
            FocusArea::Actions => match self.highlighted_action {
                ActionMenuOption::Proceed => self.proceed(),
                ActionMenuOption::Customize => self.customize(),
                ActionMenuOption::Skip => self.skip(),
                ActionMenuOption::Back => self.back_to_summary(),
            },
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        if is_ctrl_exit_combo(key_event) {
            self.skip();
            return;
        }

        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Char(number @ '1'..='9') => self.select_numbered_action(number),
            KeyCode::Char('c') if self.view == MigrationView::Summary => self.customize(),
            KeyCode::Char('b') if self.view == MigrationView::Customize => self.back_to_summary(),
            KeyCode::Char(' ') if self.view == MigrationView::Customize => {
                self.toggle_selected_item();
            }
            KeyCode::Char('a') if self.view == MigrationView::Customize => {
                self.set_all_enabled(/*enabled*/ true);
            }
            KeyCode::Char('n') if self.view == MigrationView::Customize => {
                self.set_all_enabled(/*enabled*/ false);
            }
            KeyCode::Enter => self.confirm_selection(),
            KeyCode::Esc => match self.view {
                MigrationView::Summary => self.skip(),
                MigrationView::Customize => self.back_to_summary(),
            },
            _ => {}
        }
    }

    fn select_numbered_action(&mut self, number: char) {
        let Some(index) = number.to_digit(10).and_then(|number| number.checked_sub(1)) else {
            return;
        };
        let Some(action) = self.available_actions().get(index as usize).copied() else {
            return;
        };
        self.focus = FocusArea::Actions;
        self.highlighted_action = action;
        self.confirm_selection();
    }

    fn ensure_selected_item_visible(&mut self) {
        let Some(selected_idx) = self.selected_item_idx else {
            self.scroll_top = 0;
            return;
        };
        let selected_render_idx = self.selected_render_line_index(selected_idx);
        let visible_rows = self.render_line_count().max(1);
        if selected_render_idx < self.scroll_top {
            self.scroll_top = selected_render_idx;
        } else {
            let bottom = self.scroll_top + visible_rows.saturating_sub(1);
            if selected_render_idx > bottom {
                self.scroll_top = selected_render_idx + 1 - visible_rows;
            }
        }
    }

    fn render_line_count(&self) -> usize {
        self.build_render_lines().len()
    }

    fn selected_render_line_index(&self, selected_item_idx: usize) -> usize {
        self.build_render_lines()
            .iter()
            .position(|entry| entry.item_idx == Some(selected_item_idx))
            .unwrap_or(selected_item_idx)
    }

    fn section_title(cwd: Option<&std::path::Path>) -> Line<'static> {
        match cwd {
            Some(cwd) => Line::from(vec![
                "Current project: ".bold(),
                cwd.display().to_string().dim(),
            ]),
            None => Line::from("Home".bold()),
        }
    }

    fn build_render_lines(&self) -> Vec<RenderLineEntry> {
        match self.view {
            MigrationView::Summary => self.build_summary_render_lines(),
            MigrationView::Customize => self.build_customize_render_lines(),
        }
    }

    fn build_summary_render_lines(&self) -> Vec<RenderLineEntry> {
        self.groups
            .iter()
            .enumerate()
            .flat_map(|(idx, group)| {
                let selected_items = group
                    .item_indices
                    .iter()
                    .filter_map(|idx| self.items.get(*idx))
                    .filter(|item| item.enabled)
                    .map(|item| &item.item);
                let count_summary = external_agent_config_migration_count_summary(selected_items);
                [
                    RenderLineEntry {
                        item_idx: Some(idx),
                        kind: RenderLineKind::Item,
                        line: Line::from(format!(
                            "  [{}] {}",
                            self.group_selection_marker(group),
                            group.label
                        )),
                    },
                    RenderLineEntry {
                        item_idx: None,
                        kind: RenderLineKind::ItemDetail,
                        line: Line::from(format!("      {}", group.description)),
                    },
                    RenderLineEntry {
                        item_idx: None,
                        kind: RenderLineKind::ItemDetail,
                        line: Line::from(if count_summary.is_empty() {
                            "      Importing: none".to_string()
                        } else {
                            format!("      Importing: {count_summary}")
                        }),
                    },
                ]
            })
            .collect()
    }

    fn build_customize_render_lines(&self) -> Vec<RenderLineEntry> {
        let mut lines = Vec::new();
        let mut current_scope: Option<Option<&std::path::Path>> = None;
        for (idx, item) in self.items.iter().enumerate() {
            let scope = item.item.cwd.as_deref();
            if current_scope != Some(scope) {
                if current_scope.is_some() {
                    lines.push(RenderLineEntry {
                        item_idx: None,
                        kind: RenderLineKind::Section,
                        line: Line::from(""),
                    });
                }
                lines.push(RenderLineEntry {
                    item_idx: None,
                    kind: RenderLineKind::Section,
                    line: Self::section_title(scope),
                });
                current_scope = Some(scope);
            }
            lines.push(RenderLineEntry {
                item_idx: Some(idx),
                kind: RenderLineKind::Item,
                line: Line::from(vec![
                    "  ".into(),
                    format!(
                        "[{}] {}",
                        if item.enabled { "x" } else { " " },
                        external_agent_config_migration_item_label(&item.item)
                    )
                    .into(),
                ]),
            });
            lines.push(RenderLineEntry {
                item_idx: None,
                kind: RenderLineKind::ItemDetail,
                line: Line::from(format!("      {}", Self::display_description(&item.item))),
            });
            if let Some(details) = external_agent_config_migration_item_detail(&item.item) {
                lines.push(RenderLineEntry {
                    item_idx: None,
                    kind: RenderLineKind::ItemDetail,
                    line: Line::from(format!("      {details}")),
                });
            }
            if let Some(details) = &item.item.details {
                for line in Self::plugin_detail_lines(&details.plugins) {
                    lines.push(RenderLineEntry {
                        item_idx: None,
                        kind: RenderLineKind::ItemDetail,
                        line,
                    });
                }
            }
        }
        lines
    }
}

fn is_ctrl_exit_combo(key_event: KeyEvent) -> bool {
    matches!(key_event.code, KeyCode::Char('c' | 'd'))
        && key_event.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::ExternalAgentConfigMigrationOutcome;
    use super::ExternalAgentConfigMigrationScreen;
    use super::MigrationView;
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;
    use crate::tui::FrameRequester;
    use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
    use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
    use codex_app_server_protocol::PluginsMigration;
    use codex_app_server_protocol::SessionMigration;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use ratatui::layout::Rect;
    use std::path::PathBuf;

    fn sample_plugin_details() -> codex_app_server_protocol::MigrationDetails {
        codex_app_server_protocol::MigrationDetails {
            plugins: vec![
                PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec![
                        "deployer".to_string(),
                        "formatter".to_string(),
                        "lint".to_string(),
                    ],
                },
                PluginsMigration {
                    marketplace_name: "team-marketplace".to_string(),
                    plugin_names: vec!["asana".to_string()],
                },
                PluginsMigration {
                    marketplace_name: "debug".to_string(),
                    plugin_names: vec!["sample".to_string()],
                },
                PluginsMigration {
                    marketplace_name: "data-tools".to_string(),
                    plugin_names: vec!["warehouse".to_string()],
                },
            ],
            ..Default::default()
        }
    }

    #[cfg(windows)]
    fn sample_project_root() -> PathBuf {
        PathBuf::from(r"C:\workspace\project")
    }

    #[cfg(not(windows))]
    fn sample_project_root() -> PathBuf {
        PathBuf::from("/workspace/project")
    }

    fn sample_project_path(path: &str) -> String {
        sample_project_root().join(path).display().to_string()
    }

    fn sample_items() -> Vec<ExternalAgentConfigMigrationItem> {
        let project_root = sample_project_root();
        vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description:
                    "Migrate /Users/alex/.claude/settings.json into /Users/alex/.codex/config.toml"
                        .to_string(),
                cwd: None,
                details: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Sessions,
                description: "Migrate recent Claude Code sessions".to_string(),
                cwd: None,
                details: Some(codex_app_server_protocol::MigrationDetails {
                    sessions: vec![SessionMigration {
                        path: PathBuf::from("/Users/alex/.claude/projects/project/session.jsonl"),
                        cwd: project_root.clone(),
                        title: Some("Investigate migration UX".to_string()),
                    }],
                    ..Default::default()
                }),
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Plugins,
                description: format!(
                    "Migrate enabled plugins from {}",
                    sample_project_path(".claude/settings.json")
                ),
                cwd: Some(project_root.clone()),
                details: Some(sample_plugin_details()),
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: format!(
                    "Migrate {} to {}",
                    sample_project_path("CLAUDE.md"),
                    sample_project_path("AGENTS.md")
                ),
                cwd: Some(project_root),
                details: None,
            },
        ]
    }

    fn render_screen(
        screen: &ExternalAgentConfigMigrationScreen,
        width: u16,
        height: u16,
    ) -> String {
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(screen, frame.area());
        }
        terminal.flush().expect("flush");
        terminal
            .backend()
            .to_string()
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn prompt_snapshot() {
        let items = sample_items();
        let screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );

        let rendered = render_screen(&screen, /*width*/ 80, /*height*/ 24);
        #[cfg(windows)]
        assert_snapshot!("external_agent_config_migration_prompt_windows", rendered);
        #[cfg(not(windows))]
        assert_snapshot!("external_agent_config_migration_prompt", rendered);
    }

    #[test]
    fn customize_snapshot() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );
        screen.customize();

        let rendered = render_screen(&screen, /*width*/ 80, /*height*/ 30);
        #[cfg(windows)]
        assert_snapshot!(
            "external_agent_config_migration_customize_windows",
            rendered
        );
        #[cfg(not(windows))]
        assert_snapshot!("external_agent_config_migration_customize", rendered);
    }

    #[test]
    fn customize_action_snapshot() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );
        screen.customize();
        screen.move_up();

        let rendered = render_screen(&screen, /*width*/ 80, /*height*/ 30);
        #[cfg(windows)]
        assert_snapshot!(
            "external_agent_config_migration_customize_action_windows",
            rendered
        );
        #[cfg(not(windows))]
        assert_snapshot!("external_agent_config_migration_customize_action", rendered);
    }

    #[test]
    fn proceed_returns_selected_items() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(screen.is_done());
        assert_eq!(
            screen.outcome(),
            ExternalAgentConfigMigrationOutcome::Proceed(items)
        );
    }

    #[test]
    fn toggle_item_then_proceed_keeps_remaining_selection() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));

        assert!(screen.is_done());
        assert_eq!(
            screen.outcome(),
            ExternalAgentConfigMigrationOutcome::Proceed(vec![
                items[1].clone(),
                items[2].clone(),
                items[3].clone(),
            ])
        );
    }

    #[test]
    fn escape_skips_prompt() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(screen.is_done());
        assert_eq!(screen.outcome(), ExternalAgentConfigMigrationOutcome::Skip);
    }

    #[test]
    fn numeric_shortcuts_follow_visible_actions_when_proceed_is_disabled() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));

        assert_eq!(screen.view, MigrationView::Customize);
    }

    #[test]
    fn empty_selection_enter_opens_customize_instead_of_proceeding() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &[],
            /*error*/ None,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(!screen.is_done());
        assert_eq!(screen.view, MigrationView::Customize);
    }

    #[test]
    fn control_exit_shortcuts_cancel_prompt() {
        let items = sample_items();
        for key_code in [KeyCode::Char('c'), KeyCode::Char('d')] {
            let mut screen = ExternalAgentConfigMigrationScreen::new(
                FrameRequester::test_dummy(),
                &items,
                &items,
                /*error*/ None,
            );

            screen.handle_key(KeyEvent::new(key_code, KeyModifiers::CONTROL));

            assert!(screen.is_done());
            assert_eq!(screen.outcome(), ExternalAgentConfigMigrationOutcome::Skip);
        }
    }

    #[test]
    fn numeric_shortcuts_choose_actions() {
        let items = sample_items();

        let mut proceed_screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );
        proceed_screen.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(
            proceed_screen.outcome(),
            ExternalAgentConfigMigrationOutcome::Proceed(items.clone())
        );

        let mut customize_screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );
        customize_screen.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
        assert_eq!(customize_screen.view, MigrationView::Customize);
        customize_screen.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(customize_screen.view, MigrationView::Summary);

        let mut skip_screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );
        skip_screen.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE));
        assert_eq!(
            skip_screen.outcome(),
            ExternalAgentConfigMigrationOutcome::Skip
        );
    }

    #[test]
    fn summary_does_not_toggle_selection() {
        let items = sample_items();
        let mut screen = ExternalAgentConfigMigrationScreen::new(
            FrameRequester::test_dummy(),
            &items,
            &items,
            /*error*/ None,
        );

        screen.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(screen.selected_items(), items);
    }
}
