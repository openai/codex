use codex_protocol::ThreadId;
use codex_protocol::protocol::validate_thread_goal_objective;
use codex_state::StateRuntime;

pub(super) async fn inherit_thread_goal_snapshot(
    state_db: &StateRuntime,
    source_thread_id: ThreadId,
    target_thread_id: ThreadId,
) -> anyhow::Result<bool> {
    let Some(mut goal) = state_db
        .thread_goals()
        .get_thread_goal(source_thread_id)
        .await?
    else {
        return Ok(false);
    };
    if let Err(err) = validate_thread_goal_objective(&goal.objective) {
        tracing::warn!(%source_thread_id, "skipping invalid inherited thread goal: {err}");
        return Ok(false);
    }

    goal.thread_id = target_thread_id;
    state_db
        .thread_goals()
        .replace_thread_goal_snapshot(&goal)
        .await?;
    Ok(true)
}
