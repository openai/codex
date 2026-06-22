use super::*;
use crate::bottom_pane::preview_line_for_title_items;
use pretty_assertions::assert_eq;
use ratatui::text::Line;

fn line_text(line: Line<'static>) -> String {
    line.spans
        .into_iter()
        .map(|span| span.content.into_owned())
        .collect()
}

fn status_preview_line_option(chat: &mut ChatWidget, items: &[StatusLineItem]) -> Option<String> {
    let preview_data = chat.status_surface_preview_data();
    preview_data
        .status_line_for_items(items.iter().copied(), /*use_theme_colors*/ true)
        .map(line_text)
}

fn status_preview_line(chat: &mut ChatWidget, items: &[StatusLineItem]) -> String {
    status_preview_line_option(chat, items).expect("status preview line")
}

fn title_preview_line(chat: &mut ChatWidget, items: &[TerminalTitleItem]) -> String {
    let preview_data = chat.terminal_title_preview_data();
    let preview =
        preview_line_for_title_items(items, &preview_data).expect("terminal title preview line");
    line_text(preview)
}

fn combined_preview_snapshot(
    chat: &mut ChatWidget,
    status_items: &[StatusLineItem],
    title_items: &[TerminalTitleItem],
) -> String {
    normalize_snapshot_paths(format!(
        "status line: {}\nterminal title: {}",
        status_preview_line(chat, status_items),
        title_preview_line(chat, title_items),
    ))
}

fn status_line_popup_snapshot(chat: &mut ChatWidget) -> String {
    chat.open_status_line_setup();
    normalize_snapshot_paths(strip_osc8_for_snapshot(&render_bottom_popup(
        chat, /*width*/ 100,
    )))
}

fn terminal_title_popup_snapshot(chat: &mut ChatWidget) -> String {
    chat.open_terminal_title_setup();
    normalize_snapshot_paths(strip_osc8_for_snapshot(&render_bottom_popup(
        chat, /*width*/ 100,
    )))
}

fn cache_project_root(chat: &mut ChatWidget, root_name: &str) {
    chat.status_line_project_root_name_cache = Some(CachedProjectRootName {
        cwd: chat.config.cwd.to_path_buf(),
        root_name: Some(root_name.to_string()),
    });
}

fn cache_rate_limit_snapshot(chat: &mut ChatWidget) {
    chat.on_rate_limit_snapshot(Some(RateLimitSnapshot {
        limit_id: None,
        limit_name: None,
        primary: Some(RateLimitWindow {
            used_percent: 35,
            window_duration_mins: Some(30 * 24 * 60),
            resets_at: None,
        }),
        secondary: Some(RateLimitWindow {
            used_percent: 50,
            window_duration_mins: Some(7 * 24 * 60),
            resets_at: None,
        }),
        credits: None,
        individual_limit: None,
        plan_type: None,
        rate_limit_reached_type: None,
    }));
}

#[tokio::test]
async fn status_surface_preview_lines_live_only_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    cache_project_root(&mut chat, "preview-live-root");
    chat.status_line_branch = Some("feature/live-preview-branch".to_string());
    chat.thread_name = Some("Live preview thread".to_string());
    chat.transcript.last_plan_progress = Some((2, 5));

    let snapshot = combined_preview_snapshot(
        &mut chat,
        &[
            StatusLineItem::ProjectRoot,
            StatusLineItem::GitBranch,
            StatusLineItem::ThreadTitle,
        ],
        &[
            TerminalTitleItem::Project,
            TerminalTitleItem::Thread,
            TerminalTitleItem::GitBranch,
            TerminalTitleItem::TaskProgress,
        ],
    );

    assert_chatwidget_snapshot!("status_surface_previews_live_only", snapshot);
}

#[tokio::test]
async fn status_line_setup_popup_live_only_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    cache_project_root(&mut chat, "preview-live-root");
    chat.status_line_branch = Some("feature/live-preview-branch".to_string());
    chat.thread_name = Some("Live preview thread".to_string());
    chat.config.tui_status_line = Some(vec![
        "project-name".to_string(),
        "git-branch".to_string(),
        "thread-title".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "status_line_setup_popup_live_only",
        status_line_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn status_surface_preview_lines_hardcoded_only_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let snapshot = combined_preview_snapshot(
        &mut chat,
        &[
            StatusLineItem::ProjectRoot,
            StatusLineItem::GitBranch,
            StatusLineItem::ThreadTitle,
            StatusLineItem::Permissions,
            StatusLineItem::ApprovalMode,
        ],
        &[
            TerminalTitleItem::Thread,
            TerminalTitleItem::GitBranch,
            TerminalTitleItem::TaskProgress,
        ],
    );

    assert_chatwidget_snapshot!("status_surface_previews_hardcoded_only", snapshot);
}

#[tokio::test]
async fn thread_title_falls_back_to_thread_id_when_unnamed() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);

    assert_eq!(
        status_preview_line(&mut chat, &[StatusLineItem::ThreadTitle]),
        thread_id.to_string()
    );
    assert_eq!(
        title_preview_line(&mut chat, &[TerminalTitleItem::Thread]),
        thread_id.to_string()
    );
}

