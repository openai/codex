use color_eyre::eyre::Result;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::WidgetRef;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::StreamExt;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::app_server_session::AppServerSession;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::ListSelectionView;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line_for_keymap;
use crate::hooks_rpc::HookTrustUpdate;
use crate::hooks_rpc::fetch_hooks_list;
use crate::hooks_rpc::hook_needs_review;
use crate::hooks_rpc::hooks_list_entry_for_cwd;
use crate::hooks_rpc::write_hook_trusts;
use crate::keymap::RuntimeKeymap;
use crate::legacy_core::config::Config;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use codex_app_server_protocol::HooksListEntry;

pub(crate) enum StartupHooksReviewOutcome {
    Continue,
    OpenHooksBrowser(HooksListEntry),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupHooksReviewSelection {
    ReviewHooks,
    TrustAllAndContinue,
    ContinueWithoutTrusting,
}

pub(crate) async fn maybe_run_startup_hooks_review(
    app_server: &mut AppServerSession,
    tui: &mut Tui,
    config: &Config,
) -> Result<StartupHooksReviewOutcome> {
    let cwd = config.cwd.to_path_buf();
    let response = match fetch_hooks_list(app_server.request_handle(), cwd.clone()).await {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!("failed to load startup hook review state: {err:#}");
            return Ok(StartupHooksReviewOutcome::Continue);
        }
    };
    let entry = hooks_list_entry_for_cwd(response, &cwd);
    if review_needed_count(&entry) == 0 {
        return Ok(StartupHooksReviewOutcome::Continue);
    }

    run_startup_hooks_review_app(app_server, tui, config, entry).await
}

async fn run_startup_hooks_review_app(
    app_server: &mut AppServerSession,
    tui: &mut Tui,
    config: &Config,
    entry: HooksListEntry,
) -> Result<StartupHooksReviewOutcome> {
    let keymap = RuntimeKeymap::from_config(&config.tui_keymap)
        .map_err(|err| color_eyre::eyre::eyre!(err))?;
    let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
    let app_event_tx = AppEventSender::new(tx_raw);
    let mut trust_all_error = None;
    let mut view = selection_view(
        &entry,
        trust_all_error.as_deref(),
        /*trusting_all*/ false,
        app_event_tx.clone(),
        &keymap,
    );
    draw_view(tui, &view)?;

    let tui_events = tui.event_stream();
    tokio::pin!(tui_events);

    loop {
        let Some(event) = tui_events.next().await else {
            return Ok(StartupHooksReviewOutcome::Continue);
        };
        match event {
            TuiEvent::Key(key_event) => {
                if matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    view.handle_key_event(key_event);
                }
                let Some(selection) = selected_choice(&mut view) else {
                    draw_view(tui, &view)?;
                    continue;
                };
                match selection {
                    StartupHooksReviewSelection::ReviewHooks => {
                        return Ok(StartupHooksReviewOutcome::OpenHooksBrowser(entry));
                    }
                    StartupHooksReviewSelection::ContinueWithoutTrusting => {
                        return Ok(StartupHooksReviewOutcome::Continue);
                    }
                    StartupHooksReviewSelection::TrustAllAndContinue => {
                        view = selection_view(
                            &entry,
                            trust_all_error.as_deref(),
                            /*trusting_all*/ true,
                            app_event_tx.clone(),
                            &keymap,
                        );
                        draw_view(tui, &view)?;
                        let result = write_hook_trusts(
                            app_server.request_handle(),
                            entry
                                .hooks
                                .iter()
                                .filter(|hook| hook_needs_review(hook))
                                .map(|hook| HookTrustUpdate {
                                    key: hook.key.clone(),
                                    current_hash: hook.current_hash.clone(),
                                })
                                .collect(),
                        )
                        .await
                        .map(|_| ())
                        .map_err(|err| format!("Failed to trust hooks: {err}"));
                        match result {
                            Ok(()) => return Ok(StartupHooksReviewOutcome::Continue),
                            Err(err) => {
                                trust_all_error = Some(err);
                                view = selection_view(
                                    &entry,
                                    trust_all_error.as_deref(),
                                    /*trusting_all*/ false,
                                    app_event_tx.clone(),
                                    &keymap,
                                );
                                draw_view(tui, &view)?;
                            }
                        }
                    }
                }
            }
            TuiEvent::Paste(_) => {}
            TuiEvent::Draw | TuiEvent::Resize => draw_view(tui, &view)?,
        }
    }
}

fn selected_choice(view: &mut ListSelectionView) -> Option<StartupHooksReviewSelection> {
    if !view.is_complete() {
        return None;
    }
    match view.take_last_selected_index() {
        Some(0) => Some(StartupHooksReviewSelection::ReviewHooks),
        Some(1) => Some(StartupHooksReviewSelection::TrustAllAndContinue),
        Some(2) | None => Some(StartupHooksReviewSelection::ContinueWithoutTrusting),
        Some(_) => None,
    }
}

