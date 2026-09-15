//! Authenticated account analytics dashboard.
//! A maximized section and an overview share bounded report loads, selection, and inline details.

mod chart;
mod client;
mod dashboard;
mod data;
#[cfg(test)]
#[path = "analytics/test_fixtures.rs"]
mod fixture;
mod models;
mod normalize;
mod panels;
mod plot;
mod render;
mod report_data;
mod sections;
mod styles;
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
use data::Load;
use sections::Section;
use sections::SectionState;
use sections::SectionStates;

pub(crate) struct AnalyticsView {
    model_names: std::collections::HashMap<String, String>,
    sections: SectionStates,
    account: Load<models::AccountKind>,
    reports_started: bool,
    connection: Option<(
        std::sync::Arc<crate::legacy_core::config::Config>,
        AppServerRequestHandle,
        FrameRequester,
    )>,
    live: Option<std::sync::Arc<client::Live>>,
    pub(crate) keymap: ListKeymap,
    section: Section,
    zoomed: bool,
    scroll_offset: usize,
    follow_selection: bool,
    viewport_height: usize,
    ranges: [usize; 3],
    end_date: chrono::NaiveDate,
    token_model: Option<String>,
    group_picker: Option<usize>,
    pub(crate) is_done: bool,
}

impl AnalyticsView {
    pub(crate) fn new(keymap: ListKeymap) -> Self {
        let mut view = Self {
            model_names: std::collections::HashMap::new(),
            sections: SectionStates(std::array::from_fn(|_| SectionState::default())),
            account: Load::Unavailable,
            reports_started: false,
            connection: None,
            live: None,
            keymap,
            section: Section::Usage,
            zoomed: true,
            scroll_offset: 0,
            follow_selection: true,
            viewport_height: 1,
            ranges: [0; 3],
            end_date: chrono::Utc::now().date_naive(),
            token_model: None,
            group_picker: None,
            is_done: false,
        };
        view.sections[Section::Usage].group = 1;
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
        self.connection = Some((config, handle, frame));
        self.is_done = false;
        self.refresh();
    }

    pub(crate) fn refresh(&mut self) {
        self.end_date = chrono::Utc::now().date_naive();
        self.group_picker = None;
        self.live = self.connection.as_ref().map(|(config, _, _)| {
            std::sync::Arc::new(client::Live::new(
                std::sync::Arc::clone(config),
                self.end_date,
            ))
        });
        for section in &mut self.sections.0 {
            section.history = Load::Unavailable;
        }
        self.reports_started = false;
        self.token_model = None;
        self.account = if let (Some((_, _, frame)), Some(live)) = (&self.connection, &self.live) {
            let live = std::sync::Arc::clone(live);
            Load::start(
                async move { live.session().await.map(|session| Some(session.kind)) },
                frame.clone(),
            )
        } else {
            Load::Unavailable
        };
        self.start_reports();
    }

