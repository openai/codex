#![cfg(any(not(debug_assertions), test))]
#![cfg_attr(test, allow(dead_code))]

use crate::history_cell::padded_emoji;
use crate::key_hint;
use crate::legacy_core::config::Config;
use crate::render::Insets;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::selection_list::selection_option_row;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use crate::update_action::PromptedUpdate;
use crate::update_action::UpdateAction;
use crate::update_action::UpdateActionStatus;
use crate::update_action::UpdateBlocker;
use crate::updates;
use crate::updates::UpgradeNotice;
use color_eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;
use tokio_stream::StreamExt;

pub(crate) enum UpdatePromptOutcome {
    Continue,
    RunUpdate(PromptedUpdate),
}

pub(crate) async fn run_update_prompt_if_needed(
    tui: &mut Tui,
    config: &Config,
) -> Result<UpdatePromptOutcome> {
    let Some(notice) = updates::get_upgrade_notice_for_popup(config) else {
        return Ok(UpdatePromptOutcome::Continue);
    };

    match notice {
        UpgradeNotice::Available(latest_version) => {
            let update_action = match crate::update_action::get_update_action_status() {
                UpdateActionStatus::Ready(update_action) => update_action,
                UpdateActionStatus::Blocked(blocker) => {
                    run_remediation_prompt(tui, RemediationPromptReason::Blocked(blocker)).await?;
                    return Ok(UpdatePromptOutcome::Continue);
                }
                UpdateActionStatus::Unavailable => return Ok(UpdatePromptOutcome::Continue),
            };
            run_standard_update_prompt(tui, config, latest_version, update_action).await
        }
        UpgradeNotice::RemediationNeeded(latest_version) => {
            run_remediation_prompt(
                tui,
                RemediationPromptReason::NoOpUpdate {
                    latest_version: latest_version.clone(),
                },
            )
            .await?;
            if let Err(err) =
                updates::suppress_version_after_remediation(config, &latest_version).await
            {
                tracing::error!("Failed to persist update remediation suppression: {err}");
            }
            Ok(UpdatePromptOutcome::Continue)
        }
    }
}

async fn run_standard_update_prompt(
    tui: &mut Tui,
    config: &Config,
    latest_version: String,
    update_action: UpdateAction,
) -> Result<UpdatePromptOutcome> {
    let mut screen =
        UpdatePromptScreen::new(tui.frame_requester(), latest_version.clone(), update_action);
    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    })?;

    let events = tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw | TuiEvent::Resize => {
                    tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    })?;
                }
            }
        } else {
            break;
        }
    }

    match screen.selection() {
        Some(UpdateSelection::UpdateNow) => {
            tui.terminal.clear()?;
            Ok(UpdatePromptOutcome::RunUpdate(PromptedUpdate {
                action: update_action,
                target_version: latest_version,
                version_file: updates::version_filepath(config),
            }))
        }
        Some(UpdateSelection::NotNow) | None => Ok(UpdatePromptOutcome::Continue),
        Some(UpdateSelection::DontRemind) => {
            if let Err(err) = updates::dismiss_version(config, screen.latest_version()).await {
                tracing::error!("Failed to persist update dismissal: {err}");
            }
            Ok(UpdatePromptOutcome::Continue)
        }
    }
}

