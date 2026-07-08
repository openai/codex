use super::*;
use crate::attribution::write_attribution_frame;
use crate::config::NetworkProxyConfig;
use crate::config::NetworkProxySettings;
use crate::network_policy::test_support::POLICY_DECISION_EVENT_NAME;
use crate::network_policy::test_support::capture_events;
use crate::network_policy::test_support::find_event_by_name;
use crate::runtime::network_proxy_state_for_policy;
use crate::state::NetworkProxyConstraints;
use crate::state::build_config_state;
use hickory_proto::op::Query;
use hickory_proto::rr::Name;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream as TokioTcpStream;

fn settings(allowed_domains: &[&str], denied_domains: &[&str]) -> NetworkProxySettings {
    let mut settings = NetworkProxySettings {
        enabled: true,
        allow_local_binding: false,
        ..NetworkProxySettings::default()
    };
    settings.set_allowed_domains(
        allowed_domains
            .iter()
            .map(|domain| (*domain).to_string())
            .collect(),
    );
    settings.set_denied_domains(
        denied_domains
            .iter()
            .map(|domain| (*domain).to_string())
            .collect(),
    );
    settings
}

fn query(name: &str, record_type: RecordType, id: u16) -> Vec<u8> {
    let mut message = Message::new();
    message.set_id(id);
    message.add_query(Query::query(
        Name::from_ascii(name).expect("valid DNS name"),
        record_type,
    ));
    message.to_vec().expect("serialize DNS query")
}

fn response(wire: Vec<u8>) -> Message {
    Message::from_vec(&wire).expect("parse DNS response")
}

async fn query_listener(
    addr: std::net::SocketAddr,
    attribution_token: Option<&str>,
    name: &str,
) -> io::Result<Message> {
    let mut stream = TokioTcpStream::connect(addr).await?;
    if let Some(token) = attribution_token {
        let mut attribution = Vec::new();
        write_attribution_frame(&mut attribution, token)?;
        stream.write_all(&attribution).await?;
    }
    stream.write_all(DNS_PROXY_SESSION_PREFACE).await?;
    let mut ack = [0_u8; DNS_PROXY_SESSION_PREFACE.len()];
    stream.read_exact(&mut ack).await?;
    if &ack != DNS_PROXY_SESSION_PREFACE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid DNS session acknowledgement",
        ));
    }

    let query = query(name, RecordType::A, 16);
    let query_len = u16::try_from(query.len()).expect("bounded DNS query length");
    stream.write_all(&query_len.to_be_bytes()).await?;
    stream.write_all(&query).await?;
    let response_len = stream.read_u16().await?;
    let mut wire = vec![0_u8; usize::from(response_len)];
    stream.read_exact(&mut wire).await?;
    Message::from_vec(&wire).map_err(io::Error::other)
}

#[test]
fn session_preface_cannot_be_confused_with_attribution_frame() {
    assert_ne!(DNS_PROXY_SESSION_PREFACE[0], 0);
    assert_eq!(DNS_PROXY_SESSION_PREFACE.len(), 8);
}

#[tokio::test]
async fn resolves_allowed_a_and_aaaa_queries_with_bounded_answers() {
    let state = network_proxy_state_for_policy(settings(&["example.test"], &[]));
    let addresses = vec![
        "93.184.216.34".parse().expect("IPv4 address"),
        "2001:4860:4860::8888".parse().expect("IPv6 address"),
    ];

    for (record_type, expected) in [
        (RecordType::A, addresses[0]),
        (RecordType::AAAA, addresses[1]),
    ] {
        let wire = resolve_query_with_lookup(
            &state,
            &query("example.test", record_type, 7),
            Some("127.0.0.1:1234"),
            |_| {
                let addresses = addresses.clone();
                async move { Ok(addresses) }
            },
        )
        .await;
        assert!(wire.len() <= MAX_DNS_RESPONSE_BYTES);
        let response = response(wire);
        assert_eq!(response.response_code(), ResponseCode::NoError);
        assert!(response.answers().iter().any(|answer| match answer.data() {
            RData::A(address) => IpAddr::V4(address.0) == expected,
            RData::AAAA(address) => IpAddr::V6(address.0) == expected,
            _ => false,
        }));
    }
}

#[tokio::test]
async fn synthesized_response_never_exceeds_classic_udp_size() {
    let state = network_proxy_state_for_policy(settings(&["many.example.test"], &[]));
    let addresses = (1..=64)
        .map(|suffix| {
            format!("2001:4860::{suffix}")
                .parse()
                .expect("public IPv6 address")
        })
        .collect::<Vec<_>>();
    let wire = resolve_query_with_lookup(
        &state,
        &query("many.example.test", RecordType::AAAA, 7),
        None,
        |_| async move { Ok(addresses) },
    )
    .await;

    assert!(wire.len() <= MAX_DNS_RESPONSE_BYTES);
    let response = response(wire);
    assert_eq!(response.response_code(), ResponseCode::NoError);
    assert!(!response.answers().is_empty());
    assert!(response.answers().len() <= MAX_DNS_ANSWERS);
}