fn selection_view(
    entry: &HooksListEntry,
    trust_all_error: Option<&str>,
    trusting_all: bool,
    app_event_tx: AppEventSender,
    keymap: &RuntimeKeymap,
) -> ListSelectionView {
    ListSelectionView::new(
        selection_view_params(entry, trust_all_error, trusting_all, keymap),
        app_event_tx,
        keymap.list.clone(),
    )
}

#[allow(clippy::disallowed_methods)]
fn selection_view_params(
    entry: &HooksListEntry,
    trust_all_error: Option<&str>,
    trusting_all: bool,
    keymap: &RuntimeKeymap,
) -> SelectionViewParams {
    let count = review_needed_count(entry);
    let count_line = match count {
        1 => "1 hook is new or changed.".to_string(),
        count => format!("{count} hooks are new or changed."),
    };
    let mut header = ColumnRenderable::new();
    header.push(Line::from("Hooks need review".bold()));
    header.push(Line::from(count_line).yellow());
    header.push(Line::from(
        "Hooks can run outside the sandbox after you trust them.".dim(),
    ));
    if let Some(error) = trust_all_error {
        header.push(Line::from(error.to_string()).red());
    } else if trusting_all {
        header.push(Line::from("Trusting hooks...".dim()));
    }

    SelectionViewParams {
        footer_hint: Some(standard_popup_hint_line_for_keymap(&keymap.list)),
        items: vec![
            selection_item("Review hooks", trusting_all),
            selection_item("Trust all and continue", trusting_all),
            selection_item("Continue without trusting (hooks won't run)", trusting_all),
        ],
        header: Box::new(header),
        ..Default::default()
    }
}

fn review_needed_count(entry: &HooksListEntry) -> usize {
    entry
        .hooks
        .iter()
        .filter(|hook| hook_needs_review(hook))
        .count()
}

fn selection_item(name: &str, is_disabled: bool) -> SelectionItem {
    SelectionItem {
        name: name.to_string(),
        dismiss_on_select: true,
        is_disabled,
        ..Default::default()
    }
}

fn draw_view(tui: &mut Tui, view: &ListSelectionView) -> Result<()> {
    tui.draw(u16::MAX, |frame| {
        let area = frame.area();
        frame.render_widget_ref(Clear, area);
        let view_area = Rect::new(
            area.x,
            area.y,
            area.width,
            view.desired_height(area.width).min(area.height),
        );
        frame.render_widget_ref(&StandaloneSelectionView { view }, view_area);
    })?;
    Ok(())
}

struct StandaloneSelectionView<'a> {
    view: &'a ListSelectionView,
}

impl WidgetRef for &StandaloneSelectionView<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.view.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::selection_view;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;
    use crate::keymap::RuntimeKeymap;
    use crate::render::renderable::Renderable;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use codex_app_server_protocol::HookEventName;
    use codex_app_server_protocol::HookHandlerType;
    use codex_app_server_protocol::HookMetadata;
    use codex_app_server_protocol::HookSource;
    use codex_app_server_protocol::HookTrustStatus;
    use codex_app_server_protocol::HooksListEntry;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::unbounded_channel;

    fn hook(key: &str, trust_status: HookTrustStatus) -> HookMetadata {
        HookMetadata {
            key: key.to_string(),
            event_name: HookEventName::PreToolUse,
            handler_type: HookHandlerType::Command,
            is_managed: false,
            matcher: Some("Bash".to_string()),
            command: Some("/tmp/hook.sh".to_string()),
            timeout_sec: 30,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: HookSource::User,
            plugin_id: None,
            display_order: 0,
            enabled: false,
            current_hash: format!("sha256:{key}"),
            trust_status,
        }
    }

    fn entry() -> HooksListEntry {
        HooksListEntry {
            cwd: test_path_buf("/tmp"),
            hooks: vec![
                hook("path:new", HookTrustStatus::Untrusted),
                hook("path:changed", HookTrustStatus::Modified),
            ],
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn render_lines(view: &crate::bottom_pane::ListSelectionView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        (0..area.height)
            .map(|row| {
                let rendered = (0..area.width)
                    .map(|col| {
                        let symbol = buf[(area.x + col, area.y + row)].symbol();
                        if symbol.is_empty() {
                            " ".to_string()
                        } else {
                            symbol.to_string()
                        }
                    })
                    .collect::<String>();
                format!("{rendered:width$}", width = area.width as usize)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_prompt() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let keymap = RuntimeKeymap::defaults();
        let view = selection_view(
            &entry(),
            /*trust_all_error*/ None,
            /*trusting_all*/ false,
            AppEventSender::new(tx_raw),
            &keymap,
        );

        assert_snapshot!(
            "startup_hooks_review_prompt",
            render_lines(&view, /*width*/ 80)
        );
    }

    #[test]
    fn renders_prompt_with_trust_error() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let keymap = RuntimeKeymap::defaults();
        let view = selection_view(
            &entry(),
            Some("Failed to trust hooks: disk full"),
            /*trusting_all*/ false,
            AppEventSender::new(tx_raw),
            &keymap,
        );

        assert_snapshot!(
            "startup_hooks_review_prompt_with_trust_error",
            render_lines(&view, /*width*/ 80)
        );
    }
}
