//! Experimental controls with popup-owned discovery and configured enablement.
//! Failed saves retain intent for explicit retry; cancellation remains available.
//! Server voice discovery is gated by the client package's native runtime.

use crate::experimental_features::FeatureWriteResult;
use codex_app_server_protocol::ExperimentalFeature;
use codex_app_server_protocol::ExperimentalFeatureStage;
use codex_features::FEATURES;
use codex_features::Feature;
use codex_protocol::ThreadId;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::oneshot;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::key_hint;
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ListAction;
use crate::keymap::ListKeymap;
use crate::render::Insets;
use crate::render::RectExt as _;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::style::user_message_style;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::measure_rows_height;
use super::selection_popup_common::render_rows;

pub(crate) struct ExperimentalFeatureItem {
    pub key: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub writable: bool,
}

pub(crate) struct ExperimentalFeaturesView {
    features: Vec<ExperimentalFeatureItem>,
    voice_supported: bool,
    initial_enabled: Vec<bool>,
    unconfirmed: Vec<String>,
    catalog_rx: Option<oneshot::Receiver<Result<Vec<ExperimentalFeature>, String>>>,
    discovery_status: String,
    thread_id: ThreadId,
    write_rx: Option<oneshot::Receiver<Result<FeatureWriteResult, String>>>,
    state: ScrollState,
    complete: bool,
    app_event_tx: AppEventSender,
    footer_hint: Line<'static>,
    keymap: ListKeymap,
}

impl ExperimentalFeaturesView {
    pub(crate) fn new(
        features: Vec<ExperimentalFeatureItem>,
        thread_id: ThreadId,
        catalog_rx: Option<oneshot::Receiver<Result<Vec<ExperimentalFeature>, String>>>,
        app_event_tx: AppEventSender,
        keymap: ListKeymap,
    ) -> Self {
        let mut view = Self {
            discovery_status: if catalog_rx.is_some() {
                "Loading server experiments…"
            } else {
                ""
            }
            .to_string(),
            thread_id,
            voice_supported: codex_realtime_webrtc::RealtimeWebrtcSession::is_supported(),
            write_rx: None,
            catalog_rx,
            unconfirmed: Vec::new(),
            initial_enabled: features.iter().map(|item| item.enabled).collect(),
            features,
            state: ScrollState::new(),
            complete: false,
            app_event_tx,
            footer_hint: experimental_popup_hint_line(&keymap),
            keymap,
        };
        view.initialize_selection();
        view
    }

    fn header(&self, width: u16) -> impl Renderable {
        let mut header = ColumnRenderable::new();
        header.push(Line::from("Experimental features".bold()));
        for text in [
            "Checked features are configured on. Some experimental features take effect only in new tasks or after restarting the Codex server.",
            self.discovery_status.as_str(),
        ].into_iter().filter(|text| !text.is_empty()) {
            for line in textwrap::wrap(text, usize::from(width.max(1))) {
                header.push(Line::from(line.into_owned().dim()));
            }
        }
        header
    }

    fn initialize_selection(&mut self) {
        if self.visible_len() == 0 {
            self.state.selected_idx = None;
        } else if self.state.selected_idx.is_none() {
            self.state.selected_idx = Some(0);
        }
    }

    fn visible_len(&self) -> usize {
        self.features.len()
    }

    fn build_rows(&self) -> Vec<GenericDisplayRow> {
        let mut rows = Vec::with_capacity(self.features.len());
        let selected_idx = self.state.selected_idx;
        for (idx, item) in self.features.iter().enumerate() {
            let prefix = if selected_idx == Some(idx) {
                '›'
            } else {
                ' '
            };
            let marker = if item.enabled { 'x' } else { ' ' };
            let read_only = if item.writable { "" } else { " (read-only)" };
            let name = format!("{prefix} [{marker}] {}{read_only}", item.name);
            rows.push(GenericDisplayRow {
                name,
                description: Some(item.description.clone()),
                is_disabled: !item.writable || self.write_rx.is_some(),
                ..Default::default()
            });
        }

        rows
    }

