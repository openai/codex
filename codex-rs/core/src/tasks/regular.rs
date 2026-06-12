use std::future::Future;
use std::sync::Arc;

use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

use crate::hook_runtime::run_app_bundled_internal_turn_stop_hooks;
use crate::session::TurnInput;
use crate::session::turn::run_turn;
use crate::session::turn_context::TurnContext;
use crate::session_startup_prewarm::SessionStartupPrewarmResolution;
use crate::state::TaskKind;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnStartedEvent;
use tracing::Instrument;
use tracing::trace_span;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Default)]
pub(crate) struct RegularTask {
    app_bundled_internal_stop_finalizer: AppBundledInternalStopFinalizer,
}

#[derive(Default)]
struct AppBundledInternalStopFinalizer {
    completed: OnceCell<()>,
}

impl AppBundledInternalStopFinalizer {
    async fn run_once<F, Fut>(&self, finalize: F)
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        self.completed.get_or_init(finalize).await;
    }
}

impl RegularTask {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    async fn run_app_bundled_internal_stop_once(
        &self,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        last_assistant_message: Option<String>,
    ) {
        self.app_bundled_internal_stop_finalizer
            .run_once(|| async move {
                let sess = session.clone_session();
                run_app_bundled_internal_turn_stop_hooks(&sess, &ctx, last_assistant_message).await;
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use codex_config::HookEventsToml;
    use codex_config::HookHandlerConfig;
    use codex_config::MatcherGroup;
    use codex_hooks::Hooks;
    use codex_hooks::HooksConfig;
    use codex_plugin::PluginHookSource;
    use codex_plugin::PluginHookSourceKind;
    use codex_plugin::PluginId;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::Op;
    use codex_protocol::user_input::UserInput;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use core_test_support::responses::ev_assistant_message;
    use core_test_support::responses::ev_completed;
    use core_test_support::responses::ev_response_created;
    use core_test_support::responses::mount_sse_once;
    use core_test_support::responses::sse;
    use core_test_support::responses::start_mock_server;
    use core_test_support::test_codex::test_codex;
    use pretty_assertions::assert_eq;
    use tokio::sync::Notify;
    use tokio::time::timeout;
    use tracing_test::traced_test;

    use super::AppBundledInternalStopFinalizer;

    #[tokio::test]
    async fn internal_stop_finalizer_runs_exactly_once() {
        let finalizer = AppBundledInternalStopFinalizer::default();
        let calls = AtomicUsize::new(0);

        finalizer
            .run_once(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        finalizer
            .run_once(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn canceled_internal_stop_finalizer_is_retried_by_abort_path() {
        let finalizer = Arc::new(AppBundledInternalStopFinalizer::default());
        let started = Arc::new(Notify::new());
        let never_finish = Arc::new(Notify::new());
        let first_finalizer = Arc::clone(&finalizer);
        let first_started = Arc::clone(&started);
        let first_never_finish = Arc::clone(&never_finish);
        let first = tokio::spawn(async move {
            first_finalizer
                .run_once(|| async move {
                    first_started.notify_one();
                    first_never_finish.notified().await;
                })
                .await;
        });
        started.notified().await;
        first.abort();
        let _ = first.await;

        let retries = AtomicUsize::new(0);
        finalizer
            .run_once(|| async {
                retries.fetch_add(1, Ordering::SeqCst);
            })
            .await;
        finalizer
            .run_once(|| async {
                retries.fetch_add(1, Ordering::SeqCst);
            })
            .await;

        assert_eq!(retries.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[traced_test]
    fn app_bundled_internal_stop_finalizes_once_and_stays_hidden() {
        let test_span = tracing::Span::current();
        // Exercise the full turn on the same stack size used by the Codex runtime; the default
        // libtest thread stack is too small for this end-to-end session future in debug builds.
        std::thread::Builder::new()
            .name("internal-stop-lifecycle-test".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let _enter = test_span.enter();
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("build test runtime")
                    .block_on(async {
                        let server = start_mock_server().await;
                        let _response = mount_sse_once(
                            &server,
                            sse(vec![
                                ev_response_created("resp-1"),
                                ev_assistant_message("msg-1", "finished"),
                                ev_completed("resp-1"),
                            ]),
                        )
                        .await;
                        let temp = tempfile::tempdir().expect("create internal hook fixture root");
                        let plugin_root = temp.path().join("synthetic-internal");
                        let hooks_dir = plugin_root.join("hooks");
                        let source_path = hooks_dir.join("hooks.json");
                        let plugin_data_root = temp.path().join("plugin-data");
                        fs::create_dir_all(&hooks_dir)
                            .expect("create internal hook declaration directory");
                        fs::create_dir_all(&plugin_data_root)
                            .expect("create internal hook data directory");
                        fs::write(&source_path, "{}")
                            .expect("write internal hook declaration fixture");

                        let source = PluginHookSource {
                            plugin_id: PluginId::parse("synthetic-internal@test")
                                .expect("synthetic plugin id"),
                            plugin_root: AbsolutePathBuf::try_from(plugin_root)
                                .expect("absolute plugin root"),
                            plugin_data_root: AbsolutePathBuf::try_from(plugin_data_root)
                                .expect("absolute plugin data root"),
                            source_path: AbsolutePathBuf::try_from(source_path)
                                .expect("absolute source path"),
                            source_relative_path: "hooks/hooks.json".to_string(),
                            hooks: HookEventsToml {
                                stop: vec![MatcherGroup {
                                    matcher: None,
                                    hooks: vec![HookHandlerConfig::Command {
                                        command: "synthetic-internal-stop-hook".to_string(),
                                        command_windows: None,
                                        timeout_sec: Some(10),
                                        r#async: false,
                                        status_message: None,
                                    }],
                                }],
                                ..Default::default()
                            },
                            kind: PluginHookSourceKind::AppBundledInternal,
                        };

                        let mut builder = test_codex();
                        let test = builder.build(&server).await.expect("build test Codex");
                        test.install_hooks_for_test(Hooks::new(HooksConfig {
                            plugin_hook_sources: vec![source],
                            ..Default::default()
                        }));

                        test.codex
                            .submit(Op::UserInput {
                                items: vec![UserInput::Text {
                                    text: "finish this turn".to_string(),
                                    text_elements: Vec::new(),
                                }],
                                final_output_json_schema: None,
                                responsesapi_client_metadata: None,
                                additional_context: Default::default(),
                                thread_settings: Default::default(),
                            })
                            .await
                            .expect("submit test turn");

                        loop {
                            let event = timeout(Duration::from_secs(10), test.codex.next_event())
                                .await
                                .expect("wait for terminal turn event")
                                .expect("read terminal turn event");
                            assert!(
                                !matches!(
                                    &event.msg,
                                    EventMsg::HookStarted(_) | EventMsg::HookCompleted(_)
                                ),
                                "internal hook notification leaked to the client: {:?}",
                                event.msg,
                            );
                            if matches!(event.msg, EventMsg::TurnComplete(_)) {
                                break;
                            }
                        }
                    });
            })
            .expect("spawn test runtime thread")
            .join()
            .expect("test runtime thread panicked");

        logs_assert(|lines: &[&str]| {
            let finalization_count = lines
                .iter()
                .filter(|line| {
                    line.contains("app-bundled internal hook ended in an invalid runtime state")
                })
                .count();
            if finalization_count == 1 {
                Ok(())
            } else {
                Err(format!(
                    "expected one internal hook finalization, saw {finalization_count}"
                ))
            }
        });
    }
}

impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    fn span_name(&self) -> &'static str {
        "session_task.turn"
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<TurnInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        let turn_extension_data = session.turn_extension_data();
        let run_turn_span = trace_span!("run_turn");
        // Regular turns emit `TurnStarted` inline so first-turn lifecycle does
        // not wait on startup prewarm resolution.
        let prewarmed_client_session = async {
            let event = EventMsg::TurnStarted(TurnStartedEvent {
                turn_id: ctx.sub_id.clone(),
                trace_id: ctx.trace_id.clone(),
                started_at: ctx.turn_timing_state.started_at_unix_secs().await,
                model_context_window: ctx.model_context_window(),
                collaboration_mode_kind: ctx.collaboration_mode.mode,
            });
            sess.send_event(ctx.as_ref(), event).await;
            sess.set_server_reasoning_included(/*included*/ false).await;
            sess.consume_startup_prewarm_for_regular_turn(&cancellation_token)
                .await
        }
        .instrument(trace_span!("regular_task.prepare_run_turn"))
        .await;
        let prewarmed_client_session = match prewarmed_client_session {
            SessionStartupPrewarmResolution::Cancelled => return None,
            SessionStartupPrewarmResolution::Unavailable { .. } => None,
            SessionStartupPrewarmResolution::Ready(prewarmed_client_session) => {
                Some(*prewarmed_client_session)
            }
        };
        let mut next_input = input;
        let mut prewarmed_client_session = prewarmed_client_session;
        loop {
            let last_agent_message = run_turn(
                Arc::clone(&sess),
                Arc::clone(&ctx),
                Arc::clone(&turn_extension_data),
                next_input,
                prewarmed_client_session.take(),
                cancellation_token.child_token(),
            )
            .instrument(run_turn_span.clone())
            .await;
            if !sess.input_queue.has_pending_input(&sess.active_turn).await {
                self.run_app_bundled_internal_stop_once(
                    Arc::clone(&session),
                    Arc::clone(&ctx),
                    last_agent_message.clone(),
                )
                .await;
                return last_agent_message;
            }
            next_input = Vec::new();
        }
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        // The regular task is force-aborted after a short grace period. OnceCell is cancellation
        // safe: if the normal finalizer was interrupted while awaiting the hook, this abort path
        // retries it; if it finished, the idempotent cleanup is not run twice.
        self.run_app_bundled_internal_stop_once(session, ctx, None)
            .await;
    }
}
