//cargo test --test test_message_protocol --features "client server"

mod common;
use common::handlers::{TestClientHandler, TestServer};
use rmcp::{
    ServiceExt,
    model::*,
    service::{RequestContext, Service},
};
use tokio_util::sync::CancellationToken;

// Tests start here
#[tokio::test]
async fn test_message_roles() {
    let messages = vec![
        SamplingMessage {
            role: Role::User,
            content: Content::text("user message"),
        },
        SamplingMessage {
            role: Role::Assistant,
            content: Content::text("assistant message"),
        },
    ];

    // Verify all roles can be serialized/deserialized correctly
    let json = serde_json::to_string(&messages).unwrap();
    let deserialized: Vec<SamplingMessage> = serde_json::from_str(&json).unwrap();
    assert_eq!(messages, deserialized);
}

#[tokio::test]
async fn test_context_inclusion_integration() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    // Start server
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    // Start client that honors context requests
    let handler = TestClientHandler::new(true, true);
    let client = handler.clone().serve(client_transport).await?;

    // Test ThisServer context inclusion
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test message"),
            }],
            include_context: Some(ContextInclusion::ThisServer),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            text.contains("test context"),
            "Response should include context for ThisServer"
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    // Test AllServers context inclusion
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test message"),
            }],
            include_context: Some(ContextInclusion::AllServers),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                id: NumberOrString::Number(2),
                meta: Default::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            text.contains("test context"),
            "Response should include context for AllServers"
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    // Test No context inclusion
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test message"),
            }],
            include_context: Some(ContextInclusion::None),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                id: NumberOrString::Number(3),
                meta: Default::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            !text.contains("test context"),
            "Response should not include context for None"
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_context_inclusion_ignored_integration() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    // Start server
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    // Start client that ignores context requests
    let handler = TestClientHandler::new(false, false);
    let client = handler.clone().serve(client_transport).await?;

    // Test that context requests are ignored
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test message"),
            }],
            include_context: Some(ContextInclusion::ThisServer),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            !text.contains("test context"),
            "Context should be ignored when client chooses not to honor requests"
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_message_sequence_integration() -> anyhow::Result<()> {
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

    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![
                SamplingMessage {
                    role: Role::User,
                    content: Content::text("first message"),
                },
                SamplingMessage {
                    role: Role::Assistant,
                    content: Content::text("second message"),
                },
            ],
            include_context: Some(ContextInclusion::ThisServer),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            text.contains("test context"),
            "Response should include context when ThisServer is specified"
        );
        assert_eq!(result.model, "test-model");
        assert_eq!(
            result.stop_reason,
            Some(CreateMessageResult::STOP_REASON_END_TURN.to_string())
        );
    } else {
        panic!("Expected CreateMessageResult");
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_message_sequence_validation_integration() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    let handler = TestClientHandler::new(true, true);
    let client = handler.clone().serve(client_transport).await?;

    // Test valid sequence: User -> Assistant -> User
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![
                SamplingMessage {
                    role: Role::User,
                    content: Content::text("first user message"),
                },
                SamplingMessage {
                    role: Role::Assistant,
                    content: Content::text("first assistant response"),
                },
                SamplingMessage {
                    role: Role::User,
                    content: Content::text("second user message"),
                },
            ],
            include_context: None,
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    assert!(matches!(result, ClientResult::CreateMessageResult(_)));

    // Test invalid: No user message
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::Assistant,
                content: Content::text("assistant message"),
            }],
            include_context: None,
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                id: NumberOrString::Number(2),
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await;

    assert!(result.is_err());

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_selective_context_handling_integration() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    // Client that only honors ThisServer but ignores AllServers
    let handler = TestClientHandler::new(true, false);
    let client = handler.clone().serve(client_transport).await?;

    // Test ThisServer is honored
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test message"),
            }],
            include_context: Some(ContextInclusion::ThisServer),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            text.contains("test context"),
            "ThisServer context request should be honored"
        );
    }

    // Test AllServers is ignored
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test message"),
            }],
            include_context: Some(ContextInclusion::AllServers),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                id: NumberOrString::Number(2),
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(
            !text.contains("test context"),
            "AllServers context request should be ignored"
        );
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_context_inclusion() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    let handler = TestClientHandler::new(true, true);
    let client = handler.clone().serve(client_transport).await?;

    // Test context handling
    let request = ServerRequest::CreateMessageRequest(CreateMessageRequest {
        method: Default::default(),
        params: CreateMessageRequestParam {
            messages: vec![SamplingMessage {
                role: Role::User,
                content: Content::text("test"),
            }],
            include_context: Some(ContextInclusion::ThisServer),
            model_preferences: None,
            system_prompt: None,
            temperature: None,
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
                meta: Meta::default(),
                extensions: Default::default(),
            },
        )
        .await?;

    if let ClientResult::CreateMessageResult(result) = result {
        let text = result.message.content.as_text().unwrap().text.as_str();
        assert!(text.contains("test context"));
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
