use crate::attribution::BindConnectionAttribution;
use crate::network_policy::NetworkDecision;
use crate::network_policy::NetworkDecisionSource;
use crate::network_policy::NetworkPolicyDecision;
use crate::network_policy::emit_dns_policy_decision_audit_event;
use crate::policy::is_non_public_ip;
use crate::policy::normalize_host;
use crate::reasons::REASON_PROXY_DISABLED;
use crate::runtime::HostBlockReason;
use crate::runtime::HostResolutionPolicy;
use crate::state::BlockedRequest;
use crate::state::BlockedRequestArgs;
use crate::state::NetworkProxyState;
use anyhow::Context;
use anyhow::Result;
use hickory_proto::op::Message;
use hickory_proto::op::MessageType;
use hickory_proto::op::OpCode;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::DNSClass;
use hickory_proto::rr::RData;
use hickory_proto::rr::Record;
use hickory_proto::rr::RecordType;
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::rdata::AAAA;
use rama_core::Service;
use rama_core::error::BoxError;
use rama_core::extensions::ExtensionsRef;
use rama_net::stream::SocketInfo;
use rama_tcp::TcpStream;
use rama_tcp::server::TcpListener;
use std::collections::HashSet;
use std::future::Future;
use std::io;
use std::net::IpAddr;
use std::net::TcpListener as StdTcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::lookup_host;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::info;

/// Internal endpoint handed to the trusted Linux proxy bridge.
#[doc(hidden)]
pub const DNS_PROXY_ENV_KEY: &str = "CODEX_NETWORK_PROXY_DNS";

/// Versioned application preface for the private DNS stream.
///
/// The nonzero first byte distinguishes an unattributed application stream from the attribution
/// frame, whose magic intentionally starts with a NUL byte. Each session exchanges this preface,
/// one length-prefixed query, and one length-prefixed response before closing.
#[doc(hidden)]
pub const DNS_PROXY_SESSION_PREFACE: &[u8; 8] = b"CDXDNS1\n";

const DNS_PORT: u16 = 53;
const MAX_DNS_QUERY_BYTES: usize = 4 * 1024;
const MAX_DNS_RESPONSE_BYTES: usize = 512;
const MAX_DNS_ANSWERS: usize = 16;
const DNS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);
const DNS_SESSION_PREFACE_TIMEOUT: Duration = Duration::from_secs(3);
const DNS_FRAME_IO_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_CONCURRENT_DNS_SESSIONS: usize = 64;
const RESPONSE_TTL: u32 = 0;

pub(crate) async fn run_dns_proxy_with_std_listener(
    state: Arc<NetworkProxyState>,
    listener: StdTcpListener,
    environment_id: Option<String>,
) -> Result<()> {
    let listener =
        TcpListener::try_from(listener).context("convert std listener to DNS proxy listener")?;
    let addr = listener
        .local_addr()
        .context("read DNS proxy listener local addr")?;
    info!("DNS proxy listening on {addr}");
    listener
        .serve(LimitDnsSessions {
            inner: BindConnectionAttribution::new(DnsService, state, environment_id),
            sessions: Arc::new(Semaphore::new(MAX_CONCURRENT_DNS_SESSIONS)),
        })
        .await;
    Ok(())
}

struct LimitDnsSessions<S> {
    inner: S,
    sessions: Arc<Semaphore>,
}

impl<S> Service<TcpStream> for LimitDnsSessions<S>
where
    S: Service<TcpStream>,
    S::Error: Into<BoxError>,
{
    type Output = S::Output;
    type Error = BoxError;

    async fn serve(&self, stream: TcpStream) -> std::result::Result<Self::Output, BoxError> {
        let _session = Arc::clone(&self.sessions)
            .try_acquire_owned()
            .map_err(|_| io::Error::other("DNS proxy session limit reached"))?;
        self.inner.serve(stream).await.map_err(Into::into)
    }
}

#[derive(Clone, Copy)]
struct DnsService;

impl Service<TcpStream> for DnsService {
    type Output = ();
    type Error = BoxError;

