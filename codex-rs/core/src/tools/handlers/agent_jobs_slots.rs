use super::*;
use std::collections::HashMap;

#[derive(Debug)]
struct InactiveActiveItem {
    thread_id: ThreadId,
    item_id: String,
    db_state: &'static str,
    db_assigned_thread_id: Option<String>,
}

pub(super) async fn reclaim_inactive_active_items(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    db_running_items: usize,
) -> anyhow::Result<bool> {
    let running_items =
        db_ops::retry_locked("list_running_agent_job_items_for_reclaim", || async {
            db.list_agent_job_items(
                job_id,
                Some(codex_state::AgentJobItemStatus::Running),
                /*limit*/ None,
            )
            .await
        })
        .await?;
    let running_by_item_id: HashMap<_, _> = running_items
        .into_iter()
        .map(|item| (item.item_id, item.assigned_thread_id))
        .collect();

    let mut inactive_items = Vec::new();
    for (thread_id, item) in active_items.iter() {
        let thread_id_str = thread_id.to_string();
        let Some(db_assigned_thread_id) = running_by_item_id.get(item.item_id.as_str()) else {
            inactive_items.push(InactiveActiveItem {
                thread_id: *thread_id,
                item_id: item.item_id.clone(),
                db_state: "missing",
                db_assigned_thread_id: None,
            });
            continue;
        };
        let still_running = db_assigned_thread_id.as_deref() == Some(thread_id_str.as_str());
        if still_running {
            continue;
        }
        inactive_items.push(InactiveActiveItem {
            thread_id: *thread_id,
            item_id: item.item_id.clone(),
            db_state: "running",
            db_assigned_thread_id: db_assigned_thread_id.clone(),
        });
    }
    if inactive_items.is_empty() {
        return Ok(false);
    }

    tracing::info!(
        job_id,
        db_running_items,
        active_items = active_items.len(),
        reclaimed_items = inactive_items.len(),
        "agent job reclaiming scheduler slots for items that are no longer running in state"
    );

    for inactive_item in inactive_items {
        active_items.remove(&inactive_item.thread_id);
        request_live_agent_shutdown(
            session.services.agent_control.clone(),
            inactive_item.thread_id,
        );
        tracing::debug!(
            job_id,
            item_id = inactive_item.item_id,
            thread_id = %inactive_item.thread_id,
            db_status = inactive_item.db_state,
            db_assigned_thread_id = inactive_item.db_assigned_thread_id.as_deref().unwrap_or(""),
            active_items = active_items.len(),
            "agent job reclaimed scheduler slot"
        );
    }

    Ok(true)
}

pub(super) async fn reconcile_terminal_scheduler_state(
    session: Arc<Session>,
    job_id: &str,
    progress: &codex_state::AgentJobProgress,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    startup_tasks: &mut startup::StartupTasks,
) -> anyhow::Result<bool> {
    if active_items.is_empty() && startup_tasks.is_empty() {
        return Ok(false);
    }

    let active_count = active_items.len();
    let starting_count = startup_tasks.len();
    tracing::info!(
        job_id,
        pending_items = progress.pending_items,
        db_running_items = progress.running_items,
        active_items = active_count,
        starting_items = starting_count,
        "agent job state is terminal in DB; forcing scheduler teardown"
    );

    let thread_ids: Vec<_> = active_items.keys().copied().collect();
    for thread_id in thread_ids {
        active_items.remove(&thread_id);
        request_live_agent_shutdown(session.services.agent_control.clone(), thread_id);
    }

    let aborted_startups = startup::abort_all_startups(startup_tasks).await;
    tracing::debug!(
        job_id,
        active_items_reclaimed = active_count,
        starting_items_aborted = aborted_startups,
        "agent job terminal scheduler teardown completed"
    );
    Ok(active_count > 0 || aborted_startups > 0)
}
