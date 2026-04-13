use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use hmac::Hmac;
use hmac::Mac;
use serde::Serialize;
use serde_json::json;
use sha2::Sha256;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::select;
use tokio::time::timeout;

const CLOUD_REQUIREMENTS_CACHE_FILENAME: &str = "cloud-requirements-cache.json";
const CLOUD_REQUIREMENTS_CACHE_HMAC_KEY: &[u8] =
    b"codex-cloud-requirements-cache-v3-064f8542-75b4-494c-a294-97d3ce597271";

type HmacSha256 = Hmac<Sha256>;

#[tokio::test]
async fn cloud_requirements_cli_error_includes_actionable_context() -> anyhow::Result<()> {
    if cfg!(windows) {
        return Ok(());
    }

    let tmp = tempfile::tempdir()?;
    let codex_home = tmp.path();
    let cwd = std::env::current_dir()?;
    let backend = FailingBackend::start()?;
    write_chatgpt_business_auth(codex_home)?;
    write_startup_config(codex_home, &cwd, backend.url())?;

    let CodexCliOutput { exit_code, output } = run_codex_cli(codex_home, &cwd).await?;

    assert_ne!(0, exit_code, "Codex CLI should exit nonzero.");
    assert!(
        output.contains("Error loading configuration: failed to load your workspace-managed config after 5 attempts"),
        "expected cloud requirements load error in output, got: {output}"
    );
    assert!(
        output.contains("last backend status: 503"),
        "expected backend status in output, got: {output}"
    );
    assert!(
        output.contains("valid cached copy"),
        "expected cache guidance in output, got: {output}"
    );
    assert!(
        backend.request_count() > 0,
        "expected CLI to request cloud requirements from fake backend"
    );
    Ok(())
}

#[tokio::test]
async fn stale_cloud_requirements_cache_allows_actual_cli_startup_when_remote_fails()
-> anyhow::Result<()> {
    if cfg!(windows) {
        return Ok(());
    }

    let tmp = tempfile::tempdir()?;
    let codex_home = tmp.path();
    let cwd = std::env::current_dir()?;
    let backend = FailingBackend::start()?;
    std::fs::write(
        codex_home.join("rules"),
        "rules should be a directory not a file",
    )?;
    write_chatgpt_business_auth(codex_home)?;
    write_stale_cloud_requirements_cache(
        codex_home,
        r#"allowed_approval_policies = ["on-request"]"#,
    )?;
    write_startup_config(codex_home, &cwd, backend.url())?;

    let CodexCliOutput { exit_code, output } = run_codex_cli(codex_home, &cwd).await?;

    assert_ne!(0, exit_code, "Codex CLI should exit nonzero.");
    assert!(
        backend.request_count() > 0,
        "expected CLI to retry cloud requirements before using stale cache"
    );
    assert!(
        output.contains("Error loading rules:"),
        "expected startup to proceed beyond config loading, got: {output}"
    );
    assert!(
        output.contains("failed to read rules files"),
        "expected controlled post-config startup error in output, got: {output}"
    );
    assert!(
        !output.contains("failed to load your workspace-managed config"),
        "stale cloud requirements cache should avoid config-load failure, got: {output}"
    );
    Ok(())
}

struct CodexCliOutput {
    exit_code: i32,
    output: String,
}

