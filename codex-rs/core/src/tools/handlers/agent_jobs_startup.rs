use super::*;
use std::collections::HashMap;
use std::future::Future;
use tokio::task::AbortHandle;
use tokio::task::Id as TaskId;
use tokio::task::JoinError;
use tokio::task::JoinSet;

#[derive(Debug)]
pub(super) struct WorkerStartup {
    pub(super) item_id: String,
    pub(super) started_at: Instant,
    pub(super) spawn_latency: Duration,
    pub(super) result: Result<ThreadId, CodexErr>,
}

#[derive(Debug)]
pub(super) struct LaunchingJobItem {
    item_id: String,
    started_at: Instant,
    abort_handle: AbortHandle,
}

#[derive(Debug, Default)]
pub(super) struct StartupTasks {
    starting_items: JoinSet<WorkerStartup>,
    launching_items: HashMap<TaskId, LaunchingJobItem>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StartupDrainResult {
    pub(super) progressed: bool,
    pub(super) agent_limit_reached: bool,
}

impl StartupDrainResult {
    fn merge(&mut self, other: Self) {
        self.progressed |= other.progressed;
        self.agent_limit_reached |= other.agent_limit_reached;
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SchedulerOccupancy {
    pub(super) active_items: usize,
    pub(super) db_pending_items: usize,
    pub(super) db_running_items: usize,
}

impl StartupTasks {
    pub(super) fn len(&self) -> usize {
        self.starting_items.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.starting_items.is_empty()
    }
}

fn spawn_tracked_startup_task<F>(
    startup_tasks: &mut StartupTasks,
    item_id: String,
    started_at: Instant,
    task: F,
) where
    F: Future<Output = WorkerStartup> + Send + 'static,
{
    let abort_handle = startup_tasks.starting_items.spawn(task);
    startup_tasks.launching_items.insert(
        abort_handle.id(),
        LaunchingJobItem {
            item_id,
            started_at,
            abort_handle,
        },
    );
}

pub(super) async fn launch_pending_items(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job: &codex_state::AgentJob,
    job_id: &str,
    options: &JobRunnerOptions,
    occupancy: SchedulerOccupancy,
    startup_tasks: &mut StartupTasks,
) -> anyhow::Result<bool> {
    let slots = options
        .max_concurrency
        .saturating_sub(occupancy.active_items + startup_tasks.len());
    if slots == 0 {
        return Ok(false);
    }

    let pending_items = db_ops::retry_locked("list_pending_agent_job_items_for_launch", || async {
        db.list_agent_job_items(
            job_id,
            Some(codex_state::AgentJobItemStatus::Pending),
            Some(slots),
        )
        .await
    })
    .await?;

    let mut launched = 0usize;
    let mut progressed = false;
    for item in pending_items {
        let claimed = db_ops::retry_locked("mark_agent_job_item_running_for_launch", || async {
            db.mark_agent_job_item_running(job_id, item.item_id.as_str())
                .await
        })
        .await?;
        if !claimed {
            continue;
        }

        let prompt = match build_worker_prompt(job, &item) {
            Ok(prompt) => prompt,
            Err(err) => {
                let error_message = format!("failed to build worker prompt: {err}");
                db_ops::retry_locked("mark_agent_job_item_failed_for_prompt_build", || async {
                    db.mark_agent_job_item_failed(
                        job_id,
                        item.item_id.as_str(),
                        error_message.as_str(),
                    )
                    .await
                })
                .await?;
                progressed = true;
                continue;
            }
        };

        let item_id = item.item_id.clone();
        let session = session.clone();
        let spawn_config = options.spawn_config.clone();
        let session_source =
            SessionSource::SubAgent(SubAgentSource::Other(format!("agent_job:{job_id}")));
        let started_at = Instant::now();
        spawn_tracked_startup_task(startup_tasks, item_id.clone(), started_at, async move {
            let items = vec![UserInput::Text {
                text: prompt,
                text_elements: Vec::new(),
            }];
            let result = session
                .services
                .agent_control
                .spawn_agent(spawn_config, items.into(), Some(session_source))
                .await;
            WorkerStartup {
                item_id,
                started_at,
                spawn_latency: started_at.elapsed(),
                result,
            }
        });
        launched = launched.saturating_add(1);
        progressed = true;
    }

    if launched > 0 {
        tracing::info!(
            job_id,
            launched,
            db_pending_items = occupancy.db_pending_items,
            db_running_items = occupancy.db_running_items,
            active_items = occupancy.active_items,
            starting_items = startup_tasks.len(),
            target_concurrency = options.max_concurrency,
            "agent job queued worker startups"
        );
    }
    Ok(progressed)
}

pub(super) async fn drain_ready_startups(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    startup_tasks: &mut StartupTasks,
) -> anyhow::Result<StartupDrainResult> {
    let mut drain_result = StartupDrainResult::default();
    while let Some(join_result) = startup_tasks.starting_items.try_join_next_with_id() {
        let starting_items_len = startup_tasks.starting_items.len();
        let startup_result = handle_worker_startup_result(
            session.clone(),
            db.clone(),
            job_id,
            active_items,
            startup_tasks,
            join_result,
            starting_items_len,
        )
        .await?;
        drain_result.merge(startup_result);
    }
    Ok(drain_result)
}

pub(super) async fn wait_for_startup_or_status_change(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    startup_tasks: &mut StartupTasks,
) -> anyhow::Result<()> {
    if startup_tasks.is_empty() {
        wait_for_status_change(active_items).await;
        return Ok(());
    }

    let active_items_ref = &*active_items;
    if active_items_ref.is_empty() {
        if let Some(result) = startup_tasks.starting_items.join_next_with_id().await {
            let starting_items_len = startup_tasks.starting_items.len();
            handle_worker_startup_result(
                session,
                db,
                job_id,
                active_items,
                startup_tasks,
                result,
                starting_items_len,
            )
            .await?;
        }
        return Ok(());
    }

    tokio::select! {
        startup = startup_tasks.starting_items.join_next_with_id() => {
            if let Some(result) = startup {
                let starting_items_len = startup_tasks.starting_items.len();
                handle_worker_startup_result(
                    session,
                    db,
                    job_id,
                    active_items,
                    startup_tasks,
                    result,
                    starting_items_len,
                )
                .await?;
            }
        }
        _ = wait_for_status_change(active_items_ref) => {}
    }
    Ok(())
}

pub(super) async fn abort_all_startups(startup_tasks: &mut StartupTasks) -> usize {
    let startup_count = startup_tasks.starting_items.len();
    if startup_count == 0 {
        startup_tasks.launching_items.clear();
        return 0;
    }

    for launching_item in startup_tasks.launching_items.values() {
        launching_item.abort_handle.abort();
    }
    startup_tasks.launching_items.clear();

    while startup_tasks.starting_items.join_next().await.is_some() {}
    startup_count
}

pub(super) async fn reap_stale_startups(
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    startup_tasks: &mut StartupTasks,
    runtime_timeout: Duration,
) -> anyhow::Result<bool> {
    let stale_task_ids: Vec<_> = startup_tasks
        .launching_items
        .iter()
        .filter_map(|(task_id, item)| {
            (item.started_at.elapsed() >= runtime_timeout).then_some(*task_id)
        })
        .collect();
    if stale_task_ids.is_empty() {
        return Ok(false);
    }

    for task_id in stale_task_ids {
        let Some(item) = startup_tasks.launching_items.remove(&task_id) else {
            continue;
        };
        item.abort_handle.abort();
        let error_message =
            format!("worker exceeded max runtime of {runtime_timeout:?} before startup completed");
        db_ops::retry_locked("mark_agent_job_item_failed_for_stale_startup", || async {
            db.mark_agent_job_item_failed(job_id, item.item_id.as_str(), error_message.as_str())
                .await
        })
        .await?;
        tracing::warn!(
            job_id,
            item_id = item.item_id,
            ?task_id,
            "agent job worker startup timed out"
        );
    }
    Ok(true)
}

async fn handle_worker_startup_result(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    startup_tasks: &mut StartupTasks,
    result: Result<(TaskId, WorkerStartup), JoinError>,
    starting_items_len: usize,
) -> anyhow::Result<StartupDrainResult> {
    match result {
        Ok((task_id, startup)) => {
            startup_tasks.launching_items.remove(&task_id);
            match startup.result {
                Ok(thread_id) => {
                    let thread_id_str = thread_id.to_string();
                    let assigned =
                        db_ops::retry_locked("set_agent_job_item_thread_after_startup", || async {
                            db.set_agent_job_item_thread(
                                job_id,
                                startup.item_id.as_str(),
                                thread_id_str.as_str(),
                            )
                            .await
                        })
                        .await?;
                    if !assigned {
                        let _ = session
                            .services
                            .agent_control
                            .request_live_agent_shutdown_preserving_thread(thread_id)
                            .await;
                        tracing::debug!(
                            job_id,
                            item_id = startup.item_id,
                            thread_id = %thread_id,
                            "agent job worker startup finished after item left running state"
                        );
                        return Ok(StartupDrainResult {
                            progressed: true,
                            agent_limit_reached: false,
                        });
                    }

                    let item_id = startup.item_id;
                    active_items.insert(
                        thread_id,
                        ActiveJobItem {
                            item_id: item_id.clone(),
                            started_at: startup.started_at,
                            status_rx: session
                                .services
                                .agent_control
                                .subscribe_status(thread_id)
                                .await
                                .ok(),
                        },
                    );
                    tracing::info!(
                        job_id,
                        item_id,
                        thread_id = %thread_id,
                        spawn_latency_ms = startup.spawn_latency.as_millis() as u64,
                        active_items = active_items.len(),
                        starting_items = starting_items_len,
                        "agent job worker startup completed"
                    );
                    Ok(StartupDrainResult {
                        progressed: true,
                        agent_limit_reached: false,
                    })
                }
                Err(CodexErr::AgentLimitReached { .. }) => {
                    let _ =
                        db_ops::retry_locked("mark_agent_job_item_pending_after_limit", || async {
                            db.mark_agent_job_item_pending(
                                job_id,
                                startup.item_id.as_str(),
                                /*error_message*/ None,
                            )
                            .await
                        })
                        .await?;
                    tracing::debug!(
                        job_id,
                        item_id = startup.item_id,
                        starting_items = starting_items_len,
                        "agent job worker startup hit agent limit"
                    );
                    Ok(StartupDrainResult {
                        progressed: true,
                        agent_limit_reached: true,
                    })
                }
                Err(err) => {
                    let error_message = format!("failed to spawn worker: {err}");
                    let _ = db_ops::retry_locked(
                        "mark_agent_job_item_failed_after_spawn_error",
                        || async {
                            db.mark_agent_job_item_failed(
                                job_id,
                                startup.item_id.as_str(),
                                error_message.as_str(),
                            )
                            .await
                        },
                    )
                    .await?;
                    tracing::warn!(
                        job_id,
                        item_id = startup.item_id,
                        error = %err,
                        "agent job worker startup failed"
                    );
                    Ok(StartupDrainResult {
                        progressed: true,
                        agent_limit_reached: false,
                    })
                }
            }
        }
        Err(join_error) => {
            let task_id = join_error.id();
            let Some(item) = startup_tasks.launching_items.remove(&task_id) else {
                return Ok(StartupDrainResult::default());
            };
            let error_message = format!("worker startup task failed: {join_error}");
            let _ = db_ops::retry_locked(
                "mark_agent_job_item_failed_after_startup_join_error",
                || async {
                    db.mark_agent_job_item_failed(
                        job_id,
                        item.item_id.as_str(),
                        error_message.as_str(),
                    )
                    .await
                },
            )
            .await?;
            tracing::warn!(
                job_id,
                item_id = item.item_id,
                error = %join_error,
                "agent job worker startup task exited unexpectedly"
            );
            Ok(StartupDrainResult {
                progressed: true,
                agent_limit_reached: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use crate::thread_manager::ThreadManager;
    use codex_exec_server::EnvironmentManager;
    use codex_login::CodexAuth;
    use codex_state::AgentJobCreateParams;
    use codex_state::AgentJobItemCreateParams;
    use codex_state::AgentJobItemStatus;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;
    use tokio::sync::Barrier;
    use tokio::time::timeout;

    #[tokio::test]
    async fn spawn_tracked_startup_task_starts_multiple_workers_without_serial_waiting() {
        let mut startup_tasks = StartupTasks::default();
        let started = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(4));

        for idx in 0..3usize {
            let started = Arc::clone(&started);
            let barrier = Arc::clone(&barrier);
            spawn_tracked_startup_task(
                &mut startup_tasks,
                format!("item-{idx}"),
                Instant::now(),
                async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    barrier.wait().await;
                    WorkerStartup {
                        item_id: format!("item-{idx}"),
                        started_at: Instant::now(),
                        spawn_latency: Duration::ZERO,
                        result: Err(CodexErr::ThreadNotFound(ThreadId::new())),
                    }
                },
            );
        }

        timeout(Duration::from_secs(1), async {
            while started.load(Ordering::SeqCst) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("all startup tasks should begin running");

        assert_eq!(startup_tasks.len(), 3);
        assert_eq!(startup_tasks.launching_items.len(), 3);

        barrier.wait().await;

        let mut outputs = Vec::new();
        while let Some(result) = startup_tasks.starting_items.join_next().await {
            outputs.push(result.expect("startup task should complete").item_id);
        }
        outputs.sort();
        assert_eq!(outputs, vec!["item-0", "item-1", "item-2"]);
    }

    #[tokio::test]
    async fn drain_ready_startups_reports_agent_limit_and_requeues_item() -> anyhow::Result<()> {
        let home = TempDir::new()?;
        let config = ConfigBuilder::without_managed_config_for_tests()
            .codex_home(home.path().to_path_buf())
            .build()
            .await?;

        let manager = ThreadManager::with_models_provider_and_home_for_tests(
            CodexAuth::from_api_key("dummy"),
            config.model_provider.clone(),
            config.codex_home.clone(),
            Arc::new(EnvironmentManager::new(/*exec_server_url*/ None)),
        );
        let root = manager.start_thread(config.clone()).await?;
        let session = root.thread.codex.session.clone();

        let db =
            codex_state::StateRuntime::init(config.codex_home.clone(), "test-provider".to_string())
                .await?;
        let job_id = "job-1".to_string();
        let item_id = "item-1".to_string();
        db.create_agent_job(
            &AgentJobCreateParams {
                id: job_id.clone(),
                name: "test-job".to_string(),
                instruction: "Return a result".to_string(),
                auto_export: true,
                max_runtime_seconds: None,
                output_schema_json: None,
                input_headers: vec!["path".to_string()],
                input_csv_path: "/tmp/in.csv".to_string(),
                output_csv_path: "/tmp/out.csv".to_string(),
            },
            &[AgentJobItemCreateParams {
                item_id: item_id.clone(),
                row_index: 0,
                source_id: None,
                row_json: json!({"path":"file-1"}),
            }],
        )
        .await?;
        db.mark_agent_job_running(job_id.as_str()).await?;

        let mut startup_tasks = StartupTasks::default();
        let task_item_id = item_id.clone();
        spawn_tracked_startup_task(
            &mut startup_tasks,
            item_id.clone(),
            Instant::now(),
            async move {
                WorkerStartup {
                    item_id: task_item_id,
                    started_at: Instant::now(),
                    spawn_latency: Duration::ZERO,
                    result: Err(CodexErr::AgentLimitReached { max_threads: 1 }),
                }
            },
        );

        let mut active_items = HashMap::new();
        let outcome = timeout(Duration::from_secs(1), async {
            loop {
                let outcome = drain_ready_startups(
                    session.clone(),
                    db.clone(),
                    job_id.as_str(),
                    &mut active_items,
                    &mut startup_tasks,
                )
                .await?;
                if outcome.progressed {
                    break Ok::<StartupDrainResult, anyhow::Error>(outcome);
                }
                tokio::task::yield_now().await;
            }
        })
        .await??;

        assert_eq!(
            outcome,
            StartupDrainResult {
                progressed: true,
                agent_limit_reached: true,
            }
        );
        assert!(active_items.is_empty());
        assert!(startup_tasks.is_empty());

        let item = db
            .get_agent_job_item(job_id.as_str(), item_id.as_str())
            .await?
            .expect("job item should exist");
        assert_eq!(item.status, AgentJobItemStatus::Pending);
        assert_eq!(item.assigned_thread_id, None);

        let report = manager
            .shutdown_all_threads_bounded(Duration::from_secs(10))
            .await;
        assert_eq!(report.submit_failed, Vec::new());
        assert_eq!(report.timed_out, Vec::new());
        Ok(())
    }
}
