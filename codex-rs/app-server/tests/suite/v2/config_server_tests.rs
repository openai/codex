//! Exercises required HTTPS policy through real app-server startup and session RPCs.

use super::https_mock::tls_listener;
use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::LogoutAccountResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_config::test_support::CloudConfigBundleFixture;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tempfile::tempdir;
use tokio::net::TcpStream;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_server_settings_are_fetched_and_applied_at_startup() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let setup = ConfigServerSetup::new().await?;
    let mut app = setup.start_app().await?;
    assert_eq!(
        setup.config_server.received_requests().await.unwrap().len(),
        1
    );
    let started = app
        .start_thread(ThreadStartParams {
            model_provider: Some("openai".into()),
            ..Default::default()
        })
        .await?;
    assert_eq!(started.model_provider, "company");
    setup
        .assert_turn_uses_managed_instructions(&mut app, &started.thread.id)
        .await?;
    app.shutdown_gracefully().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_server_settings_survive_openai_login_and_logout() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let setup = ConfigServerSetup::new().await?;
    let mut app = setup.start_app().await?;
    let login = app
        .send_login_account_api_key_request("test-api-key")
        .await?;
    let _: LoginAccountResponse = app.read_response(login).await?;
    let logout = app.send_logout_account_request().await?;
    let _: LogoutAccountResponse = app.read_response(logout).await?;
    let started = app.start_thread(ThreadStartParams::default()).await?;
    setup
        .assert_turn_uses_managed_instructions(&mut app, &started.thread.id)
        .await?;
    app.shutdown_gracefully().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_server_settings_are_cached_across_restart_and_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let setup = ConfigServerSetup::new().await?;
    let mut app = setup.start_app().await?;
    let started = app.start_thread(ThreadStartParams::default()).await?;
    setup
        .assert_turn_uses_managed_instructions(&mut app, &started.thread.id)
        .await?;
    app.shutdown_gracefully().await?;

    let mut app = setup.start_app().await?;
    let resumed = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: started.thread.id,
            ..Default::default()
        })
        .await?;
    let resumed: ThreadResumeResponse = app.read_response(resumed).await?;
    setup
        .assert_turn_uses_managed_instructions(&mut app, &resumed.thread.id)
        .await?;
    assert_eq!(
        setup.config_server.received_requests().await.unwrap().len(),
        1
    );
    app.shutdown_gracefully().await?;
    Ok(())
}

struct ConfigServerSetup {
    home: TempDir,
    config_server: MockServer,
    inference: MockServer,
    ca_path: PathBuf,
    _tls: AbortOnDropHandle<()>,
}

impl ConfigServerSetup {
    async fn new() -> Result<Self> {
        let home = tempdir()?;
        let config_server = MockServer::start().await;
        let inference = responses::start_mock_server().await;
        let requirements = format!(
            r#"additional_developer_instructions = 'Company requirements'
model_provider = 'company'

[model_providers.company]
name = 'Company'
base_url = {:?}
env_key = 'CONFIG_SERVER_TEST_KEY'
wire_api = 'responses'
"#,
            inference.uri()
        );
        let bundle = CloudConfigBundleFixture::enterprise_config(
            "developer_instructions = 'Company defaults'",
        )
        .add_enterprise_requirement(requirements)
        .into_bundle();
        Mock::given(method("GET"))
            .and(path("/team/codex/config/bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_json(bundle))
            .mount(&config_server)
            .await;

        let (origin, ca_path, tls) = https_mock_server(home.path(), &config_server).await?;
        let base_url = format!("{origin}/team/");
        std::fs::write(
            home.path().join("requirements.toml"),
            format!("[config_server]\nbase_url = {base_url:?}\n"),
        )?;
        std::fs::write(
            home.path().join("config.toml"),
            "model = 'gpt-5.5'\napproval_policy = 'never'\nsandbox_mode = 'read-only'\n[features]\napps = false\n",
        )?;
        Ok(Self {
            home,
            config_server,
            inference,
            ca_path,
            _tls: tls,
        })
    }

    async fn start_app(&self) -> Result<TestAppServer> {
        let ca_path = self.ca_path.to_string_lossy();
        let env = [
            ("CODEX_CA_CERTIFICATE", Some(ca_path.as_ref())),
            ("CONFIG_SERVER_TEST_KEY", Some("test-key")),
            ("NO_PROXY", Some("127.0.0.1,localhost")),
        ];
        TestAppServer::builder()
            .with_codex_home(self.home.path())
            .with_args(&["--strict-config"])
            .with_env_overrides(&env)
            .build_initialized()
            .await
    }

    async fn assert_turn_uses_managed_instructions(
        &self,
        app: &mut TestAppServer,
        thread_id: &str,
    ) -> Result<()> {
        let recorded = responses::mount_sse_once(
            &self.inference,
            responses::sse(vec![
                responses::ev_assistant_message("reply", "Policy applied"),
                responses::ev_completed("response"),
            ]),
        )
        .await;
        let completed = app
            .start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread_id.into(),
                input: vec![UserInput::Text {
                    text: "Hello".into(),
                    text_elements: vec![],
                }],
                ..Default::default()
            })
            .await?;
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        let instructions = recorded
            .single_request()
            .message_input_texts("developer")
            .join("\n");
        assert!(instructions.contains("Company defaults"));
        assert!(instructions.contains("Company requirements"));
        Ok(())
    }
}

