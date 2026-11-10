//! Standalone consumer; prints event kinds without logging payloads.
use codex_cloud_client::AddUserMessageRequest;
use codex_cloud_client::AdmissionOutcome;
use codex_cloud_client::Client;
use codex_cloud_client::Credentials;
use codex_cloud_client::ResumeRequest;
use codex_cloud_client::SubmissionMode;
use codex_cloud_client::UserMessageInput;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let endpoint = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("expected native gRPC origin"))?;
    let thread_id = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("expected existing thread ID"))?;
    // Supplying both extra arguments explicitly opts into sending one message.
    let submission = match (args.next(), args.next()) {
        (None, None) => None,
        (Some(client_user_message_id), Some(text)) => Some(AddUserMessageRequest {
            thread_id: thread_id.clone(),
            client_user_message_id,
            input: vec![UserMessageInput::Text {
                text: text,
                text_elements: vec![],
            }],
            turn_trigger: None,
            model: None,
            effort: None,
            mode: SubmissionMode::StartIfIdle,
            shared_access: false,
        }),
        _ => anyhow::bail!("expected both client message ID and text, or neither"),
    };
    if args.next().is_some() {
        anyhow::bail!("unexpected argument");
    }
    let credentials = Credentials::new(
        &std::env::var("CODEX_CLOUD_ACCESS_TOKEN")?,
        &std::env::var("CODEX_CLOUD_ACCOUNT_ID")?,
    )?;
    let client = Client::new(
        &HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        &endpoint,
        credentials,
    )?;
    client
        .resume(&ResumeRequest {
            thread_id: thread_id.clone(),
            wait: false,
            shared_access: false,
        })
        .await?;
    let mut events = client.attach(&thread_id).await?;
    if let Some(request) = submission {
        // Drain Attach while admission is pending; events can precede its response.
        let admission = client.add_user_message(&request);
        tokio::pin!(admission);
        let (result, stream_error) = loop {
            tokio::select! {
                result = &mut admission => {
                    break (result, None);
                }
                event = events.next() => match event {
                    Some(Ok(event)) => println!("{event:?}"),
                    Some(Err(error)) => break (admission.await, Some(anyhow::Error::new(error))),
                    None => break (admission.await, Some(anyhow::anyhow!("attachment ended"))),
                }
            }
        };
        match result {
            Ok(admission) => println!(
                "admission: {}",
                match admission.outcome {
                    AdmissionOutcome::Started(_) => "started",
                    AdmissionOutcome::Steered(_) => "steered",
                    AdmissionOutcome::Duplicate { .. } => "duplicate",
                    AdmissionOutcome::Queued { .. } => "queued",
                }
            ),
            Err(error) => match stream_error {
                Some(stream_error) => {
                    anyhow::bail!("attachment failed: {stream_error}; admission failed: {error}");
                }
                None => return Err(error.into()),
            },
        }
        if let Some(error) = stream_error {
            return Err(error.context("attachment failed"));
        }
    }
    while let Some(event) = events.next().await {
        println!("{:?}", event?);
    }
    Ok(())
}
