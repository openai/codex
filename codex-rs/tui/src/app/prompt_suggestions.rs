//! Hidden fork requests. The composer owns request identity and cancellation, including
//! Escape during startup. A deadline cancels the result without dropping an in-flight fork.
use super::*;
use crate::prompt_suggestions::SuggestionRequest;
use crate::temporary_structured_request::collect_structured_response;
use crate::temporary_structured_request::unsubscribe_temporary_thread;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_context_fragments::AdditionalContextUserFragment;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::config_types::CollaborationMode;
use color_eyre::eyre::eyre;

const SUGGESTION_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 30);

impl App {
    pub(super) fn generate_prompt_suggestion(
        &mut self,
        app_server: &AppServerSession,
        request: SuggestionRequest,
    ) {
        if !self.local_settings.tui.prompt_suggestions
            || request.cancellation.is_cancelled()
            || self.chat_widget.thread_id() != Some(request.thread_id)
        {
            request.cancellation.cancel();
            return;
        }
        let cancellation = request.cancellation.clone();
        let finished = request.generation_finished.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = cancellation.cancelled() => {},
                _ = finished.cancelled() => {},
                _ = tokio::time::sleep(SUGGESTION_TIMEOUT) => cancellation.cancel(),
            }
        });
        let handle = app_server.request_handle();
        let events = self.app_event_tx.clone();
        tokio::spawn(async move {
            // Await startup even after cancellation so the returned fork can be detached.
            // Fork reloads config files; only exposed live settings can be preserved here.
            let mut result = async {
                let parent: ThreadResumeResponse = handle
                    .request_typed(ClientRequest::ThreadResume {
                        request_id: RequestId::String(format!("suggestion-resume-{}", request.id)),
                        params: ThreadResumeParams {
                            thread_id: request.thread_id.to_string(),
                            exclude_turns: true,
                            ..Default::default()
                        },
                    })
                    .await?;
                if request.cancellation.is_cancelled() {
                    return Err(eyre!("suggestion cancelled"));
                }
                let mut config = std::collections::HashMap::from([(
                    "model_reasoning_summary".to_string(),
                    serde_json::to_value(request.summary)?,
                )]);
                if let Some(effort) = parent.reasoning_effort {
                    config.insert(
                        "model_reasoning_effort".to_string(),
                        serde_json::to_value(effort)?,
                    );
                }
                let fork: ThreadForkResponse = handle
                    .request_typed(ClientRequest::ThreadFork {
                        request_id: RequestId::String(format!("suggestion-fork-{}", request.id)),
                        params: ThreadForkParams {
                            thread_id: request.thread_id.to_string(),
                            last_turn_id: Some(request.turn_id.clone()),
                            model: Some(parent.model),
                            model_provider: Some(parent.model_provider),
                            service_tier: Some(parent.service_tier),
                            runtime_workspace_roots: Some(parent.runtime_workspace_roots),
                            config: Some(config),
                            ephemeral: true,
                            exclude_turns: true,
                            thread_source: Some(ThreadSource::Feature(
                                "prompt_suggestion".to_string(),
                            )),
                            ..Default::default()
                        },
                    })
                    .await?;
                color_eyre::eyre::Ok((fork.thread.id, parent.collaboration_mode))
            }
            .await
            .map_err(|error| error.to_string());
            if request.cancellation.is_cancelled() {
                if let Ok((thread_id, _)) = result {
                    unsubscribe_temporary_thread(&handle, thread_id).await;
                }
                result = Err("suggestion cancelled".to_string());
            }
            events.send(AppEvent::PromptSuggestionStarted { request, result });
        });
    }

    pub(super) fn on_prompt_suggestion_started(
        &mut self,
        app_server: &AppServerSession,
        request: SuggestionRequest,
        result: Result<(String, Option<CollaborationMode>), String>,
    ) {
        let Ok((thread_id, collaboration_mode)) = result else {
            request.generation_finished.cancel();
            request.cancellation.cancel();
            return;
        };
        let handle = app_server.request_handle();
        let Ok(temporary_thread_id) = ThreadId::from_string(&thread_id) else {
            request.generation_finished.cancel();
            request.cancellation.cancel();
            tokio::spawn(async move {
                unsubscribe_temporary_thread(&handle, thread_id).await;
            });
            return;
        };
        let (sender, receiver) = mpsc::unbounded_channel();
        self.temporary_structured_requests
            .insert(temporary_thread_id, sender);
        let events = self.app_event_tx.clone();
        tokio::spawn(async move {
            // Retain the inherited tools and settings. Await the turn ID before interrupting.
            let mut turn_id = None;
            let result = async {
                if request.cancellation.is_cancelled() {
                    return Err(eyre!("suggestion cancelled"));
                }
                let response: TurnStartResponse = handle.request_typed(ClientRequest::TurnStart {
                    request_id: RequestId::String(format!("suggestion-turn-{}", Uuid::new_v4())),
                    params: TurnStartParams {
                        thread_id: thread_id.clone(),
                        collaboration_mode,
                        input: vec![UserInput::Text {
                            text: AdditionalContextUserFragment::new("prompt_suggestion".to_string(), format!("{}\n\nReturn only JSON matching this schema: {{\"type\":\"object\",\"properties\":{{\"suggestion\":{{\"type\":[\"string\",\"null\"]}}}},\"required\":[\"suggestion\"],\"additionalProperties\":false}}.", crate::prompt_suggestions::PROMPT)).render(),
                            text_elements: Vec::new(),
                        }],
                        ..Default::default()
                    },
                }).await?;
                turn_id = Some(response.turn.id.clone());
                tokio::select! {
                    biased;
                    _ = request.cancellation.cancelled() => Err(eyre!("suggestion cancelled or timed out")),
                    result = collect_structured_response(receiver, &response.turn.id) => result,
                }
            }.await;
            if result.is_err()
                && let Some(turn_id) = turn_id
            {
                let _ = tokio::time::timeout(
                    SUGGESTION_TIMEOUT,
                    handle.request_typed::<TurnInterruptResponse>(ClientRequest::TurnInterrupt {
                        request_id: RequestId::String(format!(
                            "suggestion-interrupt-{}",
                            Uuid::new_v4()
                        )),
                        params: TurnInterruptParams {
                            thread_id: thread_id.clone(),
                            turn_id,
                        },
                    }),
                )
                .await;
            }
            unsubscribe_temporary_thread(&handle, thread_id).await;
            let text = result
                .ok()
                .and_then(|text| crate::prompt_suggestions::parse_suggestion(&text));
            request.generation_finished.cancel();
            events.send(AppEvent::PromptSuggestionFinished {
                request,
                temporary_thread_id,
                text,
            });
        });
    }
}

#[cfg(test)]
#[path = "prompt_suggestions_tests.rs"]
mod tests;
