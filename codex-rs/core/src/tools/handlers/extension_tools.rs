use std::sync::Arc;
use std::sync::Weak;

use codex_protocol::items::TurnItem;
use codex_tools::ConversationHistory;
use codex_tools::ExtensionTurnItem;
use codex_tools::ToolCall as ExtensionToolCall;
use codex_tools::ToolEnvironment;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSpec;
use codex_tools::TurnItemEmissionFuture;
use codex_tools::TurnItemEmitter;

use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::stream_events_utils::TurnItemContributorPolicy;
use crate::stream_events_utils::finalize_turn_item;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

pub(crate) struct ExtensionToolAdapter(Arc<dyn codex_tools::ToolExecutor<ExtensionToolCall>>);

impl ExtensionToolAdapter {
    pub(crate) fn new(executor: Arc<dyn codex_tools::ToolExecutor<ExtensionToolCall>>) -> Self {
        Self(executor)
    }
}

impl ToolExecutor<ToolInvocation> for ExtensionToolAdapter {
    fn tool_name(&self) -> ToolName {
        self.0.tool_name()
    }

    fn spec(&self) -> ToolSpec {
        self.0.spec()
    }

    fn exposure(&self) -> crate::tools::registry::ToolExposure {
        self.0.exposure()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        self.0.supports_parallel_tool_calls()
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        self.0.search_info()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move { self.0.handle(to_extension_call(&invocation).await).await })
    }
}

impl CoreToolRuntime for ExtensionToolAdapter {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}

struct CoreTurnItemEmitter {
    session: Weak<Session>,
    turn: Weak<TurnContext>,
}

fn extension_turn_item(item: ExtensionTurnItem) -> TurnItem {
    match item {
        ExtensionTurnItem::WebSearch(item) => TurnItem::WebSearch(item),
        ExtensionTurnItem::ImageGeneration(mut item) => {
            item.saved_path = None;
            TurnItem::ImageGeneration(item)
        }
    }
}

impl TurnItemEmitter for CoreTurnItemEmitter {
    fn emit_started<'a>(&'a self, item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a> {
        Box::pin(async move {
            let (Some(session), Some(turn)) = (self.session.upgrade(), self.turn.upgrade()) else {
                return;
            };
            session
                .emit_turn_item_started(turn.as_ref(), &extension_turn_item(item))
                .await;
        })
    }

    fn emit_completed<'a>(&'a self, item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a> {
        Box::pin(async move {
            let (Some(session), Some(turn)) = (self.session.upgrade(), self.turn.upgrade()) else {
                return;
            };
            let mut item = extension_turn_item(item);
            finalize_turn_item(
                session.as_ref(),
                turn.as_ref(),
                TurnItemContributorPolicy::Run(turn.extension_data.as_ref()),
                &mut item,
                turn.collaboration_mode.mode == codex_protocol::config_types::ModeKind::Plan,
            )
            .await;
            session.emit_turn_item_completed(turn.as_ref(), item).await;
        })
    }
}