    fn move_up(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn move_down(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        self.state.move_down_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    fn page_up(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.state.page_up_clamped(len, visible);
    }

    fn page_down(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.state.page_down_clamped(len, visible);
    }

    fn jump_top(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.state.jump_top(len, visible);
    }

    fn jump_bottom(&mut self) {
        let len = self.visible_len();
        let visible = MAX_POPUP_ROWS.min(len);
        self.state.jump_bottom(len, visible);
    }

    fn toggle_selected(&mut self) {
        let Some(selected_idx) = self.state.selected_idx else {
            return;
        };

        if self.write_rx.is_none()
            && let Some(item) = self.features.get_mut(selected_idx)
            && item.writable
        {
            item.enabled = !item.enabled;
        }
    }

    fn save_special_controls(&mut self) {
        // Keep the two unmigrated controls on their existing writer, including
        // its runtime and permission side effects. Never retry them generically.
        let mut updates = Vec::new();
        for (item, initial) in self.features.iter().zip(&mut self.initial_enabled) {
            if item.writable
                && item.enabled != *initial
                && let Some(feature) = [Feature::PreventIdleSleep, Feature::GuardianApproval]
                    .into_iter()
                    .find(|feature| feature.key() == item.key)
            {
                updates.push((feature, item.enabled));
                *initial = item.enabled;
            }
        }
        if !updates.is_empty() {
            self.app_event_tx
                .send(AppEvent::UpdateFeatureFlags { updates });
        }
    }

    fn save_changes(&mut self) {
        if self.write_rx.is_some() {
            self.complete = true;
            return;
        }
        self.save_special_controls();
        let updates: Vec<_> = self
            .features
            .iter()
            .zip(&self.initial_enabled)
            .filter(|(item, initial)| {
                item.writable && (item.enabled != **initial || self.unconfirmed.contains(&item.key))
            })
            .map(|(item, _)| (item.key.clone(), item.enabled))
            .collect();
        if !updates.is_empty() {
            let (tx, rx) = oneshot::channel();
            self.write_rx = Some(rx);
            // A failed response can follow a committed write. Keep these keys dirty
            // so reverting to the old baseline still sends a corrective write.
            self.unconfirmed = updates.iter().map(|(key, _)| key.clone()).collect();
            self.discovery_status = "Saving experimental features…".to_string();
            self.app_event_tx.send(AppEvent::SaveExperimentalFeatures {
                thread_id: self.thread_id,
                updates,
                response_tx: tx,
            });
        } else {
            self.complete = true;
        }
    }

    fn rows_width(total_width: u16) -> u16 {
        total_width.saturating_sub(2)
    }
}

impl BottomPaneView for ExperimentalFeaturesView {
    fn pre_draw_tick(&mut self, _now: Instant) -> bool {
        if let Some(receiver) = self.write_rx.as_mut() {
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(oneshot::error::TryRecvError::Empty) => return false,
                Err(oneshot::error::TryRecvError::Closed) => Err(
                    "Saving was interrupted. Reopen /experimental to check configured values."
                        .to_string(),
                ),
            };
            self.write_rx = None;
            match result {
                Ok(result) => {
                    for item in &mut self.features {
                        if matches!(
                            item.key.as_str(),
                            "prevent_idle_sleep" | "guardian_approval"
                        ) {
                            continue;
                        }
                        if let Some(feature) = result
                            .features
                            .iter()
                            .find(|feature| feature.name == item.key)
                        {
                            item.enabled = feature.enabled;
                        }
                    }
                    self.unconfirmed.clear();
                    self.initial_enabled = self.features.iter().map(|item| item.enabled).collect();
                    self.complete = result.warning.is_none();
                    self.discovery_status = result.warning.unwrap_or_default();
                }
                Err(error) => self.discovery_status = error,
            }
            return true;
        }
        let Some(receiver) = self.catalog_rx.as_mut() else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(oneshot::error::TryRecvError::Empty) => return false,
            Err(oneshot::error::TryRecvError::Closed) => {
                Err("Discovery was interrupted".to_string())
            }
        };
        self.catalog_rx = None;
        match result {
            Ok(features) => {
                let mut count = 0;
                for feature in features {
                    if feature.stage != ExperimentalFeatureStage::Beta
                        || (feature.name == Feature::RealtimeConversation.key()
                            && !self.voice_supported)
                    {
                        continue;
                    }
                    self.initial_enabled.push(feature.enabled);
                    self.features.push(ExperimentalFeatureItem {
                        // Preserve the existing metadata gate for unmigrated controls.
                        writable: !matches!(
                            feature.name.as_str(),
                            "prevent_idle_sleep" | "guardian_approval"
                        ) || FEATURES.iter().any(|spec| {
                            spec.key == feature.name
                                && spec.stage.experimental_menu_name().is_some()
                                && spec.default_enabled == feature.default_enabled
                        }),
                        key: feature.name.clone(),
                        name: feature.display_name.unwrap_or(feature.name),
                        description: feature.description.unwrap_or_default(),
                        enabled: feature.enabled,
                    });
                    count += 1;
                }
                self.discovery_status = if count == 0 {
                    "No server experiments available."
                } else {
                    ""
                }
                .to_string();
                self.initialize_selection();
            }
            Err(error) => {
                tracing::warn!(%error, "experimental feature discovery failed");
                self.discovery_status = "Server experiments unavailable. Reopen /experimental to retry; restart this Codex client if requests remain unanswered.".to_string();
            }
        }
        true
    }