    /// Abort work when closing or invalidating a retained view; reopening starts fresh requests.
    pub(crate) fn cancel_loads(&mut self) {
        for section in &mut self.sections.0 {
            section.history = Load::Unavailable;
        }
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
            Section::Usage if self.business() => &[6, 2],
            Section::Usage if self.consumer_attribution() => &[1, 2, 0, 3],
            Section::Usage => &[0, 2],
            Section::Credits => self.live.as_ref().map_or(&[0], |live| live.credit_groups()),
            Section::Activity => &[2, 0],
            Section::Plugins | Section::Skills => &[],
        }
    }

    fn consumer_attribution(&self) -> bool {
        !self.business()
            && self
                .live
                .as_ref()
                .is_none_or(|live| live.attributed_usage())
    }

    fn section_title(&self, section: Section) -> &str {
        match section {
            Section::Usage if self.business() => "Token usage history",
            Section::Usage if self.consumer_attribution() => "Total usage history",
            Section::Usage => "Usage history",
            Section::Plugins => "Plugins called",
            Section::Credits => "Credits usage history",
            Section::Activity => "Messages",
            Section::Skills => "Skills used",
        }
    }

    fn date_range(&self) -> std::ops::RangeInclusive<chrono::NaiveDate> {
        self.section_date_range(self.section)
    }

    fn model_name<'a>(&'a self, model: &'a str) -> &'a str {
        self.model_names
            .get(model)
            .filter(|name| !name.is_empty())
            .map(String::as_str)
            .unwrap_or(model)
    }

    fn row_count(&self) -> usize {
        if self.ranges[self.range_group(self.section) as usize] == 0 {
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
            self.group_picker = None;
            self.is_done = true;
            return;
        }
        let action = if key_hint::plain(KeyCode::Char(' ')).is_press(key) {
            Some(ListAction::Accept)
        } else {
            self.keymap.action_for(key)
        };
        if let Some(choice) = self.group_picker {
            let options = self.group_options();
            let Some(&group) = options.get(choice) else {
                self.group_picker = None;
                return;
            };
            match action {
                Some(ListAction::MoveUp | ListAction::MoveLeft) => {
                    self.group_picker = Some((choice + options.len() - 1) % options.len());
                }
                Some(ListAction::MoveDown | ListAction::MoveRight) => {
                    self.group_picker = Some((choice + 1) % options.len());
                }
                Some(ListAction::Accept) => {
                    self.sections[self.section].group = group;
                    self.group_picker = None;
                    self.load_report(self.section);
                }
                Some(ListAction::Cancel) => self.group_picker = None,
                _ => {}
            }
            self.follow_selection = true;
            return;
        }
        self.follow_selection = true;
        if key_hint::plain(KeyCode::Char('z')).is_press(key) {
            self.zoomed = !self.zoomed;
            self.scroll_offset = 0;
            return;
        }
        if !self.zoomed && action == Some(ListAction::Accept) {
            self.zoomed = true;
            self.scroll_offset = 0;
            return;
        }
        if key_hint::plain(KeyCode::Char('R')).is_press(key) {
            self.refresh();
            return;
        }
        if self.section == Section::Usage
            && self.business()
            && key_hint::plain(KeyCode::Char('m')).is_press(key)
        {
            let models = self
                .live
                .as_ref()
                .map_or_else(Vec::new, |live| live.token_models());
            self.token_model = match self
                .token_model
                .as_ref()
                .and_then(|model| models.iter().position(|candidate| candidate == model))
            {
                Some(index) => models.get(index + 1).cloned(),
                None => models.first().cloned(),
            };
            self.load_report(Section::Usage);
            return;
        }
        if key_hint::plain(KeyCode::Char('r')).is_press(key) && !self.visible_sections().is_empty()
        {
            self.change_range();
            return;
        }
        if key_hint::plain(KeyCode::Char('g')).is_press(key) && self.group_options().len() > 1 {
            self.group_picker = self
                .group_options()
                .iter()
                .position(|group| *group == self.sections[self.section].group);
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
            KeyCode::Char(n @ '1'..='6') if key.modifiers.is_empty() => {
                visible.get(n as usize - '1' as usize).copied()
            }
            KeyCode::Tab => Some(visible[(current + 1) % visible.len()]),
            KeyCode::BackTab => Some(visible[(current + visible.len() - 1) % visible.len()]),
            _ => None,
        };
        if let Some(section) = target {
            if self.zoomed && section != self.section {
                self.scroll_offset = 0;
            }
            self.section = section;
            self.group_picker = None;
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
        let count = self.row_count().max(/*other*/ 1);
        let SectionState { cursor, detail, .. } = &mut self.sections[self.section];
        match action {
            Some(ListAction::MoveUp) => *cursor = cursor.saturating_sub(/*rhs*/ 1),
            Some(ListAction::MoveDown) => *cursor = (*cursor + 1).min(count - 1),
            Some(ListAction::MoveLeft) => *cursor = cursor.saturating_sub(/*rhs*/ 1),
            Some(ListAction::MoveRight) => *cursor = (*cursor + 1).min(count - 1),
            Some(ListAction::JumpTop) => *cursor = 0,
            Some(ListAction::JumpBottom) => *cursor = count - 1,
            Some(ListAction::Accept) => {
                *detail = if *detail == Some(*cursor) {
                    None
                } else {
                    Some(*cursor)
                };
            }
            Some(ListAction::Cancel) => {
                self.is_done |= !self.zoomed || detail.take().is_none();
            }
            Some(ListAction::PageDown) => {
                self.scroll_offset += self.viewport_height;
                self.follow_selection = false;
            }
            Some(ListAction::PageUp) => {
                self.scroll_offset = self.scroll_offset.saturating_sub(self.viewport_height);
                self.follow_selection = false;
            }
            None => {}
        }
        if detail.is_some() {
            *detail = Some(*cursor);
        }
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
                    self.account = Load::Unavailable;
                    self.connection = None;
                }
                tui.frame_requester().schedule_frame();
            }
            TuiEvent::Draw | TuiEvent::Resume | TuiEvent::Resize(_) | TuiEvent::FocusGained => {
                if matches!(event, TuiEvent::Resize(_)) {
                    self.follow_selection = true;
                }
                tui.draw(u16::MAX, |frame| self.render(frame.area(), frame.buffer))?;
            }
            _ => {}
        }
        Ok(())
    }
}
