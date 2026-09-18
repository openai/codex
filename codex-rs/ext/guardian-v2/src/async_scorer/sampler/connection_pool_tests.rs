//! Exercises classifier fallback, cooldown, and cancellation against local servers.

use super::super::LunaSampler;
use super::super::tests::sample_request;
use super::super::tests::sampler_config;
use super::*;
use anyhow::Result;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

// Routes HTTP normally while selectively stalling WebSocket handshakes.
struct Gateway {
    url: String,
    opens: Arc<AtomicUsize>,
    allowed_opens: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl Gateway {
    async fn new(http_url: &str, websocket_url: &str) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let http_target = http_url.trim_start_matches("http://").to_owned();
        let websocket_target = websocket_url.trim_start_matches("ws://").to_owned();
        let opens = Arc::new(AtomicUsize::new(0));
        let allowed_opens = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&opens);
        let allowed = Arc::clone(&allowed_opens);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            while let Ok((mut incoming, _)) = listener.accept().await {
                let http_target = http_target.clone();
                let websocket_target = websocket_target.clone();
                let count = Arc::clone(&count);
                let allowed = Arc::clone(&allowed);
                connections.spawn(async move {
                    let mut method = [0; 4];
                    incoming.read_exact(&mut method).await?;
                    let target = if &method == b"GET " {
                        let attempt = count.fetch_add(/*val*/ 1, Ordering::SeqCst) + 1;
                        if attempt > allowed.load(Ordering::SeqCst) {
                            tokio::io::copy(&mut incoming, &mut tokio::io::sink()).await?;
                            return Ok::<_, std::io::Error>(());
                        }
                        websocket_target
                    } else {
                        http_target
                    };
                    let mut outgoing = TcpStream::connect(target).await?;
                    outgoing.write_all(&method).await?;
                    tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await?;
                    Ok(())
                });
            }
        });
        Ok(Self {
            url,
            opens,
            allowed_opens,
            task,
        })
    }
}

impl Drop for Gateway {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn cold_pool_uses_http_during_open_timeout_then_recovers_after_cooldown() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for uses_codex_backend in [false, true] {
        let http = responses::start_mock_server().await;
        let events = vec![
            responses::ev_output_text_delta("low"),
            responses::ev_completed("score"),
        ];
        let http_mock =
            responses::mount_sse_sequence(&http, vec![responses::sse(events.clone()); 3]).await;
        let mut connections = vec![Vec::new(); INITIAL_WEBSOCKET_CONNECTIONS - 1];
        connections.push(vec![events.clone()]);
        let ws = responses::start_websocket_server(connections).await;
        let gateway = Gateway::new(&http.uri(), ws.uri()).await?;
        let base_path = if uses_codex_backend {
            "/backend-api/codex"
        } else {
            "/v1"
        };
        let base_url = format!("{}{base_path}", gateway.url);
        let mut config = sampler_config(base_url.clone());
        config.provider = create_model_provider(
            ModelProviderInfo::create_openai_provider(Some(base_url)),
            Some(AuthManager::from_auth_for_testing(if uses_codex_backend {
                CodexAuth::create_dummy_chatgpt_auth_for_testing()
            } else {
                CodexAuth::from_api_key("test-api-key")
            })),
        );
        config.service_tier = Some("priority".to_owned());
        let sampler = LunaSampler::new(config);
        let opener = sampler.connections.replenish().unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while gateway.opens.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await?;
        let mut request = sample_request("parent-turn");
        request.parent_response_id = Some("resp-parent".to_owned());
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), sampler.sample(request)).await??,
            "low"
        );
        assert_eq!(
            sampler.sample(sample_request("parent-turn-2")).await?,
            "low"
        );
        assert_eq!(gateway.opens.load(Ordering::SeqCst), 1);
        assert!(!opener.is_finished());
        assert!(sampler.connections.replenish().is_none());

        tokio::time::pause();
        tokio::time::advance(Duration::from_secs(15)).await;
        opener.await?;
        tokio::time::resume();
        assert!(sampler.connections.replenish().is_none());
        assert_eq!(
            sampler.sample(sample_request("during-cooldown")).await?,
            "low"
        );
        assert_eq!(gateway.opens.load(Ordering::SeqCst), 1);

        let requests = http_mock.requests();
        let first = &requests[0];
        let expected_path = if uses_codex_backend {
            "/backend-api/codex/responses"
        } else {
            "/v1/responses"
        };
        assert_eq!(first.path(), expected_path);
        assert_eq!(
            first.header("x-codex-guardian").as_deref(),
            uses_codex_backend.then_some("classifier")
        );
        assert!(first.header("authorization").is_some());
        let body = first.body_json();
        assert_eq!(
            body["service_tier"].as_str(),
            if uses_codex_backend {
                None
            } else {
                Some("priority")
            }
        );
        assert_eq!(
            body["client_metadata"]["parent_response_id"].as_str(),
            uses_codex_backend.then_some("resp-parent")
        );

        gateway.allowed_opens.store(usize::MAX, Ordering::SeqCst);
        tokio::time::pause();
        tokio::time::advance(CONNECT_COOLDOWN).await;
        tokio::time::resume();
        sampler.connections.replenish().unwrap().await?;
        assert_eq!(
            sampler.sample(sample_request("after-cooldown")).await?,
            "low"
        );
        assert_eq!(http_mock.requests().len(), 3);
        assert_eq!(
            gateway.opens.load(Ordering::SeqCst),
            1 + INITIAL_WEBSOCKET_CONNECTIONS
        );
    }
    Ok(())
}