#[tokio::test]
async fn denied_name_is_not_sent_to_the_host_resolver() {
    let state = network_proxy_state_for_policy(settings(&["allowed.test"], &[]));
    let lookups = Arc::new(AtomicUsize::new(0));
    let response = response(
        resolve_query_with_lookup(&state, &query("denied.test", RecordType::A, 8), None, {
            let lookups = Arc::clone(&lookups);
            move |_| async move {
                lookups.fetch_add(1, Ordering::SeqCst);
                Ok(Vec::new())
            }
        })
        .await,
    );

    assert_eq!(response.response_code(), ResponseCode::Refused);
    assert_eq!(lookups.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn refuses_entire_answer_set_when_resolution_contains_a_private_address() {
    let state = network_proxy_state_for_policy(settings(&["example.test"], &[]));
    let response = response(
        resolve_query_with_lookup(
            &state,
            &query("example.test", RecordType::A, 9),
            None,
            |_| async {
                Ok(vec![
                    "93.184.216.34".parse().expect("public address"),
                    "127.0.0.1".parse().expect("private address"),
                ])
            },
        )
        .await,
    );

    assert_eq!(response.response_code(), ResponseCode::Refused);
    assert_eq!(response.answers(), &[]);
}

#[tokio::test]
async fn explicitly_allowlisted_local_literal_preserves_local_policy_semantics() {
    let state = network_proxy_state_for_policy(settings(&["127.0.0.1"], &[]));
    let response = response(
        resolve_query_with_lookup(
            &state,
            &query("127.0.0.1", RecordType::A, 9),
            None,
            |_| async { Ok(vec!["127.0.0.1".parse().expect("loopback address")]) },
        )
        .await,
    );

    assert_eq!(response.response_code(), ResponseCode::NoError);
    assert_eq!(response.answers().len(), 1);
}

#[tokio::test]
async fn policy_changes_apply_without_restarting_the_dns_service() {
    let state = network_proxy_state_for_policy(settings(&["example.test"], &[]));
    let allowed = response(
        resolve_query_with_lookup(
            &state,
            &query("example.test", RecordType::A, 10),
            None,
            |_| async { Ok(vec!["93.184.216.34".parse().expect("public address")]) },
        )
        .await,
    );
    assert_eq!(allowed.response_code(), ResponseCode::NoError);

    let mut replacement_network = settings(&["other.test"], &[]);
    replacement_network.enabled = true;
    let replacement = build_config_state(
        NetworkProxyConfig {
            network: replacement_network,
        },
        NetworkProxyConstraints::default(),
    )
    .expect("replacement config state");
    state
        .replace_config_state(replacement)
        .await
        .expect("replace proxy config state");
    let denied = response(
        resolve_query_with_lookup(
            &state,
            &query("example.test", RecordType::A, 11),
            None,
            |_| async { Ok(vec!["93.184.216.34".parse().expect("public address")]) },
        )
        .await,
    );
    assert_eq!(denied.response_code(), ResponseCode::Refused);
}

#[tokio::test]
async fn live_disable_is_a_proxy_state_denial_without_host_lookup() {
    let state = network_proxy_state_for_policy(settings(&["example.test"], &[]));
    let mut disabled_network = settings(&["example.test"], &[]);
    disabled_network.enabled = false;
    let disabled = build_config_state(
        NetworkProxyConfig {
            network: disabled_network,
        },
        NetworkProxyConstraints::default(),
    )
    .expect("disabled config state");
    state
        .replace_config_state(disabled)
        .await
        .expect("disable proxy state");

    let lookups = Arc::new(AtomicUsize::new(0));
    let (response, events) = capture_events(|| async {
        response(
            resolve_query_with_lookup(&state, &query("example.test", RecordType::A, 12), None, {
                let lookups = Arc::clone(&lookups);
                move |_| async move {
                    lookups.fetch_add(1, Ordering::SeqCst);
                    Ok(Vec::new())
                }
            })
            .await,
        )
    })
    .await;

    assert_eq!(response.response_code(), ResponseCode::Refused);
    assert_eq!(lookups.load(Ordering::SeqCst), 0);
    let event =
        find_event_by_name(&events, POLICY_DECISION_EVENT_NAME).expect("DNS policy audit event");
    assert_eq!(event.field("network.policy.source"), Some("proxy_state"));
    let blocked = state.blocked_snapshot().await.expect("blocked DNS queries");
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].reason, REASON_PROXY_DISABLED);
}