async fn to_extension_call(invocation: &ToolInvocation) -> ExtensionToolCall {
    let conversation_history =
        ConversationHistory::new(invocation.session.clone_history().await.into_raw_items());
    let environments = invocation
        .turn
        .environments
        .turn_environments
        .iter()
        .map(|environment| ToolEnvironment {
            environment_id: environment.environment_id.clone(),
            cwd: environment.cwd.clone(),
            file_system: environment.environment.get_filesystem(),
            file_system_sandbox_context: invocation.turn.file_system_sandbox_context(
                /*additional_permissions*/ None,
                &environment.cwd,
            ),
        })
        .collect::<Vec<_>>();
    ExtensionToolCall {
        turn_id: invocation.turn.sub_id.clone(),
        call_id: invocation.call_id.clone(),
        tool_name: invocation.tool_name.clone(),
        model: invocation.turn.model_info.slug.clone(),
        truncation_policy: invocation.turn.truncation_policy,
        conversation_history,
        turn_item_emitter: Arc::new(CoreTurnItemEmitter {
            session: Arc::downgrade(&invocation.session),
            turn: Arc::downgrade(&invocation.turn),
        }),
        environments,
        payload: invocation.payload.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_extension_api::ExtensionData;
    use codex_extension_api::TurnItemContributor;
    use codex_protocol::items::TurnItem;
    use codex_protocol::items::WebSearchItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::models::WebSearchAction;
    use codex_protocol::protocol::EventMsg;
    use codex_tools::ExtensionTurnItem;
    use codex_utils_absolute_path::test_support::PathExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::CoreTurnItemEmitter;
    use super::ExtensionToolAdapter;
    use crate::tools::context::ToolCallSource;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolPayload;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::CoreToolRuntime;
    use crate::tools::registry::PostToolUsePayload;
    use crate::tools::registry::PreToolUsePayload;
    use crate::turn_diff_tracker::TurnDiffTracker;

    struct StubExtensionExecutor;

    impl codex_extension_api::ToolExecutor<codex_tools::ToolCall> for StubExtensionExecutor {
        fn tool_name(&self) -> codex_tools::ToolName {
            codex_tools::ToolName::plain("extension_echo")
        }

        fn spec(&self) -> codex_tools::ToolSpec {
            codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
                name: "extension_echo".to_string(),
                description: "Echoes arguments.".to_string(),
                strict: true,
                parameters: codex_tools::parse_tool_input_schema(&json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                    },
                    "required": ["message"],
                    "additionalProperties": false,
                }))
                .expect("extension schema should parse"),
                output_schema: None,
                defer_loading: None,
            })
        }

        fn handle(&self, _call: codex_tools::ToolCall) -> codex_tools::ToolExecutorFuture<'_> {
            Box::pin(async {
                Ok(
                    Box::new(codex_tools::JsonToolOutput::new(json!({ "ok": true })))
                        as Box<dyn codex_tools::ToolOutput>,
                )
            })
        }
    }

    struct CapturingExtensionExecutor {
        captured_call: Arc<Mutex<Option<codex_tools::ToolCall>>>,
    }

    impl codex_extension_api::ToolExecutor<codex_tools::ToolCall> for CapturingExtensionExecutor {
        fn tool_name(&self) -> codex_tools::ToolName {
            codex_tools::ToolName::plain("extension_echo")
        }

        fn spec(&self) -> codex_tools::ToolSpec {
            codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
                name: "extension_echo".to_string(),
                description: "Captures arguments.".to_string(),
                strict: false,
                parameters: codex_tools::JsonSchema::default(),
                output_schema: None,
                defer_loading: None,
            })
        }

        fn handle(&self, call: codex_tools::ToolCall) -> codex_tools::ToolExecutorFuture<'_> {
            Box::pin(self.handle_call(call))
        }
    }

    impl CapturingExtensionExecutor {
        async fn handle_call(
            &self,
            call: codex_tools::ToolCall,
        ) -> Result<Box<dyn codex_tools::ToolOutput>, codex_tools::FunctionCallError> {
            let item = ExtensionTurnItem::WebSearch(WebSearchItem {
                id: call.call_id.clone(),
                query: "rust trait object".to_string(),
                action: WebSearchAction::Search {
                    query: Some("rust trait object".to_string()),
                    queries: None,
                },
            });
            call.turn_item_emitter.emit_started(item.clone()).await;
            call.turn_item_emitter.emit_completed(item).await;
            *self.captured_call.lock().await = Some(call);
            Ok(
                Box::new(codex_tools::JsonToolOutput::new(json!({ "ok": true })))
                    as Box<dyn codex_tools::ToolOutput>,
            )
        }
    }

    #[tokio::test]
    async fn exposes_generic_hook_payloads() {
        let handler = ExtensionToolAdapter::new(Arc::new(StubExtensionExecutor));
        let (session, turn) = crate::session::tests::make_session_and_context().await;
        let invocation = ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "call-extension".to_string(),
            tool_name: codex_tools::ToolName::plain("extension_echo"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: json!({ "message": "hello" }).to_string(),
            },
        };
        let output = codex_tools::JsonToolOutput::new(json!({ "ok": true }));

        assert_eq!(
            CoreToolRuntime::pre_tool_use_payload(&handler, &invocation),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("extension_echo"),
                tool_input: json!({ "message": "hello" }),
            })
        );
        assert_eq!(
            CoreToolRuntime::post_tool_use_payload(&handler, &invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("extension_echo"),
                tool_use_id: "call-extension".to_string(),
                tool_input: json!({ "message": "hello" }),
                tool_response: json!({ "ok": true }),
            })
        );
    }

    #[tokio::test]
    async fn passes_turn_fields_and_scoped_turn_item_emitter_to_extension_call() {
        let captured_call = Arc::new(Mutex::new(None));
        let handler = ExtensionToolAdapter::new(Arc::new(CapturingExtensionExecutor {
            captured_call: Arc::clone(&captured_call),
        }));
        let (session, turn, rx) = crate::session::tests::make_session_and_context_with_rx().await;
        let weak_session = Arc::downgrade(&session);
        let weak_turn = Arc::downgrade(&turn);
        let turn_id = turn.sub_id.clone();
        let model = turn.model_info.slug.clone();
        let truncation_policy = turn.truncation_policy;
        let history_item = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "extension history".to_string(),
            }],
            phase: None,
        };
        session
            .record_conversation_items(&turn, std::slice::from_ref(&history_item))
            .await;
        let raw_history_event = rx.recv().await.expect("history raw response item event");
        let EventMsg::RawResponseItem(raw_history_item) = raw_history_event.msg else {
            panic!("expected raw response item event");
        };
        assert_eq!(raw_history_item.item, history_item);
        let invocation = ToolInvocation {
            session,
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "call-extension".to_string(),
            tool_name: codex_tools::ToolName::plain("extension_echo"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: json!({ "message": "hello" }).to_string(),
            },
        };

        crate::tools::registry::ToolExecutor::handle(&handler, invocation)
            .await
            .expect("extension call should succeed");

        let captured_call = captured_call.lock().await.clone().expect("captured call");
        assert!(weak_session.upgrade().is_none());
        assert!(weak_turn.upgrade().is_none());
        assert_eq!(captured_call.turn_id, turn_id);
        assert_eq!(captured_call.call_id, "call-extension");
        assert_eq!(
            captured_call.tool_name,
            codex_tools::ToolName::plain("extension_echo")
        );
        assert_eq!(captured_call.model, model);
        assert_eq!(captured_call.truncation_policy, truncation_policy);
        assert_eq!(
            captured_call.conversation_history.items(),
            std::slice::from_ref(&history_item)
        );
        match captured_call.payload {
            ToolPayload::Function { arguments } => {
                assert_eq!(arguments, json!({ "message": "hello" }).to_string());
            }
            payload => panic!("expected function payload, got {payload:?}"),
        }

        let started = rx.recv().await.expect("item started event");
        let EventMsg::ItemStarted(started) = started.msg else {
            panic!("expected item started event");
        };
        let TurnItem::WebSearch(started_item) = started.item else {
            panic!("expected web search item");
        };
        let begin = rx.recv().await.expect("legacy web search begin event");
        let EventMsg::WebSearchBegin(begin) = begin.msg else {
            panic!("expected legacy web search begin event");
        };
        let completed = rx.recv().await.expect("item completed event");
        let EventMsg::ItemCompleted(completed) = completed.msg else {
            panic!("expected item completed event");
        };
        let TurnItem::WebSearch(completed_item) = completed.item else {
            panic!("expected web search item");
        };
        let end = rx.recv().await.expect("legacy web search end event");
        let EventMsg::WebSearchEnd(end) = end.msg else {
            panic!("expected legacy web search end event");
        };

        let expected = WebSearchItem {
            id: "call-extension".to_string(),
            query: "rust trait object".to_string(),
            action: WebSearchAction::Search {
                query: Some("rust trait object".to_string()),
                queries: None,
            },
        };
        assert_eq!(started_item, expected);
        assert_eq!(completed_item, expected);
        assert_eq!(begin.call_id, expected.id);
        assert_eq!(end.call_id, expected.id);
        assert_eq!(end.query, expected.query);
        assert_eq!(end.action, expected.action);
    }

    struct ImageGenerationExtensionExecutor;

    #[derive(Debug)]
    struct ExtensionTurnItemContributorRan;

    struct RecordExtensionTurnItemContributor;

    impl TurnItemContributor for RecordExtensionTurnItemContributor {
        fn contribute<'a>(
            &'a self,
            _thread_store: &'a ExtensionData,
            turn_store: &'a ExtensionData,
            _item: &'a mut TurnItem,
        ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
            Box::pin(async move {
                turn_store.insert(ExtensionTurnItemContributorRan);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn extension_completion_runs_turn_item_contributors() {
        let (mut session, turn) = crate::session::tests::make_session_and_context().await;
        let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
        builder.turn_item_contributor(Arc::new(RecordExtensionTurnItemContributor));
        session.services.extensions = Arc::new(builder.build());
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let emitter = CoreTurnItemEmitter {
            session: Arc::downgrade(&session),
            turn: Arc::downgrade(&turn),
        };

        codex_tools::TurnItemEmitter::emit_completed(
            &emitter,
            ExtensionTurnItem::WebSearch(WebSearchItem {
                id: "search-1".to_string(),
                query: "contributors".to_string(),
                action: WebSearchAction::Other,
            }),
        )
        .await;

        assert!(
            turn.extension_data
                .get::<ExtensionTurnItemContributorRan>()
                .is_some()
        );
    }

    impl codex_extension_api::ToolExecutor<codex_tools::ToolCall> for ImageGenerationExtensionExecutor {
        fn tool_name(&self) -> codex_tools::ToolName {
            codex_tools::ToolName::namespaced("image_gen", "imagegen")
        }

        fn spec(&self) -> codex_tools::ToolSpec {
            codex_tools::ToolSpec::Function(codex_tools::ResponsesApiTool {
                name: "imagegen".to_string(),
                description: "Generates an image.".to_string(),
                strict: false,
                parameters: codex_tools::JsonSchema::default(),
                output_schema: None,
                defer_loading: None,
            })
        }

        fn handle(&self, call: codex_tools::ToolCall) -> codex_tools::ToolExecutorFuture<'_> {
            Box::pin(self.handle_call(call))
        }
    }

    impl ImageGenerationExtensionExecutor {
        async fn handle_call(
            &self,
            call: codex_tools::ToolCall,
        ) -> Result<Box<dyn codex_tools::ToolOutput>, codex_tools::FunctionCallError> {
            call.turn_item_emitter
                .emit_started(ExtensionTurnItem::ImageGeneration(
                    codex_protocol::items::ImageGenerationItem {
                        id: call.call_id.clone(),
                        status: "in_progress".to_string(),
                        revised_prompt: None,
                        result: String::new(),
                        saved_path: None,
                    },
                ))
                .await;
            call.turn_item_emitter
                .emit_completed(ExtensionTurnItem::ImageGeneration(
                    codex_protocol::items::ImageGenerationItem {
                        id: call.call_id,
                        status: "completed".to_string(),
                        revised_prompt: Some("A tiny blue square".to_string()),
                        result: "cG5n".to_string(),
                        saved_path: Some(test_path_buf("/tmp/extension-claimed.png").abs()),
                    },
                ))
                .await;
            Ok(
                Box::new(codex_tools::JsonToolOutput::new(json!({ "ok": true })))
                    as Box<dyn codex_tools::ToolOutput>,
            )
        }
    }

    #[tokio::test]
    async fn image_generation_publication_is_finalized_by_core() {
        let handler = ExtensionToolAdapter::new(Arc::new(ImageGenerationExtensionExecutor));
        let (session, turn, rx) = crate::session::tests::make_session_and_context_with_rx().await;
        let expected_path = crate::stream_events_utils::image_generation_artifact_path(
            &turn.config.codex_home,
            &session.thread_id.to_string(),
            "call-image",
        );
        let invocation = ToolInvocation {
            session,
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "call-image".to_string(),
            tool_name: codex_tools::ToolName::namespaced("image_gen", "imagegen"),
            source: ToolCallSource::Direct,
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
        };

        crate::tools::registry::ToolExecutor::handle(&handler, invocation)
            .await
            .expect("extension call should succeed");

        let started = rx.recv().await.expect("item started event");
        let EventMsg::ItemStarted(started) = started.msg else {
            panic!("expected item started event");
        };
        let TurnItem::ImageGeneration(started_item) = started.item else {
            panic!("expected image generation item");
        };
        let begin = rx.recv().await.expect("legacy image start event");
        assert!(matches!(begin.msg, EventMsg::ImageGenerationBegin(_)));
        let completed = rx.recv().await.expect("item completed event");
        let EventMsg::ItemCompleted(completed) = completed.msg else {
            panic!("expected item completed event");
        };
        let TurnItem::ImageGeneration(completed_item) = completed.item else {
            panic!("expected image generation item");
        };
        let end = rx.recv().await.expect("legacy image end event");
        assert!(matches!(end.msg, EventMsg::ImageGenerationEnd(_)));

        assert_eq!(
            started_item,
            codex_protocol::items::ImageGenerationItem {
                id: "call-image".to_string(),
                status: "in_progress".to_string(),
                revised_prompt: None,
                result: String::new(),
                saved_path: None,
            }
        );
        assert_eq!(
            completed_item,
            codex_protocol::items::ImageGenerationItem {
                id: "call-image".to_string(),
                status: "completed".to_string(),
                revised_prompt: Some("A tiny blue square".to_string()),
                result: "cG5n".to_string(),
                saved_path: Some(expected_path.clone()),
            }
        );
        assert_eq!(
            std::fs::read(&expected_path).expect("generated artifact should be saved"),
            b"png"
        );
    }
}
