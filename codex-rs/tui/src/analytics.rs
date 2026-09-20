//! Authenticated account analytics dashboard.
//! Stable report tabs share bounded account loads, selection, and inline details.

mod activity_chart;
mod chart;
mod summary;
mod summary_panel;
pub(crate) use activity_chart::TokenActivityView;
mod chat_panel;
mod chats;
mod chrome;
mod client;
mod controls;
mod dashboard;
mod data;
#[cfg(test)]
#[path = "analytics/test_fixtures.rs"]
mod fixture;
mod hints;
mod models;
mod mouse;
mod normalize;
mod panels;
mod plan;
mod plan_panel;
mod plot;
mod render;
mod report_data;
mod sections;
mod styles;
mod task_panel;
mod tasks;
#[cfg(test)]
#[path = "analytics/test_support.rs"]
mod test_support;
mod tokens;
mod tool_panel;

#[cfg(test)]
#[path = "analytics_tests.rs"]
mod tests;

use crate::key_hint;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::tui;
use crate::tui::FrameRequester;
use crate::tui::TuiEvent;
use codex_app_server_client::AppServerRequestHandle;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::MouseButton;
use crossterm::event::MouseEventKind;
use data::Load;
use sections::Section;
use sections::SectionState;
use sections::SectionStates;

pub(crate) struct AnalyticsView {
    model_names: std::collections::HashMap<String, String>,
    sections: SectionStates,
    chats: Load<chats::Chats>,
    tasks: Load<tasks::Chats>,
    chat_metric: usize,
    profile: Load<codex_backend_client::AccountProfile>,
    plan: plan::State,
    show_zero_credit_groups: bool,
    account: Load<codex_protocol::account::PlanType>,
    reports_started: bool,
    connection: Option<(
        std::sync::Arc<crate::legacy_core::config::Config>,
        AppServerRequestHandle,
        FrameRequester,
    )>,
    live: Option<std::sync::Arc<client::Live>>,
    pub(crate) keymap: ListKeymap,
    section: Section,
    show_help: bool,
    zoomed: bool,
    dashboard_scroll: usize,
    body_area: ratatui::layout::Rect,
    report_area: ratatui::layout::Rect,
    tab_hits: Vec<(Section, ratatui::layout::Rect)>,
    control_hits: Vec<(controls::Control, ratatui::layout::Rect)>,
    mouse_context: Option<(Section, bool, bool)>,
    max_scroll: usize,
    selection_visible: bool,
    help_scroll: usize,
    follow_selection: bool,
    viewport_height: usize,
    ranges: [usize; 3],
    end_date: chrono::NaiveDate,
    token_model: Option<String>,
    pub(crate) is_done: bool,
}

impl AnalyticsView {
    pub(crate) fn new(keymap: ListKeymap) -> Self {
        let mut view = Self {
            model_names: std::collections::HashMap::new(),
            sections: SectionStates(std::array::from_fn(|_| SectionState::default())),
            chats: Load::Unavailable,
            tasks: Load::Unavailable,
            chat_metric: 0,
            profile: Load::Unavailable,
            plan: plan::State::default(),
            show_zero_credit_groups: false,
            account: Load::Unavailable,
            reports_started: false,
            connection: None,
            live: None,
            keymap,
            section: Section::Summary,
            show_help: false,
            zoomed: true,
            dashboard_scroll: 0,
            body_area: ratatui::layout::Rect::default(),
            report_area: ratatui::layout::Rect::default(),
            tab_hits: Vec::new(),
            control_hits: Vec::new(),
            mouse_context: None,
            max_scroll: 0,
            selection_visible: false,
            help_scroll: 0,
            follow_selection: true,
            viewport_height: 1,
            ranges: [0; 3],
            end_date: chrono::Utc::now().date_naive(),
            token_model: None,
            is_done: false,
        };
        view.sections[Section::Usage].group = 1;
        view.sections[Section::Plan].group = 1;
        view.sections[Section::Chats].cursor = 0;
        view.sections[Section::Activity].group = 2;
        view.refresh();
        view
    }

