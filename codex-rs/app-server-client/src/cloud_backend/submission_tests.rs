#![cfg(test)]
//! Exercises native admission and local validation through the command worker.

use super::*;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn submission(message_id: &str) -> protocol::TurnStartParams {
    protocol::TurnStartParams {
        thread_id: "t".into(),
        client_user_message_id: Some(message_id.into()),
        input: vec![protocol::UserInput::Text {
            text: "hello".into(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn native_submission_uses_one_admission_and_keeps_preflight_guards() -> Result<()> {
    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
    let native = cloud::Client::new(
        &config.http_client_factory(),
        &server.uri(),
        cloud::Credentials::new("synthetic-bearer", "synthetic-account")?,
    )?;
    let account = protocol::GetAccountResponse {
        user_id: None,
        account: None,
        requires_openai_auth: true,
        workspace_routing: None,
    };
    let (client, commands, output) = crate::CloudClient::channel(/*capacity*/ 4);
    let client = crate::AppServerClient::Cloud(client);
    let handle = client.request_handle();
    let crate::AppServerRequestHandle::Cloud(cloud_handle) = &handle else {
        panic!("expected a Cloud handle");
    };
    let (reply_sender, stream) = futures::channel::mpsc::unbounded();
    let mut replies = Replies {
        active: Some(Attachment {
            thread_id: "t".into(),
            stream: stream.boxed(),
        }),
        ..Replies::default()
    };
    let worker = tokio::spawn(async move {
        let mut requests = crate::cloud_backend::requests::Requests {
            client: native,
            account,
            loaded: None,
            warned_partial_history: None,
        };
        crate::cloud_backend::run_worker(commands, &mut requests, &mut replies, &output).await
    });
    let rpc = "/oaiproto.codex_app_server_backend.ThreadService/AddUserMessage";
    Mock::given(method("POST"))
        .and(path(rpc))
        .and(header("authorization", "Bearer synthetic-bearer"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2)
        .mount(&server)
        .await;
    let Err(crate::TypedRequestError::Server { source, .. }) =
        cloud_handle.submit_user_message(submission("x")).await
    else {
        panic!("a lost receipt must remain uncertain");
    };
    assert_eq!(source.code, -32000);
    assert!(source.message.contains("mutation outcome unknown"));
    // Existing typed turn/start callers keep StartIfIdle and the same native error.
    let Err(crate::TypedRequestError::Server { source, .. }) = handle
        .request_typed::<protocol::TurnStartResponse>(protocol::ClientRequest::TurnStart {
            request_id: protocol::RequestId::Integer(1),
            params: submission("m"),
        })
        .await
    else {
        panic!("ordinary turn/start must retain its server error");
    };
    assert_eq!(source.code, -32000);
    assert!(source.message.contains("mutation outcome unknown"));

    for params in [
        protocol::TurnStartParams {
            model: Some("other".into()),
            ..submission("invalid")
        },
        protocol::TurnStartParams {
            client_user_message_id: None,
            ..submission("missing")
        },
        protocol::TurnStartParams {
            input: Vec::new(),
            ..submission("empty")
        },
        protocol::TurnStartParams {
            thread_id: "other".into(),
            ..submission("foreign")
        },
    ] {
        let Err(crate::TypedRequestError::Server { source, .. }) =
            cloud_handle.submit_user_message(params).await
        else {
            panic!("invalid input must fail before admission");
        };
        assert_eq!(source.code, -32602);
    }
    let queued = handle
        .request(protocol::ClientRequest::TurnStart {
            request_id: protocol::RequestId::Integer(3),
            params: protocol::TurnStartParams {
                input: vec![protocol::UserInput::LocalImage {
                    path: "queued.png".into(),
                    detail: None,
                }],
                ..submission("queued-image")
            },
        })
        .await?
        .expect_err("a queued attachment must fail before admission");
    assert_eq!(queued.code, -32602);
    assert_eq!(
        queued.message,
        "Cloud conversations currently accept plain text only."
    );

    drop(reply_sender);
    let Err(crate::TypedRequestError::Server { source, .. }) =
        cloud_handle.submit_user_message(submission("closed")).await
    else {
        panic!("a disconnected stream must reject input");
    };
    assert_eq!(source.code, -32602);
    let ordinary = handle
        .request(protocol::ClientRequest::TurnStart {
            request_id: protocol::RequestId::Integer(2),
            params: submission("closed"),
        })
        .await?
        .expect_err("ordinary turn/start keeps its existing disconnected error");
    assert_eq!(ordinary.code, -32000);

    client.shutdown().await?;
    worker.await?;
    let calls = server.received_requests().await.unwrap();
    assert_eq!(
        calls
            .iter()
            .map(|request| (request.body[7], *request.body.last().unwrap()))
            .collect::<Vec<_>>(),
        vec![(b'x', 1), (b'm', 3)]
    );
    Ok(())
}
