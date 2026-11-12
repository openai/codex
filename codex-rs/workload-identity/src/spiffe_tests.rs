#![cfg(test)]

//! Exercises the SPIFFE wire protocol and the existing WIF exchange over real local sockets.

use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use pretty_assertions::assert_eq;
use tokio::net::UnixListener;
use tokio::task::JoinHandle;
use tokio::task::JoinSet;
use tonic::Code;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tonic::codegen::BoxFuture;
use tonic::server::UnaryService;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::matchers::method;

use crate::AssertionSource;
use crate::SpiffeWorkloadApi;
use crate::WorkloadIdentityError;

const ID: &str = "spiffe://example.org/workload";
const AUDIENCE: &str = "codex-test";

// Minimal server-side messages for the standard SPIFFE Workload API contract.
#[derive(Clone, PartialEq, Eq, prost::Message)]
struct JwtRequest {
    #[prost(string, repeated, tag = "1")]
    audience: Vec<String>,
    #[prost(string, tag = "2")]
    spiffe_id: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
struct JwtResponse {
    #[prost(message, repeated, tag = "1")]
    svids: Vec<JwtSvid>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
struct JwtSvid {
    #[prost(string, tag = "1")]
    spiffe_id: String,
    #[prost(string, tag = "2")]
    svid: String,
}

#[derive(Clone)]
struct WorkloadApiService {
    requests: Arc<Mutex<Vec<JwtRequest>>>,
    response: Arc<Mutex<Result<JwtResponse, Status>>>,
}

impl UnaryService<JwtRequest> for WorkloadApiService {
    type Response = JwtResponse;
    type Future = BoxFuture<Response<Self::Response>, Status>;

    fn call(&mut self, request: Request<JwtRequest>) -> Self::Future {
        assert_eq!(
            request.metadata().get("workload.spiffe.io").unwrap(),
            "true"
        );
        self.requests.lock().unwrap().push(request.into_inner());
        let response = self.response.lock().unwrap().clone();
        Box::pin(async move { response.map(Response::new) })
    }
}

struct WorkloadApiServer {
    service: WorkloadApiService,
    task: JoinHandle<()>,
}

impl WorkloadApiServer {
    fn start(path: &Path, response: Result<JwtResponse, Status>) -> Self {
        let listener = UnixListener::bind(path).unwrap();
        let service = WorkloadApiService {
            requests: Arc::default(),
            response: Arc::new(Mutex::new(response)),
        };
        let handler = service.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            while let Ok((stream, _)) = listener.accept().await {
                let handler = handler.clone();
                connections.spawn(async move {
                    let service =
                        service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
                            assert_eq!(request.uri().path(), "/SpiffeWorkloadAPI/FetchJWTSVID");
                            let handler = handler.clone();
                            async move {
                                let codec = tonic_prost::ProstCodec::default();
                                let mut grpc = tonic::server::Grpc::new(codec);
                                Ok::<_, Infallible>(grpc.unary(handler, request).await)
                            }
                        });
                    let _ = http2::Builder::new(TokioExecutor::new())
                        .serve_connection(TokioIo::new(stream), service)
                        .await;
                });
            }
        });
        Self { service, task }
    }
}

impl Drop for WorkloadApiServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn jwt(header: serde_json::Value, claims: serde_json::Value) -> String {
    let encode = |value| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&value).unwrap())
    };
    format!("{}.{}.c2lnbmF0dXJl", encode(header), encode(claims))
}

fn svid(subject: &str, audience: &str, label: &str) -> JwtSvid {
    JwtSvid {
        spiffe_id: subject.to_owned(),
        svid: jwt(
            serde_json::json!({"alg": "ES256", "typ": "JWT", "kid": "key-one"}),
            serde_json::json!({
                "sub": subject, "aud": [audience], "exp": 4102444800_i64, "jti": label,
            }),
        ),
    }
}

fn source(path: &Path, spiffe_id: Option<&str>) -> AssertionSource {
    let endpoint = url::Url::from_file_path(path)
        .unwrap()
        .to_string()
        .replacen("file:", "unix:", 1);
    AssertionSource::Spiffe(
        SpiffeWorkloadApi::new(&endpoint, AUDIENCE.to_owned(), spiffe_id.map(str::to_owned))
            .unwrap(),
    )
}