    pub(crate) fn open(
        &mut self,
        handle: AppServerRequestHandle,
        frame: FrameRequester,
        models: Vec<codex_protocol::openai_models::ModelPreset>,
        config: std::sync::Arc<crate::legacy_core::config::Config>,
    ) {
        self.model_names = models
            .into_iter()
            .map(|model| (model.model, model.display_name))
            .collect();
        self.plan.enabled = config
            .features
            .enabled(codex_features::Feature::AnalyticsPlanHistory);
        self.connection = Some((config, handle, frame));
        self.is_done = false;
        self.show_help = false;
        self.refresh();
    }

    pub(crate) fn refresh(&mut self) {
        self.invalidate_mouse_targets();
        self.end_date = chrono::Utc::now().date_naive();
        self.live = self.connection.as_ref().map(|(config, _, _)| {
            std::sync::Arc::new(client::Live::new(
                std::sync::Arc::clone(config),
                self.end_date,
            ))
        });
        for section in &mut self.sections.0 {
            section.history = Load::Unavailable;
        }
        self.sections[Section::Chats].detail = None;
        self.chats = Load::Unavailable;
        self.tasks = Load::Unavailable;
        self.profile = Load::Unavailable;
        self.plan.report = Load::Unavailable;
        self.reports_started = false;
        self.token_model = None;
        self.account = if let (Some((_, _, frame)), Some(live)) = (&self.connection, &self.live) {
            let live = std::sync::Arc::clone(live);
            Load::start(
                async move {
                    live.session().await.map(|session| {
                        Some(
                            session
                                .backend
                                .account()
                                .plan_type
                                .unwrap_or(codex_protocol::account::PlanType::Unknown),
                        )
                    })
                },
                frame.clone(),
            )
        } else {
            Load::Unavailable
        };
        self.start_reports();
    }

    /// Abort work when closing or invalidating a retained view; reopening starts fresh requests.
    pub(crate) fn cancel_loads(&mut self) {
        self.invalidate_mouse_targets();
        for section in &mut self.sections.0 {
            section.history = Load::Unavailable;
        }
        self.chats = Load::Unavailable;
        self.tasks = Load::Unavailable;
        self.profile = Load::Unavailable;
        self.plan.report = Load::Unavailable;
        self.account = Load::Unavailable;
        self.live = None;
        self.connection = None;
    }

    fn load_report(&mut self, section: Section) {
        let Some(report) = section.report() else {
            return;
        };
        let group = self.sections[section].group;
        let range = self.ranges[self.range_group(section) as usize];
        self.sections[section].history =
            if let (Some((_, _, frame)), Some(live)) = (&self.connection, &self.live) {
                let live = std::sync::Arc::clone(live);
                let days = if range == 0 { 7 } else { 30 };
                let model = self.token_model.clone();
                Load::start(
                    async move {
                        if let Some(model) = model
                            .as_deref()
                            .filter(|_| report == models::AccountAnalyticsReport::Usage)
                        {
                            live.filtered_history(report, days, data::GROUPINGS[group], Some(model))
                                .await
                        } else {
                            live.history(report, days, data::GROUPINGS[group]).await
                        }
                    },
                    frame.clone(),
                )
            } else {
                Load::Unavailable
            };
    }

    fn group_options(&self) -> &[usize] {
        if self.visible_sections().is_empty() {
            return &[];
        }
        match self.section {
            Section::Summary => &[0, 1, 2],
            Section::Plan => &[1, 2, 0, 3],
            Section::Usage if self.business() => &[6, 2],
            Section::Usage if self.consumer_attribution() => &[1, 2, 0, 3],
            Section::Usage => &[0, 2],
            Section::Credits => self.live.as_ref().map_or(&[0], |live| live.credit_groups()),
            Section::Activity => &[2, 0],
            Section::Plugins | Section::Skills | Section::Chats => &[],
        }
    }

    fn consumer_attribution(&self) -> bool {
        !self.business()
            && self
                .live
                .as_ref()
                .is_none_or(|live| live.attributed_usage())
    }

    fn model_name<'a>(&'a self, model: &'a str) -> &'a str {
        self.model_names
            .get(model)
            .filter(|name| !name.is_empty())
            .map(String::as_str)
            .unwrap_or(model)
    }