#[cfg(unix)]
enum StdinTokenStartup {
    SkipPolicy,
    LoadPolicy,
    RejectAccount,
    CloseDuringPolicy,
}

#[cfg(unix)]
#[test_case::test_case("pro", Some("endor"), StdinTokenStartup::SkipPolicy; "personal_skips_policy")]
#[test_case::test_case("enterprise", Some("endor"), StdinTokenStartup::LoadPolicy; "enterprise_loads_policy")]
#[test_case::test_case("pro", None, StdinTokenStartup::LoadPolicy; "unbound_account_loads_policy")]
#[test_case::test_case("unknown", Some("endor"), StdinTokenStartup::LoadPolicy; "unknown_plan_loads_policy")]
#[test_case::test_case("pro", Some("alderaan"), StdinTokenStartup::RejectAccount; "mismatched_account_never_registers")]
#[test_case::test_case("enterprise", Some("endor"), StdinTokenStartup::CloseDuringPolicy; "stdin_close_cancels_pending_policy")]
#[tokio::test]
async fn stdin_token_startup(
    plan: &str,
    claimed_id: Option<&str>,
    expected: StdinTokenStartup,
) -> Result<()> {
    use StdinTokenStartup::*;
    use anyhow::Context;
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use wiremock::matchers::header;

    skip_if_no_network!(Ok(()));
    let home = tempdir()?;
    std::fs::write(
        home.path().join("requirements.toml"),
        "allowed_chatgpt_workspaces = ['endor']",
    )?;
    let registry = MockServer::start().await;
    let (origin, ca, _tls) = https_mock_server(home.path(), &registry).await?;
    let timeout = Duration::from_secs(60);
    let mut claims = app_test_support::ChatGptIdTokenClaims::new().plan_type(plan);
    claims.chatgpt_account_id = claimed_id.map(str::to_owned);
    let token = app_test_support::encode_id_token(&claims)?;
    let authenticated = |route| {
        Mock::given(path(route))
            .and(header("authorization", format!("Bearer {token}")))
            .and(header("chatgpt-account-id", "endor"))
    };
    let policy_response = match expected {
        LoadPolicy => ResponseTemplate::new(200).set_body_json(serde_json::json!({})),
        CloseDuringPolicy => ResponseTemplate::new(200).set_delay(timeout),
        SkipPolicy | RejectAccount => ResponseTemplate::new(400)
            .set_body_json(serde_json::json!({"detail": "No active workspace"})),
    };
    let policy = authenticated("/api/codex/config/bundle")
        .respond_with(policy_response)
        .expect(u64::from(matches!(
            expected,
            LoadPolicy | CloseDuringPolicy
        )))
        .mount_as_scoped(&registry)
        .await;
    let register = authenticated("/cloud/environment/env/register")
        .respond_with(ResponseTemplate::new(200).set_delay(timeout))
        .expect(u64::from(matches!(expected, SkipPolicy | LoadPolicy)))
        .mount_as_scoped(&registry)
        .await;
    let mut child = tokio::process::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .current_dir(home.path())
        .env("CODEX_HOME", home.path())
        .env("CODEX_CA_CERTIFICATE", ca)
        .env("NO_PROXY", "127.0.0.1")
        .args(["-c", &format!("chatgpt_base_url={origin:?}")])
        .arg("exec-server")
        .args(["--remote", &origin])
        .args(["--environment-id", "env"])
        .args(["--chatgpt-token-stdin", "--exit-on-stdin-close"])
        .args(["--chatgpt-account-id", "endor"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().context("exec-server stdin")?;
    stdin.write_all(format!("{token}\n").as_bytes()).await?;
    let (pending, exit_timeout) = match expected {
        RejectAccount => {
            let output = tokio::time::timeout(timeout, child.wait_with_output()).await??;
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("--chatgpt-account-id does not match the token")
            );
            assert!(
                registry
                    .received_requests()
                    .await
                    .context("request recording")?
                    .is_empty()
            );
            return Ok(());
        }
        CloseDuringPolicy => (&policy, Duration::from_secs(5)),
        SkipPolicy | LoadPolicy => (&register, timeout),
    };
    tokio::time::timeout(timeout, pending.wait_until_satisfied()).await?;
    drop(stdin);
    assert!(
        tokio::time::timeout(exit_timeout, child.wait())
            .await??
            .success()
    );
    assert!(!home.path().join("auth.json").exists());
    Ok(())
}

async fn https_mock_server(
    home: &Path,
    policy: &MockServer,
) -> Result<(String, PathBuf, AbortOnDropHandle<()>)> {
    let (listener, acceptor, ca_path) = tls_listener(home).await?;
    let origin_url = format!("https://{}", listener.local_addr()?);
    let origin = *policy.address();
    let tls = AbortOnDropHandle::new(tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let mut stream = acceptor.accept(stream).await?;
                let mut upstream = TcpStream::connect(origin).await?;
                tokio::io::copy_bidirectional(&mut stream, &mut upstream).await?;
                Ok::<_, std::io::Error>(())
            });
        }
    }));
    Ok((origin_url, ca_path, tls))
}
