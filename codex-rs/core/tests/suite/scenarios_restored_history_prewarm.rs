//! Complete request histories for cold resumes with historical images.

use super::*;
use codex_protocol::models::ResponseInputItem;
use core_test_support::responses;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::WebSocketTestServer;
use core_test_support::responses::start_websocket_server_with_headers;
use pretty_assertions::assert_eq;

fn image_input(id: &str) -> UserInput {
    UserInput::Image {
        image: ImageReference::File { file_id: id.into() },
        detail: None,
    }
}

fn socket(warmup: &str, delay: Option<Duration>) -> WebSocketConnectionConfig {
    WebSocketConnectionConfig {
        requests: vec![
            vec![ev_response_created(warmup), ev_completed(warmup)],
            vec![
                ev_response_created("generation"),
                ev_assistant_message("msg_seen", "seen"),
                ev_completed("generation"),
            ],
        ],
        response_headers: vec![],
        accept_delay: delay,
        close_after_requests: true,
    }
}

async fn saved_image_thread(
    server: &WebSocketTestServer,
    store: Arc<RecordingFileAttachmentStore>,
) -> anyhow::Result<TestCodex> {
    let test = test_codex()
        .with_image_store(store)
        .with_config(|config| {
            config
                .features
                .enable(Feature::RestoredHistoryPrewarm)
                .expect("enable restored history prewarm");
        })
        .build_with_websocket_server(server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![image_input(
            "file_prior",
        )]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    Ok(test)
}

async fn reopen(test: &mut TestCodex, base_url: Option<String>) -> anyhow::Result<()> {
    let path = test.codex.rollout_path().expect("saved thread rollout");
    test.codex.shutdown_and_wait().await?;
    let mut options = codex_core::StartThreadOptions::new(test.config.clone());
    if let Some(url) = base_url {
        options.config.model_provider.base_url = Some(url);
    }
    options.initial_history = codex_rollout::RolloutRecorder::get_rollout_history(&path).await?;
    test.codex = test.thread_manager.start_thread(options).await?.thread;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn text_history_uses_base_warmup_and_sends_history_once() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_websocket_server_with_headers(vec![
        socket("initial", /*delay*/ None),
        socket("base", /*delay*/ None),
    ])
    .await;
    let mut test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::RestoredHistoryPrewarm)
                .expect("enable restored history prewarm");
        })
        .build_with_websocket_server(&server)
        .await?;
    test.submit_text_turn("saved text marker").await?;
    reopen(&mut test, /*base_url*/ None).await?;
    test.submit_text_turn("continue").await?;

    let connections = server.connections();
    let [warm, generation] = connections[1].as_slice() else {
        panic!("expected warm-up and generation");
    };
    assert_eq!(warm.body_json()["generate"], false);
    assert_eq!(
        warm.body_json()["input"],
        connections[0][0].body_json()["input"]
    );
    let generation = generation.body_json();
    assert_eq!(generation["previous_response_id"], "base");
    assert!(
        generation["input"]
            .to_string()
            .contains("saved text marker")
    );
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn warmup_includes_interrupted_image_and_generation_adds_injected_image() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));
    let server = start_websocket_server_with_headers(vec![
        socket("initial", /*delay*/ None),
        socket("held", /*delay*/ None),
        socket("restored", Some(Duration::from_millis(500))),
    ])
    .await;
    let mut test = saved_image_thread(&server, Arc::default()).await?;
    let release = server.gate_response_event(&ev_completed("held"));
    reopen(&mut test, /*base_url*/ None).await?;
    tokio::time::timeout(
        Duration::from_secs(15),
        server.wait_for_request(/*connection_index*/ 1, /*request_index*/ 0),
    )
    .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![image_input(
            "file_interrupted",
        )]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnStarted(_))).await;
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;
    let _ = release.send(());
    reopen(&mut test, /*base_url*/ None).await?;
    test.codex
        .inject_response_items(vec![ResponseItem::from(ResponseInputItem::from(vec![
            image_input("file_injected"),
        ]))])
        .await?;
    test.submit_text_turn("continue").await?;

    let connections = server.connections();
    assert_eq!(connections[1].len(), 1, "interrupt must precede generation");
    let [warm, generation] = connections[2].as_slice() else {
        panic!("expected warm-up and generation");
    };
    assert_eq!(warm.body_json()["generate"], false);
    // The snapshot formatter summarizes image slots, so check their file IDs here.
    let warm = warm.body_json()["input"].to_string();
    assert!(warm.contains("file_prior"));
    assert!(warm.contains("file_interrupted"));
    assert!(!warm.contains("file_injected"));
    let generation = generation.body_json();
    assert!(generation.get("previous_response_id").is_none());
    let items = generation["input"].as_array().expect("generation input");
    let image_position = |id: &str| {
        items
            .iter()
            .position(|item| {
                item["content"]
                    .as_array()
                    .is_some_and(|parts| parts.iter().any(|part| part["file_id"] == id))
            })
            .expect("submitted image")
    };
    let submit = items
        .iter()
        .position(|item| {
            item["role"] == "user"
                && item["content"]
                    .as_array()
                    .is_some_and(|parts| parts.iter().any(|part| part["text"] == "continue"))
        })
        .expect("current user message");
    assert!(image_position("file_prior") < image_position("file_interrupted"));
    assert!(image_position("file_interrupted") < image_position("file_injected"));
    assert!(image_position("file_injected") < submit);

    let bodies: Vec<_> = connections
        .iter()
        .flatten()
        .map(responses::WebSocketRequest::body_json)
        .collect();
    let labels = [
        "initial warm-up",
        "original image submitted",
        "warm-up before interrupt",
        "resumed warm-up",
        "generation after injection",
    ];
    assert_eq!(bodies.len(), labels.len());
    let entries = bodies
        .iter()
        .zip(labels)
        .map(|(body, label)| SnapshotEntry::body(body).labeled(label))
        .collect::<Vec<_>>();
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "Warm-up includes the previously sampled image and the restored interrupted submit. Generation resends both images and adds the image injected after resume.",
        &entries,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upgrade_required_prepares_saved_image_only_for_http_generation() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path_regex;
    let server = start_websocket_server_with_headers(vec![socket("initial", /*delay*/ None)]).await;
    let store = Arc::new(RecordingFileAttachmentStore::default());
    let mut test = saved_image_thread(&server, Arc::clone(&store)).await?;
    let http = responses::start_mock_server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/responses$"))
        .respond_with(ResponseTemplate::new(426))
        .expect(1)
        .mount(&http)
        .await;
    let post = responses::mount_sse_once(
        &http,
        responses::sse(vec![ev_response_created("http"), ev_completed("http")]),
    )
    .await;
    store.resolves.lock().unwrap().clear();
    reopen(&mut test, Some(format!("{}/v1", http.uri()))).await?;
    test.submit_text_turn("continue").await?;
    let body = post.single_request().body_json();
    assert_ne!(body["generate"], false);
    assert!(body["input"].to_string().contains("file_prior"));
    assert_eq!(store.resolves.lock().unwrap().get("file_prior"), Some(&1));
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vision_resume_resends_previously_omitted_image_then_continues_from_generation()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    let mut resumed = socket("vision", /*delay*/ None);
    resumed.requests.push(vec![
        ev_response_created("next"),
        ev_assistant_message("msg_next", "seen again"),
        ev_completed("next"),
    ]);
    let server =
        start_websocket_server_with_headers(vec![socket("text", /*delay*/ None), resumed]).await;
    let mut test = test_codex()
        .with_model_info_override("text-only-prewarm-test", |model| {
            model.input_modalities = vec![InputModality::Text]
        })
        .with_model("text-only-prewarm-test")
        .with_config(|config| {
            config
                .features
                .enable(Feature::RestoredHistoryPrewarm)
                .expect("enable restored history prewarm");
        })
        .build_with_websocket_server(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![image_input(
            "file_omitted",
        )]))
        .await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    test.config.model = Some("gpt-5.5".into());
    reopen(&mut test, /*base_url*/ None).await?;
    test.submit_text_turn("Describe the image.").await?;
    test.submit_text_turn("continue").await?;

    let connections = server.connections();
    let bodies: Vec<_> = connections
        .iter()
        .flatten()
        .map(responses::WebSocketRequest::body_json)
        .collect();
    assert_eq!(connections.len(), 2);
    assert_eq!(connections[1].len(), 3);
    assert_eq!(bodies.len(), 5);
    assert!(
        bodies[1]["input"]
            .to_string()
            .contains("image content omitted")
    );
    assert!(!bodies[1]["input"].to_string().contains("file_omitted"));
    assert_eq!(bodies[2]["model"], "gpt-5.5");
    assert!(bodies[2]["input"].to_string().contains("file_omitted"));
    assert!(bodies[3]["input"].to_string().contains("file_omitted"));
    assert!(bodies[3].get("previous_response_id").is_none());
    assert_eq!(bodies[4]["previous_response_id"], "generation");
    assert!(!bodies[4]["input"].to_string().contains("file_omitted"));
    let entries = bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(context_snapshot::format_context_snapshot(
        "A text-only model sees a placeholder. A vision resume warms the image and submits it again on generation. The following turn continues from the real response.",
        &entries,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