    fn row_count(&self) -> usize {
        if self.section == Section::Chats && !self.business() {
            self.tasks
                .ready()
                .map_or(/*default*/ 0, |chats| chats.rows.len())
        } else if self.section == Section::Chats {
            self.chats
                .ready()
                .map_or(/*default*/ 0, |chats| chats.rows.len())
        } else if self.ranges[self.range_group(self.section) as usize] == 0 {
            7
        } else {
            30
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if key_hint::plain(KeyCode::Char('q')).is_press(key)
            || key_hint::ctrl(KeyCode::Char('c')).is_press(key)
        {
            self.is_done = true;
            return;
        }
        let action = if key_hint::plain(KeyCode::Char(' ')).is_press(key) {
            Some(ListAction::Accept)
        } else {
            self.keymap.action_for(key).or_else(|| {
                if !key.modifiers.is_empty() {
                    return None;
                }
                // Keep explicit bindings first and honor remapped or unbound arrows.
                let code = match key.code {
                    KeyCode::Char('h') => KeyCode::Left,
                    KeyCode::Char('l') => KeyCode::Right,
                    _ => return None,
                };
                self.keymap.action_for(KeyEvent { code, ..key })
            })
        };

        self.follow_selection = matches!(
            action,
            Some(
                ListAction::Accept
                    | ListAction::Cancel
                    | ListAction::MoveLeft
                    | ListAction::MoveRight
                    | ListAction::MoveUp
                    | ListAction::MoveDown
                    | ListAction::JumpTop
                    | ListAction::JumpBottom
            )
        );
        if self.help_shortcut_available()
            && (key_hint::plain(KeyCode::Char('?')).is_press(key)
                || key_hint::shift(KeyCode::Char('?')).is_press(key))
        {
            self.show_help = !self.show_help;
            self.help_scroll = 0;
            self.follow_selection = false;
            return;
        }
        if self.show_help && action == Some(ListAction::Cancel) {
            self.show_help = false;
            self.follow_selection = false;
            return;
        }
        if self.show_help {
            self.summary_action(action);
            return;
        }
        if key_hint::plain(KeyCode::Char('z')).is_press(key)
            && self.control_available(controls::Control::Dashboard)
        {
            self.activate_control(controls::Control::Dashboard);
            return;
        }
        if !self.zoomed && action == Some(ListAction::Accept) {
            self.toggle_dashboard();
            return;
        }
        if key_hint::plain(KeyCode::Char('R')).is_press(key) {
            self.refresh();
            return;
        }
        if self.control_available(controls::Control::ZeroCreditGroups)
            && key_hint::plain(KeyCode::Char('a')).is_press(key)
        {
            self.activate_control(controls::Control::ZeroCreditGroups);
            return;
        }
        if self.control_available(controls::Control::Model)
            && key_hint::plain(KeyCode::Char('m')).is_press(key)
        {
            self.activate_control(controls::Control::Model);
            return;
        }
        if key_hint::plain(KeyCode::Char('r')).is_press(key)
            && self.control_available(controls::Control::Range)
        {
            self.activate_control(controls::Control::Range);
            return;
        }
        if key_hint::plain(KeyCode::Char('g')).is_press(key)
            && self.control_available(controls::Control::Group)
        {
            self.activate_control(controls::Control::Group);
            return;
        }
        let visible = self.visible_sections();
        if visible.is_empty() {
            self.is_done |= action == Some(ListAction::Cancel);
            return;
        }
        let current = visible
            .iter()
            .position(|section| *section == self.section)
            .unwrap_or_default();
        let target = match key.code {
            KeyCode::Char(n @ '1'..='7') if key.modifiers.is_empty() => {
                visible.get(n as usize - '1' as usize).copied()
            }
            KeyCode::Tab => Some(visible[(current + 1) % visible.len()]),
            KeyCode::BackTab => Some(visible[(current + visible.len() - 1) % visible.len()]),
            _ => None,
        };
        if let Some(section) = target {
            self.select_section(section);
            return;
        }
        if !self.zoomed && action == Some(ListAction::Cancel) {
            self.is_done = true;
            return;
        }
        if self.section == Section::Summary {
            self.summary_action(action);
            return;
        }
        if self.section == Section::Plan {
            self.plan_action(action);
            return;
        }
        if self.control_available(controls::Control::TaskMetric)
            && key_hint::plain(KeyCode::Char('s')).is_press(key)
        {
            self.activate_control(controls::Control::TaskMetric);
            return;
        }
        if matches!(
            self.section,
            Section::Plugins | Section::Activity | Section::Skills
        ) && action == Some(ListAction::Accept)
            && !self.day_has_details(self.section, self.sections[self.section].cursor)
        {
            return;
        }
        if self.section == Section::Chats
            && matches!(action, Some(ListAction::Accept | ListAction::MoveRight))
            && !self.chat_has_details()
        {
            return;
        }
        let chart = self.section != Section::Chats;
        let count = self.row_count().max(/*other*/ 1);
        let mut scroll_offset = self.scroll_offset();
        let SectionState {
            cursor,
            detail,
            follow_selection_on_focus,
            ..
        } = &mut self.sections[self.section];
        let previous_cursor = *cursor;
        match action {
            Some(ListAction::MoveUp) => *cursor = cursor.saturating_sub(/*rhs*/ 1),
            Some(ListAction::MoveDown) => *cursor = (*cursor + 1).min(count - 1),
            Some(ListAction::MoveLeft) if chart => *cursor = cursor.saturating_sub(/*rhs*/ 1),
            Some(ListAction::MoveRight) if chart => *cursor = (*cursor + 1).min(count - 1),
            Some(ListAction::JumpTop) => *cursor = 0,
            Some(ListAction::JumpBottom) => *cursor = count - 1,
            Some(ListAction::Accept | ListAction::MoveRight) => {
                *detail = if *detail == Some(*cursor) {
                    None
                } else {
                    Some(*cursor)
                };
            }
            Some(ListAction::MoveLeft) => *detail = None,
            Some(ListAction::Cancel) => {
                self.is_done |= detail.take().is_none();
            }
            Some(ListAction::PageDown) => {
                scroll_offset += self.viewport_height;
                self.follow_selection = false;
            }
            Some(ListAction::PageUp) => {
                scroll_offset = scroll_offset.saturating_sub(self.viewport_height);
                self.follow_selection = false;
            }
            None => {}
        }
        if chart && detail.is_some() {
            *detail = Some(*cursor);
        } else if !chart && *cursor != previous_cursor {
            *detail = None;
        }
        *follow_selection_on_focus |= !self.zoomed && *cursor != previous_cursor;
        *self.scroll_offset_mut() = scroll_offset;
    }

    pub(crate) fn handle_event(
        &mut self,
        tui: &mut tui::Tui,
        event: TuiEvent,
    ) -> std::io::Result<()> {
        match event {
            TuiEvent::Key(key) => {
                self.handle_key(key);
                if self.is_done {
                    for section in &mut self.sections.0 {
                        if matches!(section.history, Load::Loading(_)) {
                            section.history = Load::Unavailable;
                        }
                    }
                    self.chats = Load::Unavailable;
                    self.tasks = Load::Unavailable;
                    self.profile = Load::Unavailable;
                    self.plan.report = Load::Unavailable;
                    self.account = Load::Unavailable;
                    self.connection = None;
                }
                tui.frame_requester().schedule_frame();
            }
            TuiEvent::Mouse(mouse)
                if matches!(
                    mouse.kind,
                    MouseEventKind::Down(MouseButton::Left)
                        | MouseEventKind::ScrollUp
                        | MouseEventKind::ScrollDown
                ) =>
            {
                self.handle_mouse(mouse);
                tui.frame_requester().schedule_frame();
            }
            TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
                // Rendering follows a selection on resize only when it was visible before
                // reflow. Preserve an intentional reading position away from that selection.
                tui.draw(u16::MAX, |frame| self.render(frame.area(), frame.buffer))?;
            }
            _ => {}
        }
        Ok(())
    }
}