#[tokio::test]
async fn cooldown_preserves_healthy_sockets_and_both_transports_share_capacity() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let http = responses::start_mock_server().await;
    let ws = responses::start_websocket_server_with_headers(vec![
        responses::WebSocketConnectionConfig {
            requests: Vec::new(),
            response_headers: Vec::new(),
            accept_delay: None,
            close_after_requests: false,
        },
    ])
    .await;
    let gateway = Gateway::new(&http.uri(), ws.uri()).await?;
    gateway.allowed_opens.store(/*val*/ 1, Ordering::SeqCst);
    let sampler = LunaSampler::new(sampler_config(format!("{}/v1", gateway.url)));
    let opener = sampler.connections.replenish().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while gateway.opens.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(15)).await;
    opener.await?;
    tokio::time::resume();
    let mut leases = Vec::new();
    leases.push(sampler.connections.lease().await?);
    assert!(matches!(leases[0].connection, Connection::Websocket(_)));
    for _ in 1..MAX_CONCURRENT_REQUESTS {
        let lease = sampler.connections.lease().await?;
        assert!(matches!(lease.connection, Connection::Http(_)));
        leases.push(lease);
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), sampler.connections.lease())
            .await
            .is_err()
    );
    leases.pop();
    assert!(matches!(
        sampler.connections.lease().await?.connection,
        Connection::Http(_)
    ));
    assert_eq!(gateway.opens.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn stalled_http_headers_exhaust_the_sampling_retry_budget() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let http = responses::start_mock_server().await;
    let mock = responses::mount_response_sequence(
        &http,
        vec![responses::sse_response(String::new()).set_delay(Duration::from_secs(60)); 3],
    )
    .await;
    let mut config = sampler_config(format!("{}/v1", http.uri()));
    let mut provider = config.provider.info().clone();
    provider.stream_idle_timeout_ms = Some(1_000);
    config.provider = create_model_provider(provider, config.provider.auth_manager());
    let sampler = LunaSampler::new(config);

    tokio::time::pause();
    let sample = tokio::time::timeout(
        Duration::from_secs(10),
        sampler.sample(sample_request("stalled-headers")),
    );
    tokio::pin!(sample);
    let deadline = std::time::Instant::now() + Duration::from_secs(/*secs*/ 30);
    for expected_requests in 1..=3 {
        // Keep the paused runtime runnable until the server records this attempt.
        // Otherwise auto-advance can exhaust a retry before its request arrives.
        while mock.requests().len() < expected_requests {
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "server did not receive request {expected_requests}"
            );
            tokio::select! {
                result = &mut sample => {
                    anyhow::bail!("sampling finished before request {expected_requests}: {result:?}");
                }
                () = tokio::task::yield_now() => {}
            }
        }
        tokio::time::advance(Duration::from_millis(/*millis*/ 1_001)).await;
    }
    let result = sample.await?;
    tokio::time::resume();
    assert!(matches!(
        result,
        Err(LunaSamplerError::Api(ApiError::Transport(
            TransportError::Timeout
        )))
    ));
    assert_eq!(mock.requests().len(), 3);
    assert_eq!(
        sampler.connections.classifications.available_permits(),
        MAX_CONCURRENT_REQUESTS
    );
    Ok(())
}

#[tokio::test]
async fn supersession_closes_http_before_headers_and_while_draining_the_body() -> Result<()> {
    skip_if_no_network!(Ok(()));
    // The second case publishes a score, then leaves the response body open forever.
    for response in [
        String::new(),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n{}",
            responses::sse(vec![responses::ev_output_text_delta("low")]),
        ),
    ] {
        let has_score = !response.is_empty();
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let sampler = Arc::new(LunaSampler::new(sampler_config(format!(
            "http://{}/v1",
            listener.local_addr()?,
        ))));
        let (received, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let (closed, mut disconnects) = tokio::sync::mpsc::unbounded_channel();
        let mut tasks = tokio::task::JoinSet::new();
        tasks.spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            let mut request_id = 0;
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut method = [0; 4];
                socket.read_exact(&mut method).await?;
                if &method != b"POST" {
                    continue;
                }
                request_id += 1;
                let received = received.clone();
                let closed = closed.clone();
                let response = response.clone();
                connections.spawn(async move {
                    socket.write_all(response.as_bytes()).await?;
                    let _ = received.send(request_id);
                    let result = tokio::io::copy(&mut socket, &mut tokio::io::sink()).await;
                    let _ = closed.send(request_id);
                    result
                });
            }
            Ok::<_, std::io::Error>(())
        });
        let mut samples = tokio::task::JoinSet::new();
        let first = Arc::clone(&sampler);
        samples.spawn(async move { first.sample(sample_request("first")).await });
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), requests.recv()).await?,
            Some(1)
        );
        if has_score {
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(5), samples.join_next())
                    .await?
                    .unwrap()??,
                "low",
            );
            assert!(disconnects.try_recv().is_err());
        }
        for _ in 0..MAX_CONCURRENT_REQUESTS {
            let sampler = Arc::clone(&sampler);
            samples.spawn(async move { sampler.sample(sample_request("newer")).await });
        }
        if !has_score {
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(5), samples.join_next())
                    .await?
                    .unwrap()?,
                Err(LunaSamplerError::Superseded)
            ));
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), disconnects.recv()).await?,
            Some(1)
        );
    }
    Ok(())
}
