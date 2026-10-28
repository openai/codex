//! Exercise catalog auth through the public provider factory and real HTTPS transport.
#![cfg(test)]

use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider::ProviderAuthContext;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::ModelsResponse;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const CHILD_SERVER: &str = "CODEX_RUNTIME_CATALOG_TEST_SERVER";
const TEST_NAME: &str = "production_catalog_uses_runtime_auth_and_preserves_provider_isolation";

#[tokio::test]
async fn production_catalog_uses_runtime_auth_and_preserves_provider_isolation() {
    if let Ok(server) = std::env::var(CHILD_SERVER) {
        check_catalog(&server).await;
        return;
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-a", "agent_host_id": "host-a",
            "actor": {"kind":"platform", "user_id":"user-a", "organization_id":"org-a", "project_id":null},
        })))
        .expect(2).mount(&server).await;
    let catalog = ModelsResponse {
        models: vec![model_info_from_slug("runtime-catalog-marker")],
    };
    for (url, count) in [("/backend-api/codex/models", 4), ("/models", 2)] {
        Mock::given(method("GET"))
            .and(path(url))
            .respond_with(ResponseTemplate::new(200).set_body_json(&catalog))
            .expect(count)
            .mount(&server)
            .await;
    }

    // Scope proxy and CA configuration to a child process, preserving the real OpenAI
    // URL classification without changing environment variables used by other tests.
    let (ca, tls) = tls_config();
    let home = tempfile::tempdir().unwrap();
    let ca_path = home.path().join("catalog-ca.pem");
    std::fs::write(&ca_path, ca).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let upstream = *server.address();
    let proxy = tokio::spawn(async move {
        for _ in 0..4 {
            let (stream, _) = tokio::time::timeout(Duration::from_secs(30), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let stream = stream.into_std().unwrap();
            let tls = Arc::clone(&tls);
            tokio::task::spawn_blocking(move || proxy_catalog(stream, tls, upstream))
                .await
                .unwrap();
        }
    });
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(TEST_NAME)
        .arg("--nocapture")
        .kill_on_drop(/*kill_on_drop*/ true);
    for key in [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
        "OPENAI_API_KEY",
        "OPENAI_PROJECT",
        "OPENAI_ORGANIZATION",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ] {
        command.env_remove(key);
    }
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        command
            .env(CHILD_SERVER, server.uri())
            .env("HTTPS_PROXY", proxy_url)
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("SSL_CERT_FILE", ca_path)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    if !output.status.success() {
        proxy.abort();
    }
    assert!(
        output.status.success(),
        "catalog child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    proxy.await.unwrap();

    let requests = server.received_requests().await.unwrap();
    let catalogs: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == "/backend-api/codex/models")
        .collect();
    assert_eq!(catalogs.len(), 4);
    let mut proofs = std::collections::HashSet::new();
    for request in catalogs {
        let authorization = request.headers["authorization"].to_str().unwrap();
        let proof = if let Some(proof) = request.headers.get("x-openai-agent-assertion") {
            assert_eq!(authorization, "Bearer catalog-key");
            proof.to_str().unwrap()
        } else {
            authorization.strip_prefix("AgentAssertion ").unwrap()
        };
        let payload: Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.split('.').nth(2).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(payload["runtime_agent_instance"], "runtime-a");
        assert!(proofs.insert(payload["id"].as_str().unwrap().to_owned()));
    }
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/backend-api/codex/models"
                && request.headers["originator"] == "codex_work_cca")
            .count(),
        2
    );
    for request in requests
        .iter()
        .filter(|request| request.url.path() == "/models")
    {
        assert_eq!(request.headers["authorization"], "Bearer catalog-key");
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    server.verify().await;
}

async fn check_catalog(server: &str) {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let credentials =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("catalog-key"));
        let runtime = Arc::new(RuntimeAuthManager::new(
            Arc::clone(&credentials),
            AgentHostAuthorization::InstallationId {
                installation_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
            },
            RuntimeRegistrationMetadata {
                harness: None,
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            format!("{server}/register").parse().unwrap(),
        ));
        let context = ProviderAuthContext {
            credentials,
            runtime_auth: Some((runtime, mode)),
        };
        let info = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
        let provider = context.model_provider(info);
        let selected = provider
            .request_auth(/*scope*/ None)
            .await
            .unwrap()
            .runtime
            .unwrap();
        assert_eq!(selected.runtime().id(), "runtime-a");
        let models = provider.models_manager_without_cache(/*config_model_catalog*/ None);
        models.set_api_key_model_discovery_enabled(/*enabled*/ true);
        let work = models.for_originator("codex_work_cca").unwrap();
        for manager in [models, work] {
            let result = manager
                .raw_model_catalog(
                    RefreshStrategy::Online,
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                )
                .await;
            assert!(
                result
                    .models
                    .iter()
                    .any(|model| model.slug == "runtime-catalog-marker"),
                "the returned catalog must contain the remote model"
            );
        }
        let mut other_info = ModelProviderInfo::create_openai_provider(Some(server.to_owned()));
        other_info.model_catalog_url = Some(format!("{server}/models").into());
        let other = context.model_provider(other_info);
        let models = other.models_manager_without_cache(/*config_model_catalog*/ None);
        models.set_api_key_model_discovery_enabled(/*enabled*/ true);
        let result = models
            .raw_model_catalog(
                RefreshStrategy::Online,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await;
        assert!(
            result
                .models
                .iter()
                .any(|model| model.slug == "runtime-catalog-marker"),
            "the other provider must also receive its remote catalog"
        );
    }
}

fn tls_config() -> (String, Arc<rustls::ServerConfig>) {
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let mut ca_params = rcgen::CertificateParams::default();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![rcgen::KeyUsagePurpose::KeyCertSign];
    let ca = rcgen::CertifiedIssuer::self_signed(ca_params, rcgen::KeyPair::generate().unwrap())
        .unwrap();
    let mut leaf = rcgen::CertificateParams::new(vec!["chatgpt.com".into()]).unwrap();
    leaf.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = leaf.signed_by(&key, &ca).unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            rustls::pki_types::PrivateKeyDer::from(key),
        )
        .unwrap();
    (ca.pem(), Arc::new(config))
}

fn proxy_catalog(
    mut stream: std::net::TcpStream,
    config: Arc<rustls::ServerConfig>,
    upstream: std::net::SocketAddr,
) {
    stream.set_nonblocking(/*nonblocking*/ false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let connect = read_headers(&mut stream);
    assert!(connect.starts_with(b"CONNECT chatgpt.com:443 "));
    stream
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .unwrap();
    let mut tls = rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), stream);
    let mut headers = read_headers(&mut tls);
    assert!(headers.starts_with(b"GET /backend-api/codex/models?"));
    headers.truncate(headers.len() - 2);
    headers.extend_from_slice(b"connection: close\r\n\r\n");
    let mut origin = std::net::TcpStream::connect(upstream).unwrap();
    origin
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    origin.write_all(&headers).unwrap();
    let mut response = Vec::new();
    origin.read_to_end(&mut response).unwrap();
    tls.write_all(&response).unwrap();
    tls.flush().unwrap();
}

fn read_headers(reader: &mut impl Read) -> Vec<u8> {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        let mut byte = [0_u8];
        reader.read_exact(&mut byte).unwrap();
        headers.push(byte[0]);
        assert!(headers.len() < 64 * 1024);
    }
    headers
}