    fn next_frame_delay(&self) -> Option<Duration> {
        (self.catalog_rx.is_some() || self.write_rx.is_some())
            .then(|| Duration::from_millis(/*millis*/ 100))
    }

    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::List)
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            _ if self.keymap.move_up.is_pressed(key_event) => self.move_up(),
            _ if self.keymap.move_down.is_pressed(key_event) => self.move_down(),
            _ if self.keymap.page_up.is_pressed(key_event) => self.page_up(),
            _ if self.keymap.page_down.is_pressed(key_event) => self.page_down(),
            _ if self.keymap.jump_top.is_pressed(key_event) => self.jump_top(),
            _ if self.keymap.jump_bottom.is_pressed(key_event) => self.jump_bottom(),
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => self.toggle_selected(),
            _ if self.keymap.accept.is_pressed(key_event) => {
                self.save_changes();
            }
            _ if self.keymap.cancel.is_pressed(key_event) => {
                self.on_ctrl_c();
            }
            _ => {}
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        // After an attempted save, cancellation must remain possible even if the
        // server rejects retries immediately. Explicit accept retries dirty keys.
        if !self.unconfirmed.is_empty() {
            self.save_special_controls();
        } else {
            self.save_changes();
        }
        self.complete = true;
        CancellationEvent::Handled
    }
}

impl Renderable for ExperimentalFeaturesView {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let [content_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        Block::default()
            .style(user_message_style())
            .render(content_area, buf);

        let header = self.header(content_area.width.saturating_sub(4));
        let header_height = header.desired_height(content_area.width.saturating_sub(4));
        let rows = self.build_rows();
        let rows_width = Self::rows_width(content_area.width);
        let rows_height = measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, rows_width);
        let [header_area, _, list_area] = Layout::vertical([
            Constraint::Max(header_height),
            Constraint::Max(1),
            Constraint::Length(rows_height),
        ])
        .areas(content_area.inset(Insets::vh(/*v*/ 1, /*h*/ 2)));

        header.render(header_area, buf);

        if list_area.height > 0 && (!rows.is_empty() || self.catalog_rx.is_none()) {
            let render_area = Rect {
                x: list_area.x.saturating_sub(2),
                y: list_area.y,
                width: rows_width.max(1),
                height: list_area.height,
            };
            render_rows(
                render_area,
                buf,
                &rows,
                &self.state,
                MAX_POPUP_ROWS,
                "  No experimental features available for now",
            );
        }

        let hint_area = Rect {
            x: footer_area.x + 2,
            y: footer_area.y,
            width: footer_area.width.saturating_sub(2),
            height: footer_area.height,
        };
        let hint = if self.write_rx.is_some() {
            Line::from("Saving… Closing this popup will not cancel the write.")
        } else if !self.unconfirmed.is_empty() {
            Line::from("Selections retained. Save to retry, or cancel to close.")
        } else {
            self.footer_hint.clone()
        };
        hint.dim().render(hint_area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        let rows = self.build_rows();
        let rows_width = Self::rows_width(width);
        let rows_height = measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, rows_width);

        let mut height = self
            .header(width.saturating_sub(4))
            .desired_height(width.saturating_sub(4));
        height = height.saturating_add(rows_height + 3);
        height.saturating_add(1)
    }
}

fn experimental_popup_hint_line(keymap: &ListKeymap) -> Line<'static> {
    let mut spans = vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Char(' ')).into(),
        " to select".into(),
    ];
    if let Some(accept) = keymap.primary_hint(ListAction::Accept) {
        spans.extend([" or ".into(), accept.into(), " to save".into()]);
    }
    Line::from(spans)
}

#[cfg(test)]
#[path = "experimental_features_view_tests.rs"]
mod tests;