#[tokio::test]
async fn status_line_setup_popup_hardcoded_only_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.tui_status_line = Some(vec![
        "project-name".to_string(),
        "git-branch".to_string(),
        "thread-title".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "status_line_setup_popup_hardcoded_only",
        status_line_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn status_line_setup_popup_workspace_headline_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.status_line_workspace_headline = Some("Workspace maintenance starts at 5pm".to_string());
    chat.config.tui_status_line = Some(vec!["workspace-headline".to_string()]);

    assert_chatwidget_snapshot!(
        "status_line_setup_popup_workspace_headline",
        status_line_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn status_surface_preview_lines_mixed_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.status_line_branch = Some("feature/mixed-preview".to_string());
    chat.thread_name = Some("Mixed preview thread".to_string());

    let snapshot = combined_preview_snapshot(
        &mut chat,
        &[
            StatusLineItem::ProjectRoot,
            StatusLineItem::GitBranch,
            StatusLineItem::ThreadTitle,
        ],
        &[
            TerminalTitleItem::Project,
            TerminalTitleItem::Thread,
            TerminalTitleItem::TaskProgress,
        ],
    );

    assert_chatwidget_snapshot!("status_surface_previews_mixed", snapshot);
}

#[tokio::test]
async fn status_surface_preview_lines_rate_limits_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    cache_rate_limit_snapshot(&mut chat);

    let snapshot = combined_preview_snapshot(
        &mut chat,
        &[StatusLineItem::FiveHourLimit, StatusLineItem::WeeklyLimit],
        &[
            TerminalTitleItem::FiveHourLimit,
            TerminalTitleItem::WeeklyLimit,
        ],
    );

    assert_chatwidget_snapshot!("status_surface_previews_rate_limits", snapshot);
}

#[tokio::test]
async fn status_surface_preview_omits_unavailable_rate_limit_items() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.on_rate_limit_snapshot(Some(RateLimitSnapshot {
        limit_id: None,
        limit_name: None,
        primary: Some(RateLimitWindow {
            used_percent: 9,
            window_duration_mins: Some(7 * 24 * 60),
            resets_at: None,
        }),
        secondary: None,
        credits: None,
        individual_limit: None,
        plan_type: None,
        rate_limit_reached_type: None,
    }));

    assert_eq!(
        status_preview_line_option(&mut chat, &[StatusLineItem::FiveHourLimit]),
        None
    );
    assert_eq!(
        status_preview_line(
            &mut chat,
            &[StatusLineItem::FiveHourLimit, StatusLineItem::WeeklyLimit]
        ),
        "weekly 91% left"
    );
    assert_eq!(
        title_preview_line(
            &mut chat,
            &[
                TerminalTitleItem::FiveHourLimit,
                TerminalTitleItem::WeeklyLimit
            ],
        ),
        "weekly 91% left"
    );
}

#[tokio::test]
async fn status_line_setup_popup_rate_limits_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    cache_rate_limit_snapshot(&mut chat);
    chat.config.tui_status_line = Some(vec![
        "five-hour-limit".to_string(),
        "weekly-limit".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "status_line_setup_popup_rate_limits",
        status_line_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn status_line_setup_popup_mixed_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.status_line_branch = Some("feature/mixed-preview".to_string());
    chat.thread_name = Some("Mixed preview thread".to_string());
    chat.config.tui_status_line = Some(vec![
        "project-name".to_string(),
        "git-branch".to_string(),
        "thread-title".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "status_line_setup_popup_mixed",
        status_line_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn terminal_title_setup_popup_live_only_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    cache_project_root(&mut chat, "preview-live-root");
    chat.status_line_branch = Some("feature/live-preview-branch".to_string());
    chat.thread_name = Some("Live preview thread".to_string());
    chat.transcript.last_plan_progress = Some((2, 5));
    chat.config.tui_terminal_title = Some(vec![
        "project-name".to_string(),
        "thread-title".to_string(),
        "git-branch".to_string(),
        "task-progress".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "terminal_title_setup_popup_live_only",
        terminal_title_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn terminal_title_setup_popup_hardcoded_only_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.tui_terminal_title = Some(vec![
        "thread-title".to_string(),
        "git-branch".to_string(),
        "task-progress".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "terminal_title_setup_popup_hardcoded_only",
        terminal_title_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn terminal_title_setup_popup_mixed_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_name = Some("Mixed preview thread".to_string());
    chat.config.tui_terminal_title = Some(vec![
        "project-name".to_string(),
        "thread-title".to_string(),
        "task-progress".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "terminal_title_setup_popup_mixed",
        terminal_title_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn terminal_title_setup_popup_rate_limits_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    cache_rate_limit_snapshot(&mut chat);
    chat.config.tui_terminal_title = Some(vec![
        "five-hour-limit".to_string(),
        "weekly-limit".to_string(),
    ]);

    assert_chatwidget_snapshot!(
        "terminal_title_setup_popup_rate_limits",
        terminal_title_popup_snapshot(&mut chat)
    );
}

#[tokio::test]
async fn missing_project_root_uses_different_status_and_title_preview_sources() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let status_preview = status_preview_line(&mut chat, &[StatusLineItem::ProjectRoot]);
    let title_preview = title_preview_line(&mut chat, &[TerminalTitleItem::Project]);

    assert_eq!(status_preview, "my-project");
    assert_eq!(title_preview, "project");
}

#[tokio::test]
async fn terminal_title_preview_uses_title_truncation_for_live_values() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let long_thread = "This thread title is intentionally much longer than forty-eight characters";
    let long_branch = "feature/this-branch-name-is-intentionally-longer-than-thirty-two";
    chat.thread_name = Some(long_thread.to_string());
    chat.status_line_branch = Some(long_branch.to_string());

    let preview = title_preview_line(
        &mut chat,
        &[TerminalTitleItem::Thread, TerminalTitleItem::GitBranch],
    );
    let truncated_thread =
        ChatWidget::truncate_terminal_title_part(long_thread.to_string(), /*max_chars*/ 48);
    let truncated_branch =
        ChatWidget::truncate_terminal_title_part(long_branch.to_string(), /*max_chars*/ 32);

    assert_eq!(preview, format!("{truncated_thread} | {truncated_branch}"));
}
