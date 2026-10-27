#![cfg(test)]

use super::HostToolInvocationLookup;
use super::InvocationContexts;
use crate::session::step_context::StepContext;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::ListMcpResourcesHandler;
use crate::tools::parallel::ToolCallRuntime;
use crate::tools::registry::ToolRegistry;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolRouter;
use codex_extension_api::ToolFinishInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::MultiAgentVersion;
use codex_tools::ToolName;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct CallbackGate {
    entered: Notify,
    release: Notify,
}

impl ToolLifecycleContributor for CallbackGate {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            self.entered.notify_one();
            self.release.notified().await;
            if matches!(
                input.source,
                codex_extension_api::ToolCallSource::CodeMode { .. }
            ) {
                panic!("dispatch unwound");
            }
        })
    }

    fn on_tool_finish<'a>(&'a self, _input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async {
            self.entered.notify_one();
            self.release.notified().await;
        })
    }
}

#[tokio::test]
async fn dispatch_revokes_access_and_holds_call_id_through_callbacks() -> anyhow::Result<()> {
    let (mut session, mut turn, _events) =
        crate::session::tests::make_session_and_context_with_rx().await;
    Arc::get_mut(&mut turn).unwrap().multi_agent_version = MultiAgentVersion::V2;
    session
        .services
        .thread_extension_data
        .insert(HostToolInvocationLookup);
    let gate = Arc::new(CallbackGate::default());
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.tool_lifecycle_contributor(gate.clone());
    Arc::get_mut(&mut session).unwrap().services.extensions = Arc::new(extensions.build());
    let mut registry = ToolRegistry::default();
    registry.add(ListMcpResourcesHandler::new(/*messages*/ None));
    let router = ToolRouter::from_parts(
        registry,
        vec![],
        ToolMode::Direct,
        Default::default(),
        /*tool_namespaces_info*/ None,
        &[],
    );
    let step = StepContext::for_test(turn.clone()).with_tool_router_for_test(Arc::new(router));
    let runtime = ToolCallRuntime::new(session.clone(), step.clone(), Default::default());
    let dispatch = |source, token, args: &str| {
        runtime.clone().handle_tool_call_with_source(
            step.clone(),
            ToolCall {
                tool_name: ToolName::plain("list_mcp_resources"),
                call_id: "shared".into(),
                payload: ToolPayload::Function {
                    arguments: args.into(),
                },
                encrypted_function_args: None,
            },
            source,
            token,
            Default::default(),
        )
    };
    let wait_limit = Duration::from_secs(5);
    let duplicate = async |args: &str| {
        let result = tokio::time::timeout(
            wait_limit,
            dispatch(ToolCallSource::Direct, CancellationToken::new(), args),
        )
        .await
        .unwrap();
        assert!(
            matches!(result, Err(crate::function_tool::FunctionCallError::RespondToModel(msg)) if msg.contains("already active"))
        );
    };
    let key = (turn.sub_id.clone(), String::from("shared"));
    let at_callback = || tokio::time::timeout(wait_limit, gate.entered.notified());
    let contexts = session
        .services
        .thread_extension_data
        .get_or_init(InvocationContexts::default);
    for (source, abort) in [
        (
            ToolCallSource::CodeMode {
                cell_id: "cell".into(),
                runtime_tool_call_id: "nested".into(),
            },
            false,
        ),
        (ToolCallSource::Direct, true),
        (ToolCallSource::Direct, false),
    ] {
        let panics = matches!(source, ToolCallSource::CodeMode { .. });
        let token = CancellationToken::new();
        let owner = tokio::spawn(dispatch(source, token.clone(), "{}"));
        at_callback()
            .await
            .expect("owner entered its start callback");
        let captured = contexts.0.lock().unwrap()[&key].upgrade().unwrap();
        duplicate("invalid json").await;
        assert!(Arc::ptr_eq(
            &captured,
            &contexts.0.lock().unwrap()[&key].upgrade().unwrap()
        ));
        assert!(!captured.finished().is_cancelled());
        if abort {
            token.cancel();
        } else {
            gate.release.notify_one();
        }
        if !panics {
            at_callback()
                .await
                .expect("owner entered its terminal callback");
            assert!(captured.finished().is_cancelled());
            duplicate("{}").await;
            gate.release.notify_one();
        }
        let outcome = tokio::time::timeout(wait_limit, owner).await??;
        if panics {
            assert!(
                matches!(outcome, Err(crate::function_tool::FunctionCallError::Fatal(message)) if message.contains("dispatch unwound"))
            );
        } else {
            outcome?;
        }
        assert!(captured.finished().is_cancelled());
        assert!(!contexts.0.lock().unwrap().contains_key(&key));
    }
    Ok(())
}
