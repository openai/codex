//! Archive notifications freeze cached threads; external unarchive resumes the displayed thread.
//! Archive metadata is separate from attachment; archived channels only replay cached events.

use super::*;
use crate::chatwidget::ReadOnlyThread;

pub(super) fn update_archived_thread(
    app: &mut App,
    thread_id: ThreadId,
    notification: &ServerNotification,
) {
    let Some(channel) = app.thread_event_channels.get_mut(&thread_id) else {
        return;
    };
    match notification {
        ServerNotification::ThreadArchived(_) => {
            channel.archived = true;
            channel.mark_replay_only();
        }
        ServerNotification::ThreadUnarchived(_) if channel.archived => {
            channel.archived = false;
        }
        _ => return,
    }
    let resume = !channel.archived;
    if app.current_displayed_thread_id() == Some(thread_id) {
        show_cached_read_only_thread(app, thread_id);
        if resume {
            app.app_event_tx
                .send(AppEvent::ResumeUnarchivedThread(thread_id));
        }
    }
}

pub(super) async fn resume_displayed_thread(
    app: &mut App,
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
) {
    let target = SessionTarget {
        backend_id: app_server.backend_id(),
        path: app
            .primary_session_configured
            .as_ref()
            .and_then(|session| session.rollout_path.clone()),
        thread_id,
        cwd: None,
        history_mode: None,
    };
    if let Ok(AppRunControl::Exit(_)) = app.resume_target_session(tui, app_server, target).await {
        app.app_event_tx.send(AppEvent::Exit(ExitMode::Immediate));
    }
    // A writer conflict does not establish that the thread was unarchived.
    // Keep the archive flag if resume could not replace the archived view.
    if app.current_displayed_thread_id() == Some(thread_id)
        && !app.chat_widget.is_archived_view()
        && let Some(channel) = app.thread_event_channels.get_mut(&thread_id)
    {
        channel.archived = false;
    }
}

pub(super) fn show_cached_read_only_thread(app: &mut App, thread_id: ThreadId) {
    let Some(channel) = app.thread_event_channels.get(&thread_id) else {
        return;
    };
    let reason = if channel.archived {
        ReadOnlyThread::Archived
    } else if channel.attachment() == ThreadEventAttachment::ExternalWriter {
        ReadOnlyThread::ExternalWriter
    } else {
        return;
    };
    app.chat_widget.show_read_only_thread(reason);
}
