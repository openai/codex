use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

use crate::client::ModelClientSession;
use crate::codex::INITIAL_SUBMIT_ID;
use crate::codex::Session;
use crate::codex::build_prompt;
use crate::codex::built_tools;
use codex_app_server_protocol::AuthMode;
use codex_otel::STARTUP_PREWARM_AGE_AT_FIRST_TURN_METRIC;
use codex_otel::STARTUP_PREWARM_DURATION_METRIC;
use codex_otel::SessionTelemetry;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::BaseInstructions;

pub(crate) struct SessionStartupPrewarmHandle {
    task: JoinHandle<CodexResult<ModelClientSession>>,
    started_at: Instant,
    timeout: Duration,
}

pub(crate) enum SessionStartupPrewarmResolution {
    Cancelled,
    Ready(Box<ModelClientSession>),
    Unavailable {
        status: &'static str,
        prewarm_duration: Option<Duration>,
    },
}

impl SessionStartupPrewarmHandle {
    pub(crate) fn new(
        task: JoinHandle<CodexResult<ModelClientSession>>,
        started_at: Instant,
        timeout: Duration,
    ) -> Self {
        Self {
            task,
            started_at,
            timeout,
        }
    }

    async fn resolve(
        self,
        session_telemetry: &SessionTelemetry,
        cancellation_token: &CancellationToken,
    ) -> SessionStartupPrewarmResolution {
        let Self {
            mut task,
            started_at,
            timeout,
        } = self;
        let age_at_first_turn = started_at.elapsed();
        let remaining = timeout.saturating_sub(age_at_first_turn);

        let resolution = if task.is_finished() {
            Self::resolution_from_join_result(task.await, started_at)
        } else {
            match tokio::select! {
                _ = cancellation_token.cancelled() => None,
                result = tokio::time::timeout(remaining, &mut task) => Some(result),
            } {
                Some(Ok(result)) => Self::resolution_from_join_result(result, started_at),
                Some(Err(_elapsed)) => {
                    task.abort();
                    info!("startup websocket prewarm timed out before the first turn could use it");
                    SessionStartupPrewarmResolution::Unavailable {
                        status: "timed_out",
                        prewarm_duration: Some(started_at.elapsed()),
                    }
                }
                None => {
                    task.abort();
                    session_telemetry.record_duration(
                        STARTUP_PREWARM_AGE_AT_FIRST_TURN_METRIC,
                        age_at_first_turn,
                        &[("status", "cancelled")],
                    );
                    session_telemetry.record_duration(
                        STARTUP_PREWARM_DURATION_METRIC,
                        started_at.elapsed(),
                        &[("status", "cancelled")],
                    );
                    return SessionStartupPrewarmResolution::Cancelled;
                }
            }
        };

        match resolution {
            SessionStartupPrewarmResolution::Cancelled => {
                SessionStartupPrewarmResolution::Cancelled
            }
            SessionStartupPrewarmResolution::Ready(prewarmed_session) => {
                session_telemetry.record_duration(
                    STARTUP_PREWARM_AGE_AT_FIRST_TURN_METRIC,
                    age_at_first_turn,
                    &[("status", "consumed")],
                );
                SessionStartupPrewarmResolution::Ready(prewarmed_session)
            }
            SessionStartupPrewarmResolution::Unavailable {
                status,
                prewarm_duration,
            } => {
                session_telemetry.record_duration(
                    STARTUP_PREWARM_AGE_AT_FIRST_TURN_METRIC,
                    age_at_first_turn,
                    &[("status", status)],
                );
                if let Some(prewarm_duration) = prewarm_duration {
                    session_telemetry.record_duration(
                        STARTUP_PREWARM_DURATION_METRIC,
                        prewarm_duration,
                        &[("status", status)],
                    );
                }
                SessionStartupPrewarmResolution::Unavailable {
                    status,
                    prewarm_duration,
                }
            }
        }
    }

