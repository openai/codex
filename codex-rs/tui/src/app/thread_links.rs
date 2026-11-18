//! Resolve linked children into their owning session before selecting the requested transcript.
//! Parent metadata is read without loading threads; only the resolved root is attached as a task.

use super::*;
use codex_app_server_protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;

pub(super) async fn open_thread_link(
    app: &mut App,
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
    thread_id: ThreadId,
) -> Result<AppRunControl> {
    if app.current_displayed_thread_id() == Some(thread_id)
        || app.windows_sandbox_blocks_thread_switch()
        || app.reject_pending_permission_root_switch()
    {
        return Ok(AppRunControl::Continue);
    }
    let mut root = thread_id;
    let mut ancestors = Vec::new();
    let root_session = loop {
        if ancestors.contains(&root) || ancestors.len() >= 64 {
            app.add_agents_overview_error("Invalid linked thread ancestry.".to_string());
            return Ok(AppRunControl::Continue);
        }
        ancestors.push(root);
        let thread = match app_server.thread_read(root, /*include_turns*/ false).await {
            Ok(thread) => thread,
            Err(error) => {
                app.add_agents_overview_error(format!(
                    "Linked conversation is unavailable: {error}"
                ));
                return Ok(AppRunControl::Continue);
            }
        };
        if thread.ephemeral {
            app.add_agents_overview_error("This conversation has no saved transcript.".to_string());
            return Ok(AppRunControl::Continue);
        }
        let parent = thread.parent_thread_id.or_else(|| match thread.source {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id, ..
            }) => Some(parent_thread_id.to_string()),
            _ => None,
        });
        let Some(parent) = parent else {
            break SessionTarget {
                backend_id: app_server.backend_id(),
                path: thread.path,
                thread_id: root,
                cwd: Some(thread.cwd.to_path_buf()),
                history_mode: Some(thread.history_mode),
            };
        };
        match ThreadId::from_string(&parent) {
            Ok(parent) => root = parent,
            Err(error) => {
                app.add_agents_overview_error(format!("Invalid linked thread parent: {error}"));
                return Ok(AppRunControl::Continue);
            }
        }
    };
    if app.primary_thread_id != Some(root) || thread_id == root {
        // Keep the requested child on this stack until its cloud root has attached.
        let control = if thread_id != root
            && app.backend_registry.owner(root) == Some(app_server.backend_id())
            && app_server.thread_params_mode() == crate::app_server_session::ThreadParamsMode::Cloud
            && !app.chat_widget.fork_in_progress
        {
            app.apply_resume_picker_selection(
                tui,
                app_server,
                SessionSelection::Resume(root_session),
            )
            .await?
        } else {
            app.select_agents_overview_thread(tui, app_server, root)
                .await?
        };
        if !matches!(control, AppRunControl::Continue) {
            return Ok(control);
        }
    }
    if thread_id != root && app.primary_thread_id == Some(root) {
        for ancestor in ancestors.into_iter().rev().skip(1) {
            if ancestor != thread_id
                && app.agent_navigation.get(&ancestor).is_none()
                && !app
                    .refresh_agent_picker_thread_liveness(app_server, ancestor)
                    .await
            {
                app.add_agents_overview_error(format!("Linked agent {ancestor} is unavailable."));
                return Ok(AppRunControl::Continue);
            }
            if let Err(error) = app
                .attach_live_thread_for_selection(app_server, ancestor)
                .await
            {
                app.add_agents_overview_error(format!("Linked agent is unavailable: {error}"));
                return Ok(AppRunControl::Continue);
            }
        }
        app.select_agent_thread_and_discard_side(tui, app_server, thread_id)
            .await?;
    }
    Ok(AppRunControl::Continue)
}
