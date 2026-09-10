//! Exercises terminal OAuth pasteback and its shared callback validation through the CLI.

use std::collections::BTreeMap;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time::timeout;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const TEST_TIMEOUT: Duration = Duration::from_secs(30);

struct Fixture {
    server: MockServer,
    home: TempDir,
    issuer: String,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let server = MockServer::start().await;
        let home = TempDir::new()?;
        let issuer = format!("{}/mcp", server.uri());
        std::fs::write(
            home.path().join("config.toml"),
            format!(
                "mcp_oauth_credentials_store = \"file\"\n\
             [mcp_servers.manual]\nurl = \"{issuer}\"\nscopes = [\"mcp.read\"]\n\
             [mcp_servers.manual.oauth]\nclient_id = \"registered-client\"\n"
            ),
        )?;
        Mock::given(method("GET"))
            .and(path("/.well-known/oauth-authorization-server/mcp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "issuer": issuer,
                "authorization_endpoint": format!("{}/authorize", server.uri()),
                "token_endpoint": format!("{}/token", server.uri()),
                "response_types_supported": ["code"],
                "code_challenge_methods_supported": ["S256"],
                "scopes_supported": ["mcp.read"],
                "authorization_response_iss_parameter_supported": true,
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "mock-access-token",
                "token_type": "Bearer",
                "refresh_token": "mock-refresh-token",
                "scope": "mcp.read",
            })))
            .mount(&server)
            .await;
        Ok(Self {
            server,
            home,
            issuer,
        })
    }

    async fn start(&self) -> Result<Login> {
        let mut child = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .kill_on_drop(true)
            .current_dir(self.home.path())
            .env("CODEX_HOME", self.home.path())
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .args(["mcp", "login", "manual", "--no-browser"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut stdout = BufReader::new(child.stdout.take().context("missing stdout")?);
        let mut transcript = String::new();
        let authorization_url = read_authorization_url(&mut stdout, &mut transcript).await?;
        Ok(Login {
            child,
            stdout,
            transcript,
            authorization_url,
        })
    }
}

async fn read_authorization_url(
    stdout: &mut BufReader<ChildStdout>,
    transcript: &mut String,
) -> Result<Url> {
    timeout(TEST_TIMEOUT, async {
        loop {
            let mut line = String::new();
            if stdout.read_line(&mut line).await? == 0 {
                bail!("login exited before printing its authorization URL: {transcript}");
            }
            transcript.push_str(&line);
            if line.starts_with("http://") {
                return Ok(Url::parse(line.trim())?);
            }
        }
    })
    .await
    .context("authorization URL timeout")?
}

struct Login {
    child: Child,
    stdout: BufReader<ChildStdout>,
    transcript: String,
    authorization_url: Url,
}

impl Login {
    fn callback(&self, issuer: &str) -> Result<Url> {
        let params: BTreeMap<_, _> = self.authorization_url.query_pairs().into_owned().collect();
        let mut callback = Url::parse(&params["redirect_uri"])?;
        callback
            .query_pairs_mut()
            .append_pair("code", "mock-code")
            .append_pair("state", &params["state"])
            .append_pair("iss", issuer);
        Ok(callback)
    }

    async fn finish(mut self) -> Result<Output> {
        let (mut output, _) = timeout(TEST_TIMEOUT, async {
            tokio::try_join!(
                self.child.wait_with_output(),
                self.stdout.read_to_string(&mut self.transcript),
            )
        })
        .await
        .context("login completion timeout")??;
        output.stdout = self.transcript.into_bytes();
        Ok(output)
    }
}

#[tokio::test]
async fn no_browser_exchanges_pasted_callback_and_saves_credentials() -> Result<()> {
    let fixture = Fixture::new().await?;
    let mut login = fixture.start().await?;
    let authorization_url = login.authorization_url.to_string();
    let params: BTreeMap<_, _> = login.authorization_url.query_pairs().into_owned().collect();
    let callback = login.callback(&fixture.issuer)?;
    login
        .child
        .stdin
        .as_mut()
        .context("missing stdin")?
        .write_all(format!("{callback}\n").as_bytes())
        .await?;
    let output = login.finish().await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    insta::assert_snapshot!(String::from_utf8(output.stdout)?.replace(&authorization_url, "[AUTHORIZATION_URL]"), @r"
    Authorize the MCP server by opening this URL in your browser:
    [AUTHORIZATION_URL]

    After signing in, copy the full URL from your browser's address bar.
    If the callback page cannot load, paste that URL here anyway.
    Successfully logged in to MCP server 'manual'.
    ");
    assert!(String::from_utf8(output.stderr)?.contains("Callback URL (input hidden): "));
    let requests = fixture
        .server
        .received_requests()
        .await
        .context("missing requests")?;
    let tokens: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path() == "/token")
        .collect();
    assert_eq!(tokens.len(), 1);
    let mut exchange: BTreeMap<_, _> = url::form_urlencoded::parse(&tokens[0].body)
        .into_owned()
        .collect();
    let verifier = exchange
        .remove("code_verifier")
        .context("missing PKCE verifier")?;
    assert!((43..=128).contains(&verifier.len()));
    assert_eq!(params["code_challenge_method"], "S256");
    assert!(!params["code_challenge"].is_empty());
    assert_eq!(
        exchange,
        BTreeMap::from([
            ("client_id".to_string(), "registered-client".to_string()),
            ("code".to_string(), "mock-code".to_string()),
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("redirect_uri".to_string(), params["redirect_uri"].clone()),
            ("resource".to_string(), fixture.issuer.clone()),
        ])
    );
    let credentials: BTreeMap<String, Value> = serde_json::from_slice(&std::fs::read(
        fixture.home.path().join(".credentials.json"),
    )?)?;
    assert_eq!(
        credentials.into_values().collect::<Vec<_>>(),
        vec![json!({
            "server_name": "manual", "server_url": fixture.issuer, "issuer": fixture.issuer,
            "client_id": "registered-client", "access_token": "mock-access-token",
            "refresh_token": "mock-refresh-token", "expires_at": null, "scopes": ["mcp.read"],
        })]
    );
    Ok(())
}

#[tokio::test]
async fn no_browser_rejects_invalid_callbacks_without_exchanging_tokens() -> Result<()> {
    for (field, value) in [
        ("state", Some("wrong-state")),
        ("iss", None),
        ("iss", Some("https://secret-issuer-marker.example")),
        ("eof", None),
    ] {
        let fixture = Fixture::new().await?;
        let mut login = fixture.start().await?;
        let mut callback = login.callback(&fixture.issuer)?;
        let mut pairs: BTreeMap<_, _> = callback.query_pairs().into_owned().collect();
        pairs.remove(field);
        if let Some(value) = value {
            pairs.insert(field.to_string(), value.to_string());
        }
        callback.set_query(None);
        callback.query_pairs_mut().extend_pairs(pairs);
        if field == "eof" {
            drop(login.child.stdin.take());
        } else {
            login
                .child
                .stdin
                .as_mut()
                .context("missing stdin")?
                .write_all(format!("{callback}\n").as_bytes())
                .await?;
        }
        let output = login.finish().await?;
        assert!(
            !output.status.success(),
            "accepted invalid {field}: {value:?}"
        );
        let stderr = String::from_utf8(output.stderr)?;
        for secret in ["mock-code", "secret-issuer-marker"] {
            assert!(
                !stderr.contains(secret),
                "callback error leaked {secret}: {stderr}"
            );
        }
        assert!(!fixture.home.path().join(".credentials.json").exists());
        assert!(
            fixture
                .server
                .received_requests()
                .await
                .context("missing requests")?
                .iter()
                .all(|request| request.url.path() != "/token")
        );
    }
    Ok(())
}

#[tokio::test]
async fn no_browser_accepts_http_callback_while_terminal_input_remains_open() -> Result<()> {
    let fixture = Fixture::new().await?;
    let mut login = fixture.start().await?;
    let callback = login.callback(&fixture.issuer)?;
    let stdin = login.child.stdin.take().context("missing stdin")?;
    let mut stream = TcpStream::connect((
        "127.0.0.1",
        callback.port().context("missing callback port")?,
    ))
    .await?;
    stream
        .write_all(
            format!(
                "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                callback.path(),
                callback.query().context("missing callback query")?
            )
            .as_bytes(),
        )
        .await?;
    let output = login.finish().await?;
    drop(stdin);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.home.path().join(".credentials.json").exists());
    Ok(())
}