    fn resolution_from_join_result(
        result: std::result::Result<CodexResult<ModelClientSession>, tokio::task::JoinError>,
        started_at: Instant,
    ) -> SessionStartupPrewarmResolution {
        match result {
            Ok(Ok(prewarmed_session)) => {
                SessionStartupPrewarmResolution::Ready(Box::new(prewarmed_session))
            }
            Ok(Err(err)) => {
                warn!("startup websocket prewarm setup failed: {err:#}");
                SessionStartupPrewarmResolution::Unavailable {
                    status: "failed",
                    prewarm_duration: None,
                }
            }
            Err(err) => {
                warn!("startup websocket prewarm setup join failed: {err}");
                SessionStartupPrewarmResolution::Unavailable {
                    status: "join_failed",
                    prewarm_duration: Some(started_at.elapsed()),
                }
            }
        }
    }
}

impl Session {
    pub(crate) async fn schedule_startup_prewarm(self: &Arc<Self>, base_instructions: String) {
        if self.services.agent_identity_manager.is_enabled()
            && self.services.auth_manager.auth_mode() != Some(AuthMode::ApiKey)
        {
            info!(
                "skipping startup websocket prewarm because agent identity requires task-scoped auth"
            );
            return;
        }

        let session_telemetry = self.services.session_telemetry.clone();
        let websocket_connect_timeout = self.provider().await.websocket_connect_timeout();
        let started_at = Instant::now();
        let startup_prewarm_session = Arc::clone(self);
        let startup_prewarm = tokio::spawn(async move {
            let result =
                schedule_startup_prewarm_inner(startup_prewarm_session, base_instructions).await;
            let status = if result.is_ok() { "ready" } else { "failed" };
            session_telemetry.record_duration(
                STARTUP_PREWARM_DURATION_METRIC,
                started_at.elapsed(),
                &[("status", status)],
            );
            result
        });
        self.set_session_startup_prewarm(SessionStartupPrewarmHandle::new(
            startup_prewarm,
            started_at,
            websocket_connect_timeout,
        ))
        .await;
    }

    pub(crate) async fn consume_startup_prewarm_for_regular_turn(
        &self,
        cancellation_token: &CancellationToken,
    ) -> SessionStartupPrewarmResolution {
        let Some(startup_prewarm) = self.take_session_startup_prewarm().await else {
            return SessionStartupPrewarmResolution::Unavailable {
                status: "not_scheduled",
                prewarm_duration: None,
            };
        };
        startup_prewarm
            .resolve(&self.services.session_telemetry, cancellation_token)
            .await
    }
}

async fn schedule_startup_prewarm_inner(
    session: Arc<Session>,
    base_instructions: String,
) -> CodexResult<ModelClientSession> {
    let startup_turn_context = session
        .new_default_turn_with_sub_id(INITIAL_SUBMIT_ID.to_owned())
        .await;
    let startup_cancellation_token = CancellationToken::new();
    let startup_router = built_tools(
        session.as_ref(),
        startup_turn_context.as_ref(),
        &[],
        &HashSet::new(),
        /*skills_outcome*/ None,
        &startup_cancellation_token,
    )
    .await?;
    let startup_prompt = build_prompt(
        Vec::new(),
        startup_router.as_ref(),
        startup_turn_context.as_ref(),
        BaseInstructions {
            text: base_instructions,
        },
    );
    let startup_turn_metadata_header = startup_turn_context
        .turn_metadata_state
        .current_header_value();
    let mut client_session = session.services.model_client.new_session();
    client_session
        .prewarm_websocket(
            &startup_prompt,
            &startup_turn_context.model_info,
            &startup_turn_context.session_telemetry,
            startup_turn_context.reasoning_effort,
            startup_turn_context.reasoning_summary,
            startup_turn_context.config.service_tier,
            startup_turn_metadata_header.as_deref(),
        )
        .await?;

    Ok(client_session)
}