    async fn serve(&self, mut stream: TcpStream) -> std::result::Result<(), BoxError> {
        let state = stream
            .extensions()
            .get::<Arc<NetworkProxyState>>()
            .cloned()
            .ok_or_else(|| io::Error::other("missing network proxy state"))?;
        let client_addr = stream
            .extensions()
            .get::<SocketInfo>()
            .map(|info| info.peer_addr().to_string());

        exchange_session_preface(&mut stream).await?;
        let query = read_query_frame(&mut stream).await?;
        let response = resolve_query(&state, &query, client_addr.as_deref()).await;
        write_response_frame(&mut stream, &response).await?;
        Ok(())
    }
}

async fn exchange_session_preface(stream: &mut TcpStream) -> io::Result<()> {
    timeout(DNS_SESSION_PREFACE_TIMEOUT, async {
        let mut preface = [0_u8; DNS_PROXY_SESSION_PREFACE.len()];
        stream.read_exact(&mut preface).await?;
        if &preface != DNS_PROXY_SESSION_PREFACE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid DNS proxy session preface",
            ));
        }
        stream.write_all(DNS_PROXY_SESSION_PREFACE).await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS proxy preface timed out"))?
}

async fn read_query_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    timeout(DNS_FRAME_IO_TIMEOUT, async {
        let mut len = [0_u8; 2];
        stream.read_exact(&mut len).await?;
        let len = usize::from(u16::from_be_bytes(len));
        if len == 0 || len > MAX_DNS_QUERY_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid DNS proxy query length",
            ));
        }
        let mut payload = vec![0_u8; len];
        stream.read_exact(&mut payload).await?;
        Ok(payload)
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS query frame timed out"))?
}