#[tokio::test]
async fn no_browser_retries_rejected_discovered_scopes_with_pasteback() -> Result<()> {
    let fixture = Fixture::new().await?;
    let config_path = fixture.home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(config_path, config.replace("scopes = [\"mcp.read\"]\n", ""))?;
    let mut login = fixture.start().await?;
    assert!(
        login
            .authorization_url
            .query_pairs()
            .any(|(key, value)| key == "scope" && value == "mcp.read")
    );
    let mut callback = login.callback(&fixture.issuer)?;
    let mut params: BTreeMap<_, _> = callback.query_pairs().into_owned().collect();
    params.remove("code");
    params.insert("error".to_string(), "invalid_scope".to_string());
    callback.set_query(None);
    callback.query_pairs_mut().extend_pairs(params);
    login
        .child
        .stdin
        .as_mut()
        .context("missing stdin")?
        .write_all(format!("{callback}\n").as_bytes())
        .await?;
    login.authorization_url =
        read_authorization_url(&mut login.stdout, &mut login.transcript).await?;
    assert!(
        !login
            .authorization_url
            .query_pairs()
            .any(|(key, _)| key == "scope")
    );
    let callback = login.callback(&fixture.issuer)?;
    login
        .child
        .stdin
        .as_mut()
        .context("missing stdin")?
        .write_all(format!("{callback}\n").as_bytes())
        .await?;
    let output = login.finish().await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8(output.stdout)?.contains("Retrying without scopes"));
    let requests = fixture
        .server
        .received_requests()
        .await
        .context("missing requests")?;
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/token")
            .count(),
        1
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn no_browser_sigint_cancels_token_exchange_after_pasted_or_http_callback() -> Result<()> {
    for callback_delivery in ["paste", "http"] {
        let fixture = Fixture::new().await?;
        let request_started = std::sync::Arc::new(tokio::sync::Notify::new());
        let notify = std::sync::Arc::clone(&request_started);
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(move |_: &wiremock::Request| {
                notify.notify_one();
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(/*secs*/ 30))
                    .set_body_json(
                        json!({"access_token": "mock-access-token", "token_type": "Bearer"}),
                    )
            })
            .with_priority(/*priority*/ 1)
            .expect(1)
            .mount(&fixture.server)
            .await;
        let mut login = fixture.start().await?;
        let callback = login.callback(&fixture.issuer)?;
        // Keep the input pipe open after an HTTP callback, including while waiting for exit.
        let mut stdin = login.child.stdin.take().context("missing stdin")?;
        let mut http_callback = None;
        if callback_delivery == "http" {
            let mut stream = TcpStream::connect((
                "127.0.0.1",
                callback.port().context("missing callback port")?,
            ))
            .await?;
            stream
                .write_all(
                    format!(
                        "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                        callback.path(),
                        callback.query().context("missing callback query")?,
                    )
                    .as_bytes(),
                )
                .await?;
            http_callback = Some(stream);
        } else {
            stdin.write_all(format!("{callback}\n").as_bytes()).await?;
        }
        timeout(TEST_TIMEOUT, request_started.notified())
            .await
            .context("token exchange never started")?;
        let child_pid = i32::try_from(login.child.id().context("login exited before SIGINT")?)?;
        // SAFETY: this positive PID belongs to the live child process owned by this test.
        assert_eq!(unsafe { libc::kill(child_pid, libc::SIGINT) }, 0);
        let output = timeout(Duration::from_secs(/*secs*/ 5), login.finish())
            .await
            .with_context(|| {
                format!("SIGINT did not cancel {callback_delivery} callback token exchange")
            })??;
        drop((stdin, http_callback));
        assert!(
            !output.status.success(),
            "SIGINT login unexpectedly succeeded"
        );
        assert!(String::from_utf8(output.stderr)?.contains("OAuth login cancelled"));
        assert!(!fixture.home.path().join(".credentials.json").exists());
    }
    Ok(())
}
