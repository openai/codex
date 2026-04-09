use super::*;
use std::collections::HashMap;

#[derive(Debug)]
struct InactiveActiveItem {
    thread_id: ThreadId,
    item_id: String,
    db_status: Option<codex_state::AgentJobItemStatus>,
    db_assigned_thread_id: Option<String>,
}

pub(super) async fn reclaim_inactive_active_items(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    db_running_items: usize,
) -> anyhow::Result<bool> {
    let mut inactive_items = Vec::new();
    for (thread_id, item) in active_items.iter() {
        let thread_id_str = thread_id.to_string();
        let db_item = db.get_agent_job_item(job_id, item.item_id.as_str()).await?;
        let Some(db_item) = db_item else {
            inactive_items.push(InactiveActiveItem {
                thread_id: *thread_id,
                item_id: item.item_id.clone(),
                db_status: None,
                db_assigned_thread_id: None,
            });
            continue;
        };
        let still_running = matches!(db_item.status, codex_state::AgentJobItemStatus::Running)
            && db_item.assigned_thread_id.as_deref() == Some(thread_id_str.as_str());
        if still_running {
            continue;
        }
        inactive_items.push(InactiveActiveItem {
            thread_id: *thread_id,
            item_id: item.item_id.clone(),
            db_status: Some(db_item.status),
            db_assigned_thread_id: db_item.assigned_thread_id,
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
        let _ = session
            .services
            .agent_control
            .shutdown_live_agent(inactive_item.thread_id)
            .await;
        active_items.remove(&inactive_item.thread_id);
        tracing::debug!(
            job_id,
            item_id = inactive_item.item_id,
            thread_id = %inactive_item.thread_id,
            db_status = inactive_item
                .db_status
                .as_ref()
                .map(|status| status.as_str())
                .unwrap_or("missing"),
            db_assigned_thread_id = inactive_item.db_assigned_thread_id.as_deref().unwrap_or(""),
            active_items = active_items.len(),
            "agent job reclaimed scheduler slot"
        );
    }

    Ok(true)
}