#[tokio::test]
async fn resolver_errors_are_not_reported_as_policy_denials() {
    let state = network_proxy_state_for_policy(settings(&["example.test"], &[]));
    let (_, events) = capture_events(|| async {
        let response = response(
            resolve_query_with_lookup(
                &state,
                &query("example.test", RecordType::A, 12),
                None,
                |_| async { Err(io::Error::other("resolver unavailable")) },
            )
            .await,
        );
        assert_eq!(response.response_code(), ResponseCode::ServFail);
    })
    .await;

    assert!(find_event_by_name(&events, POLICY_DECISION_EVENT_NAME).is_none());
    assert!(
        state
            .blocked_snapshot()
            .await
            .expect("blocked request snapshot")
            .is_empty()
    );
}

#[tokio::test]
async fn refuses_unsupported_and_untrusted_query_shapes() {
    let state = network_proxy_state_for_policy(settings(&["example.test"], &[]));
    let mut multi_question = Message::new();
    multi_question.set_id(12);
    multi_question.add_query(Query::query(
        Name::from_ascii("example.test").expect("valid DNS name"),
        RecordType::A,
    ));
    multi_question.add_query(Query::query(
        Name::from_ascii("example.test").expect("valid DNS name"),
        RecordType::AAAA,
    ));
    let queries = [
        query("example.test", RecordType::CNAME, 12),
        query("example.test", RecordType::TXT, 13),
        multi_question.to_vec().expect("serialize query"),
        vec![0, 14, 0xff],
    ];

    for query in queries {
        let response = response(
            resolve_query_with_lookup(&state, &query, None, |_| async { Ok(Vec::new()) }).await,
        );
        assert_eq!(response.response_code(), ResponseCode::Refused);
        assert_eq!(response.answers(), &[]);
    }
}

#[tokio::test]
async fn denied_query_is_audited_and_reported_with_execution_attribution() {
    let state = Arc::new(network_proxy_state_for_policy(settings(
        &["allowed.test"],
        &[],
    )));
    let blocked = Arc::new(Mutex::new(Vec::new()));
    state
        .set_blocked_request_observer(Some(Arc::new({
            let blocked = Arc::clone(&blocked);
            move |request: BlockedRequest| {
                let blocked = Arc::clone(&blocked);
                async move {
                    blocked
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(request);
                }
            }
        })))
        .await;
    state.register_execution("token-1", "local", "execution-1");
    let attributed = state
        .for_execution_token("token-1")
        .expect("registered execution state");

    let (_, events) = capture_events(|| async {
        resolve_query_with_lookup(
            &attributed,
            &query("denied.test", RecordType::A, 15),
            Some("127.0.0.1:1234"),
            |_| async { Ok(Vec::new()) },
        )
        .await
    })
    .await;
    let event =
        find_event_by_name(&events, POLICY_DECISION_EVENT_NAME).expect("DNS policy audit event");
    assert_eq!(event.field("network.transport.protocol"), Some("dns"));
    assert_eq!(event.field("execution.id"), Some("execution-1"));

    let blocked = blocked
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].protocol, "dns");
    assert_eq!(blocked[0].execution_id.as_deref(), Some("execution-1"));
}

#[tokio::test]
async fn listener_accepts_unscoped_policy_and_binds_attributed_execution() {
    let mut network = settings(&["127.0.0.1"], &[]);
    network.allow_local_binding = true;
    let state = Arc::new(network_proxy_state_for_policy(network));
    state.register_execution("token-1", "local", "execution-1");
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("bind DNS listener");
    let addr = listener.local_addr().expect("DNS listener address");
    let task = tokio::spawn(run_dns_proxy_with_std_listener(
        Arc::clone(&state),
        listener,
        Some("local".to_string()),
    ));

    let unscoped = query_listener(addr, None, "127.0.0.1")
        .await
        .expect("unscoped DNS query");
    assert_eq!(unscoped.response_code(), ResponseCode::NoError);

    assert!(
        query_listener(addr, Some("unknown-token"), "127.0.0.1")
            .await
            .is_err()
    );

    let attributed = query_listener(addr, Some("token-1"), "denied.test")
        .await
        .expect("attributed DNS query");
    assert_eq!(attributed.response_code(), ResponseCode::Refused);
    let blocked = state.blocked_snapshot().await.expect("blocked DNS queries");
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].execution_id.as_deref(), Some("execution-1"));

    task.abort();
    let _ = task.await;
}
