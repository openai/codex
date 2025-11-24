//cargo test --test test_sampling --features "client server"

mod common;

use anyhow::Result;
use common::handlers::{TestClientHandler, TestServer};
use rmcp::{
    ServiceExt,
    model::*,
    service::{RequestContext, Service},
};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_basic_sampling_message_creation() -> Result<()> {
    // Test basic sampling message structure
    let message = SamplingMessage {
        role: Role::User,
        content: Content::text("What is the capital of France?"),
    };

    // Verify serialization/deserialization
    let json = serde_json::to_string(&message)?;
    let deserialized: SamplingMessage = serde_json::from_str(&json)?;
    assert_eq!(message, deserialized);
    assert_eq!(message.role, Role::User);

    Ok(())
}

#[tokio::test]
async fn test_sampling_request_params() -> Result<()> {
    // Test sampling request parameters structure
    let params = CreateMessageRequestParam {
        messages: vec![SamplingMessage {
            role: Role::User,
            content: Content::text("Hello, world!"),
        }],
        model_preferences: Some(ModelPreferences {
            hints: Some(vec![ModelHint {
                name: Some("claude".to_string()),
            }]),
            cost_priority: Some(0.5),
            speed_priority: Some(0.8),
            intelligence_priority: Some(0.7),
        }),
        system_prompt: Some("You are a helpful assistant.".to_string()),
        temperature: Some(0.7),
        max_tokens: 100,
        stop_sequences: Some(vec!["STOP".to_string()]),
        include_context: Some(ContextInclusion::None),
        metadata: Some(serde_json::json!({"test": "value"})),
    };

    // Verify serialization/deserialization
    let json = serde_json::to_string(&params)?;
    let deserialized: CreateMessageRequestParam = serde_json::from_str(&json)?;
    assert_eq!(params, deserialized);

    // Verify specific fields
    assert_eq!(params.messages.len(), 1);
    assert_eq!(params.max_tokens, 100);
    assert_eq!(params.temperature, Some(0.7));

    Ok(())
}

#[tokio::test]
async fn test_sampling_result_structure() -> Result<()> {
    // Test sampling result structure
    let result = CreateMessageResult {
        message: SamplingMessage {
            role: Role::Assistant,
            content: Content::text("The capital of France is Paris."),
        },
        model: "test-model".to_string(),
        stop_reason: Some(CreateMessageResult::STOP_REASON_END_TURN.to_string()),
    };

    // Verify serialization/deserialization
    let json = serde_json::to_string(&result)?;
    let deserialized: CreateMessageResult = serde_json::from_str(&json)?;
    assert_eq!(result, deserialized);

    // Verify specific fields
    assert_eq!(result.message.role, Role::Assistant);
    assert_eq!(result.model, "test-model");
    assert_eq!(
        result.stop_reason,
        Some(CreateMessageResult::STOP_REASON_END_TURN.to_string())
    );

    Ok(())
}

#[tokio::test]
async fn test_sampling_context_inclusion_enum() -> Result<()> {
    // Test context inclusion enum values
    let test_cases = vec![
        (ContextInclusion::None, "none"),
        (ContextInclusion::ThisServer, "thisServer"),
        (ContextInclusion::AllServers, "allServers"),
    ];

    for (context, expected_json) in test_cases {
        let json = serde_json::to_string(&context)?;
        assert_eq!(json, format!("\"{}\"", expected_json));

        let deserialized: ContextInclusion = serde_json::from_str(&json)?;
        assert_eq!(context, deserialized);
    }

    Ok(())
}

#[tokio::test]
async fn test_sampling_integration_with_test_handlers() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    // Start server
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    // Start client that honors sampling requests
    let handler = TestClientHandler::new(true, true);
    let client = handler.clone().serve(client_transport).await?;

    // Wait for initialization
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test sampling with context inclusion
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("What is the capital of France?"),
            }],
            include_context: Some(ContextInclusion::ThisServer),
            model_preferences: Some(ModelPreferences {
                hints: Some(vec![ModelHint {
                    name: Some("test-model".to_string()),
                }]),
                cost_priority: Some(0.5),
                speed_priority: Some(0.8),
                intelligence_priority: Some(0.7),
            }),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            temperature: Some(0.7),
            max_tokens: 100,
            stop_sequences: None,
            metadata: None,
        },
        extensions: Default::default(),
    });

    let result = handler
        .handle_request(
            request.clone(),
            RequestContext {
                peer: client.peer().clone(),
                ct: CancellationToken::new(),
                id: NumberOrString::Number(1),
                meta: Default::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    // Verify the response
    if let ClientResult::CreateMessageResult(result) = result {
        assert_eq!(result.message.role, Role::Assistant);
        assert_eq!(result.model, "test-model");
        assert_eq!(
            result.stop_reason,
            Some(CreateMessageResult::STOP_REASON_END_TURN.to_string())
        );

        let response_text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            response_text.contains("test context"),
            "Response should include context for ThisServer inclusion"
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_sampling_no_context_inclusion() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    // Start server
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    // Start client that honors sampling requests
    let handler = TestClientHandler::new(true, true);
    let client = handler.clone().serve(client_transport).await?;

    // Wait for initialization
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test sampling without context inclusion
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("Hello"),
            }],
            include_context: Some(ContextInclusion::None),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
            max_tokens: 50,
            stop_sequences: None,
            metadata: None,
        },
        extensions: Default::default(),
    });

    let result = handler
        .handle_request(
            request.clone(),
            RequestContext {
                peer: client.peer().clone(),
                ct: CancellationToken::new(),
                id: NumberOrString::Number(2),
                meta: Default::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    // Verify the response
    if let ClientResult::CreateMessageResult(result) = result {
        assert_eq!(result.message.role, Role::Assistant);
        assert_eq!(result.model, "test-model");

        let response_text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            !response_text.contains("test context"),
            "Response should not include context for None inclusion"
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_sampling_error_invalid_message_sequence() -> Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    // Start server
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    // Start client
    let handler = TestClientHandler::new(true, true);
    let client = handler.clone().serve(client_transport).await?;

    // Wait for initialization
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test sampling with no user messages (should fail)
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::Assistant,
                content: Content::text("I'm an assistant message without a user message"),
            }],
            include_context: Some(ContextInclusion::None),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
            max_tokens: 50,
            stop_sequences: None,
            metadata: None,
        },
        extensions: Default::default(),
    });

    let result = handler
        .handle_request(
            request.clone(),
            RequestContext {
                peer: client.peer().clone(),
                ct: CancellationToken::new(),
                id: NumberOrString::Number(3),
                meta: Default::default(),
                extensions: Default::default(),
            },
        )
        .await;

    // This should result in an error
    assert!(result.is_err());

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
