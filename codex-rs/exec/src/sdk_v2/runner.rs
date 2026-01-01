//! Main SDK mode conversation runner.
//!
//! Handles the SDK mode lifecycle including handshake, message processing,
//! and event streaming.

use codex_sdk_protocol::control::CliHello;
use codex_sdk_protocol::control::SdkHello;
use codex_sdk_protocol::messages::CliMessage;
use codex_sdk_protocol::messages::ResultMessage;
use codex_sdk_protocol::messages::ResultSubtype;
use codex_sdk_protocol::messages::SdkMessage;
use codex_sdk_protocol::messages::StreamEventMessage;
use codex_sdk_protocol::PROTOCOL_VERSION;
use tracing::debug;
use tracing::error;
use tracing::info;

use super::transport::SdkTransport;
use super::transport::TransportError;
use crate::Cli;

/// Run the CLI in SDK mode.
///
/// This function handles the complete SDK mode lifecycle:
/// 1. Protocol handshake with version negotiation
/// 2. Reading user messages from stdin
/// 3. Processing messages through the conversation engine
/// 4. Streaming events back to the SDK via stdout
pub async fn run_sdk_mode(cli: Cli) -> anyhow::Result<()> {
    info!("Starting SDK mode");

    let mut transport = SdkTransport::new();

    // 1. Perform handshake
    let (session_id, _sdk_hello) = perform_handshake(&mut transport).await?;
    info!("SDK handshake complete, session_id={session_id}");

    // 2. Read and process user messages
    loop {
        match transport.read_message().await {
            Ok(message) => {
                debug!("Received SDK message: {message:?}");
                match handle_sdk_message(&mut transport, &session_id, message, &cli).await {
                    Ok(should_continue) => {
                        if !should_continue {
                            info!("SDK session complete");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error handling SDK message: {e}");
                        // Send error result
                        send_error_result(&mut transport, &session_id, &e.to_string())?;
                        break;
                    }
                }
            }
            Err(TransportError::EndOfInput) => {
                info!("SDK client disconnected");
                break;
            }
            Err(e) => {
                error!("Transport error: {e}");
                break;
            }
        }
    }

    Ok(())
}

/// Perform protocol handshake with the SDK.
async fn perform_handshake(
    transport: &mut SdkTransport,
) -> Result<(String, SdkHello), anyhow::Error> {
    // Read SDK hello
    let message = transport.read_message().await?;
    let sdk_hello = match message {
        SdkMessage::SdkHello(hello) => hello,
        other => {
            return Err(anyhow::anyhow!(
                "Expected SdkHello, got {:?}",
                std::mem::discriminant(&other)
            ));
        }
    };

    info!(
        "Received SDK hello: version={}, capabilities={:?}",
        sdk_hello.version, sdk_hello.capabilities
    );

    // Generate session ID
    let session_id = uuid::Uuid::new_v4().to_string();

    // Send CLI hello response
    let cli_hello = CliHello {
        version: PROTOCOL_VERSION.to_string(),
        session_id: session_id.clone(),
        capabilities: vec![
            "hooks".to_string(),
            "tool_permissions".to_string(),
            "mcp_routing".to_string(),
        ],
    };

    transport.write_message(&CliMessage::CliHello(cli_hello))?;

    Ok((session_id, sdk_hello))
}

/// Handle an incoming SDK message.
///
/// Returns `true` if the session should continue, `false` if it should end.
async fn handle_sdk_message(
    transport: &mut SdkTransport,
    session_id: &str,
    message: SdkMessage,
    _cli: &Cli,
) -> Result<bool, anyhow::Error> {
    match message {
        SdkMessage::SdkHello(_) => {
            // Handshake already done, this is unexpected
            Err(anyhow::anyhow!("Unexpected SdkHello after handshake"))
        }
        SdkMessage::User(user_msg) => {
            info!("Received user message");
            // TODO: Process user message through conversation engine
            // For now, send a placeholder response
            process_user_message(transport, session_id, user_msg).await?;
            Ok(true)
        }
        SdkMessage::ControlRequest(envelope) => {
            info!("Received control request: {:?}", envelope.request_id);
            // Handle outbound control requests (SDK → CLI commands like interrupt)
            handle_outbound_control_request(transport, session_id, envelope).await?;
            Ok(true)
        }
        SdkMessage::ControlResponse(envelope) => {
            // Handle response to our control request
            transport.handle_control_response(envelope).await?;
            Ok(true)
        }
        SdkMessage::ControlCancelRequest(cancel) => {
            info!("Received cancel request for: {}", cancel.request_id);
            // TODO: Cancel pending request
            Ok(true)
        }
    }
}

/// Process a user message through the conversation engine.
async fn process_user_message(
    transport: &mut SdkTransport,
    session_id: &str,
    user_msg: codex_sdk_protocol::messages::UserMessage,
) -> Result<(), anyhow::Error> {
    use codex_sdk_protocol::events::ThreadEvent;
use codex_sdk_protocol::events::ThreadStartedEvent;
use codex_sdk_protocol::events::TurnCompletedEvent;
use codex_sdk_protocol::events::TurnStartedEvent;
use codex_sdk_protocol::events::Usage;

    // TODO: This is a placeholder. The real implementation should:
    // 1. Create or resume a conversation
    // 2. Process the user message through the agent loop
    // 3. Stream events back via the transport

    // For now, send minimal events to demonstrate the protocol
    let event_id = uuid::Uuid::new_v4().to_string();

    // Send thread started event
    let thread_id = uuid::Uuid::new_v4().to_string();
    transport.write_message(&CliMessage::StreamEvent(StreamEventMessage {
        session_id: session_id.to_string(),
        uuid: event_id.clone(),
        event: ThreadEvent::ThreadStarted(ThreadStartedEvent {
            thread_id: thread_id.clone(),
        }),
    }))?;

    // Send turn started event
    transport.write_message(&CliMessage::StreamEvent(StreamEventMessage {
        session_id: session_id.to_string(),
        uuid: uuid::Uuid::new_v4().to_string(),
        event: ThreadEvent::TurnStarted(TurnStartedEvent {}),
    }))?;

    // Send turn completed event
    transport.write_message(&CliMessage::StreamEvent(StreamEventMessage {
        session_id: session_id.to_string(),
        uuid: uuid::Uuid::new_v4().to_string(),
        event: ThreadEvent::TurnCompleted(TurnCompletedEvent {
            usage: Usage::default(),
        }),
    }))?;

    // Send result message
    transport.write_message(&CliMessage::Result(ResultMessage {
        session_id: session_id.to_string(),
        subtype: ResultSubtype::Success,
        is_error: false,
        response: Some("Message processed (SDK mode placeholder)".to_string()),
        error: None,
        thread_id: Some(thread_id),
    }))?;

    info!(
        "Processed user message (placeholder), role={:?}",
        user_msg.message.role
    );

    Ok(())
}

/// Handle an outbound control request from the SDK.
async fn handle_outbound_control_request(
    transport: &mut SdkTransport,
    _session_id: &str,
    envelope: codex_sdk_protocol::control::ControlRequestEnvelope,
) -> Result<(), anyhow::Error> {
    use codex_sdk_protocol::control::ControlResponse;
    use codex_sdk_protocol::control::ControlResponseEnvelope;
    use codex_sdk_protocol::control::OutboundControlRequest;
    use codex_sdk_protocol::control::OutboundControlResponse;

    let response = match &envelope.request {
        codex_sdk_protocol::control::ControlRequest::Outbound(outbound) => match outbound {
            OutboundControlRequest::Interrupt => {
                info!("Received interrupt request");
                // TODO: Actually interrupt the current operation
                ControlResponse::Outbound(OutboundControlResponse::Success)
            }
            OutboundControlRequest::SetPermissionMode { mode } => {
                info!("Received set permission mode request: {mode:?}");
                // TODO: Actually set the permission mode
                ControlResponse::Outbound(OutboundControlResponse::Success)
            }
            OutboundControlRequest::SetModel { model } => {
                info!("Received set model request: {model:?}");
                // TODO: Actually set the model
                ControlResponse::Outbound(OutboundControlResponse::Success)
            }
            OutboundControlRequest::RewindFiles { user_message_id } => {
                info!("Received rewind files request: {user_message_id}");
                // TODO: Actually rewind files
                ControlResponse::Outbound(OutboundControlResponse::Success)
            }
            OutboundControlRequest::StreamInput { input: _ } => {
                info!("Received stream input request");
                // TODO: Handle streaming input
                ControlResponse::Outbound(OutboundControlResponse::Success)
            }
        },
        codex_sdk_protocol::control::ControlRequest::Inbound(_) => {
            // Inbound requests shouldn't come from the SDK
            ControlResponse::Outbound(OutboundControlResponse::Error {
                message: "Unexpected inbound control request from SDK".to_string(),
            })
        }
    };

    transport.write_message(&CliMessage::ControlResponse(ControlResponseEnvelope {
        request_id: envelope.request_id,
        response,
    }))?;

    Ok(())
}

/// Send an error result to the SDK.
fn send_error_result(
    transport: &mut SdkTransport,
    session_id: &str,
    error: &str,
) -> Result<(), TransportError> {
    transport.write_message(&CliMessage::Result(ResultMessage {
        session_id: session_id.to_string(),
        subtype: ResultSubtype::Error,
        is_error: true,
        response: None,
        error: Some(error.to_string()),
        thread_id: None,
    }))
}