async fn write_response_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    let len = u16::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "DNS response is too large"))?;
    timeout(DNS_FRAME_IO_TIMEOUT, async {
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(payload).await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS response frame timed out"))?
}

async fn resolve_query(
    state: &NetworkProxyState,
    wire: &[u8],
    client_addr: Option<&str>,
) -> Vec<u8> {
    resolve_query_with_lookup(state, wire, client_addr, |host| async move {
        lookup_host((host.as_str(), 0))
            .await
            .map(|addresses| addresses.map(|address| address.ip()).collect())
    })
    .await
}

async fn resolve_query_with_lookup<F, Fut>(
    state: &NetworkProxyState,
    wire: &[u8],
    client_addr: Option<&str>,
    lookup: F,
) -> Vec<u8>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = io::Result<Vec<IpAddr>>>,
{
    let parsed = Message::from_vec(wire).ok();
    let Some(query) = parsed.as_ref().filter(|query| {
        query.message_type() == MessageType::Query
            && query.op_code() == OpCode::Query
            && query.queries().len() == 1
            && query.queries()[0].query_class() == DNSClass::IN
            && matches!(
                query.queries()[0].query_type(),
                RecordType::A | RecordType::AAAA
            )
    }) else {
        return bounded_response(refused_response(wire, parsed.as_ref()));
    };
    let question = &query.queries()[0];
    let host = normalize_host(&question.name().to_utf8());
    match state.enabled().await {
        Ok(true) => {}
        Ok(false) => {
            record_blocked_dns_request(
                state,
                &host,
                REASON_PROXY_DISABLED,
                NetworkDecisionSource::ProxyState,
                client_addr,
            )
            .await;
            return bounded_response(refused_response(wire, Some(query)));
        }
        Err(_) => return bounded_response(response_message(query, ResponseCode::ServFail)),
    }
    let policy = match state.host_resolution_policy(&host).await {
        Ok(policy) => policy,
        Err(_) => return bounded_response(response_message(query, ResponseCode::ServFail)),
    };
    let allow_non_public_ips = match policy {
        HostResolutionPolicy::Allowed {
            allow_non_public_ips,
        } => allow_non_public_ips,
        HostResolutionPolicy::Blocked(reason) => {
            record_blocked_dns_request(
                state,
                &host,
                reason.as_str(),
                NetworkDecisionSource::BaselinePolicy,
                client_addr,
            )
            .await;
            return bounded_response(refused_response(wire, Some(query)));
        }
    };

    let addresses = match timeout(DNS_LOOKUP_TIMEOUT, lookup(host.clone())).await {
        Ok(Ok(addresses)) => addresses,
        Ok(Err(_)) | Err(_) => {
            return bounded_response(response_message(query, ResponseCode::ServFail));
        }
    };
    if !allow_non_public_ips && addresses.iter().copied().any(is_non_public_ip) {
        record_blocked_dns_request(
            state,
            &host,
            HostBlockReason::NotAllowedLocal.as_str(),
            NetworkDecisionSource::BaselinePolicy,
            client_addr,
        )
        .await;
        return bounded_response(refused_response(wire, Some(query)));
    }

    emit_dns_policy_decision_audit_event(state, &host, &NetworkDecision::Allow, client_addr);
    bounded_answer_response(query, question.query_type(), addresses)
}

async fn record_blocked_dns_request(
    state: &NetworkProxyState,
    host: &str,
    reason: &str,
    source: NetworkDecisionSource,
    client_addr: Option<&str>,
) {
    let decision = NetworkDecision::deny_with_source(reason, source);
    emit_dns_policy_decision_audit_event(state, host, &decision, client_addr);
    let _ = state
        .record_blocked(BlockedRequest::new(BlockedRequestArgs {
            host: host.to_string(),
            reason: reason.to_string(),
            client: client_addr.map(str::to_string),
            method: None,
            mode: None,
            protocol: "dns".to_string(),
            decision: Some(NetworkPolicyDecision::Deny.as_str().to_string()),
            source: Some(source.as_str().to_string()),
            port: Some(DNS_PORT),
        }))
        .await;
}

fn bounded_answer_response(
    query: &Message,
    record_type: RecordType,
    addresses: Vec<IpAddr>,
) -> Vec<u8> {
    let mut response = response_message(query, ResponseCode::NoError);
    let answer_name = query.queries()[0].name().clone();
    let mut seen = HashSet::new();
    for data in addresses
        .into_iter()
        .filter(|address| seen.insert(*address))
        .filter_map(|address| match (record_type, address) {
            (RecordType::A, IpAddr::V4(ip)) => Some(RData::A(A(ip))),
            (RecordType::AAAA, IpAddr::V6(ip)) => Some(RData::AAAA(AAAA(ip))),
            _ => None,
        })
        .take(MAX_DNS_ANSWERS)
    {
        response.add_answer(Record::from_rdata(answer_name.clone(), RESPONSE_TTL, data));
        if !response
            .to_vec()
            .is_ok_and(|wire| wire.len() <= MAX_DNS_RESPONSE_BYTES)
        {
            response.answers_mut().pop();
            break;
        }
    }
    bounded_response(response)
}

fn response_message(query: &Message, code: ResponseCode) -> Message {
    let mut response = Message::error_msg(query.id(), query.op_code(), code);
    response
        .set_recursion_desired(query.recursion_desired())
        .set_recursion_available(true)
        .add_queries(query.queries().iter().take(1).cloned());
    response
}

fn refused_response(wire: &[u8], query: Option<&Message>) -> Message {
    if let Some(query) = query {
        return response_message(query, ResponseCode::Refused);
    }
    let id = wire
        .get(..2)
        .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
        .unwrap_or_default();
    Message::error_msg(id, OpCode::Query, ResponseCode::Refused)
}

fn bounded_response(message: Message) -> Vec<u8> {
    let id = message.id();
    let op_code = message.op_code();
    match message.to_vec() {
        Ok(wire) if wire.len() <= MAX_DNS_RESPONSE_BYTES => wire,
        _ => Message::error_msg(id, op_code, ResponseCode::ServFail)
            .to_vec()
            .unwrap_or_default(),
    }
}

#[cfg(test)]
#[path = "dns_tests.rs"]
mod tests;
