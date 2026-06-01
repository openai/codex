use std::sync::Arc;
use std::sync::Weak;

use codex_protocol::items::TurnItem;
use codex_tools::ConversationHistory;
use codex_tools::ExtensionTurnItem;
use codex_tools::ImageGenerationCompletionFuture;
use codex_tools::ToolCall as ExtensionToolCall;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::TurnItemEmissionFuture;
use codex_tools::TurnItemEmitter;

use crate::context::ContextualUserFragment;
use crate::context::ImageGenerationInstructions;
use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::stream_events_utils::persist_image_generation_item;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

pub(crate) struct ExtensionToolAdapter(Arc<dyn codex_tools::ToolExecutor<ExtensionToolCall>>);

impl ExtensionToolAdapter {
    pub(crate) fn new(executor: Arc<dyn codex_tools::ToolExecutor<ExtensionToolCall>>) -> Self {
        Self(executor)
    }
}

#[async_trait::async_trait]
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

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        self.0.handle(to_extension_call(&invocation).await).await
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
    }
}

impl TurnItemEmitter for CoreTurnItemEmitter {
    fn emit_started<'a>(&'a self, item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a> {
        Box::pin(async move {
            let (Some(session), Some(turn)) = (self.session.upgrade(), self.turn.upgrade()) else {
                return;
            };
            let item = extension_turn_item(item);
            session.emit_turn_item_started(turn.as_ref(), &item).await;
        })
    }

    fn emit_completed<'a>(&'a self, item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a> {
        Box::pin(async move {
            let (Some(session), Some(turn)) = (self.session.upgrade(), self.turn.upgrade()) else {
                return;
            };
            let item = extension_turn_item(item);
            session.emit_turn_item_completed(turn.as_ref(), item).await;
        })
    }

    fn image_generation_completed<'a>(
        &'a self,
        call_id: String,
        prompt: String,
        result: String,
    ) -> ImageGenerationCompletionFuture<'a> {
        Box::pin(async move {
            let (Some(session), Some(turn)) = (self.session.upgrade(), self.turn.upgrade()) else {
                return None;
            };
            let mut item = codex_protocol::items::ImageGenerationItem {
                id: call_id,
                status: "completed".to_string(),
                revised_prompt: Some(prompt),
                result,
                saved_path: None,
            };
            let output_hint =
                persist_image_generation_item(session.as_ref(), turn.as_ref(), &mut item)
                    .await
                    .map(|saved_path| {
                        let output_dir = saved_path
                            .parent()
                            .unwrap_or_else(|| turn.config.codex_home.clone());
                        ImageGenerationInstructions::new(output_dir.display(), saved_path.display())
                            .body()
                    });
            let started_item = codex_protocol::items::ImageGenerationItem {
                id: item.id.clone(),
                status: "in_progress".to_string(),
                revised_prompt: None,
                result: String::new(),
                saved_path: None,
            };
            session
                .emit_turn_item_started(turn.as_ref(), &TurnItem::ImageGeneration(started_item))
                .await;
            session
                .emit_turn_item_completed(turn.as_ref(), TurnItem::ImageGeneration(item))
                .await;
            output_hint
        })
    }
}

async fn to_extension_call(invocation: &ToolInvocation) -> ExtensionToolCall {
    let conversation_history =
        ConversationHistory::new(invocation.session.clone_history().await.into_raw_items());
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
        payload: invocation.payload.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_protocol::items::TurnItem;
    use codex_protocol::items::WebSearchItem;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseItem;
    use codex_protocol::models::WebSearchAction;
    use codex_protocol::protocol::EventMsg;
    use codex_tools::ExtensionTurnItem;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::sync::Mutex;

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

    #[async_trait::async_trait]
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

        async fn handle(
            &self,
            _call: codex_tools::ToolCall,
        ) -> Result<Box<dyn codex_tools::ToolOutput>, codex_tools::FunctionCallError> {
            Ok(Box::new(codex_tools::JsonToolOutput::new(
                json!({ "ok": true }),
            )))
        }
    }

    struct CapturingExtensionExecutor {
        captured_call: Arc<Mutex<Option<codex_tools::ToolCall>>>,
    }

    #[async_trait::async_trait]
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

        async fn handle(
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
            Ok(Box::new(codex_tools::JsonToolOutput::new(
                json!({ "ok": true }),
            )))
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

    struct ImageGenerationExtensionExecutor {
        output_hint: Arc<Mutex<Option<String>>>,
    }

    #[async_trait::async_trait]
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

        async fn handle(
            &self,
            call: codex_tools::ToolCall,
        ) -> Result<Box<dyn codex_tools::ToolOutput>, codex_tools::FunctionCallError> {
            let output_hint = call
                .turn_item_emitter
                .image_generation_completed(
                    call.call_id,
                    "A tiny blue square".to_string(),
                    "cG5n".to_string(),
                )
                .await;
            *self.output_hint.lock().await = output_hint;
            Ok(Box::new(codex_tools::JsonToolOutput::new(
                json!({ "ok": true }),
            )))
        }
    }

    #[tokio::test]
    async fn image_generation_publication_is_finalized_by_core() {
        let output_hint = Arc::new(Mutex::new(None));
        let handler = ExtensionToolAdapter::new(Arc::new(ImageGenerationExtensionExecutor {
            output_hint: Arc::clone(&output_hint),
        }));
        let (session, turn, rx) = crate::session::tests::make_session_and_context_with_rx().await;
        let expected_path = crate::stream_events_utils::image_generation_artifact_path(
            &turn.config.codex_home,
            &session.conversation_id.to_string(),
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
        assert_eq!(
            *output_hint.lock().await,
            Some(format!(
                "Generated images are saved to {} as {} by default.\n\
                 If you need to use a generated image at another path, copy it and leave the original in place unless the user explicitly asks you to delete it.",
                expected_path
                    .parent()
                    .expect("generated image path should have a parent")
                    .display(),
                expected_path.display(),
            ))
        );
    }
}
