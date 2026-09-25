//! Explicit recovery from required-daemon incompatibility. Restart is managed-only,
//! requires a fresh confirmation, and is followed by one compatibility check, never a loop.

use crate::AppServerTarget;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::daemon_startup;
use crate::daemon_startup::CompatibilityError;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::config::Config;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::startup_draft::StartupDraft;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use std::io;
use std::io::IsTerminal;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::StreamExt;

pub(super) async fn check(
    startup: &mut StartupDraft,
    target: &AppServerTarget,
    config: &Config,
    managed_daemon: bool,
) -> io::Result<Option<String>> {
    let issue = match startup
        .run_until(daemon_startup::compatibility_warning(target, config))
        .await?
    {
        Ok(warning) => return Ok(warning),
        Err(issue) => issue,
    };
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Err(io::Error::other(issue));
    }
    startup.flush_pending_events().await?;
    let keymap = RuntimeKeymap::from_config(&config.tui_keymap).map_err(io::Error::other)?;
    let mut view = recovery_view(&issue, managed_daemon, &keymap);
    let mut chord_matcher = crate::keymap::KeyChordMatcher::default();
    let tui = startup.tui_mut();
    tui.discard_pending_input_before_interactive_screen()?;
    let selection = {
        let events = tui.event_stream();
        tokio::pin!(events);
        loop {
            let height = view.desired_height(tui.terminal.size()?.width);
            tui.draw(height, |frame| {
                view.render(frame.area(), frame.buffer_mut())
            })?;
            let Some(event) = events.next().await else {
                break None;
            };
            tui.screen_size_for_event(&event)?;
            if let crate::tui::TuiEvent::Key(key) = event
                && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'd'))
                {
                    break None;
                }
                let key = match chord_matcher.advance(
                    key,
                    &keymap.chords,
                    crate::keymap::KeymapContextSet::new(crate::keymap::KeymapContext::List),
                ) {
                    crate::keymap::KeyChordMatch::PassThrough => key,
                    crate::keymap::KeyChordMatch::Completed(key) => key,
                    crate::keymap::KeyChordMatch::Pending(_)
                    | crate::keymap::KeyChordMatch::Cancelled
                    | crate::keymap::KeyChordMatch::Ignored => continue,
                };
                view.handle_key_event(key);
            }
            if view.is_complete() {
                break view.take_last_selected_index();
            }
        }
    };
    tui.terminal.clear()?;
    match (selection, issue.restart_features.as_ref()) {
        (Some(0), _) => Ok(Some(format!(
            "Running without the shared background server: {}.",
            issue.reason
        ))),
        (Some(1), Some(features)) if managed_daemon => {
            tui.with_restored(crate::tui::TerminalHandoff::Restore, || async {
                crossterm::terminal::disable_raw_mode()?;
                codex_app_server_daemon::restart_with_features(features)
                    .await
                    .map_err(|err| {
                        io::Error::other(format!("{err:#}\n{}", daemon_startup::FAILURE_HINT))
                    })
            })
            .await?;
            startup
                .run_until(daemon_startup::compatibility_warning(target, config))
                .await?
                .map_err(io::Error::other)
        }
        (Some(_) | None, _) => Err(io::Error::other(issue)),
    }
}

fn recovery_view(
    issue: &CompatibilityError,
    managed_daemon: bool,
    keymap: &RuntimeKeymap,
) -> ListSelectionView {
    let mut header = ColumnRenderable::new();
    header.push(Line::from(
        if issue.restart_features.is_some() {
            "Background server has incompatible feature settings"
        } else {
            "Cannot use the background server"
        }
        .bold(),
    ));
    header.push(Paragraph::new(issue.reason.clone()).wrap(Wrap { trim: false }));
    if let Some(features) = &issue.restart_features {
        header.push(Line::from(
            "Restart will use these shared feature settings:",
        ));
        for (name, enabled) in features {
            header.push(Line::from(format!("  {name} = {enabled}").dim()));
        }
        header.push(Paragraph::new(
            "These settings persist and can disable functionality for other clients. Restart may interrupt active or queued work."
        ).wrap(Wrap { trim: false }));
    }
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    ListSelectionView::new(
        SelectionViewParams {
            header: Box::new(header),
            initial_selected_idx: Some(2),
            items: vec![
                SelectionItem {
                    name: "Run without daemon this time".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Restart with these settings".to_string(),
                    dismiss_on_select: true,
                    require_explicit_confirmation: true,
                    is_disabled: !managed_daemon || issue.restart_features.is_none(),
                    disabled_reason: if !managed_daemon {
                        Some("This server is not managed by Codex.".to_string())
                    } else if issue.restart_features.is_none() {
                        Some("Restart cannot resolve this compatibility check.".to_string())
                    } else {
                        None
                    },
                    ..Default::default()
                },
                SelectionItem {
                    name: "Cancel".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..SelectionViewParams::picker()
        },
        AppEventSender::new(tx),
        keymap.list.clone(),
    )
}

#[cfg(test)]
#[path = "daemon_recovery_tests.rs"]
mod tests;