async fn run_codex_cli(
    codex_home: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
) -> anyhow::Result<CodexCliOutput> {
    let codex_cli = codex_utils_cargo_bin::cargo_bin("codex")?;
    let mut env = HashMap::new();
    env.insert(
        "CODEX_HOME".to_string(),
        codex_home.as_ref().display().to_string(),
    );

    let args = vec!["-c".to_string(), "analytics.enabled=false".to_string()];
    let spawned = codex_utils_pty::spawn_pty_process(
        codex_cli.to_string_lossy().as_ref(),
        &args,
        cwd.as_ref(),
        &env,
        &None,
        codex_utils_pty::TerminalSize::default(),
    )
    .await?;
    let mut output = Vec::new();
    let codex_utils_pty::SpawnedProcess {
        session,
        stdout_rx,
        stderr_rx,
        exit_rx,
    } = spawned;
    let mut output_rx = codex_utils_pty::combine_output_receivers(stdout_rx, stderr_rx);
    let mut exit_rx = exit_rx;
    let writer_tx = session.writer_sender();
    let exit_code_result = timeout(Duration::from_secs(10), async {
        loop {
            select! {
                result = output_rx.recv() => match result {
                    Ok(chunk) => {
                        if chunk.windows(4).any(|window| window == b"\x1b[6n") {
                            let _ = writer_tx.send(b"\x1b[1;1R".to_vec()).await;
                        }
                        output.extend_from_slice(&chunk);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break exit_rx.await,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                },
                result = &mut exit_rx => break result,
            }
        }
    })
    .await;
    let exit_code = match exit_code_result {
        Ok(Ok(code)) => code,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => {
            session.terminate();
            anyhow::bail!("timed out waiting for codex CLI to exit");
        }
    };
    while let Ok(chunk) = output_rx.try_recv() {
        output.extend_from_slice(&chunk);
    }

    let output = String::from_utf8_lossy(&output);
    Ok(CodexCliOutput {
        exit_code,
        output: output.to_string(),
    })
}

#[derive(Serialize)]
struct CloudRequirementsCacheFile {
    signed_payload: CloudRequirementsCacheSignedPayload,
    signature: String,
}

#[derive(Clone, Serialize)]
struct CloudRequirementsCacheSignedPayload {
    cached_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    chatgpt_user_id: Option<String>,
    account_id: Option<String>,
    contents: Option<String>,
}

struct FailingBackend {
    addr: SocketAddr,
    request_count: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FailingBackend {
    fn start() -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let request_count = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_request_count = request_count.clone();
        let thread_shutdown = shutdown.clone();
        let handle = std::thread::spawn(move || {
            while !thread_shutdown.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buffer = [0; 4096];
                        let _ = stream.read(&mut buffer);
                        thread_request_count.fetch_add(1, Ordering::SeqCst);
                        let _ = stream.write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            addr,
            request_count,
            shutdown,
            handle: Some(handle),
        })
    }

    fn url(&self) -> String {
        format!("http://{}/backend-api", self.addr)
    }

    fn request_count(&self) -> usize {
        self.request_count.load(Ordering::SeqCst)
    }
}

impl Drop for FailingBackend {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn write_startup_config(
    codex_home: &Path,
    cwd: impl AsRef<Path>,
    chatgpt_base_url: String,
) -> anyhow::Result<()> {
    let cwd = cwd.as_ref();
    let config_contents = format!(
        r#"
chatgpt_base_url = "{chatgpt_base_url}"
cli_auth_credentials_store = "file"
model_provider = "ollama"

[projects]
"{cwd}" = {{ trust_level = "trusted" }}
"#,
        cwd = cwd.display()
    );
    std::fs::write(codex_home.join("config.toml"), config_contents)?;
    Ok(())
}

fn write_chatgpt_business_auth(codex_home: &Path) -> anyhow::Result<()> {
    let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({
        "alg": "none",
        "typ": "JWT",
    }))?);
    let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_plan_type": "business",
            "chatgpt_user_id": "user-12345",
            "user_id": "user-12345",
        },
    }))?);
    let signature_b64 = URL_SAFE_NO_PAD.encode(b"sig");
    let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");
    let auth_json = json!({
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": fake_jwt,
            "access_token": "test-access-token",
            "refresh_token": "test-refresh-token",
            "account_id": "account-12345",
        },
        "last_refresh": "2025-01-01T00:00:00Z",
    });
    std::fs::write(
        codex_home.join("auth.json"),
        serde_json::to_vec_pretty(&auth_json)?,
    )?;
    Ok(())
}

fn write_stale_cloud_requirements_cache(codex_home: &Path, contents: &str) -> anyhow::Result<()> {
    let signed_payload = CloudRequirementsCacheSignedPayload {
        cached_at: Utc::now() - ChronoDuration::minutes(31),
        expires_at: Utc::now() - ChronoDuration::minutes(1),
        chatgpt_user_id: Some("user-12345".to_string()),
        account_id: Some("account-12345".to_string()),
        contents: Some(contents.to_string()),
    };
    let payload_bytes = serde_json::to_vec(&signed_payload)?;
    let mut mac = HmacSha256::new_from_slice(CLOUD_REQUIREMENTS_CACHE_HMAC_KEY)?;
    mac.update(&payload_bytes);
    let cache_file = CloudRequirementsCacheFile {
        signed_payload,
        signature: BASE64_STANDARD.encode(mac.finalize().into_bytes()),
    };
    std::fs::write(
        codex_home.join(CLOUD_REQUIREMENTS_CACHE_FILENAME),
        serde_json::to_vec_pretty(&cache_file)?,
    )?;
    Ok(())
}