async fn run_remediation_prompt(tui: &mut Tui, reason: RemediationPromptReason) -> Result<()> {
    let mut screen = RemediationPromptScreen::new(tui.frame_requester(), reason);
    tui.draw(u16::MAX, |frame| {
        frame.render_widget_ref(&screen, frame.area());
    })?;

    let events = tui.event_stream();
    tokio::pin!(events);

    while !screen.is_done() {
        if let Some(event) = events.next().await {
            match event {
                TuiEvent::Key(key_event) => screen.handle_key(key_event),
                TuiEvent::Paste(_) => {}
                TuiEvent::Draw | TuiEvent::Resize => {
                    tui.draw(u16::MAX, |frame| {
                        frame.render_widget_ref(&screen, frame.area());
                    })?;
                }
            }
        } else {
            break;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateSelection {
    UpdateNow,
    NotNow,
    DontRemind,
}

struct UpdatePromptScreen {
    request_frame: FrameRequester,
    latest_version: String,
    current_version: String,
    update_action: UpdateAction,
    highlighted: UpdateSelection,
    selection: Option<UpdateSelection>,
}

impl UpdatePromptScreen {
    fn new(
        request_frame: FrameRequester,
        latest_version: String,
        update_action: UpdateAction,
    ) -> Self {
        Self {
            request_frame,
            latest_version,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            update_action,
            highlighted: UpdateSelection::UpdateNow,
            selection: None,
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.select(UpdateSelection::NotNow);
            return;
        }
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => self.set_highlight(self.highlighted.prev()),
            KeyCode::Down | KeyCode::Char('j') => self.set_highlight(self.highlighted.next()),
            KeyCode::Char('1') => self.select(UpdateSelection::UpdateNow),
            KeyCode::Char('2') => self.select(UpdateSelection::NotNow),
            KeyCode::Char('3') => self.select(UpdateSelection::DontRemind),
            KeyCode::Enter => self.select(self.highlighted),
            KeyCode::Esc => self.select(UpdateSelection::NotNow),
            _ => {}
        }
    }

    fn set_highlight(&mut self, highlight: UpdateSelection) {
        if self.highlighted != highlight {
            self.highlighted = highlight;
            self.request_frame.schedule_frame();
        }
    }

    fn select(&mut self, selection: UpdateSelection) {
        self.highlighted = selection;
        self.selection = Some(selection);
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.selection.is_some()
    }

    fn selection(&self) -> Option<UpdateSelection> {
        self.selection
    }

    fn latest_version(&self) -> &str {
        self.latest_version.as_str()
    }
}

impl UpdateSelection {
    fn next(self) -> Self {
        match self {
            UpdateSelection::UpdateNow => UpdateSelection::NotNow,
            UpdateSelection::NotNow => UpdateSelection::DontRemind,
            UpdateSelection::DontRemind => UpdateSelection::UpdateNow,
        }
    }

    fn prev(self) -> Self {
        match self {
            UpdateSelection::UpdateNow => UpdateSelection::DontRemind,
            UpdateSelection::NotNow => UpdateSelection::UpdateNow,
            UpdateSelection::DontRemind => UpdateSelection::NotNow,
        }
    }
}

impl WidgetRef for &UpdatePromptScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = ColumnRenderable::new();

        let update_command = self.update_action.command_str();

        column.push("");
        column.push(Line::from(vec![
            padded_emoji("  ✨").bold().cyan(),
            "Update available!".bold(),
            " ".into(),
            format!(
                "{current} -> {latest}",
                current = self.current_version,
                latest = self.latest_version
            )
            .dim(),
        ]));
        column.push("");
        column.push(
            Line::from(vec![
                "Release notes: ".dim(),
                "https://github.com/openai/codex/releases/latest"
                    .dim()
                    .underlined(),
            ])
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.push("");
        column.push(selection_option_row(
            0,
            format!("Update now (runs `{update_command}`)"),
            self.highlighted == UpdateSelection::UpdateNow,
        ));
        column.push(selection_option_row(
            1,
            "Skip".to_string(),
            self.highlighted == UpdateSelection::NotNow,
        ));
        column.push(selection_option_row(
            2,
            "Skip until next version".to_string(),
            self.highlighted == UpdateSelection::DontRemind,
        ));
        column.push("");
        column.push(
            Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue".dim(),
            ])
            .inset(Insets::tlbr(0, 2, 0, 0)),
        );
        column.render(area, buf);
    }
}

#[derive(Clone, Debug)]
enum RemediationPromptReason {
    Blocked(UpdateBlocker),
    NoOpUpdate { latest_version: String },
}