#[tokio::test]
async fn exchanges_and_refreshes_after_agent_restart_without_an_assertion_file() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("agent socket");
    let first_svid = svid(ID, AUDIENCE, "first");
    let second_svid = svid(ID, AUDIENCE, "second");
    let mut api = WorkloadApiServer::start(
        &socket,
        Ok(JwtResponse {
            svids: vec![first_svid.clone()],
        }),
    );
    let tokens = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(super::success("access-token", /*expires_in*/ 600))
        .expect(2)
        .mount(&tokens)
        .await;
    let exchange = Arc::new(super::make_exchange(source(&socket, Some(ID)), &tokens));
    let initial = exchange.resolve().await.unwrap();
    assert_eq!(initial, exchange.resolve().await.unwrap());
    assert_eq!(
        api.service.requests.lock().unwrap().as_slice(),
        &[JwtRequest {
            audience: vec![AUDIENCE.to_owned()],
            spiffe_id: ID.to_owned(),
        }]
    );

    api.task.abort();
    assert!((&mut api.task).await.unwrap_err().is_cancelled());
    std::fs::remove_file(&socket).unwrap();
    api = WorkloadApiServer::start(
        &socket,
        Ok(JwtResponse {
            svids: vec![second_svid.clone()],
        }),
    );
    let mut refreshes = JoinSet::new();
    for _ in 0..12 {
        let exchange = Arc::clone(&exchange);
        let version = initial.version();
        refreshes.spawn(async move { exchange.refresh(version).await });
    }
    while let Some(result) = refreshes.join_next().await {
        assert!(result.unwrap().unwrap().version() > initial.version());
    }
    assert_eq!(api.service.requests.lock().unwrap().len(), 1);
    let assertions: Vec<String> = tokens
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| {
            url::form_urlencoded::parse(&request.body)
                .find(|(key, _)| key == "assertion")
                .unwrap()
                .1
                .into_owned()
        })
        .collect();
    assert_eq!(assertions, vec![first_svid.svid, second_svid.svid]);
}

#[tokio::test]
async fn exchanges_jwt_svids_without_optional_key_ids() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("agent.sock");
    let api = WorkloadApiServer::start(&socket, Ok(JwtResponse { svids: vec![] }));
    let tokens = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(super::success("access-token", /*expires_in*/ 600))
        .mount(&tokens)
        .await;
    let mut expected_assertions = Vec::new();
    for audience in [serde_json::json!(AUDIENCE), serde_json::json!([AUDIENCE])] {
        let token = jwt(
            serde_json::json!({"alg": "ES256", "typ": "JWT"}),
            serde_json::json!({"sub": ID, "aud": audience, "exp": 4102444800_i64}),
        );
        *api.service.response.lock().unwrap() = Ok(JwtResponse {
            svids: vec![JwtSvid {
                spiffe_id: ID.to_owned(),
                svid: token.clone(),
            }],
        });
        for identity in [None, Some(ID)] {
            super::make_exchange(source(&socket, identity), &tokens)
                .resolve()
                .await
                .unwrap();
            expected_assertions.push(token.clone());
        }
    }
    let assertions: Vec<String> = tokens
        .received_requests()
        .await
        .unwrap()
        .iter()
        .map(|request| {
            url::form_urlencoded::parse(&request.body)
                .find(|(key, _)| key == "assertion")
                .unwrap()
                .1
                .into_owned()
        })
        .collect();
    assert_eq!(assertions, expected_assertions);
}

#[tokio::test]
async fn refuses_ambiguous_or_mismatched_identities_and_redacts_agent_errors() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("agent.sock");
    let sole_svid = svid(ID, AUDIENCE, "one");
    let api = WorkloadApiServer::start(
        &socket,
        Ok(JwtResponse {
            svids: vec![sole_svid.clone()],
        }),
    );
    let selected_source = source(&socket, Some(ID));
    let default_source = source(&socket, /*spiffe_id*/ None);
    assert_eq!(default_source.acquire().await.unwrap(), sole_svid.svid);
    for svids in [
        vec![],
        vec![svid(ID, "other-audience", "one")],
        vec![svid("spiffe://example.org/other", AUDIENCE, "one")],
    ] {
        *api.service.response.lock().unwrap() = Ok(JwtResponse { svids });
        assert!(matches!(
            selected_source.acquire().await,
            Err(WorkloadIdentityError::InvalidAssertion)
        ));
    }
    *api.service.response.lock().unwrap() = Ok(JwtResponse {
        svids: vec![svid(ID, AUDIENCE, "one"), svid(ID, AUDIENCE, "two")],
    });
    assert!(matches!(
        default_source.acquire().await,
        Err(WorkloadIdentityError::AmbiguousSpiffeIdentity)
    ));
    for code in [
        Code::PermissionDenied,
        Code::InvalidArgument,
        Code::Unavailable,
    ] {
        *api.service.response.lock().unwrap() = Err(Status::new(code, "sensitive-agent-response"));
        let error = selected_source.acquire().await.unwrap_err();
        assert!(matches!(error, WorkloadIdentityError::SpiffeRejected(actual) if actual == code));
        assert_eq!(error.is_transient(), code == Code::Unavailable);
        assert!(!format!("{error:?} {error}").contains("sensitive-agent-response"));
    }
}

#[tokio::test]
async fn bounds_a_stalled_workload_api_connection() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("agent.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let source = source(&socket, /*spiffe_id*/ None);
    let fetch = tokio::spawn(async move { source.acquire().await });
    let (_connection, _) = listener.accept().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(11)).await;
    assert!(matches!(
        fetch.await.unwrap(),
        Err(WorkloadIdentityError::SpiffeUnavailable)
    ));
}