impl RemediationPromptReason {
    fn lines(&self) -> Vec<String> {
        match self {
            Self::Blocked(blocker) => blocker.remediation_lines(),
            Self::NoOpUpdate { latest_version } => vec![
                "The previous update command completed, but this Codex executable did not change."
                    .to_string(),
                format!(
                    "Codex is still running {} while {latest_version} is available.",
                    env!("CARGO_PKG_VERSION")
                ),
                "Check which `codex` your shell runs before trying again.".to_string(),
            ],
        }
    }
}

struct RemediationPromptScreen {
    request_frame: FrameRequester,
    reason: RemediationPromptReason,
    done: bool,
}

impl RemediationPromptScreen {
    fn new(request_frame: FrameRequester, reason: RemediationPromptReason) -> Self {
        Self {
            request_frame,
            reason,
            done: false,
        }
    }

    fn handle_key(&mut self, key_event: KeyEvent) {
        if key_event.kind == KeyEventKind::Release {
            return;
        }
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            self.finish();
            return;
        }
        if matches!(key_event.code, KeyCode::Enter | KeyCode::Esc) {
            self.finish();
        }
    }

    fn finish(&mut self) {
        self.done = true;
        self.request_frame.schedule_frame();
    }

    fn is_done(&self) -> bool {
        self.done
    }
}

impl WidgetRef for &RemediationPromptScreen {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = ColumnRenderable::new();

        column.push("");
        column.push(Line::from(vec![
            padded_emoji("  ✨").bold().cyan(),
            "Update needs attention".bold(),
        ]));
        column.push("");
        for line in self.reason.lines() {
            column.push(Line::from(line).inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )));
        }
        column.push("");
        column.push(
            Line::from(vec![
                "Press ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " to continue".dim(),
            ])
            .inset(Insets::tlbr(
                /*top*/ 0, /*left*/ 2, /*bottom*/ 0, /*right*/ 0,
            )),
        );
        column.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::VT100Backend;
    use crate::tui::FrameRequester;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;

    fn new_prompt() -> UpdatePromptScreen {
        UpdatePromptScreen::new(
            FrameRequester::test_dummy(),
            "9.9.9".into(),
            UpdateAction::NpmGlobalLatest,
        )
    }

    #[test]
    fn update_prompt_snapshot() {
        let screen = new_prompt();
        let mut terminal = Terminal::new(VT100Backend::new(80, 12)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render update prompt");
        insta::assert_snapshot!("update_prompt_modal", terminal.backend());
    }

    #[test]
    fn update_prompt_confirm_selects_update() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(screen.is_done());
        assert_eq!(screen.selection(), Some(UpdateSelection::UpdateNow));
    }

    #[test]
    fn update_prompt_dismiss_option_leaves_prompt_in_normal_state() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(screen.is_done());
        assert_eq!(screen.selection(), Some(UpdateSelection::NotNow));
    }

    #[test]
    fn update_prompt_dont_remind_selects_dismissal() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        screen.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(screen.is_done());
        assert_eq!(screen.selection(), Some(UpdateSelection::DontRemind));
    }

    #[test]
    fn update_prompt_ctrl_c_skips_update() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(screen.is_done());
        assert_eq!(screen.selection(), Some(UpdateSelection::NotNow));
    }

    #[test]
    fn update_prompt_navigation_wraps_between_entries() {
        let mut screen = new_prompt();
        screen.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(screen.highlighted, UpdateSelection::DontRemind);
        screen.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(screen.highlighted, UpdateSelection::UpdateNow);
    }

    #[test]
    fn update_remediation_prompt_snapshot() {
        let screen = RemediationPromptScreen::new(
            FrameRequester::test_dummy(),
            RemediationPromptReason::Blocked(UpdateBlocker::NpmGlobalRootMismatch {
                running_package_root: "/prefix-a/lib/node_modules/@openai/codex".into(),
                npm_package_root: "/prefix-b/lib/node_modules/@openai/codex".into(),
            }),
        );
        let mut terminal =
            Terminal::new(VT100Backend::new(/*width*/ 96, /*height*/ 12)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&screen, frame.area()))
            .expect("render update remediation prompt");
        insta::assert_snapshot!("update_remediation_prompt_modal", terminal.backend());
    }
}
